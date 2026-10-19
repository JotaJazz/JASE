from jase.models import ServerAddress, Player
from jase.protocols import ProtocolError
from jase.protocols import quake3 as q3
import pytest

def test_build_getservers():
    assert q3.build_getservers(118) == b"\xff\xff\xff\xffgetservers 118 full empty"

def test_parse_getservers_fixed_stride_and_eot():
    # 92 == ord("\\"): una IP con ese byte NO debe romper el parseo
    data = (b"\xff\xff\xff\xffgetserversResponse"
            b"\\" + bytes([1, 2, 3, 4]) + (28960).to_bytes(2, "big") +
            b"\\" + bytes([5, 6, 7, 92]) + (28961).to_bytes(2, "big") +
            b"\\EOT\x00\x00\x00")
    servers, eot = q3.parse_getservers_response(data)
    assert servers == [ServerAddress("1.2.3.4", 28960), ServerAddress("5.6.7.92", 28961)]
    assert eot is True

def test_parse_getservers_without_eot():
    data = b"\xff\xff\xff\xffgetserversResponse\\" + bytes([1, 2, 3, 4, 0x71, 0x20])
    assert q3.parse_getservers_response(data) == ([ServerAddress("1.2.3.4", 28960)], False)

STATUS = (b'\xff\xff\xff\xffstatusResponse\n'
          b'\\sv_hostname\\^1Jimmy^7 TDM\\mapname\\mp_carentan\\g_gametype\\tdm'
          b'\\sv_maxclients\\20\\pswrd\\1\\shortversion\\1.3\n'
          b'24 41 "Sgt.^2Jimmy"\n0 999 "Kowalski \\"K\\""\n')

def test_parse_status():
    addr = ServerAddress("1.2.3.4", 28960)
    info = q3.parse_status_response(STATUS, addr, 52)
    assert info.name == "Jimmy TDM" and info.map == "mp_carentan" and info.gametype == "tdm"
    assert (info.num_players, info.max_players) == (2, 20)
    assert info.password is True and info.version == "1.3" and info.ping_ms == 52
    assert info.players[0] == Player("Sgt.Jimmy", 24, 41)
    assert info.connect_address == addr
    assert info.rules["sv_hostname"] == "^1Jimmy^7 TDM"   # reglas crudas

def test_parse_status_empty_server_and_needpass():
    data = b'\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\Empty\\sv_maxclients\\16\\g_needpass\\0\n'
    info = q3.parse_status_response(data, ServerAddress("1.1.1.1", 1), 10)
    assert info.num_players == 0 and info.players == [] and info.password is False
    assert info.version == "" and info.map == ""

@pytest.mark.parametrize("bad", [b"", b"garbage", b"\xff\xff\xff\xffstatusResponse\n\\sv_maxclients\\abc\n"])
def test_parse_status_malformed_raises(bad):
    with pytest.raises(ProtocolError):
        q3.parse_status_response(bad, ServerAddress("1.1.1.1", 1), 10)

def test_strip_colors():
    assert q3.strip_colors("^1Red^7White^") == "RedWhite^"
