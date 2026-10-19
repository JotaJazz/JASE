import asyncio
import contextlib
import dataclasses
import socket
import time
from pathlib import Path

from jase.games import GAMES
from jase.models import Player, ServerAddress
from jase.protocols import gamespy as gs
from jase.protocols import quake3 as q3
from jase import scanner

# Fixtures copiados de tests/test_quake3.py y tests/test_gamespy.py
STATUS = (b'\xff\xff\xff\xffstatusResponse\n'
          b'\\sv_hostname\\^1Jimmy^7 TDM\\mapname\\mp_carentan\\g_gametype\\tdm'
          b'\\sv_maxclients\\20\\pswrd\\1\\shortversion\\1.3\n'
          b'24 41 "Sgt.^2Jimmy"\n0 999 "Kowalski \\"K\\""\n')
P1 = (b"\\hostname\\Jimmy UT\\hostport\\7777\\mapname\\DM-Deck16][\\gametype\\DeathMatchPlus"
      b"\\numplayers\\1\\maxplayers\\16\\password\\True\\gamever\\469\\queryid\\7.1")
P2 = b"\\player_0\\Jimmy\\frags_0\\12\\ping_0\\40\\queryid\\7.2\\final\\"

T = 0.3  # timeouts cortos para tests


def _entry(addr: ServerAddress) -> bytes:
    ip = bytes(int(p) for p in addr.host.split("."))
    return b"\\" + ip + addr.port.to_bytes(2, "big")


def _master_packet(addrs, eot: bool) -> bytes:
    data = q3.OOB + b"getserversResponse" + b"".join(_entry(a) for a in addrs)
    if eot:
        data += b"\\EOT\x00\x00\x00"
    return data


class _FakeUDP(asyncio.DatagramProtocol):
    def __init__(self, handler):
        self.handler = handler
        self.received: list[bytes] = []

    def connection_made(self, transport):
        self.transport = transport

    def datagram_received(self, data, addr):
        self.received.append(data)
        for reply in self.handler(data):
            self.transport.sendto(reply, addr)


@contextlib.asynccontextmanager
async def fake_udp(handler):
    loop = asyncio.get_running_loop()
    transport, proto = await loop.create_datagram_endpoint(
        lambda: _FakeUDP(handler), local_addr=("127.0.0.1", 0))
    try:
        yield transport.get_extra_info("sockname")[1], proto
    finally:
        transport.close()


def dead_udp_port() -> int:
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def test_quake3_master_multi_packet():
    a1, a2, a3 = (ServerAddress("1.2.3.4", 28960), ServerAddress("5.6.7.92", 28961),
                  ServerAddress("9.9.9.9", 28962))

    def handler(data):
        if data == q3.build_getservers(118):
            return [_master_packet([a1, a2], False), _master_packet([a3], True)]
        return []

    async def main():
        async with fake_udp(handler) as (port, _):
            return await scanner.fetch_master_quake3("127.0.0.1", port, [118], timeout=T)

    assert sorted(asyncio.run(main()), key=str) == sorted([a1, a2, a3], key=str)


def test_quake3_master_dedupes_across_protocols():
    a = ServerAddress("1.2.3.4", 28960)
    b = ServerAddress("4.3.2.1", 28960)

    def handler(data):
        if data == q3.build_getservers(117):
            return [_master_packet([a], True)]
        if data == q3.build_getservers(118):
            return [_master_packet([a, b], True)]
        return []

    async def main():
        async with fake_udp(handler) as (port, _):
            return await scanner.fetch_master_quake3("127.0.0.1", port, [117, 118], timeout=T)

    result = asyncio.run(main())
    assert sorted(result, key=str) == sorted([a, b], key=str)


def test_gamespy_master_handshake():
    received = bytearray()

    async def handle(reader, writer):
        writer.write(b"\\basic\\\\secure\\ABCDEF")
        await writer.drain()
        while b"\\list\\" not in received or not received.endswith(b"\\final\\"):
            chunk = await reader.read(1024)
            if not chunk:
                break
            received.extend(chunk)
        writer.write(b"\\ip\\127.0.0.1:7778\\final\\")
        await writer.drain()
        writer.close()
        await writer.wait_closed()

    async def main():
        server = await asyncio.start_server(handle, "127.0.0.1", 0)
        port = server.sockets[0].getsockname()[1]
        async with server:
            return await scanner.fetch_master_gamespy("127.0.0.1", port, timeout=T)

    assert asyncio.run(main()) == [ServerAddress("127.0.0.1", 7778)]
    assert b"\\validate\\" + gs.gsmsalg("ABCDEF", "Z5Nfb0").encode() in received
    assert b"\\list\\\\gamename\\ut" in received


