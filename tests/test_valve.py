"""Protocolo Valve (A2S / Master Server Query) y su integración en el escáner.

Fixtures reales capturados el 2026-10-19 de servidores CS 1.6 (HLDS 1.1.2.7):
- cs16_info.bin: respuesta A2S_INFO 0x49 directa (sin challenge).
- cs16_info_goldsrc.bin: respuesta 0x6D (GoldSrc obsoleta) que algunos HLDS envían
  además de la 0x49 a la misma consulta.
- cs16_player_challenge.bin / cs16_player.bin: S2C_CHALLENGE y A2S_PLAYER.
La página del master y los multipaquete se sintetizan: el master de Valve no
respondió en el sondeo (DNS NXDOMAIN) y ningún servidor envió paquetes partidos.
"""
import asyncio
import dataclasses
import struct
from pathlib import Path

import pytest

from jase import scanner
from jase.games import GAMES, GameDef
from jase.launcher import build_launch_command
from jase.models import ServerAddress
from jase.protocols import ProtocolError
from jase.protocols import valve

from test_scanner import fake_udp

FIX = Path(__file__).parent / "fixtures"
INFO = (FIX / "cs16_info.bin").read_bytes()
INFO_GOLDSRC = (FIX / "cs16_info_goldsrc.bin").read_bytes()
PLAYER_CHALLENGE = (FIX / "cs16_player_challenge.bin").read_bytes()
PLAYER = (FIX / "cs16_player.bin").read_bytes()
ADDR = ServerAddress("139.162.153.162", 27015)
T = 0.3

CS = GameDef(id="cs16t", name="CS test", family="valve", masters=(), protocols=(),
             known_versions=(), default_version="Todas", exe_relpath="hl.exe",
             launch_style="valve", valve_appid=10, valve_gamedir="cstrike")


def _master_page(addrs, end: bool) -> bytes:
    body = b"".join(bytes(int(p) for p in a.host.split(".")) + a.port.to_bytes(2, "big")
                    for a in addrs)
    if end:
        body += b"\0" * 6
    return valve.MASTER_HEADER + body


# --- master -------------------------------------------------------------------

def test_build_master_query_first_page():
    assert valve.build_master_query(None) == b"\x31\xff0.0.0.0:0\x00\\appid\\10\x00"


def test_build_master_query_seed_and_filter():
    q = valve.build_master_query(ServerAddress("1.2.3.4", 27015), "\\gamedir\\cstrike")
    assert q == b"\x31\xff1.2.3.4:27015\x00\\gamedir\\cstrike\x00"


def test_parse_master_page_continues_from_last():
    a, b = ServerAddress("1.2.3.4", 27015), ServerAddress("5.6.7.8", 27016)
    servers, seed = valve.parse_master_response(_master_page([a, b], end=False))
    assert servers == [a, b]
    assert seed == b


def test_parse_master_page_end_marker():
    a = ServerAddress("1.2.3.4", 27015)
    servers, seed = valve.parse_master_response(_master_page([a], end=True))
    assert servers == [a]
    assert seed is None


def test_parse_master_bad_header():
    with pytest.raises(ProtocolError):
        valve.parse_master_response(b"\xff\xff\xff\xffgetserversResponse")


def test_fetch_master_paginates_until_end():
    first = [ServerAddress("10.0.0.1", 27015), ServerAddress("10.0.0.2", 27015)]
    second = [ServerAddress("10.0.0.3", 27016)]

    def handler(data):
        assert data.startswith(b"\x31\xff")
        if b"0.0.0.0:0\x00" in data:
            return [_master_page(first, end=False)]
        assert b"10.0.0.2:27015\x00" in data
        return [_master_page(second, end=True)]

    async def main():
        async with fake_udp(handler) as (port, proto):
            got = await scanner.fetch_master_valve("127.0.0.1", port, "\\appid\\10",
                                                   timeout=1.0)
            return got, proto.received

    got, received = asyncio.run(main())
    assert got == first + second
    assert len(received) == 2


def test_fetch_master_stops_at_max_pages():
    n = [0]

    def handler(data):
        n[0] += 1
        return [_master_page([ServerAddress("10.0.0.%d" % n[0], 27015)], end=False)]

    async def main():
        async with fake_udp(handler) as (port, _):
            return await scanner.fetch_master_valve("127.0.0.1", port, "\\appid\\10",
                                                    timeout=1.0, max_pages=3)

    got = asyncio.run(main())
    assert [a.host for a in got] == ["10.0.0.1", "10.0.0.2", "10.0.0.3"]


