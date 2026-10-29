import asyncio
import io
import json
import os
import subprocess
import sys

import pytest

from jase import cli, i18n
from jase.games import GAMES
from jase.geo import CountryDB
from jase.models import Player, ServerAddress, ServerInfo
from jase.scanner import MasterError, ScanResult


@pytest.fixture(autouse=True)
def _restore_language():
    before = i18n.current_language()
    yield
    i18n.set_language(before)


def _info(host="1.2.3.4", port=28960, **kw):
    addr = ServerAddress(host, port)
    base = dict(address=addr, connect_address=addr, name="Jimmy TDM", map="mp_toujane",
                gametype="tdm", num_players=3, max_players=20, password=False,
                version="1.3", ping_ms=42,
                players=[Player("Jimmy", 10, 40), Player("bot", 0, 0)],
                rules={"fs_game": "mods/pam", "g_gametype": "tdm"}, bots=1)
    base.update(kw)
    return ServerInfo(**base)


class FakeScan:
    def __init__(self, servers=(), result=None, delay=0.0):
        self.servers = list(servers)
        self.result = result
        self.delay = delay
        self.calls = []

    async def __call__(self, game, masters, on_server, on_status, **kw):
        self.calls.append((game.id, list(masters), kw))
        for info in self.servers:
            on_server(info)
        await asyncio.sleep(self.delay)
        return self.result or ScanResult(masters_ok=len(masters),
                                         servers_listed=len(self.servers),
                                         servers_responded=len(self.servers))


def run(argv, scan=None, env=None, geo=None):
    out, err = io.StringIO(), io.StringIO()
    code = cli.main(argv, scan_fn=scan or FakeScan(), stdout=out, stderr=err,
                    environ=env if env is not None else {}, country_source=geo)
    return code, out.getvalue(), err.getvalue()


def test_games_json_lists_every_game(tmp_path):
    code, out, _ = run(["games", "--json", "--cache-dir", str(tmp_path), "--no-update"])
    data = json.loads(out)
    assert code == 0 and data["schema"] == cli.SCHEMA == 1
    ids = [g["id"] for g in data["games"]]
    assert ids == list(GAMES)
    l4d2 = next(g for g in data["games"] if g["id"] == "l4d2")
    assert l4d2["needs_steam_key"] is True and l4d2["family"] == "valve"
    cod2 = next(g for g in data["games"] if g["id"] == "cod2")
    assert cod2["needs_steam_key"] is False and cod2["name"] == GAMES["cod2"].name
    assert cod2["masters"] and all(set(m) == {"host", "port"} for m in cod2["masters"])


def test_scan_json_contract(tmp_path):
    scan = FakeScan([_info()])
    code, out, _ = run(["scan", "cod2", "--json", "--cache-dir", str(tmp_path), "--no-update"],
                       scan)
    data = json.loads(out)
    assert code == 0
    assert data["schema"] == 1 and data["game"] == {"id": "cod2", "name": GAMES["cod2"].name}
    assert data["timed_out"] is False and data["errors"] == []
    assert data["masters_ok"] == len(data["masters"]) >= 1
    assert data["servers_listed"] == data["servers_responded"] == 1
    srv = data["servers"][0]
    assert srv["address"] == "1.2.3.4:28960" and srv["connect"] == "1.2.3.4:28960"
    assert srv["connect_port_deduced"] is False
    assert (srv["players"], srv["max_players"], srv["bots"], srv["humans"]) == (3, 20, 1, 2)
    assert srv["mod"] == "pam" and srv["map"] == "mp_toujane" and srv["version"] == "1.3"
    assert srv["country"] is None and srv["password"] is False and srv["ping_ms"] == 42
    assert srv["player_list"] == [{"name": "Jimmy", "score": 10, "ping": 40},
                                  {"name": "bot", "score": 0, "ping": 0}]
    assert srv["rules"]["g_gametype"] == "tdm"


def test_scan_base_mod_is_null(tmp_path):
    scan = FakeScan([_info(rules={})])
    _, out, _ = run(["scan", "cod2", "--json", "--cache-dir", str(tmp_path), "--no-update"],
                    scan)
    assert json.loads(out)["servers"][0]["mod"] is None


def test_scan_reports_master_errors_and_exit_code(tmp_path):
    result = ScanResult(0, 0, 0, errors=(MasterError("a:1", "timeout", "x"),))
    code, out, _ = run(["scan", "cod2", "--json", "--cache-dir", str(tmp_path), "--no-update"],
                       FakeScan(result=result))
    data = json.loads(out)
    assert code == 1
    assert data["errors"] == [{"master": "a:1", "kind": "timeout", "message": "x"}]
    assert data["masters_ok"] == 0


