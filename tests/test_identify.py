"""«Detectar juego»: cada captura real de tests/fixtures se reconoce como su juego."""
from dataclasses import replace
from pathlib import Path

import pytest

from jase import identify
from jase.models import ServerAddress, ServerInfo
from jase.protocols import gamespy, quake2, quake3, ut2004, valve

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 1)


def _gamespy(name):
    asm = gamespy.StatusAssembler()
    assert asm.feed((FIXTURES / name).read_bytes())
    return gamespy.parse_status(asm.fields(), ADDR, 1)


def _q3(name):
    return quake3.parse_status_response((FIXTURES / name).read_bytes(), ADDR, 1)


@pytest.mark.parametrize(("name", "expected"), [
    ("cod2x_status.bin", "cod2"), ("cod4_status_17.bin", "cod4"),
    ("cod4_status_cod4x.bin", "cod4"), ("coduo_status.bin", "coduo"),
    ("et_status.bin", "et"), ("jk2_status.bin", "jk2"), ("jka_status.bin", "jka"),
    ("oa_status.bin", "oa"), ("q3_status.bin", "q3"), ("rtcw_status.bin", "rtcw"),
    ("smokinguns_status.bin", "smokinguns"), ("sof2_status_2002.bin", "sof2"),
    ("sof2_status_2004.bin", "sof2"), ("tremulous_status.bin", "tremulous"),
    ("urt_status.bin", "urt"), ("warfork_status.bin", "warfork"),
    ("warsow_status.bin", "warsow"), ("xonotic_status.bin", "xonotic"),
])
def test_quake3_fixtures(name, expected):
    assert identify.identify("quake3", _q3(name)) == [expected]


@pytest.mark.parametrize(("name", "expected"), [
    ("bf1942_status.bin", "bf1942"), ("mohaa_status.bin", "mohaa"),
    ("mohaas_status.bin", "mohaas"), ("mohaab_status.bin", "mohaab"),
])
def test_gamespy_fixtures(name, expected):
    assert identify.identify("gamespy", _gamespy(name)) == [expected]


def test_cod1_needs_exact_gamename():
    info = ServerInfo(ADDR, ADDR, "x", "m", "dm", 0, 8, False, "", 1,
                      rules={"gamename": "Call of Duty", "protocol": "6"})
    assert identify.identify("quake3", info) == ["cod1"]
    info.rules["gamename"] = "main"  # sin pista: los dos que usan el protocolo 6
    assert identify.identify("quake3", info) == ["cod1", "cod4"]


def test_valve_appid_or_gamedir():
    assert identify.identify("valve", valve.parse_info(
        (FIXTURES / "cs16_info.bin").read_bytes(), ADDR, 1)) == ["cs16"]
    # GoldSrc antiguo sin appid: «cstrike» puede ser CS 1.6 o CS:S.
    assert identify.identify("valve", valve.parse_info(
        (FIXTURES / "cs16_info_goldsrc.bin").read_bytes(), ADDR, 1)) == ["cs16", "css"]


def test_quake2_and_ut2004():
    def q2(name):
        return quake2.parse_status((FIXTURES / name).read_bytes(), ADDR, 1)
    assert identify.identify("quake2", q2("q2_status.bin")) == ["q2"]
    assert identify.identify("quake2", q2("dday_status.bin")) == ["dday"]
    info = ut2004.parse_info((FIXTURES / "ut2004_info.bin").read_bytes(), ADDR, 1)
    assert identify.identify("ut2004", info) == ["ut2004"]


def test_probe_plan_tries_query_ports():
    plan = dict(identify.probe_plan(ServerAddress("h", 7777)))
    assert set(plan) == set(identify.FAMILIES)
    assert plan["quake3"] == [ServerAddress("h", 7777)]
    assert [a.port for a in plan["gamespy"]] == [7777, 7778, 7874, 16210]
    assert [a.port for a in plan["ut2004"]] == [7777, 7778]
    assert [a.port for a in dict(identify.probe_plan(ServerAddress("h", 65535)))["gamespy"]] \
        == [65535]


def test_detected_games_dedupes_and_keeps_first_address():
    a, b = ServerAddress("h", 1), ServerAddress("h", 2)
    mohaa = _gamespy("mohaa_status.bin")
    responses = [("gamespy", replace(mohaa, address=a)), ("gamespy", replace(mohaa, address=b))]
    assert identify.detected_games(responses) == [("mohaa", a)]


def _info(**rules):
    return ServerInfo(ADDR, ADDR, "x", "m", "dm", 0, 8, False, "", 1, rules=rules)


def test_iortcw_without_protocol():
    # Visto en el sondeo real: iortcw sin `protocol` y con el mod en `gamename`.
    info = _info(gamename="omnibot", version="iortcw 1.51c-MP linux-x86_64 Mar 16 2019")
    assert identify.identify("quake3", info) == ["rtcw"]


def test_warsow_answering_a2s():
    # Warsow 2.1 contesta también por A2S de Valve, con appid 0 y carpeta basewsw.
    info = _info(appid="0", gamedir="basewsw", protocol="22")
    assert identify.identify("valve", info) == ["warsow"]
