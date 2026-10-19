"""Jedi Academy y Soldier of Fortune II con respuestas reales capturadas (2026-10-19)."""
from pathlib import Path

import pytest

from jase.browse import BASE_MOD, mod_of
from jase.games import ALL_VERSIONS, GAMES
from jase.models import ServerAddress
from jase.protocols import quake3 as q3

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 29070)


def _status(name: str, game_id: str):
    data = (FIXTURES / name).read_bytes()
    return q3.parse_status_response(data, ADDR, 42, version_key=GAMES[game_id].version_key)


def test_raven_gamedefs():
    jka, sof2 = GAMES["jka"], GAMES["sof2"]
    for game in (jka, sof2):
        assert game.family == "quake3" and game.launch_style == "q3"
        assert game.default_path == "" and len(game.badge) == 2
    assert jka.name == "Star Wars Jedi Knight: Jedi Academy" and jka.short_name == "Jedi Academy"
    assert jka.masters == (("master.jkhub.org", 29060),)
    assert jka.protocols == (26,)
    assert jka.default_version == ALL_VERSIONS
    assert jka.exe_relpath == "GameData\\jamp.exe" and jka.exe_candidates == ("jamp.exe",)
    assert jka.gametracker_id == ""  # GameTracker devuelve 404 para mapas de JKA
    assert sof2.name == "Soldier of Fortune II" and sof2.short_name == "SoF2"
    assert sof2.masters == (("master.1fxmod.org", 20110),)
    assert sof2.protocols == (2002, 2004)
    assert sof2.known_versions == ("1.00", "1.03") and sof2.default_version == ALL_VERSIONS
    assert sof2.exe_relpath == "SoF2MP.exe" and sof2.gametracker_id == "sof2"


def test_jka_master_fixture_parses():
    servers, _ = q3.parse_getservers_response((FIXTURES / "jka_master_26_1.bin").read_bytes())
    assert len(servers) > 0
    assert all(s.port > 0 for s in servers)


@pytest.mark.parametrize("fixture", ["sof2_master_2004_1.bin", "sof2_master_2002_1.bin"])
def test_sof2_master_fixtures_parse(fixture):
    servers, _ = q3.parse_getservers_response((FIXTURES / fixture).read_bytes())
    assert len(servers) > 0
    assert all(s.port > 0 for s in servers)


def test_jka_status():
    info = _status("jka_status.bin", "jka")
    assert info.name == "Taivas 2v2 Tourney"
    assert info.map == "um_colosseum"
    assert info.max_players > 0
    assert info.version == "JAmp: v1.0.1.0"
    assert info.password is True


def test_jka_mod_from_fixture():
    # El mod solo viene en `gamename` (no hay fs_game en la captura).
    info = _status("jka_status.bin", "jka")
    assert "fs_game" not in info.rules
    assert mod_of(GAMES["jka"], info) == "movie battles ii r22.2.01"
    assert mod_of(GAMES["jka"], info) != BASE_MOD
    info.rules["gamename"] = "basejka"
    assert mod_of(GAMES["jka"], info) == BASE_MOD


def test_sof2_status_both_versions():
    new = _status("sof2_status_2004.bin", "sof2")
    old = _status("sof2_status_2002.bin", "sof2")
    assert new.version == "1.03" and old.version == "1.00"
    assert new.map == "mp_col1" and old.map == "mp_shop"
    assert (new.max_players, old.max_players) == (10, 64)
    assert {new.version, old.version} <= set(GAMES["sof2"].known_versions)


def test_sof2_gametype_text():
    assert _status("sof2_status_2004.bin", "sof2").gametype == "ctf"
    assert _status("sof2_status_2002.bin", "sof2").gametype == "inf"


def test_sof2_without_fs_game_is_base():
    assert mod_of(GAMES["sof2"], _status("sof2_status_2004.bin", "sof2")) == BASE_MOD
