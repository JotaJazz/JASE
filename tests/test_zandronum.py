"""Zandronum (Doom): Huffman, master y consulta con capturas reales (2026-10-27)."""
import asyncio
import socket
import struct
import threading
from pathlib import Path

import pytest

from jase import identify, scanner
from jase.browse import mod_of
from jase.games import GAMES
from jase.launcher import build_launch_command
from jase.models import ServerAddress
from jase.protocols import ProtocolError, huffman
from jase.protocols import zandronum as z

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 10666)


def _fixture(name):
    return (FIXTURES / name).read_bytes()


def test_huffman_raw_and_compressed():
    assert huffman.decode(b"\xffhola") == b"hola"
    assert huffman.decode(b"") == b""
    body = huffman.decode(_fixture("zandronum_players.bin"))
    assert struct.unpack_from("<i", body)[0] == z.SERVER_ACCEPTED
    assert huffman.encode_raw(b"x") == b"\xffx"


def test_master_packet_lists_servers_and_end_marker():
    found, last = z.parse_master_packet(_fixture("zandronum_master.bin"))
    assert last and len(found) == 78
    assert all(a.port for a in found)


def test_master_errors_and_truncation():
    with pytest.raises(ProtocolError, match="bloqueada"):
        z.parse_master_packet(huffman.encode_raw(struct.pack("<i", 3)))
    with pytest.raises(ProtocolError):
        z.parse_master_packet(huffman.encode_raw(struct.pack("<ib", 6, 0) + b"\x08\x02\x01"))


def test_parse_team_game_with_bots():
    info = z.parse_response(_fixture("zandronum_team.bin"), ADDR, 30)
    assert info.name == "[TSPG-PK] Ultimate Railgun" and info.map == "BUR08"
    assert info.gametype == "teamplay" and info.version == "3.3"
    assert (info.num_players, info.max_players, info.bots, info.humans) == (4, 32, 4, 0)
    assert [p.name for p in info.players] == ["Pluto", "KnightMan", "Neptune", "Sunstar"]
    assert info.rules["iwad"] == "megagame.wad"


def test_parse_coop_players():
    info = z.parse_response(_fixture("zandronum_players.bin"), ADDR, 30)
    assert info.gametype == "cooperative" and info.map == "DARK_27"
    assert [(p.name, p.score, p.ping) for p in info.players] == [("Player", 0, 76),
                                                                ("leg", 757, 45)]
    assert info.bots == 0 and not info.password
    assert mod_of(GAMES["zandronum"], info) == "doom2.wad"


def test_parse_rejects_bad_answers():
    with pytest.raises(ProtocolError):
        z.parse_response(huffman.encode_raw(struct.pack("<ii", z.SERVER_IGNORING, 0)), ADDR, 1)
    body = huffman.decode(_fixture("zandronum_team.bin"))
    with pytest.raises(ProtocolError):
        z.parse_response(huffman.encode_raw(body[:-5]), ADDR, 1)


def test_strip_doom_colors():
    assert z.strip_colors("\x1cGRed\x1c[Gold]Gold\x1c-x") == "RedGoldx"


def test_gamedef_launch_and_identify():
    game = GAMES["zandronum"]
    assert game.family == "zandronum" and game.masters == (("master.zandronum.com", 15300),)
    cmd, cwd = build_launch_command(game, "C:\\Doom\\zandronum.exe", ADDR, "pw",
                                    mod="DOOM2.WAD")
    assert cmd[1:] == ["-iwad", "DOOM2.WAD", "+cl_password", "pw", "+cl_joinpassword", "pw",
                       "-connect", "1.2.3.4:10666"] and cwd == "C:\\Doom"
    assert build_launch_command(game, "z.exe", ADDR, None)[0][1:] == ["-connect",
                                                                     "1.2.3.4:10666"]
    info = z.parse_response(_fixture("zandronum_team.bin"), ADDR, 30)
    assert identify.identify("zandronum", info) == ["zandronum"]
    assert "zandronum" in identify.FAMILIES


