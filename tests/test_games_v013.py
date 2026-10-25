"""D-Day: Normandy (mod de Quake II) y Garry's Mod (API web de Steam), JASE 0.13."""
import os
from pathlib import Path

import pytest

from jase import detect, launcher, masters, scanner
from jase.games import ALL_VERSIONS, GAMES
from jase.models import ServerAddress
from jase.protocols import ProtocolError, quake2

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("38.7.207.148", 27950)


def _dday_status(game_id: str):
    opts = scanner.game_query_options(GAMES[game_id])
    return quake2.parse_status((FIXTURES / "dday_status.bin").read_bytes(), ADDR, 42,
                               version_key=opts["version_key"],
                               expected_gamedir=opts["expected_gamedir"],
                               exclude_mods=opts["exclude_mods"])


def test_dday_gamedef():
    game = GAMES["dday"]
    assert (game.name, game.short_name, game.badge) == ("D-Day: Normandy", "D-Day", "DY")
    assert game.family == "quake2" and game.masters == GAMES["q2"].masters
    assert game.status_gamedir == "dday" and game.version_key == "dll_version"
    assert game.default_version == ALL_VERSIONS and game.mod_key == ""
    assert game.gametracker_id == "q2"


def test_dday_status_fixture():
    # Captura real (2026-10-25) del único servidor D-Day del master de Quake II.
    info = _dday_status("dday")
    assert info.name == "D-Day Chile - Capture The Briefcase!"
    assert info.map == "calais" and (info.num_players, info.max_players) == (0, 20)
    assert info.version == "5.067 Chile"  # dll_version, no el motor q2proded


def test_q2_excludes_dday_servers():
    with pytest.raises(ProtocolError):
        _dday_status("q2")


def test_q2_keeps_engine_version():
    info = quake2.parse_status((FIXTURES / "q2_status.bin").read_bytes(), ADDR, 1,
                               **_q2_options("q2"))
    assert info.version == "q2proded r1~277d2ef"


def _q2_options(game_id):
    opts = scanner.game_query_options(GAMES[game_id])
    return {k: opts[k] for k in ("version_key", "expected_gamedir", "exclude_mods")}


def test_dday_rejects_other_gamedirs():
    with pytest.raises(ProtocolError):
        quake2.parse_status((FIXTURES / "q2_status.bin").read_bytes(), ADDR, 1,
                            **_q2_options("dday"))


def test_gamedir_falls_back_to_game_and_ignores_case():
    data = b"\xff\xff\xff\xffprint\n\\hostname\\X\\game\\DDay\\maxclients\\8\n"
    assert quake2.parse_status(data, ADDR, 1, expected_gamedir="dday").name == "X"


def test_dday_launch_sets_game_dir():
    argv, cwd = launcher.build_launch_command(GAMES["dday"], r"C:\Q2\q2pro.exe", ADDR, "pw")
    assert argv == [r"C:\Q2\q2pro.exe", "+set", "game", "dday", "+password", "pw",
                    "+connect", "38.7.207.148:27950"]
    assert cwd == r"C:\Q2"


def test_dday_needs_dday_folder(tmp_path):
    (tmp_path / "quake2.exe").write_bytes(b"")
    assert detect.resolve_game_exe(GAMES["dday"], str(tmp_path)) is None
    assert detect.resolve_game_exe(GAMES["q2"], str(tmp_path)) is not None
    (tmp_path / "dday").mkdir()
    assert detect.resolve_game_exe(GAMES["dday"], str(tmp_path)) == os.path.join(
        str(tmp_path), "quake2.exe")


def test_gmod_gamedef():
    game = GAMES["gmod"]
    assert (game.name, game.badge) == ("Garry's Mod", "GM")
    assert game.family == "valve" and game.steam_webapi and game.launch_style == "steam"
    assert (game.valve_appid, game.valve_gamedir) == (4000, "garrysmod")
    assert game.masters == (("api.steampowered.com", 443),)
    assert scanner.game_query_options(game)["expected_gamedir"] == "garrysmod"


def test_gmod_connects_through_steam():
    opened = []
    launcher.launch(GAMES["gmod"], None, ADDR, None, open_url=opened.append)
    assert opened == ["steam://connect/38.7.207.148:27950"]


def test_gmod_folder_is_not_css(tmp_path):
    (tmp_path / "hl2.exe").write_bytes(b"")
    (tmp_path / "cstrike").mkdir()
    assert detect.resolve_game_exe(GAMES["gmod"], str(tmp_path)) is None
    (tmp_path / "garrysmod").mkdir()
    assert detect.resolve_game_exe(GAMES["gmod"], str(tmp_path)) is not None


def test_gmod_64bit_exe_found(tmp_path):
    (tmp_path / "bin" / "win64").mkdir(parents=True)
    (tmp_path / "bin" / "win64" / "gmod.exe").write_bytes(b"")
    (tmp_path / "garrysmod").mkdir()
    assert detect.resolve_game_exe(GAMES["gmod"], str(tmp_path)) is not None


def test_bundled_masters_json_lists_v013_games():
    official = masters.bundled_official().games
    assert official["dday"] == (("master.quakeservers.net", 27900),)
    assert official["gmod"] == (("api.steampowered.com", 443),)