def test_query_servers_quake3():
    got = []
    retries = 1

    async def main():
        async with fake_udp(lambda d: [STATUS] if d == q3.GETSTATUS else []) as (good, _), \
                   fake_udp(lambda d: [b"garbage"]) as (bad, _):
            addrs = [ServerAddress("127.0.0.1", good), ServerAddress("127.0.0.1", bad),
                     ServerAddress("127.0.0.1", dead_udp_port())]
            await scanner.query_servers("quake3", addrs, got.append, timeout=T, retries=retries)

    start = time.monotonic()
    asyncio.run(main())
    elapsed = time.monotonic() - start
    assert len(got) == 1
    assert got[0].name == "Jimmy TDM" and got[0].ping_ms >= 0
    assert elapsed < T * (retries + 1) + 1


def test_query_servers_gamespy_multipacket():
    got = []

    async def main():
        async with fake_udp(lambda d: [P2, P1] if d == gs.STATUS_QUERY else []) as (port, _):
            await scanner.query_servers("gamespy", [ServerAddress("127.0.0.1", port)],
                                        got.append, timeout=T)

    asyncio.run(main())
    assert len(got) == 1
    assert got[0].players == [Player("Jimmy", 12, 40)]
    assert got[0].name == "Jimmy UT"


def test_scan_survives_dead_master():
    got, status = [], []
    game = dataclasses.replace(GAMES["cod2"], protocols=(118,))

    async def main():
        async with fake_udp(lambda d: [STATUS] if d == q3.GETSTATUS else []) as (srv, _):
            server = ServerAddress("127.0.0.1", srv)
            dead = ServerAddress("127.0.0.1", dead_udp_port())
            handler = (lambda d: [_master_packet([server, dead], True)]
                       if d.startswith(q3.OOB + b"getservers ") else [])
            async with fake_udp(handler) as (mport, _):
                return await scanner.scan(
                    game, [("nonexistent.invalid", 20710), ("127.0.0.1", mport)],
                    got.append, status.append, master_timeout=T, query_timeout=T)

    result = asyncio.run(main())
    assert result == scanner.ScanResult(masters_ok=1, servers_listed=2, servers_responded=1)
    assert result.ok
    assert any("nonexistent.invalid" in s for s in status)
    assert "Master nonexistent.invalid no responde" in status
    assert "2 servidores en el master, consultando…" in status
    assert [s.name for s in got] == ["Jimmy TDM"]


def test_scan_all_masters_dead():
    got, status = [], []
    game = GAMES["cod2"]

    async def main():
        return await scanner.scan(game, [("127.0.0.1", dead_udp_port())], got.append,
                                  status.append, master_timeout=T, query_timeout=T)

    result = asyncio.run(main())
    assert result == scanner.ScanResult(masters_ok=0, servers_listed=0, servers_responded=0)
    assert not result.ok
    assert "Ningún master respondió" in status
    assert got == []


def test_scan_gamespy_end_to_end():
    got, status = [], []

    async def main():
        async with fake_udp(lambda d: [P1, P2] if d == gs.STATUS_QUERY else []) as (srv, _):
            async def handle(reader, writer):
                writer.write(b"\\basic\\\\secure\\ABCDEF")
                await reader.readuntil(b"\\list\\")
                writer.write(f"\\ip\\127.0.0.1:{srv}\\ip\\127.0.0.1:{srv}".encode())
                writer.close()  # cierra sin \final\: se usa lo recibido
                await writer.wait_closed()

            server = await asyncio.start_server(handle, "127.0.0.1", 0)
            port = server.sockets[0].getsockname()[1]
            async with server:
                await scanner.scan(GAMES["ut99"], [("127.0.0.1", port)], got.append,
                                   status.append, master_timeout=T, query_timeout=T)

    asyncio.run(main())
    assert "1 servidores en el master, consultando…" in status
    assert [s.name for s in got] == ["Jimmy UT"]


def test_scan_cancel_closes_cleanly():
    status = []
    game = dataclasses.replace(GAMES["cod2"], protocols=(118,))

    async def main():
        async with fake_udp(lambda d: []) as (mport, _):  # master que nunca responde
            task = asyncio.create_task(scanner.scan(
                game, [("127.0.0.1", mport)], lambda s: None, status.append,
                master_timeout=5, query_timeout=5))
            await asyncio.sleep(0.1)
            task.cancel()
            try:
                await task
            except asyncio.CancelledError:
                return True
            return False

    assert asyncio.run(main()) is True


