import json

import pytest

from jase import i18n, masters
from jase.config import Config
from jase.masters import (CHECK_INTERVAL_S, MAX_BYTES, check_official, fetch_official,
                          should_check)
from jase.ui import masters_loader
from jase.ui.main_window import MainWindow
from test_main_window import FakeLaunch, FakeMaps, FakeScan


def doc(games, updated="2026-11-01"):
    return json.dumps({"schema": 1, "updated": updated, "games": games}).encode()


NEW = doc({"cod2": [{"host": "new.example.com", "port": 20510}]})


def test_should_check_interval(tmp_path):
    meta = tmp_path / "masters.meta.json"
    assert should_check(meta, 1000.0)  # sin meta
    meta.write_text("basura")
    assert should_check(meta, 1000.0)
    meta.write_text(json.dumps({"last_check": 1000.0}))
    assert not should_check(meta, 1000.0 + CHECK_INTERVAL_S - 1)
    assert should_check(meta, 1000.0 + CHECK_INTERVAL_S)
    assert should_check(meta, 500.0)  # reloj retrasado


def test_update_writes_cache_and_meta(tmp_path):
    result = check_official(tmp_path, 123.0, lambda url: NEW)[0]
    assert result is not None and result.games["cod2"] == (("new.example.com", 20510),)
    assert (tmp_path / "masters.json").read_bytes() == NEW
    assert json.loads((tmp_path / "masters.meta.json").read_text()) == {
        "last_attempt": 123.0, "last_attempt_ok": True, "last_success": 123.0}
    assert masters.load_official(tmp_path) == result
    assert not [p for p in tmp_path.iterdir() if p.name.endswith(".tmp")]


def test_update_keeps_previous_on_invalid(tmp_path):
    check_official(tmp_path, 1.0, lambda url: NEW)
    check_official(tmp_path, 2.0, lambda url: b"{no json")[0]
    assert (tmp_path / "masters.json").read_bytes() == NEW
    assert json.loads((tmp_path / "masters.meta.json").read_text())["last_attempt"] == 2.0


def test_update_network_error_writes_meta_only(tmp_path):
    def boom(url):
        raise OSError("sin red")

    assert check_official(tmp_path, 5.0, boom)[0] is None
    assert not (tmp_path / "masters.json").exists()
    assert json.loads((tmp_path / "masters.meta.json").read_text()) == {
        "last_attempt": 5.0, "last_attempt_ok": False, "last_success": None}


def test_update_returns_none_when_unchanged(tmp_path):
    assert check_official(tmp_path, 1.0, lambda url: NEW)[0] is not None
    assert check_official(tmp_path, 2.0, lambda url: NEW)[0] is None


def test_date_only_change_counts_as_updated(tmp_path):
    check_official(tmp_path, 1.0, lambda url: NEW)
    same_games = doc({"cod2": [{"host": "new.example.com", "port": 20510}]},
                     updated="2026-12-01")
    result = check_official(tmp_path, 2.0, lambda url: same_games)[0]
    assert result is not None and result.updated == "2026-12-01"


@pytest.mark.parametrize("bad", ["", "ayer", "2026-1-01", "2026-11-01\nX", "<b>2026-11-01</b>"])
def test_parse_rejects_bad_updated_format(bad):
    with pytest.raises(ValueError):
        masters.parse_official(doc({"cod2": [{"host": "a.b", "port": 1}]}, updated=bad))


def test_should_check_retries_one_hour_after_failure(tmp_path):
    meta = tmp_path / "masters.meta.json"

    def boom(url):
        raise OSError("sin red")

    check_official(tmp_path, 1000.0, lambda url: NEW)  # éxito
    check_official(tmp_path, 2000.0, boom)  # intento fallido posterior
    data = json.loads(meta.read_text())
    assert data["last_success"] == 1000.0 and data["last_attempt"] == 2000.0
    assert not should_check(meta, 2000.0 + 3599)
    assert should_check(meta, 2000.0 + 3601)  # reintento aunque el éxito sea reciente
    assert should_check(meta, 1000.0 + CHECK_INTERVAL_S + 1) is True
    check_official(tmp_path, 9000.0, lambda url: NEW)
    assert not should_check(meta, 9000.0 + 3601)  # éxito: vuelve el ciclo de 24 h


def test_should_check_tolerates_odd_meta(tmp_path):
    meta = tmp_path / "masters.meta.json"
    for bad in ("[]", '{"last_success": "x"}', '{"last_attempt_ok": false, "last_attempt": null}'):
        meta.write_text(bad)
        assert should_check(meta, 1000.0)


def test_fetch_rejects_oversize():
    with pytest.raises(ValueError):
        fetch_official(lambda url: b"x" * (MAX_BYTES + 1))
    assert fetch_official(lambda url: b"x" * MAX_BYTES) == b"x" * MAX_BYTES


def make_window(qtbot, tmp_path, fetch):
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=FakeScan({}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps(), masters_fetch=fetch)
    qtbot.addWidget(w)
    return w


def test_window_applies_update_and_shows_status(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, lambda url: NEW)
    qtbot.waitUntil(lambda: w._official.games.get("cod2") == (("new.example.com", 20510),))
    assert "2026-11-01" in w.statusBar().currentMessage()
    assert not w.check_masters_update(force=False)  # revisado hace instantes
    w.close()


def test_stale_update_result_discarded(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, None)
    before = w._official
    stale = masters_loader.MastersUpdateRequest(tmp_path, 0.0, None)
    masters_loader.shared_relay().loaded.emit(stale, masters.parse_official(NEW), None)
    assert w._official is before
    w.close()


def test_masters_update_status_in_english(qtbot, tmp_path):
    i18n.set_language("en")
    try:
        w = make_window(qtbot, tmp_path, lambda url: NEW)
        qtbot.waitUntil(lambda: "Masters list updated (2026-11-01)"
                        in w.statusBar().currentMessage())
        w.close()
    finally:
        i18n.set_language("es")
