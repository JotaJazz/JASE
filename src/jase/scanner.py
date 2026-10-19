"""Scanner asíncrono: consulta masters (Quake3/GameSpy) y servidores en paralelo."""
from __future__ import annotations

import asyncio
import contextlib
import ctypes
import ipaddress
import logging
import socket
import sys
import time
from collections.abc import Callable, Iterable
from dataclasses import dataclass

from jase.games import GameDef
from jase.models import ServerAddress, ServerInfo
from jase.protocols import ProtocolError
from jase.protocols import gamespy, quake3

_MASTER_ERRORS = (OSError, TimeoutError, ProtocolError)

_log = logging.getLogger(__name__)

# El módulo socket de CPython no expone SIO_UDP_CONNRESET y sock.ioctl() en Windows
# solo admite SIO_RCVALL/SIO_KEEPALIVE_VALS/SIO_LOOPBACK_FAST_PATH: hay que ir a WSAIoctl.
SIO_UDP_CONNRESET = 0x9800000C
_IS_WINDOWS = sys.platform == "win32"

# Reloj de los pings: perf_counter (en Windows loop.time()/monotonic avanzan a ~15,6 ms).
_clock = time.perf_counter


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
    """Un socket UDP por protocolo; recoge paquetes hasta el último (\\EOF) en todos o `timeout`."""
    loop = asyncio.get_running_loop()
    protocols = list(protocols)
    queue: asyncio.Queue = asyncio.Queue()
    transports: list[asyncio.DatagramTransport] = []
    servers: dict[ServerAddress, None] = {}
    pending = set(range(len(protocols)))
    got_valid = False
    first_error: Exception | None = None
    deadline = loop.time() + timeout  # un único plazo: DNS + envío + lectura
    try:
        async with asyncio.timeout_at(deadline):
            remote = await _resolve(host, port, socket.SOCK_DGRAM)
            for tag, proto in enumerate(protocols):
                transport, _ = await loop.create_datagram_endpoint(
                    lambda tag=tag: _QueueProtocol(queue, tag), remote_addr=remote)
                transports.append(transport)
                transport.sendto(quake3.build_getservers(proto))
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
                if b"\\final\\" in hello or (idx >= 0 and len(hello) - idx - 8 >= 6):
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


def _load_wsaioctl():
    """ws2_32.WSAIoctl con su firma (solo Windows)."""
    fn = ctypes.WinDLL("ws2_32").WSAIoctl
    fn.argtypes = [ctypes.c_size_t, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_uint32,
                   ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint32),
                   ctypes.c_void_p, ctypes.c_void_p]
    fn.restype = ctypes.c_int
    return fn


def _disable_udp_connreset(sock) -> None:
    """Windows: que un ICMP port-unreachable no produzca WSAECONNRESET en el siguiente
    recv (el transporte Proactor puede dejar de leer y el endpoint compartido quedaría
    sordo para todos los servidores). Usa WSAIoctl(SIO_UDP_CONNRESET, FALSE) vía ctypes;
    si falla se anota en debug y el escaneo sigue. No-op fuera de Windows."""
    if not _IS_WINDOWS:
        return
    try:
        wsaioctl = _load_wsaioctl()
        flag = ctypes.c_int(0)  # BOOL FALSE
        returned = ctypes.c_uint32(0)
        rc = wsaioctl(sock.fileno(), SIO_UDP_CONNRESET, ctypes.pointer(flag),
                      ctypes.sizeof(flag), None, 0, ctypes.pointer(returned), None, None)
    except Exception as exc:  # noqa: BLE001 - nunca romper el escaneo por esto
        _log.debug("No se pudo desactivar SIO_UDP_CONNRESET: %s", exc)
        return
    if rc != 0:
        _log.debug("WSAIoctl(SIO_UDP_CONNRESET) devolvió %s", rc)


class _Router(asyncio.DatagramProtocol):
    """Endpoint UDP compartido: enruta respuestas por (ip, puerto) de origen."""

    def __init__(self) -> None:
        self.handlers: dict[tuple[str, int], Callable[[bytes, float], None]] = {}

    def datagram_received(self, data: bytes, addr) -> None:
        handler = self.handlers.get((addr[0], addr[1]))
        if handler is not None:
            handler(data, _clock())

    def error_received(self, exc: Exception) -> None:
        pass  # ICMP sueltos de servidores caídos: se resuelven por timeout


