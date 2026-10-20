"""Juegos de la familia Quake 3 verificados con respuestas reales capturadas (2026-10-19)."""
from pathlib import Path

import pytest

from jase.games import ALL_VERSIONS, GAMES
from jase.models import ServerAddress
from jase.protocols import quake3 as q3

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 27960)

NEW_GAMES = ("coduo", "cod4", "q3", "rtcw", "et")


def _status(name: str, game_id: str):
    data = (FIXTURES / name).read_bytes()
    return q3.parse_status_response(data, ADDR, 42, version_key=GAMES[game_id].version_key)


@pytest.mark.parametrize("game_id", NEW_GAMES)
def test_new_games_are_quake3_family_with_masters(game_id):
    game = GAMES[game_id]
    assert game.family == "quake3"
    assert game.masters and game.protocols
    assert game.launch_style == "q3"
    assert game.default_path == ""
    assert game.default_version == ALL_VERSIONS or game.default_version in game.known_versions


@pytest.mark.parametrize("fixture", [
    "coduo_master_22_1.bin", "cod4_master_6_1.bin", "q3_master_68_1.bin",
    "rtcw_master_60_1.bin", "et_master_84_1.bin", "cod2_master_120_1.bin",
])
def test_captured_master_packets_parse(fixture):
    servers, _ = q3.parse_getservers_response((FIXTURES / fixture).read_bytes())
    assert len(servers) > 0
    assert all(s.port > 0 for s in servers)


def test_coduo_gamedef_and_status():
    game = GAMES["coduo"]
    assert game.masters == (("coduomaster.activision.com", 20610),)
    assert game.protocols == (21, 22, 24)
    assert game.known_versions == ("1.41", "1.51") and game.default_version == "1.51"
    assert game.exe_relpath == "CoDUOMP.exe" and game.gametracker_id == "uo"
    info = _status("coduo_status.bin", "coduo")
    assert info.name == "DEATHRUN|STARWARS|"
    assert info.version == "1.51"
    assert info.map == "mp_killhouse"
    assert (info.num_players, info.max_players) == (2, 20)
    assert [p.name for p in info.players] == ["Jedi", "Darth Vader"]


def test_cod4_gamedef_and_status():
    game = GAMES["cod4"]
    assert game.masters == (("cod4master.activision.com", 20810),)
    assert game.protocols == (1, 5, 6, 7, 8)
    assert game.known_versions == ("1.0", "1.6", "1.7", "1.8")
    assert game.default_version == ALL_VERSIONS
    assert game.exe_relpath == "iw3mp.exe" and game.gametracker_id == "cod4"
    stock = _status("cod4_status_17.bin", "cod4")
    assert stock.name == "1stMSOBTactical4.2UPik" and stock.version == "1.7"
    assert (stock.num_players, stock.max_players) == (0, 36)
    cod4x = _status("cod4_status_cod4x.bin", "cod4")
    assert cod4x.name == "|ZONA ATO|DOM SOFT" and cod4x.version == "-"
    assert (cod4x.num_players, cod4x.max_players) == (6, 36)


def test_q3_gamedef_and_status():
    game = GAMES["q3"]
    assert game.masters == (("master.ioquake3.org", 27950), ("master.quake3arena.com", 27950),
                            ("master.maverickservers.com", 27950))
    assert game.protocols == (68,)
    assert game.version_key == "version" and game.default_version == ALL_VERSIONS
    assert game.exe_relpath == "quake3.exe" and game.gametracker_id == "q3"
    info = _status("q3_status.bin", "q3")
    assert info.name == "AU-Q3a-CHAOS-Server"
    assert info.version == "ioq3 1.36"
    assert info.map == "q3dm8"
    assert (info.num_players, info.max_players) == (10, 20)
    assert info.players[0].name == "Gorre"


