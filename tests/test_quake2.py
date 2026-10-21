import asyncio
from pathlib import Path

import pytest

from jase import scanner
from jase.games import GAMES
from jase.models import ServerAddress
from jase.protocols import ProtocolError, quake2

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 27910)


def test_master_fixture_parses():
    servers = quake2.parse_master_response((FIXTURES / "q2_master_1.bin").read_bytes())
    assert len(servers) >= 100
    assert all(s.port > 0 for s in servers)


def test_master_without_header_raises():
    with pytest.raises(ProtocolError):
        quake2.parse_master_response(b"\xff\xff\xff\xffprint\n" + b"\x01\x02\x03\x04\x6b\x86")


def test_master_decodes_entries():
    got = quake2.parse_master_response(b"\xff\xff\xff\xffservers \x01\x02\x03\x04\x6b\x86\x05")
    assert got == [ServerAddress("1.2.3.4", 27526)]


def test_status_fixture():
    info = quake2.parse_status((FIXTURES / "q2_status.bin").read_bytes(), ADDR, 42)
    assert info.name == "Feast of Frags [US] [DM] [Bots]"
    assert info.map == "act2r2"
    assert (info.num_players, info.max_players) == (6, 18)
    assert info.gametype == "action" and info.version == "q2proded r1~277d2ef"
    assert info.password is False and info.ping_ms == 42
    assert [(p.name, p.score, p.ping) for p in info.players][:2] == [("Paril", 25, 9), ("Mekhi", 6, 23)]
    assert info.players[3].name == "[DoS] Xavier"


def test_status_needpass_and_high_bit_name():
    data = (b"\xff\xff\xff\xffprint\n\\hostname\\X\\needpass\\1\\maxclients\\8\\version\\3.20 x86\n"
            b'1 5 "J\xf6rg"\n')
    info = quake2.parse_status(data, ADDR, 1)
    assert info.password is True and info.version == "3.20"
    assert info.players[0].name == "J\xf6rg"


def test_status_without_header_raises():
    with pytest.raises(ProtocolError):
        quake2.parse_status(b"\xff\xff\xff\xffinfo\n", ADDR, 1)


def test_q2_gamedef():
    game = GAMES["q2"]
    assert (game.name, game.badge, game.family) == ("Quake II", "Q2", "quake2")
    assert game.masters == (("master.quakeservers.net", 27900),)
    assert game.exe_relpath == "quake2.exe" and game.exe_candidates == ("q2pro.exe", "yquake2.exe")
    assert game.mod_key == "gamename" and game.base_mods == ("baseq2",)
    assert game.launch_style == "q3"


def test_fetch_master_dispatches_to_quake2():
    coro = scanner.fetch_master(GAMES["q2"], "127.0.0.1", 1, 0.1)
    assert coro.cr_code is scanner.fetch_master_quake2.__code__
    coro.close()


def test_fetch_master_quake2_accumulates_datagrams():
    pk = b"\xff\xff\xff\xffservers "

    class Master(asyncio.DatagramProtocol):
        def connection_made(self, transport):
            self.t = transport

        def datagram_received(self, data, addr):
            assert data == quake2.MASTER_QUERY
            self.t.sendto(pk + b"\x01\x01\x01\x01\x6b\x86", addr)
            self.t.sendto(pk + b"\x02\x02\x02\x02\x6b\x86", addr)

    async def main():
        loop = asyncio.get_running_loop()
        transport, _ = await loop.create_datagram_endpoint(Master, local_addr=("127.0.0.1", 0))
        try:
            port = transport.get_extra_info("sockname")[1]
            return await scanner.fetch_master_quake2("127.0.0.1", port, timeout=2.0)
        finally:
            transport.close()

    assert asyncio.run(main()) == [ServerAddress("1.1.1.1", 27526), ServerAddress("2.2.2.2", 27526)]


def test_query_servers_accepts_quake2_family():
    data = (FIXTURES / "q2_status.bin").read_bytes()

    class Server(asyncio.DatagramProtocol):
        def connection_made(self, transport):
            self.t = transport

        def datagram_received(self, req, addr):
            assert req == quake2.STATUS_QUERY
            self.t.sendto(data, addr)

    async def main():
        loop = asyncio.get_running_loop()
        transport, _ = await loop.create_datagram_endpoint(Server, local_addr=("127.0.0.1", 0))
        found = []
        try:
            port = transport.get_extra_info("sockname")[1]
            await scanner.query_servers("quake2", [ServerAddress("127.0.0.1", port)],
                                        found.append, timeout=1.0,
                                        **scanner.game_query_options(GAMES["q2"]))
        finally:
            transport.close()
        return found

    found = asyncio.run(main())
    assert len(found) == 1 and found[0].map == "act2r2"
