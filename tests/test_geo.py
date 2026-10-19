import gzip
from datetime import date

import pytest

from jase import geo
from jase.geo import CountryDB, CountrySource, country_name, download_db

CSV = """\
0.0.0.0,0.255.255.255,ZZ
1.0.0.0,1.0.0.255,AU
1.0.1.0,1.0.3.255,CN
2001:200::,2001:200:ffff:ffff:ffff:ffff:ffff:ffff,JP
1.0.4.0,1.0.7.255,AU
190.0.0.0,190.0.255.255,CL
200.0.0.0,200.0.0.255,BR
"""


@pytest.fixture
def db_path(tmp_path):
    path = tmp_path / "dbip-country-lite.csv"
    path.write_text(CSV, encoding="ascii")
    return path


def test_lookup_contiguous_ranges(db_path):
    db = CountryDB.from_csv(db_path)
    assert db.lookup("1.0.0.0") == "au"
    assert db.lookup("1.0.0.255") == "au"
    assert db.lookup("1.0.1.0") == "cn"
    assert db.lookup("1.0.3.255") == "cn"
    assert db.lookup("1.0.4.0") == "au"
    assert db.lookup("190.0.12.34") == "cl"
    assert db.lookup("200.0.0.255") == "br"


def test_lookup_outside_ranges_and_unknown(db_path):
    db = CountryDB.from_csv(db_path)
    assert db.lookup("1.0.8.0") is None  # hueco entre rangos
    assert db.lookup("200.0.1.0") is None  # después del último
    assert db.lookup("255.255.255.255") is None
    assert db.lookup("0.1.2.3") is None  # ZZ = desconocido


@pytest.mark.parametrize("ip", ["", "abc", "1.2.3", "1.2.3.256", "::1", "2001:200::1",
                                "example.com", "1.2.3.4 ", None])
def test_lookup_invalid_ip_returns_none(db_path, ip):
    assert CountryDB.from_csv(db_path).lookup(ip) is None


def test_ipv6_rows_are_skipped(db_path):
    assert len(CountryDB.from_csv(db_path)) == 5  # 6 IPv4; ZZ descartado


def test_malformed_rows_are_skipped_and_unsorted_input_sorted(tmp_path):
    path = tmp_path / "db.csv"
    path.write_text("200.0.0.0,200.0.0.255,BR\nbasura\n1.2.3,1.2.3.9,XX\n"
                    "1.0.0.0,1.0.0.255,AU\n5.0.0.0,4.0.0.0,US\n", encoding="ascii")
    db = CountryDB.from_csv(path)
    assert len(db) == 2
    assert db.lookup("1.0.0.7") == "au" and db.lookup("200.0.0.1") == "br"


def test_from_csv_missing_file_raises(tmp_path):
    with pytest.raises(OSError):
        CountryDB.from_csv(tmp_path / "nope.csv")


def test_country_name_spanish_with_fallback():
    assert country_name("cl") == "Chile"
    assert country_name("DE") == "Alemania"
    assert country_name("us") == "Estados Unidos"
    assert country_name("br") == "Brasil"
    assert country_name("qq") == "QQ"
    assert country_name("") == ""


def test_country_name_covers_all_flags():
    from jase.ui.flags import FLAGS_DIR
    codes = {p.stem for p in FLAGS_DIR.glob("*.svg")}
    assert len(codes) >= 250
    missing = [c for c in codes if country_name(c) == c.upper()]
    assert missing == []


# ------------------------------------------------------------------ descarga
class FakeFetch:
    def __init__(self, responses):
        self.responses = responses  # url -> bytes | None
        self.calls = []

    def __call__(self, url):
        self.calls.append(url)
        return self.responses.get(url)


def url(month):
    return f"https://download.db-ip.com/free/dbip-country-lite-{month}.csv.gz"


def test_download_current_month(tmp_path):
    dest = tmp_path / "cache" / "dbip-country-lite.csv"
    fetch = FakeFetch({url("2026-10"): gzip.compress(CSV.encode())})
    assert download_db(dest, date(2026, 10, 19), fetch=fetch) is True
    assert fetch.calls == [url("2026-10")]
    assert dest.read_text() == CSV
    assert list(dest.parent.iterdir()) == [dest]  # sin temporales


def test_download_falls_back_to_previous_month(tmp_path):
    dest = tmp_path / "dbip-country-lite.csv"
    fetch = FakeFetch({url("2025-12"): gzip.compress(CSV.encode())})
    assert download_db(dest, date(2026, 1, 2), fetch=fetch) is True
    assert fetch.calls == [url("2026-01"), url("2025-12")]
    assert CountryDB.from_csv(dest).lookup("190.0.0.1") == "cl"


@pytest.mark.parametrize("payload", [None, b"", b"no es gzip", gzip.compress(b"<html>404</html>")])
def test_download_failure_leaves_no_file(tmp_path, payload):
    dest = tmp_path / "dbip-country-lite.csv"
    fetch = FakeFetch({url("2026-10"): payload, url("2026-09"): payload})
    assert download_db(dest, date(2026, 10, 19), fetch=fetch) is False
    assert len(fetch.calls) == 2
    assert list(tmp_path.iterdir()) == []


def test_failed_download_keeps_existing_file(tmp_path, db_path):
    assert download_db(db_path, date(2026, 10, 19), fetch=FakeFetch({})) is False
    assert db_path.read_text() == CSV


def test_fetch_exception_is_failure(tmp_path):
    def boom(url):
        raise OSError("sin red")
    assert download_db(tmp_path / "x.csv", date(2026, 10, 19), fetch=boom) is False


# ---------------------------------------------------------------- CountrySource
def test_source_loads_existing_file_without_download(db_path):
    fetch = FakeFetch({})
    db, error = CountrySource(fetch=fetch, today=lambda: date(2026, 10, 19)).load(db_path)
    assert error is None and db.lookup("200.0.0.1") == "br"
    assert fetch.calls == []


def test_source_downloads_when_missing(tmp_path):
    dest = tmp_path / "cache" / "dbip-country-lite.csv"
    fetch = FakeFetch({url("2026-10"): gzip.compress(CSV.encode())})
    db, error = CountrySource(fetch=fetch, today=lambda: date(2026, 10, 19)).load(dest)
    assert error is None and db.lookup("1.0.1.1") == "cn"


def test_source_download_failure_reports_error(tmp_path):
    db, error = CountrySource(fetch=FakeFetch({}), today=lambda: date(2026, 10, 19)).load(
        tmp_path / "dbip-country-lite.csv")
    assert db is None and "descargar" in error


def test_source_refresh_failure_keeps_old_file(db_path):
    fetch = FakeFetch({})
    db, error = CountrySource(fetch=fetch, today=lambda: date(2026, 10, 19)).load(
        db_path, refresh=True)
    assert len(fetch.calls) == 2
    assert db is None and error
    assert db_path.read_text() == CSV


def test_default_fetch_allows_large_download(monkeypatch):
    captured = {}

    def fake_default_fetch(u, *, max_bytes, deadline):
        captured.update(url=u, max_bytes=max_bytes, deadline=deadline)
        return None

    monkeypatch.setattr(geo, "default_fetch", fake_default_fetch)
    geo._download_fetch("http://x")
    assert captured["max_bytes"] >= 20 * 1024 * 1024 and captured["deadline"] >= 60
