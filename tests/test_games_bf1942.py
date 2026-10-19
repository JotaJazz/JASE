"""Battlefield 1942 (familia GameSpy). Fixtures capturados el 2026-10-19 de
master.openspy.net y de un servidor real."""
from pathlib import Path

import pytest
from jase.games import ALL_VERSIONS, GAMES
from jase.launcher import build_launch_command
from jase.models import ServerAddress
from jase.protocols import ProtocolError
from jase.protocols import gamespy as gs

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 23001)


def _status(address=ADDR, version=None):
    data = (FIXTURES / "bf1942_status.bin").read_bytes()
    if version:
        data = data.replace(b"\\gamever\\v1.61\\", b"\\gamever\\" + version.encode() + b"\\")
    asm = gs.StatusAssembler()
    assert asm.feed(data)
    game = GAMES["bf1942"]
    return gs.parse_status(asm.fields(), address, 50,
                           connect_port_fallback=game.connect_port_fallback,
                           expected_gamename=game.status_gamename)


def test_bf1942_gamedef():
    g = GAMES["bf1942"]
    assert g.name == "Battlefield 1942" and g.short_name == "BF1942"
    assert g.family == "gamespy" and len(g.badge) == 2 and g.default_path == ""
    assert g.masters == (("master.openspy.net", 28900),)
    assert (g.gamespy_gamename, g.gamespy_key) == ("bfield1942", "HpWx9z")
    assert g.status_gamename == "bfield1942"
    assert g.gametracker_id == "bf1942"
    assert g.exe_relpath == "BF1942.exe" and g.launch_style == "bf1942"
    assert g.default_version == ALL_VERSIONS


def test_bf1942_master_fixture():
    servers = gs.parse_master_list((FIXTURES / "bf1942_master_openspy.bin").read_bytes())
    assert len(servers) == 82 and len(set(servers)) == 82
    # El master lista el puerto de consulta (23000+), no el de juego (14567).
    assert all(s.port >= 23000 for s in servers)


def test_bf1942_status():
    info = _status()
    assert info.name == "BmC Butch War Server #2 [GER]"
    assert info.map == "kursk custom"
    assert (info.num_players, info.max_players) == (0, 32)
    assert info.gametype == "conquest"
    assert info.version == "v1.61"
    assert info.password is True
    assert info.connect_address.port == 14568  # hostport


def test_bf1942_status_v1612():
    assert _status(version="v1.612").version == "v1.612"


def test_bf1942_connect_port_without_hostport():
    # hostport - puerto de consulta no es constante (sondeo: 23000->14567, 23090->14657,
    # 23000->14568), así que no hay desplazamiento fijo: connect_port_fallback = 0 y sin
    # `hostport` se usa el puerto de consulta.
    fields = {"hostname": "x", "numplayers": "0", "maxplayers": "8", "gamename": "bfield1942"}
    game = GAMES["bf1942"]
    info = gs.parse_status(fields, ADDR, 1, connect_port_fallback=game.connect_port_fallback,
                           expected_gamename=game.status_gamename)
    assert GAMES["bf1942"].connect_port_fallback == 0
    assert info.connect_address.port == ADDR.port


def test_bf1942_rejects_other_gamename():
    game = GAMES["bf1942"]
    fields = {"hostname": "x", "gamename": "mohaa"}
    with pytest.raises(ProtocolError):
        gs.parse_status(fields, ADDR, 1, expected_gamename=game.status_gamename)


def test_bf1942_launch_command():
    exe = r"C:\Games\BF1942\BF1942.exe"
    addr = ServerAddress("1.2.3.4", 14567)
    argv, cwd = build_launch_command(GAMES["bf1942"], exe, addr, None)
    assert argv == [exe, "+restart", "1", "+joinServer", "1.2.3.4:14567"]
    assert cwd == r"C:\Games\BF1942"
    argv, _ = build_launch_command(GAMES["bf1942"], exe, addr, "pw")
    assert argv == [exe, "+restart", "1", "+password", "pw", "+joinServer", "1.2.3.4:14567"]


@pytest.mark.parametrize("game_id,expected", [
    ("cod2", ["+connect", "1.2.3.4:14567"]),
    ("ut99", ["1.2.3.4:14567"]),
    ("cs16", ["-game", "cstrike", "+connect", "1.2.3.4:14567"]),
])
def test_existing_launch_styles_unchanged(game_id, expected):
    argv, _ = build_launch_command(GAMES[game_id], r"C:\G\x.exe",
                                   ServerAddress("1.2.3.4", 14567), None)
    assert argv[1:] == expected