def test_on_server_exception_propagates():
    class Boom(Exception):
        pass

    def explode(info):
        raise Boom

    async def main():
        async with fake_udp(lambda d: [STATUS]) as (port, _):
            await scanner.query_servers("quake3", [ServerAddress("127.0.0.1", port)],
                                        explode, timeout=T)

    try:
        asyncio.run(main())
    except Boom:
        pass
    else:
        raise AssertionError("Boom no se propagó")


def test_scan_cancel_during_queries():
    game = dataclasses.replace(GAMES["cod2"], protocols=(118,))

    async def main():
        async with fake_udp(lambda d: []) as (srv, _):  # servidor mudo
            server = ServerAddress("127.0.0.1", srv)
            handler = lambda d: [_master_packet([server], True)]
            async with fake_udp(handler) as (mport, _):
                status = []
                task = asyncio.create_task(scanner.scan(
                    game, [("127.0.0.1", mport)], lambda s: None, status.append,
                    master_timeout=5, query_timeout=5))
                loop = asyncio.get_running_loop()
                deadline = loop.time() + 2
                while "1 servidores en el master, consultando…" not in status:
                    assert loop.time() < deadline, status
                    await asyncio.sleep(0.01)
                await asyncio.sleep(0.05)  # ya en fase de consulta
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    return True
                return False

    assert asyncio.run(main()) is True


class _FakeSock:
    def fileno(self):
        return 1234


def test_disable_udp_connreset_calls_wsaioctl_on_windows(monkeypatch):
    calls = []

    def fake_wsaioctl(handle, code, in_buf, in_len, out_buf, out_len, returned, ov, cb):
        calls.append((handle, code, in_buf.contents.value, in_len, out_buf, out_len))
        return 0

    monkeypatch.setattr(scanner, "_IS_WINDOWS", True)
    monkeypatch.setattr(scanner, "_load_wsaioctl", lambda: fake_wsaioctl)
    scanner._disable_udp_connreset(_FakeSock())
    assert calls == [(1234, 0x9800000C, 0, 4, None, 0)]
    assert scanner.SIO_UDP_CONNRESET == 0x9800000C


def test_disable_udp_connreset_swallows_failures(monkeypatch, caplog):
    def broken():
        raise OSError("sin ws2_32")

    monkeypatch.setattr(scanner, "_IS_WINDOWS", True)
    monkeypatch.setattr(scanner, "_load_wsaioctl", broken)
    with caplog.at_level("DEBUG", logger="jase.scanner"):
        scanner._disable_udp_connreset(_FakeSock())  # no lanza
    monkeypatch.setattr(scanner, "_load_wsaioctl",
                        lambda: (lambda *a: -1))  # WSAIoctl devuelve SOCKET_ERROR
    with caplog.at_level("DEBUG", logger="jase.scanner"):
        scanner._disable_udp_connreset(_FakeSock())
    assert len([r for r in caplog.records if r.name == "jase.scanner"]) == 2


def test_disable_udp_connreset_noop_off_windows(monkeypatch):
    def unexpected():
        raise AssertionError("no debe cargar ws2_32 fuera de Windows")

    monkeypatch.setattr(scanner, "_IS_WINDOWS", False)
    monkeypatch.setattr(scanner, "_load_wsaioctl", unexpected)
    scanner._disable_udp_connreset(_FakeSock())


def test_query_servers_applies_connreset_to_shared_socket(monkeypatch):
    seen = []
    monkeypatch.setattr(scanner, "_disable_udp_connreset", lambda s: seen.append(s))

    async def main():
        await scanner.query_servers("quake3", [], lambda i: None, timeout=T)

    asyncio.run(main())
    assert len(seen) == 1 and seen[0].type == socket.SOCK_DGRAM


class _SecondTryServer(asyncio.DatagramProtocol):
    """Ignora la primera consulta y responde a la segunda tras un retardo."""

    def __init__(self, delay):
        self.delay = delay
        self.count = 0

    def connection_made(self, transport):
        self.transport = transport

    def datagram_received(self, data, addr):
        self.count += 1
        if self.count == 2:
            asyncio.get_running_loop().call_later(
                self.delay, self.transport.sendto, STATUS, addr)