async def _query_one(
    family: str,
    address: ServerAddress,
    transport: asyncio.DatagramTransport,
    router: _Router,
    timeout: float,
    retries: int,
    connect_port_fallback: int = -1,
    version_key: str = "shortversion",
    expected_gamename: str = "",
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

    first_sent: float | None = None
    for attempt in range(retries + 1):
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
            sent = _clock()
            if first_sent is None:
                first_sent = sent
            transport.sendto(query, key)
            try:
                payload, received = await asyncio.wait_for(future, timeout)
            except TimeoutError:
                continue
            except ProtocolError:
                return None
        finally:
            router.handlers.pop(key, None)
        # En un reintento la respuesta puede ser tardía del intento anterior:
        # medir desde el primer envío (conservador, un servidor lento nunca sale verde).
        ping = round((received - (sent if attempt == 0 else first_sent)) * 1000)
        try:
            if family == "quake3":
                return quake3.parse_status_response(payload, address, ping,
                                                    version_key=version_key)
            return gamespy.parse_status(payload, address, ping,
                                        connect_port_fallback=connect_port_fallback,
                                        expected_gamename=expected_gamename)
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
    on_done: Callable[[], None] | None = None,
    connect_port_fallback: int = -1,
    version_key: str = "shortversion",
    expected_gamename: str = "",
) -> None:
    """Consulta servidores en paralelo; llama a `on_server` por cada respuesta válida
    y `on_done` por cada servidor cuya consulta terminó (respondiera o no).

    `connect_port_fallback` y `expected_gamename` (GameSpy) y `version_key` (Quake3)
    salen del GameDef; ver `game_query_options`."""
    if family not in ("quake3", "gamespy"):
        raise ValueError(f"Familia de protocolo desconocida: {family!r}")
    loop = asyncio.get_running_loop()
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.setblocking(False)
        _disable_udp_connreset(sock)
        sock.bind(("0.0.0.0", 0))
        transport, router = await loop.create_datagram_endpoint(_Router, sock=sock)
    except BaseException:
        sock.close()
        raise
    semaphore = asyncio.Semaphore(concurrency)

    async def worker(address: ServerAddress) -> None:
        async with semaphore:
            info = await _query_one(family, address, transport, router, timeout, retries,
                                    connect_port_fallback, version_key,
                                    expected_gamename)
        if info is not None:
            on_server(info)
        if on_done is not None:
            on_done()

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


def game_query_options(game: GameDef) -> dict[str, object]:
    """Opciones de `query_servers` que dependen del juego."""
    return {"connect_port_fallback": game.connect_port_fallback,
            "version_key": game.version_key,
            "expected_gamename": game.status_gamename}


async def query_one_server(
    game: GameDef, address: ServerAddress, timeout: float = 1.5,
) -> ServerInfo | None:
    """Consulta un único servidor; None si no respondió (o la respuesta era inválida)."""
    found: list[ServerInfo] = []
    await query_servers(game.family, [address], found.append, timeout=timeout,
                        **game_query_options(game))
    return found[0] if found else None


@dataclass(frozen=True)
class ScanResult:
    """Resumen de un escaneo; `ok` es False si ningún master respondió."""

    masters_ok: int
    servers_listed: int
    servers_responded: int

    @property
    def ok(self) -> bool:
        return self.masters_ok > 0


def fetch_master(game: GameDef, host: str, port: int, timeout: float):
    """Corrutina que pide la lista al master según la familia del juego."""
    if game.family == "quake3":
        return fetch_master_quake3(host, port, game.protocols, timeout=timeout)
    if game.family == "gamespy":
        if not game.gamespy_gamename or not game.gamespy_key:
            raise ValueError(f"{game.id}: faltan gamespy_gamename/gamespy_key")
        return fetch_master_gamespy(host, port, game.gamespy_gamename, game.gamespy_key,
                                    timeout=timeout)
    raise NotImplementedError(f"Familia {game.family!r} aún no soportada ({game.id})")


async def scan(
    game: GameDef,
    masters: list[tuple[str, int]],
    on_server: Callable[[ServerInfo], None],
    on_status: Callable[[str], None],
    master_timeout: float = 3.0,
    query_timeout: float = 1.5,
    on_progress: Callable[[int, int], None] | None = None,
) -> ScanResult:
    """Consulta los masters en paralelo, deduplica y consulta cada servidor."""
    fetches = [fetch_master(game, h, p, master_timeout) for h, p in masters]
    results = await asyncio.gather(*fetches, return_exceptions=True)

    addresses: dict[ServerAddress, None] = {}
    masters_ok = 0
    for (host, _), result in zip(masters, results):
        if isinstance(result, _MASTER_ERRORS):
            on_status(f"Master {host} no responde")
        elif isinstance(result, BaseException):
            raise result
        else:
            masters_ok += 1
            addresses.update(dict.fromkeys(result))
    if not masters_ok:
        on_status("Ningún master respondió")
        return ScanResult(masters_ok=0, servers_listed=0, servers_responded=0)
    on_status(f"{len(addresses)} servidores en el master, consultando…")
    responded = 0
    done = 0
    total = len(addresses)
    if on_progress is not None:
        on_progress(0, total)

    def count(info: ServerInfo) -> None:
        nonlocal responded
        responded += 1
        on_server(info)

    def finished() -> None:
        nonlocal done
        done += 1
        on_progress(done, total)  # type: ignore[misc]

    await query_servers(game.family, addresses, count, timeout=query_timeout,
                        on_done=finished if on_progress is not None else None,
                        **game_query_options(game))
    return ScanResult(masters_ok=masters_ok, servers_listed=len(addresses),
                      servers_responded=responded)
