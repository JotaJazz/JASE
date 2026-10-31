"""Juegos Valve listados por la API web de Steam (L4D, L4D2, CS:S, TF2, HL DM, DoD, TFC)."""
import asyncio
from pathlib import Path

import pytest

from jase import launcher, masters, scanner
from jase.games import GAMES
from jase.models import ServerAddress
from jase.protocols import steamweb

STEAM_GAMES = ("l4d", "l4d2", "css", "tf2", "hldm", "dod", "tfc")
ADDR = ServerAddress("1.2.3.4", 27015)
FIXTURES = Path(__file__).parent / "fixtures"


@pytest.mark.parametrize("game_id", STEAM_GAMES)
def test_steam_games_are_valve_webapi(game_id):
    g = GAMES[game_id]
    assert g.family == "valve" and g.steam_webapi and g.valve_appid and g.valve_gamedir
    assert g.masters == (("api.steampowered.com", 443),)
    assert len(g.badge) == 2


@pytest.mark.parametrize("game_id", ("l4d", "l4d2", "css", "tf2"))
def test_source_games_connect_through_steam(game_id):
    assert GAMES[game_id].launch_style == "steam"


@pytest.mark.parametrize("game_id", ("hldm", "dod", "tfc"))
def test_goldsrc_steam_games_launch_with_hl_exe(game_id):
    g = GAMES[game_id]
    assert g.launch_style == "valve" and g.exe_relpath == "hl.exe"
    argv, _ = launcher.build_launch_command(g, r"C:\HL\hl.exe", ADDR, None)
    assert argv == [r"C:\HL\hl.exe", "-game", g.valve_gamedir, "+connect", "1.2.3.4:27015"]


def test_hl_mods_are_filtered_by_gamedir():
    assert GAMES["hldm"].valve_master_filter == "\\appid\\70\\gamedir\\valve"
    assert GAMES["tfc"].valve_master_filter == "\\appid\\20\\gamedir\\tfc"
    assert GAMES["dod"].valve_master_filter == ""


def test_steam_url_with_and_without_password():
    assert launcher.steam_url(ADDR, None) == "steam://connect/1.2.3.4:27015"
    assert launcher.steam_url(ADDR, "pw") == "steam://connect/1.2.3.4:27015/pw"


def test_steam_launch_needs_no_install_dir():
    opened = []
    launcher.launch(GAMES["l4d2"], None, ADDR, None, open_url=opened.append)
    assert opened == ["steam://connect/1.2.3.4:27015"]


def test_steam_launch_oserror_becomes_launch_error_without_url():
    def broken(url):
        raise OSError(f"no handler for {url}")
    with pytest.raises(launcher.LaunchError) as err:
        launcher.launch(GAMES["tf2"], None, ADDR, "secretpw", open_url=broken)
    msg = str(err.value)
    assert "Steam" in msg and "secretpw" not in msg and "steam://" not in msg
    assert err.value.__cause__ is None and err.value.__suppress_context__


def test_steam_launch_with_password():
    opened = []
    launcher.launch(GAMES["css"], "", ADDR, "pw", open_url=opened.append)
    assert opened == ["steam://connect/1.2.3.4:27015/pw"]


def test_bundled_masters_json_lists_new_games():
    assert all(gid in masters.bundled_official().games for gid in STEAM_GAMES)


def test_scan_lists_fixture_and_queries_with_expected_gamedir(monkeypatch):
    addresses = steamweb.parse_response((FIXTURES / "steamweb_l4d2.json").read_bytes())[0]
    seen = {}

    async def fake_fetch(game, host, port, timeout, steam_key="", steam_include_empty=False):
        return addresses

    async def fake_query(family, addrs, on_server, **kw):
        seen["family"] = family
        seen["addrs"] = list(addrs)
        seen["gamedir"] = kw.get("expected_gamedir")

    monkeypatch.setattr(scanner, "fetch_master", fake_fetch)
    monkeypatch.setattr(scanner, "query_servers", fake_query)
    game = GAMES["l4d2"]
    result = asyncio.run(scanner.scan(game, list(game.masters), lambda i: None,
                                      lambda m: None, steam_key="K"))
    assert result.servers_listed == len(addresses) > 0
    assert seen["family"] == "valve" and seen["addrs"] == addresses
    assert seen["gamedir"] == "left4dead2"