def test_retry_ping_measured_from_first_send():
    got = []

    async def main():
        loop = asyncio.get_running_loop()
        transport, _ = await loop.create_datagram_endpoint(
            lambda: _SecondTryServer(0.05), local_addr=("127.0.0.1", 0))
        try:
            port = transport.get_extra_info("sockname")[1]
            await scanner.query_servers("quake3", [ServerAddress("127.0.0.1", port)],
                                        got.append, timeout=T, retries=1)
        finally:
            transport.close()

    asyncio.run(main())
    assert len(got) == 1
    assert got[0].ping_ms >= T * 1000  # ~350 ms: desde el primer envío, no ~50 ms


def test_ping_uses_perf_counter_clock(monkeypatch):
    assert scanner._clock is time.perf_counter  # resolución fina también en Windows
    ticks = iter([10.0, 10.25])  # envío, recepción
    monkeypatch.setattr(scanner, "_clock", lambda: next(ticks))
    got = []

    async def main():
        async with fake_udp(lambda d: [STATUS]) as (port, _):
            await scanner.query_servers("quake3", [ServerAddress("127.0.0.1", port)],
                                        got.append, timeout=T, retries=0)

    asyncio.run(main())
    assert [s.ping_ms for s in got] == [250]


def test_quake3_master_real_cod2_packets_collects_all_until_eof():
    fixtures = Path(__file__).parent / "fixtures"
    packets = [(fixtures / f"cod2_master_118_{i}.bin").read_bytes() for i in range(1, 5)]
    expected = set()
    for p in packets:
        expected.update(q3.parse_getservers_response(p)[0])

    def handler(data):
        return packets if data == q3.build_getservers(118) else []

    async def main():
        async with fake_udp(handler) as (port, _):
            t0 = time.monotonic()
            result = await scanner.fetch_master_quake3("127.0.0.1", port, [118], timeout=2.0)
            return result, time.monotonic() - t0

    result, elapsed = asyncio.run(main())
    assert set(result) == expected and len(expected) > 300
    assert elapsed < 1.0  # \EOF corta la espera; no se agota el timeout


def test_scan_reports_progress():
    progress, got = [], []
    game = dataclasses.replace(GAMES["cod2"], protocols=(118,))

    async def main():
        reply = lambda d: [STATUS] if d == q3.GETSTATUS else []  # noqa: E731
        async with fake_udp(reply) as (p1, _), fake_udp(reply) as (p2, _):
            servers = [ServerAddress("127.0.0.1", p1), ServerAddress("127.0.0.1", p2),
                       ServerAddress("127.0.0.1", dead_udp_port())]
            handler = (lambda d: [_master_packet(servers, True)]
                       if d.startswith(q3.OOB + b"getservers ") else [])
            async with fake_udp(handler) as (mport, _):
                return await scanner.scan(
                    game, [("127.0.0.1", mport)], got.append, lambda _s: None,
                    master_timeout=T, query_timeout=T,
                    on_progress=lambda done, total: progress.append((done, total)))

    result = asyncio.run(main())
    assert result.servers_responded == 2 and len(got) == 2
    assert progress[0] == (0, 3)
    assert progress[-1] == (3, 3)
    assert [d for d, _ in progress] == [0, 1, 2, 3]


def test_query_servers_calls_on_done_per_server():
    done = []

    async def main():
        async with fake_udp(lambda d: [STATUS] if d == q3.GETSTATUS else []) as (port, _):
            addrs = [ServerAddress("127.0.0.1", port), ServerAddress("127.0.0.1", dead_udp_port())]
            await scanner.query_servers("quake3", addrs, lambda _i: None, timeout=T,
                                        retries=0, on_done=lambda: done.append(1))

    asyncio.run(main())
    assert len(done) == 2


def test_scan_without_on_progress_still_works():
    # on_progress es opcional: el escaneo sin callback no cambia.
    game = GAMES["cod2"]

    async def main():
        return await scanner.scan(game, [("127.0.0.1", dead_udp_port())], lambda _i: None,
                                  lambda _s: None, master_timeout=T, query_timeout=T)

    assert not asyncio.run(main()).ok


def test_query_one_server():
    async def main():
        async with fake_udp(lambda d: [STATUS] if d == q3.GETSTATUS else []) as (good, _):
            ok = await scanner.query_one_server(GAMES["cod2"], ServerAddress("127.0.0.1", good),
                                                timeout=T)
            dead = await scanner.query_one_server(
                GAMES["cod2"], ServerAddress("127.0.0.1", dead_udp_port()), timeout=T)
            return ok, dead

    ok, dead = asyncio.run(main())
    assert ok is not None and ok.name == "Jimmy TDM"
    assert ok.address.port > 0 and ok.ping_ms >= 0
    assert dead is None