def test_query_servers_against_local_fake_server():
    server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    server.bind(("127.0.0.1", 0))
    server.settimeout(3)
    port = server.getsockname()[1]
    asked = []

    def answer():
        try:
            data, addr = server.recvfrom(2048)
        except OSError:
            return
        asked.append(data)
        server.sendto(_fixture("zandronum_players.bin"), addr)

    thread = threading.Thread(target=answer)
    thread.start()
    found = []
    try:
        asyncio.run(scanner.query_servers("zandronum", [ServerAddress("127.0.0.1", port)],
                                          found.append, timeout=1.0, retries=0))
    finally:
        thread.join()
        server.close()
    assert [i.map for i in found] == ["DARK_27"]
    assert struct.unpack_from("<i", huffman.decode(asked[0]))[0] == z.SERVER_CHALLENGE


def test_busy_master_gets_its_own_status(monkeypatch):
    async def busy(host, port, timeout=3.0):
        raise z.MasterBusy("consulta ignorada")
    monkeypatch.setattr(scanner, "fetch_master_zandronum", busy)
    status = []
    result = asyncio.run(scanner.scan(GAMES["zandronum"], [("m.example", 15300)],
                                      lambda i: None, status.append))
    assert not result.ok
    assert status[0] == "El master m.example pide esperar unos segundos entre consultas"
    with pytest.raises(z.MasterBusy):
        z.parse_master_packet(huffman.encode_raw(struct.pack("<i", z.MASTER_IGNORING)))


def test_iwad_from_server_must_be_a_plain_wad_name():
    from jase.launcher import is_wad_name
    game = GAMES["zandronum"]
    for evil in ("+quit", "-file x.wad", "+logfile C:\\x\\autoexec.cfg", "..\\doom2.wad",
                 "C:\\doom2.wad", "doom2.exe", ""):
        assert not is_wad_name(evil)
        cmd, _ = build_launch_command(game, "z.exe", ADDR, None, mod=evil)
        assert cmd[1:] == ["-connect", "1.2.3.4:10666"]
    for good in ("DOOM2.WAD", "freedoom2.wad", "megagame.wad", "chex quest (v2).pk3"):
        assert is_wad_name(good)


def test_ignored_query_waits_and_retries(monkeypatch):
    monkeypatch.setattr(z, "IGNORE_WAIT_S", 0.2)
    server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    server.bind(("127.0.0.1", 0))
    server.settimeout(3)
    port = server.getsockname()[1]
    asked = []

    def answer():
        for reply in (huffman.encode_raw(struct.pack("<ii", z.SERVER_IGNORING, 0)),
                      _fixture("zandronum_players.bin")):
            try:
                _data, addr = server.recvfrom(2048)
            except OSError:
                return
            asked.append(1)
            server.sendto(reply, addr)

    thread = threading.Thread(target=answer)
    thread.start()
    found = []
    try:
        asyncio.run(scanner.query_servers("zandronum", [ServerAddress("127.0.0.1", port)],
                                          found.append, timeout=1.0, retries=0))
    finally:
        thread.join()
        server.close()
    assert len(asked) == 2 and [i.map for i in found] == ["DARK_27"]


def test_player_data_fallback_when_first_pass_raises():
    body = huffman.decode(_fixture("zandronum_team.bin"))
    info = z.parse_response(_fixture("zandronum_team.bin"), ADDR, 1)
    # Mismo paquete, pero con el modo cambiado a uno sin equipos: la primera pasada
    # (sin byte de equipo) se desalinea y el segundo intento la rescata.
    pos = body.index(b"BUR08\0") + 6 + 2  # mapa, máx. clientes y máx. jugadores
    count, pos = body[pos], pos + 1
    for _ in range(count):  # los PWAD
        pos = body.index(b"\0", pos) + 1
    assert body[pos] == 4  # teamplay
    patched = body[:pos] + b"\x03" + body[pos + 1:]  # deathmatch: sin byte de equipo
    again = z.parse_response(huffman.encode_raw(patched), ADDR, 1)
    assert [p.name for p in again.players] == [p.name for p in info.players]