def test_steam_key_from_flag_or_env(tmp_path):
    base = ["scan", "l4d2", "--json", "--cache-dir", str(tmp_path), "--no-update"]
    scan = FakeScan()
    run(base, scan, env={"JASE_STEAM_KEY": "ENVKEY"})
    run(base + ["--steam-key", "FLAGKEY", "--include-empty"], scan, env={"JASE_STEAM_KEY": "E"})
    assert scan.calls[0][2]["steam_key"] == "ENVKEY"
    assert scan.calls[0][2]["steam_include_empty"] is False
    assert scan.calls[1][2]["steam_key"] == "FLAGKEY"
    assert scan.calls[1][2]["steam_include_empty"] is True


def test_missing_steam_key_message_points_to_cli_options(tmp_path):
    result = ScanResult(0, 0, 0, errors=(
        MasterError("api.steampowered.com:443", "missing_key", "ponla en Ajustes"),))
    _, out, _ = run(["scan", "l4d2", "--json", "--cache-dir", str(tmp_path), "--no-update"],
                    FakeScan(result=result))
    message = json.loads(out)["errors"][0]["message"]
    assert "JASE_STEAM_KEY" in message and "Ajustes" not in message


def test_master_override(tmp_path):
    scan = FakeScan()
    run(["scan", "cod2", "--json", "--cache-dir", str(tmp_path), "--no-update",
         "--master", "m.example.com:20710", "--master", "1.1.1.1:1"], scan)
    assert scan.calls[0][1] == [("m.example.com", 20710), ("1.1.1.1", 1)]


@pytest.mark.parametrize("bad", ["nohost", "h:0", "h:99999", ":20710"])
def test_bad_master_is_usage_error(tmp_path, bad):
    code, out, err = run(["scan", "cod2", "--cache-dir", str(tmp_path), "--no-update",
                          "--master", bad])
    assert code == 2 and out == "" and err


def test_unknown_game_is_usage_error(tmp_path):
    code, out, err = run(["scan", "nope", "--json", "--cache-dir", str(tmp_path), "--no-update"])
    assert code == 2 and out == "" and "nope" in err


def test_timeout_returns_partial_results(tmp_path):
    scan = FakeScan([_info()], delay=5)
    code, out, _ = run(["scan", "cod2", "--json", "--cache-dir", str(tmp_path), "--no-update",
                        "--timeout", "0.2"], scan)
    data = json.loads(out)
    assert code == 0 and data["timed_out"] is True
    assert len(data["servers"]) == 1 and data["servers_listed"] is None


class FakeGeo:
    def __init__(self, db, error=None):
        self.db, self.error, self.paths = db, error, []

    def load(self, path, refresh=False):
        self.paths.append(path)
        return self.db, self.error


def _db(mapping):
    class DB(CountryDB):
        def __init__(self):
            pass

        def lookup(self, ip):
            return mapping.get(ip)
    return DB()


def test_country_flag_uses_cached_db(tmp_path):
    geo = FakeGeo(_db({"1.2.3.4": "CL"}))
    scan = FakeScan([_info(), _info(host="5.6.7.8")])
    _, out, _ = run(["scan", "cod2", "--json", "--cache-dir", str(tmp_path), "--no-update",
                     "--country"], scan, geo=geo)
    servers = json.loads(out)["servers"]
    assert [s["country"] for s in servers] == ["cl", None]
    assert geo.paths == [tmp_path / "dbip-country-lite.csv"]


def test_country_db_failure_is_a_warning_not_an_error(tmp_path):
    geo = FakeGeo(None, "sin red")
    code, out, err = run(["scan", "cod2", "--json", "--cache-dir", str(tmp_path),
                          "--no-update", "--country"], FakeScan([_info()]), geo=geo)
    assert code == 0 and json.loads(out)["servers"][0]["country"] is None
    assert "sin red" in err


def test_text_output_is_a_table(tmp_path):
    code, out, _ = run(["scan", "cod2", "--cache-dir", str(tmp_path), "--no-update"],
                       FakeScan([_info()]))
    assert code == 0 and "1.2.3.4:28960" in out and "Jimmy TDM" in out and "3/20" in out


def test_language_from_environment(tmp_path):
    code, out, err = run(["scan", "nope", "--cache-dir", str(tmp_path), "--no-update"],
                         env={"JASE_LANG": "es"})
    assert code == 2 and "Juego desconocido" in err
    code, out, err = run(["scan", "nope", "--cache-dir", str(tmp_path), "--no-update"],
                         env={"LANG": "C.UTF-8"})
    assert "Unknown game" in err


def test_cli_does_not_import_gui_dependencies():
    code = ("import sys, jase.cli; "
            "bad = [m for m in sys.modules if m.split('.')[0] in "
            "('PySide6', 'PIL', 'keyring', 'shiboken6')]; print(bad)")
    out = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True,
                         check=True).stdout
    assert out.strip() == "[]"


def test_gui_entry_without_pyside6_explains_the_extra():
    code = ("import sys, runpy; sys.modules['PySide6'] = None; sys.argv = ['jase']; "
            "runpy.run_module('jase', run_name='__main__')")
    proc = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True,
                          env={**os.environ, "JASE_LANG": "en"})
    assert proc.returncode == 1
    assert 'pip install "jase[gui]"' in proc.stderr and "jase-cli" in proc.stderr
