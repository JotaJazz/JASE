"""Scanner asíncrono: consulta masters (Quake3/GameSpy) y servidores en paralelo."""
from __future__ import annotations

import asyncio
import contextlib
import ipaddress
import socket
from collections.abc import Callable, Iterable

from jase.games import GameDef
from jase.models import ServerAddress, ServerInfo
from jase.protocols import ProtocolError
from jase.protocols import gamespy, quake3

_MASTER_ERRORS = (OSError, TimeoutError, ProtocolError)


async def _resolve(host: str, port: int, type_: int) -> tuple[str, int]:
    loop = asyncio.get_running_loop()
    infos = await loop.getaddrinfo(host, port, family=socket.AF_INET, type=type_)
    if not infos:
        raise socket.gaierror(f"No se pudo resolver {host}")
    return infos[0][4][:2]


class _QueueProtocol(asyncio.DatagramProtocol):
    """Encola paquetes y errores recibidos en un socket UDP conectado al master."""

    def __init__(self, queue: asyncio.Queue, tag: int) -> None:
        self._queue = queue
        self._tag = tag

    def datagram_received(self, data: bytes, addr) -> None:
        self._queue.put_nowait((self._tag, data, None))

    def error_received(self, exc: Exception) -> None:
        self._queue.put_nowait((self._tag, None, exc))


async def fetch_master_quake3(
    host: str, port: int, protocols: Iterable[int], timeout: float = 3.0
) -> list[ServerAddress]:
    """Un socket UDP por protocolo; recoge paquetes hasta EOT en todos o `timeout`."""
    loop = asyncio.get_running_loop()
    protocols = list(protocols)
    queue: asyncio.Queue = asyncio.Queue()
    transports: list[asyncio.DatagramTransport] = []
    servers: dict[ServerAddress, None] = {}
    pending = set(range(len(protocols)))
    got_valid = False
    first_error: Exception | None = None
    try:
        async with asyncio.timeout(timeout):
            remote = await _resolve(host, port, socket.SOCK_DGRAM)
            for tag, proto in enumerate(protocols):
                transport, _ = await loop.create_datagram_endpoint(
                    lambda tag=tag: _QueueProtocol(queue, tag), remote_addr=remote)
                transports.append(transport)
                transport.sendto(quake3.build_getservers(proto))
        deadline = loop.time() + timeout
        while pending:
            remaining = deadline - loop.time()
            if remaining <= 0:
                break
            try:
                async with asyncio.timeout(remaining):
                    tag, data, exc = await queue.get()
            except TimeoutError:
                break
            if exc is not None:
                first_error = first_error or exc
                pending.discard(tag)
                continue
            try:
                found, eot = quake3.parse_getservers_response(data)
            except ProtocolError as err:
                first_error = first_error or err
                continue
            got_valid = True
            servers.update(dict.fromkeys(found))
            if eot:
                pending.discard(tag)
    finally:
        for transport in transports:
            transport.close()
    if not got_valid:
        raise first_error or TimeoutError(f"El master {host} no respondió")
    return list(servers)


async def fetch_master_gamespy(
    host: str,
    port: int,
    gamename: str = gamespy.UT_GAMENAME,
    gamekey: str = gamespy.UT_GAMEKEY,
    timeout: float = 5.0,
) -> list[ServerAddress]:
    """TCP: leer \\secure\\, enviar validate + list, leer hasta \\final\\ (o EOF)."""
    writer: asyncio.StreamWriter | None = None
    buf = bytearray()
    try:
        async with asyncio.timeout(timeout):
            reader, writer = await asyncio.open_connection(host, port)
            hello = bytearray()
            while True:
                idx = hello.find(b"\\secure\\")
                if idx >= 0 and (len(hello) - idx - 8 >= 6 or b"\\final\\" in hello):
                    break
                chunk = await reader.read(4096)
                if not chunk:
                    break
                hello.extend(chunk)
            secure = gamespy.parse_secure(bytes(hello))
            writer.write(gamespy.build_validate(secure, gamename, gamekey)
                         + gamespy.build_list_request(gamename))
            await writer.drain()
            while b"\\final\\" not in buf:
                chunk = await reader.read(65536)
                if not chunk:
                    break
                buf.extend(chunk)
    except TimeoutError:
        # Timeout a mitad de la lista: usar lo recibido si sirve de algo.
        partial = gamespy.parse_master_list(bytes(buf))
        if partial:
            return partial
        raise
    finally:
        if writer is not None:
            writer.close()
            with contextlib.suppress(Exception):
                await writer.wait_closed()
    return gamespy.parse_master_list(bytes(buf))