def test_fetch_master_partial_on_timeout():
    def handler(data):
        if b"0.0.0.0:0\x00" in data:
            return [_master_page([ServerAddress("10.0.0.1", 27015)], end=False)]
        return []  # el master se calla (throttle)

    async def main():
        async with fake_udp(handler) as (port, _):
            return await scanner.fetch_master_valve("127.0.0.1", port, "\\appid\\10",
                                                    timeout=T)

    assert asyncio.run(main()) == [ServerAddress("10.0.0.1", 27015)]


def test_fetch_master_silent_raises():
    async def main():
        async with fake_udp(lambda d: []) as (port, _):
            await scanner.fetch_master_valve("127.0.0.1", port, "\\appid\\10", timeout=T)

    with pytest.raises(TimeoutError):
        asyncio.run(main())


def test_fetch_master_dispatch_uses_appid():
    coro = scanner.fetch_master(CS, "127.0.0.1", 1, 0.1)
    assert coro.cr_code is scanner.fetch_master_valve.__code__
    coro.close()
    with pytest.raises(ValueError):
        scanner.fetch_master(dataclasses.replace(CS, valve_appid=0), "h", 1, 0.1)


# --- A2S_INFO -----------------------------------------------------------------

def test_parse_info_source_format_real():
    info = valve.parse_info(INFO, ADDR, 42)
    assert info.name == "ROMANIA.FULLBOOST.RO V.I.P FREE"
    assert info.map == "de_dust2"
    assert info.gametype == "Hosted by STRONG-HOST.DE"
    assert info.num_players == 30 and info.max_players == 32
    assert info.password is False
    assert info.version == "1.1.2.7/Stdio"
    assert info.ping_ms == 42
    assert info.address == ADDR and info.connect_address == ADDR
    assert info.rules["gamedir"] == "cstrike"
    assert info.rules["appid"] == "10"


def test_parse_info_goldsrc_format_real():
    info = valve.parse_info(INFO_GOLDSRC, ServerAddress("51.38.60.53", 27015), 5)
    assert info.name.startswith("EMIRATES.KiNGS \u4e97 GAMEAREA")  # UTF-8
    assert info.map == "de_dust2"
    assert info.gametype == "Counter-Strike"
    assert info.num_players == 31 and info.max_players == 32
    assert info.password is False
    assert info.rules["gamedir"] == "cstrike"


def _source_info(name=b"Srv", password=1, players=3, bots=1) -> bytes:
    return (valve.OOB + b"I\x30" + name + b"\0de_nuke\0cstrike\0Counter-Strike\0"
            + struct.pack("<h", 10) + bytes([players, 16, bots]) + b"dw"
            + bytes([password, 0]) + b"1.1.2.7\0")


def test_parse_info_password_and_bots():
    info = valve.parse_info(_source_info(), ADDR, 1)
    assert info.password is True
    assert info.num_players == 3 and info.max_players == 16
    assert info.rules["bots"] == "1"


def test_parse_info_gametype_falls_back_to_gamedir():
    data = (valve.OOB + b"I\x30Srv\0de_nuke\0cstrike\0\0" + struct.pack("<h", 10)
            + bytes([0, 16, 0]) + b"dl\x00\x001.0\0")
    assert valve.parse_info(data, ADDR, 1).gametype == "cstrike"


def test_parse_info_latin1_fallback():
    info = valve.parse_info(_source_info(name=b"Caf\xe9"), ADDR, 1)
    assert info.name == "Caf\xe9"


@pytest.mark.parametrize("data", [b"", valve.OOB + b"I", valve.OOB + b"I\x30abc",
                                  valve.OOB + b"Z\x00", INFO[:20]])
def test_parse_info_malformed(data):
    with pytest.raises(ProtocolError):
        valve.parse_info(data, ADDR, 1)


def test_info_challenge_packet():
    assert valve.A2S_INFO == b"\xff\xff\xff\xffTSource Engine Query\x00"
    assert valve.challenge_of(PLAYER_CHALLENGE) == PLAYER_CHALLENGE[5:9]
    assert valve.challenge_of(INFO) is None
    assert valve.build_info_query(b"\x01\x02\x03\x04") == valve.A2S_INFO + b"\x01\x02\x03\x04"
    assert valve.build_player_query(None) == b"\xff\xff\xff\xffU\xff\xff\xff\xff"


# --- A2S_PLAYER ---------------------------------------------------------------

