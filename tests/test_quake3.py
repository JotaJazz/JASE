from pathlib import Path

import pytest

from jase.models import ServerAddress, Player
from jase.protocols import ProtocolError
from jase.protocols import quake3 as q3

def test_build_getservers():
    assert q3.build_getservers(118) == b"\xff\xff\xff\xffgetservers 118 full empty"


def test_build_getservers_with_gamename():
    # dpmaster mezcla juegos: la consulta lleva el gamename antes del protocolo.
    assert q3.build_getservers(3, "Xonotic") == b"\xff\xff\xff\xffgetservers Xonotic 3 full empty"
    assert q3.build_getservers(68, "") == q3.build_getservers(68)

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


def test_strip_colors_removes_control_characters():
    assert q3.strip_colors("^1Jim\x01my\tTDM\x7f\x00") == "JimmyTDM"
    assert q3.strip_colors("^\x011Red") == "Red"  # control entre ^ y el dígito


def test_status_names_drop_control_characters():
    data = (b"\xff\xff\xff\xffstatusResponse\n"
            b"\\sv_hostname\\^1Jim\x01my\tCoD1\\sv_maxclients\\20\n"
            b'3 50 "\x01Sgt.\t^2Jimmy"\n')
    info = q3.parse_status_response(data, ServerAddress("1.1.1.1", 1), 10)
    assert info.name == "JimmyCoD1"
    assert info.players == [Player("Sgt.Jimmy", 3, 50)]


# Respuestas reales capturadas de los masters de Activision (2026-10-18).
# Formato real: "getserversResponse\n\x00" + entradas; los paquetes intermedios de
# CoD2 terminan en "\EOT" sin relleno y el último en "\EOF".
FIXTURES = Path(__file__).parent / "fixtures"


def _fixture(name: str) -> bytes:
    return (FIXTURES / name).read_bytes()


def test_parse_real_cod2_intermediate_packet_ends_with_bare_eot():
    servers, final = q3.parse_getservers_response(_fixture("cod2_master_118_1.bin"))
    assert len(servers) == 111
    assert final is False  # \EOT sin relleno: vienen más paquetes
    assert all(1 <= s.port <= 65535 for s in servers)


def test_parse_real_cod2_last_packet_ends_with_eof():
    servers, final = q3.parse_getservers_response(_fixture("cod2_master_118_4.bin"))
    assert len(servers) == 46
    assert final is True


def test_parse_real_cod1_single_packet():
    servers, final = q3.parse_getservers_response(_fixture("cod1_master_1.bin"))
    assert len(servers) == 54
    assert final is True
    assert servers[0].host.startswith("15.")  # primer byte tras "\n\x00\" es 0x0f


def test_parse_status_keeps_raw_names():
    info = q3.parse_status_response(STATUS, ServerAddress("1.2.3.4", 28960), 52)
    assert info.raw_name == "^1Jimmy^7 TDM"
    assert info.players[0].raw_name == "Sgt.^2Jimmy"


def test_strip_colors_double_caret_codes():
    """«^^7» y «^^00» (truco de nombres de CoD) no dejan un «^» suelto."""
    assert q3.strip_colors("^^7Sugus") == "Sugus"
    assert q3.strip_colors("^^00B E N D E R") == "B E N D E R"
    assert q3.strip_colors("^^71337") == "1337"  # solo se consume el dígito repetido


def test_parse_status_counts_bots():
    data = (b'\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\B\\sv_maxclients\\16\n'
            b'3 45 "Persona"\n0 0 "Orbb"\n2 999 "bot12"\n1 30 "[BOT]Shadow"\n'
            b'5 999 "Interrumpido"\n')
    info = q3.parse_status_response(data, ServerAddress("1.1.1.1", 1), 10)
    assert (info.num_players, info.bots, info.humans) == (5, 3, 2)


def test_is_bot_heuristic():
    from jase.protocols import is_bot
    assert is_bot(Player("Xaero", 0, 0)) and is_bot(Player(" [bot]Rush", 3, 50))
    assert is_bot(Player("BOT581", 0, 999))
    assert not is_bot(Player("Jimmy", 0, 999)) and not is_bot(Player("robot", 0, 40))