class _Router(asyncio.DatagramProtocol):
    """Endpoint UDP compartido: enruta respuestas por (ip, puerto) de origen."""

    def __init__(self) -> None:
        self.handlers: dict[tuple[str, int], Callable[[bytes, float], None]] = {}

    def datagram_received(self, data: bytes, addr) -> None:
        handler = self.handlers.get((addr[0], addr[1]))
        if handler is not None:
            handler(data, asyncio.get_running_loop().time())

    def error_received(self, exc: Exception) -> None:
        pass  # ICMP sueltos de servidores caídos: se resuelven por timeout


async def _query_one(
    family: str,
    address: ServerAddress,
    transport: asyncio.DatagramTransport,
    router: _Router,
    timeout: float,
    retries: int,
) -> ServerInfo | None:
    loop = asyncio.get_running_loop()
    host = address.host
    try:
        ipaddress.IPv4Address(host)
    except ValueError:
        try:
            host, _ = await _resolve(host, address.port, socket.SOCK_DGRAM)
        except OSError:
            return None
    key = (host, address.port)
    query = quake3.GETSTATUS if family == "quake3" else gamespy.STATUS_QUERY

    for _ in range(retries + 1):
        future: asyncio.Future[tuple[object, float]] = loop.create_future()
        if family == "quake3":
            def handler(data: bytes, now: float, future=future) -> None:
                if not future.done():
                    future.set_result((data, now))
        else:
            assembler = gamespy.StatusAssembler()
            first: list[float] = []

            def handler(data: bytes, now: float, future=future,
                        assembler=assembler, first=first) -> None:
                if future.done():
                    return
                first.append(now)
                try:
                    if assembler.feed(data):
                        future.set_result((assembler.fields(), first[0]))
                except ProtocolError as exc:
                    future.set_exception(exc)

        router.handlers[key] = handler
        try:
            sent = loop.time()
            transport.sendto(query, key)
            try:
                payload, received = await asyncio.wait_for(future, timeout)
            except TimeoutError:
                continue
            except ProtocolError:
                return None
        finally:
            router.handlers.pop(key, None)
        ping = round((received - sent) * 1000)
        try:
            if family == "quake3":
                return quake3.parse_status_response(payload, address, ping)
            return gamespy.parse_status(payload, address, ping)
        except ProtocolError:
            return None
    return None


async def query_servers(
    family: str,
    addresses: Iterable[ServerAddress],
    on_server: Callable[[ServerInfo], None],
    timeout: float = 1.5,
    retries: int = 1,
    concurrency: int = 64,
) -> None:
    """Consulta servidores en paralelo; llama a `on_server` por cada respuesta válida."""
    if family not in ("quake3", "gamespy"):
        raise ValueError(f"Familia de protocolo desconocida: {family!r}")
    loop = asyncio.get_running_loop()
    transport, router = await loop.create_datagram_endpoint(
        _Router, local_addr=("0.0.0.0", 0))
    semaphore = asyncio.Semaphore(concurrency)

    async def worker(address: ServerAddress) -> None:
        async with semaphore:
            info = await _query_one(family, address, transport, router, timeout, retries)
        if info is not None:
            on_server(info)

    tasks: list[asyncio.Task] = []
    try:
        tasks = [asyncio.create_task(worker(a)) for a in dict.fromkeys(addresses)]
        if tasks:
            await asyncio.gather(*tasks)
    finally:
        for task in tasks:
            task.cancel()
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
        transport.close()


async def scan(
    game: GameDef,
    masters: list[tuple[str, int]],
    on_server: Callable[[ServerInfo], None],
    on_status: Callable[[str], None],
    master_timeout: float = 3.0,
    query_timeout: float = 1.5,
) -> None:
    """Consulta los masters en paralelo, deduplica y consulta cada servidor."""
    if game.family == "quake3":
        fetches = [fetch_master_quake3(h, p, game.protocols, timeout=master_timeout)
                   for h, p in masters]
    else:
        fetches = [fetch_master_gamespy(h, p, timeout=master_timeout) for h, p in masters]
    results = await asyncio.gather(*fetches, return_exceptions=True)

    addresses: dict[ServerAddress, None] = {}
    any_ok = False
    for (host, _), result in zip(masters, results):
        if isinstance(result, _MASTER_ERRORS):
            on_status(f"Master {host} no responde")
        elif isinstance(result, BaseException):
            raise result
        else:
            any_ok = True
            addresses.update(dict.fromkeys(result))
    if not any_ok:
        on_status("Ningún master respondió")
        return
    on_status(f"{len(addresses)} servidores en el master, consultando…")
    await query_servers(game.family, addresses, on_server, timeout=query_timeout)