def test_parse_players_real():
    players = valve.parse_players(PLAYER)
    assert len(players) == 30
    assert players[0].name == "IUBESC CS" and players[0].score == 0
    assert players[1].name == "\u0274\u1d0f\u1d0f\u0299\u026a\u1d07" and players[1].score == 14
    assert players[2].name == "rip" and players[2].score == 19
    assert all(p.ping == 0 for p in players)


def test_parse_players_truncated():
    with pytest.raises(ProtocolError):
        valve.parse_players(PLAYER[:40])
    with pytest.raises(ProtocolError):
        valve.parse_players(INFO)


# --- multipaquete -------------------------------------------------------------

def _goldsrc_split(payload: bytes, n: int, req_id: int = 7, order=None) -> list[bytes]:
    size = -(-len(payload) // n)
    chunks = [payload[i * size:(i + 1) * size] for i in range(n)]
    pkts = [valve.SPLIT + struct.pack("<l", req_id) + bytes([(i << 4) | n]) + c
            for i, c in enumerate(chunks)]
    return [pkts[i] for i in (order or range(n))]


def _source_split(payload: bytes, n: int, req_id: int = 9) -> list[bytes]:
    size = -(-len(payload) // n)
    chunks = [payload[i * size:(i + 1) * size] for i in range(n)]
    return [valve.SPLIT + struct.pack("<lBBh", req_id, n, i, 1248) + c
            for i, c in enumerate(chunks)]


def test_assembler_single_packet():
    asm = valve.PacketAssembler()
    assert asm.feed(INFO) == INFO


def test_assembler_goldsrc_split_out_of_order():
    asm = valve.PacketAssembler()
    pkts = _goldsrc_split(PLAYER, 3, order=[2, 0, 1])
    assert asm.feed(pkts[0]) is None
    assert asm.feed(pkts[1]) is None
    assert asm.feed(pkts[2]) == PLAYER
    assert len(valve.parse_players(PLAYER)) == 30


def test_assembler_source_split():
    asm = valve.PacketAssembler()
    pkts = _source_split(PLAYER, 2)
    assert asm.feed(pkts[1]) is None
    assert asm.feed(pkts[0]) == PLAYER


def test_assembler_compressed_source_split_rejected():
    asm = valve.PacketAssembler()
    pkt = valve.SPLIT + struct.pack("<lBBh", -0x7FFFFFFF, 1, 0, 1248) + b"\0" * 8 + INFO
    with pytest.raises(ProtocolError):
        asm.feed(pkt)


def test_assembler_garbage():
    with pytest.raises(ProtocolError):
        valve.PacketAssembler().feed(b"\x01\x02\x03\x04\x05")


# --- escáner ------------------------------------------------------------------

def _query(handler, **kw):
    async def main():
        async with fake_udp(handler) as (port, proto):
            found = []
            done = []
            await scanner.query_servers("valve", [ServerAddress("127.0.0.1", port)],
                                        found.append, timeout=kw.get("timeout", T),
                                        retries=kw.get("retries", 1),
                                        on_done=lambda: done.append(1))
            return found, proto.received, done
    return asyncio.run(main())


def test_query_info_and_players_with_challenge():
    challenge = b"\xaa\xbb\xcc\xdd"

    def handler(data):
        if data == valve.A2S_INFO:
            return [valve.OOB + b"A" + challenge]
        if data == valve.A2S_INFO + challenge:
            return [INFO]
        if data == valve.build_player_query(None):
            return [valve.OOB + b"A" + challenge]
        if data == valve.build_player_query(challenge):
            return _goldsrc_split(PLAYER, 2)
        return []

    found, received, done = _query(handler)
    assert len(found) == 1 and done == [1]
    info = found[0]
    assert info.name == "ROMANIA.FULLBOOST.RO V.I.P FREE"
    assert len(info.players) == 30
    assert received == [valve.A2S_INFO, valve.A2S_INFO + challenge,
                        valve.build_player_query(None), valve.build_player_query(challenge)]


def test_query_direct_info_double_reply_and_no_players():
    """HLDS que responde 0x6D y 0x49 a la vez y no contesta A2S_PLAYER."""

    def handler(data):
        if data == valve.A2S_INFO:
            return [INFO_GOLDSRC, INFO]
        return []

    found, received, done = _query(handler)
    assert len(found) == 1 and done == [1]
    assert found[0].version == "1.1.2.7/Stdio"  # la 0x49 completa a la 0x6D
    assert found[0].players == []
    assert received.count(valve.A2S_INFO) == 1  # sin reintentos de INFO


def test_query_goldsrc_first_completed_by_late_source_reply():
    challenge = b"\x01\x02\x03\x04"

    def handler(data):
        if data == valve.A2S_INFO:
            return [INFO_GOLDSRC, INFO]
        if data == valve.build_player_query(None):
            return [valve.OOB + b"A" + challenge]
        if data == valve.build_player_query(challenge):
            return [PLAYER]
        return []

    found, _, _ = _query(handler)
    assert len(found) == 1
    assert found[0].version == "1.1.2.7/Stdio"  # de la 0x49 tardía
    assert len(found[0].players) == 30


def test_query_info_retry_counts_once():
    calls = [0]

    def handler(data):
        if data == valve.A2S_INFO:
            calls[0] += 1
            return [] if calls[0] == 1 else [INFO]
        return []

    found, received, done = _query(handler)
    assert len(found) == 1 and done == [1]
    assert received.count(valve.A2S_INFO) == 2


def test_query_silent_server():
    found, received, done = _query(lambda d: [])
    assert found == [] and done == [1]
    assert received == [valve.A2S_INFO, valve.A2S_INFO]  # 1 intento + 1 reintento


def test_query_malformed_info():
    found, _, done = _query(lambda d: [valve.OOB + b"I\x30x"] if d == valve.A2S_INFO else [])
    assert found == [] and done == [1]


class _ChallengeDelay(asyncio.DatagramProtocol):
    """Challenge inmediato; INFO final con retraso fijo (ping de la 2.ª ida y vuelta)."""

    def __init__(self, delay):
        self.delay = delay

    def connection_made(self, transport):
        self.transport = transport

    def datagram_received(self, data, addr):
        loop = asyncio.get_running_loop()
        if data == valve.A2S_INFO:
            loop.call_later(0.2, self.transport.sendto, valve.OOB + b"A1234", addr)
        elif data == valve.A2S_INFO + b"1234":
            loop.call_later(self.delay, self.transport.sendto, INFO, addr)


def test_ping_measured_on_final_exchange():
    async def main():
        loop = asyncio.get_running_loop()
        transport, _ = await loop.create_datagram_endpoint(
            lambda: _ChallengeDelay(0.05), local_addr=("127.0.0.1", 0))
        port = transport.get_extra_info("sockname")[1]
        found = []
        try:
            await scanner.query_servers("valve", [ServerAddress("127.0.0.1", port)],
                                        found.append, timeout=1.0, retries=0)
        finally:
            transport.close()
        return found

    found = asyncio.run(main())
    assert len(found) == 1
    assert 40 <= found[0].ping_ms < 180  # sin los 200 ms del challenge


def test_scan_valve_end_to_end():
    async def main():
        async with fake_udp(lambda d: [INFO] if d == valve.A2S_INFO else []) as (sport, _):
            page = _master_page([ServerAddress("127.0.0.1", sport)], end=True)
            async with fake_udp(lambda d: [page]) as (mport, _):
                found, statuses, progress = [], [], []
                result = await scanner.scan(CS, [("127.0.0.1", mport)], found.append,
                                            statuses.append, master_timeout=1.0,
                                            query_timeout=T,
                                            on_progress=lambda d, t: progress.append((d, t)))
                return result, found, progress

    result, found, progress = asyncio.run(main())
    assert result.servers_listed == 1 and result.servers_responded == 1
    assert found[0].map == "de_dust2"
    assert progress == [(0, 1), (1, 1)]


# --- lanzamiento y juego --------------------------------------------------------

def test_launch_command_valve():
    argv, cwd = build_launch_command(CS, r"D:\Juegos\CS16\hl.exe",
                                     ServerAddress("1.2.3.4", 27015), None)
    assert argv == [r"D:\Juegos\CS16\hl.exe", "-game", "cstrike", "+connect", "1.2.3.4:27015"]
    assert cwd == r"D:\Juegos\CS16"


def test_launch_command_valve_password():
    argv, _ = build_launch_command(CS, r"C:\hl.exe", ServerAddress("1.2.3.4", 27015), "x y")
    assert argv == [r"C:\hl.exe", "-game", "cstrike", "+password", "x y",
                    "+connect", "1.2.3.4:27015"]


def test_cs16_not_registered_without_master():
    # Sondeo 2026-10-19: hl1master/hl2master.steampowered.com sin DNS → no se agrega.
    assert not any(g.family == "valve" for g in GAMES.values())
