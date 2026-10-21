import asyncio
from pathlib import Path

import pytest

from jase import launcher, scanner
from jase.games import ALL_VERSIONS, GAMES
from jase.models import ServerAddress
from jase.protocols import ProtocolError, ut2004

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 7778)  # puerto de consulta


def _master_frames() -> list[bytes]:
    frames, rest = ut2004.split_frames((FIXTURES / "ut2004_master_1.bin").read_bytes())
    assert rest == b""
    return frames


def _fstring(text: str) -> bytes:
    raw = text.encode("latin-1") + b"\0"
    return bytes([len(raw)]) + raw


def test_fake_hashes_are_fixed_fictitious_hex():
    for value in (ut2004.FAKE_CDKEY_HASH, ut2004.FAKE_RESPONSE_HASH):
        assert len(value) == 32 and set(value) <= set("0123456789abcdef")
    assert ut2004.FAKE_CDKEY_HASH == "0" * 32 and ut2004.FAKE_RESPONSE_HASH == "0" * 32


def test_challenge_response_exact_bytes():
    zeros = b"\x21" + b"0" * 32 + b"\0"
    body = (zeros + zeros + b"\x0cUT2K4CLIENT\0" + (3369).to_bytes(4, "little") + b"\x03"
            + b"\x04int\0" + b"\0" * 12 + b"\0")
    assert ut2004.build_challenge_response() == len(body).to_bytes(4, "little") + body


def test_list_request_has_no_filters():
    assert ut2004.LIST_REQUEST == b"\x02\0\0\0\0\0"


def test_compact_index_round_trip():
    for value in (0, 1, 63, 64, 76, 417, 8191, 8192, 1_000_000):
        assert ut2004.read_compact_int(ut2004.compact_int(value), 0) == (value, len(ut2004.compact_int(value)))
    assert ut2004.read_compact_int(b"\x61\x06", 0) == (417, 2)


def test_read_fstring_latin1_utf16_and_truncated():
    assert ut2004.read_fstring(b"\x05J\xf6rg\0", 0) == ("J\xf6rg", 6)
    assert ut2004.read_fstring(b"\x83" + "Añ\0".encode("utf-16-le"), 0) == ("Añ", 7)
    with pytest.raises(ProtocolError):
        ut2004.read_fstring(b"\x09abc", 0)


def test_master_fixture_handshake_frames():
    frames = _master_frames()
    assert ut2004.read_fstring(frames[0], 0)[0]  # challenge
    ut2004.check_reply(frames[1], "APPROVED")
    ut2004.check_reply(frames[2], "VERIFIED")
    assert ut2004.parse_list_header(frames[3]) == len(frames) - 4


def test_master_fixture_entries():
    entries = [ut2004.parse_list_entry(f) for f in _master_frames()[4:]]
    assert len(entries) > 500
    assert all(0 < e.port < 65536 for e in entries)
    assert entries[0] == ServerAddress("79.24.244.55", 7780)


def test_check_reply_denied_raises():
    with pytest.raises(ProtocolError):
        ut2004.check_reply(_fstring("DENIED"), "APPROVED")


def test_list_entry_truncated_raises():
    with pytest.raises(ProtocolError):
        ut2004.parse_list_entry(b"\x01\x02\x03")


def test_parse_info_fixture():
    info = ut2004.parse_info((FIXTURES / "ut2004_info.bin").read_bytes(), ADDR, 42)
    assert info.name == "UT2004 3374p23 - Brasil - Multimode"
    assert info.map == "ONS-ArcticStronghold"
    assert info.gametype == "ONSOnslaughtGame"
    assert (info.num_players, info.max_players) == (10, 16)
    assert info.ping_ms == 42 and info.password is False
    assert info.address == ADDR
    assert info.connect_address == ServerAddress("1.2.3.4", 7777)
    assert info.connect_address.port == ADDR.port - 1


def test_parse_info_strips_colour_codes():
    data = (b"\x80\0\0\0\0" + b"\0" * 4 + b"\x01\0" + (7777).to_bytes(4, "little") + b"\0" * 4
            + b"\x0d\x1b\xff\x01\x01Red\x1b\x01\x01\xffX\0" + _fstring("DM-Rankin") + _fstring("xDeathMatch")
            + (1).to_bytes(4, "little") + (8).to_bytes(4, "little") + b"\0" * 8)
    info = ut2004.parse_info(data, ADDR, 1)
    assert info.name == "RedX" and info.map == "DM-Rankin"


def test_parse_players_names_utf8_or_latin1():
    data = (b"\x80\0\0\0\x02" + (1).to_bytes(4, "little") + b"\x0eRetro\xc2\xa0Heaven\0"
            + b"\0" * 12 + (2).to_bytes(4, "little") + b"\x05J\xf6rg\0" + b"\0" * 12)
    assert [p.name for p in ut2004.parse_players(data)] == ["Retro\xa0Heaven", "J\xf6rg"]


def test_parse_info_truncated_raises():
    data = (FIXTURES / "ut2004_info.bin").read_bytes()
    with pytest.raises(ProtocolError):
        ut2004.parse_info(data[:40], ADDR, 1)
    with pytest.raises(ProtocolError):
        ut2004.parse_info(b"\x80\0\0\0\x02" + data[5:], ADDR, 1)


