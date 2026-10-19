import re

import pytest

from jase.models import Player, ServerAddress
from jase.protocols import ProtocolError
from jase.protocols import gamespy as gs


def test_parse_secure():
    assert gs.parse_secure(b"\\basic\\\\secure\\ABCDEF") == "ABCDEF"


def test_gsmsalg_shape_and_determinism():
    out = gs.gsmsalg("ABCDEF", gs.UT_GAMEKEY)
    assert re.fullmatch(r"[A-Za-z0-9+/]{8}", out)
    assert out == gs.gsmsalg("ABCDEF", gs.UT_GAMEKEY) != gs.gsmsalg("ABCDEG", gs.UT_GAMEKEY)


def test_build_validate():
    v = gs.build_validate("ABCDEF")
    assert v.startswith(b"\\gamename\\ut\\enctype\\0\\validate\\") and v.endswith(b"\\final\\")


def test_parse_master_list():
    data = b"\\ip\\1.2.3.4:7778\\ip\\5.6.7.8:7790\\final\\"
    assert gs.parse_master_list(data) == [ServerAddress("1.2.3.4", 7778), ServerAddress("5.6.7.8", 7790)]


P1 = (b"\\hostname\\Jimmy UT\\hostport\\7777\\mapname\\DM-Deck16][\\gametype\\DeathMatchPlus"
      b"\\numplayers\\1\\maxplayers\\16\\password\\True\\gamever\\469\\queryid\\7.1")
P2 = b"\\player_0\\Jimmy\\frags_0\\12\\ping_0\\40\\queryid\\7.2\\final\\"


def test_assembler_out_of_order():
    a = gs.StatusAssembler()
    assert a.feed(P2) is False
    assert a.feed(P1) is True
    f = a.fields()
    assert f["hostname"] == "Jimmy UT" and f["player_0"] == "Jimmy"


def test_parse_status():
    a = gs.StatusAssembler(); a.feed(P1); a.feed(P2)
    info = gs.parse_status(a.fields(), ServerAddress("1.2.3.4", 7778), 33)
    assert info.name == "Jimmy UT" and info.map == "DM-Deck16][" and info.gametype == "DeathMatchPlus"
    assert (info.num_players, info.max_players, info.password, info.version) == (1, 16, True, "469")
    assert info.players == [Player("Jimmy", 12, 40)]
    assert info.connect_address == ServerAddress("1.2.3.4", 7777)
    assert "queryid" not in info.rules and "player_0" not in info.rules


def test_parse_status_without_hostport_uses_query_port_minus_one():
    info = gs.parse_status({"hostname": "X", "numplayers": "0", "maxplayers": "8"}, ServerAddress("1.1.1.1", 7778), 5)
    assert info.connect_address.port == 7777 and info.password is False


def test_parse_status_bad_numbers():
    with pytest.raises(ProtocolError):
        gs.parse_status({"hostname": "X", "numplayers": "lots"}, ServerAddress("1.1.1.1", 7778), 5)