def test_rtcw_gamedef_and_status():
    game = GAMES["rtcw"]
    assert game.masters == (("wolfmaster.idsoftware.com", 27950),)
    assert game.protocols == (57, 60)
    assert game.version_key == "version" and game.default_version == ALL_VERSIONS
    assert game.exe_relpath == "WolfMP.exe"
    assert game.gametracker_id == ""  # GameTracker devuelve 404 para mapas de RtCW
    info = _status("rtcw_status.bin", "rtcw")
    assert info.name == "W@R"
    assert info.version == "Wolf 1.41b"
    assert (info.num_players, info.max_players) == (8, 32)


def test_et_gamedef_and_status():
    game = GAMES["et"]
    assert game.masters == (("etmaster.idsoftware.com", 27950), ("master.etlegacy.com", 27950))
    assert game.protocols == (84,)
    assert game.version_key == "version" and game.default_version == ALL_VERSIONS
    assert game.exe_relpath == "ET.exe" and game.gametracker_id == "et"
    info = _status("et_status.bin", "et")
    assert info.name == "=F|A= NoQ#2 XPSAVE FOREVER"
    assert info.version == "ET Legacy v2.83.2"
    assert info.map == "oasis"
    assert (info.num_players, info.max_players) == (20, 64)


@pytest.mark.parametrize("raw, short", [
    ("Q3 1.32e linux-x86_64 May 16 2026", "Q3 1.32e"),
    ("Q3 1.31 linux-i386 Dec 17 2001", "Q3 1.31"),
    ("ioq3 1.36+u20221123.70d07d9+dfsg-1/Debian linux-aarch64 Jan  8 2023", "ioq3 1.36"),
    ("ioq3 1.36_g87a99d18 linux-arm64 Oct 16 2026", "ioq3 1.36"),
    ("CNQ3 1.53 linux-x64 Oct 30 2023", "CNQ3 1.53"),
    ("Wolf 1.0 win-x86 Nov 13 2001", "Wolf 1.0"),
    ("Wolf 1.41-MP linux-i386 Dec  4 2002", "Wolf 1.41"),
    ("Wolf 1.0/WolfSE win-x86 Oct  8 2026", "Wolf 1.0"),
    ("iortcw 1.51d-MP_GIT_438e7d4-2024-05-26 linux", "iortcw 1.51d"),
    ("ET Legacy v2.86.2 linux-x86_64 Oct 11 2026", "ET Legacy v2.86.2"),
    ("ET 3.00 - TB 0.7.4 win-x86", "ET 3.00"),
    ("ET 2.60b linux-i386 May  8 2006", "ET 2.60b"),
    ("", ""),
    ("sin numero", "sin numero"),
])
def test_short_engine_version(raw, short):
    assert q3.short_engine_version(raw) == short


def test_version_key_version_is_normalized_but_shortversion_untouched():
    data = (b"\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\X\\shortversion\\1.3 beta"
            b"\\version\\ET 2.60b linux-i386 May  8 2006\n")
    assert q3.parse_status_response(data, ADDR, 1).version == "1.3 beta"
    assert q3.parse_status_response(data, ADDR, 1, version_key="version").version == "ET 2.60b"


def test_cod2_protocols_include_cod2x():
    assert GAMES["cod2"].protocols == (115, 117, 118, 119, 120)
    assert "1.4" in GAMES["cod2"].known_versions and GAMES["cod2"].default_version == "1.3"


def test_cod2x_status_normalizes_version_keeping_raw():
    game = GAMES["cod2"]
    data = (FIXTURES / "cod2x_status.bin").read_bytes()
    info = q3.parse_status_response(data, ADDR, 42, version_key=game.version_key,
                                    version_aliases=game.version_aliases)
    assert info.version == "1.4"
    assert info.rules["shortversion"].startswith("1.4.")
    assert info.rules["protocol"] == "120"
    assert game.version_label("1.4") == "1.4 (CoD2x)" and game.version_label("1.3") == "1.3"