def test_parse_players_fixture_skips_team_scores():
    players = ut2004.parse_players((FIXTURES / "ut2004_players.bin").read_bytes())
    assert [(p.name, p.score, p.ping) for p in players] == [("Player1", 0, 96)]


def test_parse_players_truncated_raises():
    data = (FIXTURES / "ut2004_players.bin").read_bytes()
    with pytest.raises(ProtocolError):
        ut2004.parse_players(data[:20])


def test_parse_rules_joins_repeated_keys_and_password():
    data = (b"\x80\0\0\0\x01" + _fstring("ServerVersion") + _fstring("3369")
            + _fstring("Mutator") + _fstring("MutA") + _fstring("Mutator") + _fstring("MutB")
            + _fstring("GamePassword") + _fstring("True"))
    rules = ut2004.parse_rules(data)
    assert rules == {"ServerVersion": "3369", "Mutator": "MutA, MutB", "GamePassword": "True"}
    info = ut2004.parse_info((FIXTURES / "ut2004_info.bin").read_bytes(), ADDR, 1)
    ut2004.apply_details(info, rules, [])
    assert info.password is True and info.version == "3369" and info.rules == rules


def test_ut2004_gamedef():
    game = GAMES["ut2004"]
    assert (game.name, game.short_name, game.badge, game.family) == (
        "Unreal Tournament 2004", "UT2004", "U4", "ut2004")
    assert game.masters == (("utmaster.openspy.net", 28902), ("ut2004master.333networks.com", 28902))
    assert game.exe_relpath == "System\\UT2004.exe" and game.launch_style == "unreal"
    assert game.default_version == ALL_VERSIONS


def test_launch_with_password():
    exe = "C:\\Games\\UT2004\\System\\UT2004.exe"
    args, cwd = launcher.build_launch_command(GAMES["ut2004"], exe,
                                              ServerAddress("1.2.3.4", 7777), "pw")
    assert args == [exe, "1.2.3.4:7777?password=pw"]
    assert cwd == "C:\\Games\\UT2004\\System"


def test_fetch_master_dispatches_to_ut2004():
    coro = scanner.fetch_master(GAMES["ut2004"], "127.0.0.1", 1, 0.1)
    assert coro.cr_code is scanner.fetch_master_ut2004.__code__
    coro.close()


def _serve(frames: list[bytes], received: list[bytes]):
    async def handle(reader, writer):
        writer.write(ut2004.frame(frames[0]))
        for reply in ([frames[1]], [frames[2]], frames[3:]):
            n = int.from_bytes(await reader.readexactly(4), "little")
            received.append(await reader.readexactly(n))
            for body in reply:
                writer.write(ut2004.frame(body))
        await writer.drain()
        writer.close()
    return handle


def test_fetch_master_ut2004_replays_fixture():
    frames = _master_frames()
    received: list[bytes] = []

    async def main():
        server = await asyncio.start_server(_serve(frames, received), "127.0.0.1", 0)
        async with server:
            port = server.sockets[0].getsockname()[1]
            return await scanner.fetch_master_ut2004("127.0.0.1", port, timeout=3.0)

    servers = asyncio.run(main())
    assert len(servers) == len(set(servers)) > 500
    assert received[0] == ut2004.build_challenge_response()[4:]
    assert received[2] == ut2004.LIST_REQUEST[4:]


def test_fetch_master_ut2004_denied_raises():
    async def handle(reader, writer):
        writer.write(ut2004.frame(_fstring("111111111")))
        n = int.from_bytes(await reader.readexactly(4), "little")
        await reader.readexactly(n)
        writer.write(ut2004.frame(_fstring("DENIED")))
        await writer.drain()
        writer.close()

    async def main():
        server = await asyncio.start_server(handle, "127.0.0.1", 0)
        async with server:
            port = server.sockets[0].getsockname()[1]
            return await scanner.fetch_master_ut2004("127.0.0.1", port, timeout=3.0)

    with pytest.raises(ProtocolError):
        asyncio.run(main())


def test_query_servers_ut2004_combines_packets():
    info = (FIXTURES / "ut2004_info.bin").read_bytes()
    players = (FIXTURES / "ut2004_players.bin").read_bytes()
    rules = b"\x80\0\0\0\x01" + _fstring("ServerVersion") + _fstring("3374")

    class Server(asyncio.DatagramProtocol):
        def connection_made(self, transport):
            self.t = transport

        def datagram_received(self, req, addr):
            reply = {ut2004.INFO_QUERY: info, ut2004.RULES_QUERY: rules,
                     ut2004.PLAYERS_QUERY: players}[req]
            self.t.sendto(reply, addr)

    async def main():
        loop = asyncio.get_running_loop()
        transport, _ = await loop.create_datagram_endpoint(Server, local_addr=("127.0.0.1", 0))
        found = []
        try:
            port = transport.get_extra_info("sockname")[1]
            await scanner.query_servers("ut2004", [ServerAddress("127.0.0.1", port)],
                                        found.append, timeout=1.0,
                                        **scanner.game_query_options(GAMES["ut2004"]))
        finally:
            transport.close()
        return found, port

    found, port = asyncio.run(main())
    assert len(found) == 1
    got = found[0]
    assert got.map == "ONS-ArcticStronghold" and got.version == "3374"
    assert [p.name for p in got.players] == ["Player1"]
    assert got.connect_address == ServerAddress("127.0.0.1", 7777)
    assert got.address == ServerAddress("127.0.0.1", port)
