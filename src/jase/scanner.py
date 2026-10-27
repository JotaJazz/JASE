"""Scanner asíncrono: consulta masters (Quake3/GameSpy/Valve/UT2004) y servidores en paralelo."""
from __future__ import annotations

import asyncio
import contextlib
import ctypes
import functools
import ipaddress
import logging
import socket
import sys
import threading
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from typing import Any

from jase.games import GameDef
from jase.i18n import tr, tr_n
from jase.models import ServerAddress, ServerInfo
from jase.protocols import ProtocolError
from jase.protocols import gamespy, quake2, quake3, steamweb, ut2004, valve, zandronum

_MASTER_ERRORS = (OSError, TimeoutError, ProtocolError)
FAMILIES = ("quake3", "quake2", "gamespy", "valve", "ut2004", "zandronum")

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
    addr = infos[0][4]
    return str(addr[0]), int(addr[1])


# Quake II: espera tras el último datagrama válido por si llegan más del mismo master.
_QUAKE2_MASTER_GRACE = 0.5
# UT2004: espera tras el último datagrama (reglas y jugadores pueden ocupar varios).
_UT2004_QUERY_GRACE = 0.25


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
    host: str, port: int, protocols: Iterable[int], timeout: float = 3.0, gamename: str = ""
) -> list[ServerAddress]:
    """Un socket UDP por protocolo; recoge paquetes hasta el último (\\EOF) en todos o `timeout`.
    `gamename` va en la consulta para los masters que mezclan juegos (dpmaster)."""
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
                    functools.partial(_QueueProtocol, queue, tag), remote_addr=remote)
                transports.append(transport)
                transport.sendto(quake3.build_getservers(proto, gamename))
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


async def fetch_master_quake2(host: str, port: int, timeout: float = 3.0) -> list[ServerAddress]:
    """Un datagrama `query`; la lista llega en uno o más datagramas (se acumulan los que
    lleguen dentro del plazo, tras el primero se espera poco por más)."""
    loop = asyncio.get_running_loop()
    queue: asyncio.Queue = asyncio.Queue()
    servers: dict[ServerAddress, None] = {}
    got_valid = False
    first_error: Exception | None = None
    transport: asyncio.DatagramTransport | None = None
    deadline = loop.time() + timeout
    try:
        async with asyncio.timeout_at(deadline):
            remote = await _resolve(host, port, socket.SOCK_DGRAM)
            transport, _ = await loop.create_datagram_endpoint(
                lambda: _QueueProtocol(queue, 0), remote_addr=remote)
            transport.sendto(quake2.MASTER_QUERY)
        while True:
            remaining = deadline - loop.time()
            if got_valid:
                remaining = min(remaining, _QUAKE2_MASTER_GRACE)
            if remaining <= 0:
                break
            try:
                async with asyncio.timeout(remaining):
                    _, data, exc = await queue.get()
            except TimeoutError:
                break
            if exc is not None:
                first_error = first_error or exc
                break
            try:
                servers.update(dict.fromkeys(quake2.parse_master_response(data)))
            except ProtocolError as err:
                first_error = first_error or err
                continue
            got_valid = True
    finally:
        if transport is not None:
            transport.close()
    if not got_valid:
        raise first_error or TimeoutError(f"El master {host} no respondió")
    return list(servers)


async def fetch_master_zandronum(host: str, port: int,
                                 timeout: float = 3.0) -> list[ServerAddress]:
    """La lista llega en varios datagramas numerados; el último trae MSC_ENDSERVERLIST.
    Si no llega el final dentro del plazo, vale lo recibido (como en Quake II)."""
    loop = asyncio.get_running_loop()
    queue: asyncio.Queue = asyncio.Queue()
    servers: dict[ServerAddress, None] = {}
    got_valid = False
    first_error: Exception | None = None
    transport: asyncio.DatagramTransport | None = None
    deadline = loop.time() + timeout
    try:
        async with asyncio.timeout_at(deadline):
            remote = await _resolve(host, port, socket.SOCK_DGRAM)
            transport, _ = await loop.create_datagram_endpoint(
                lambda: _QueueProtocol(queue, 0), remote_addr=remote)
            transport.sendto(zandronum.MASTER_REQUEST)
        while True:
            remaining = deadline - loop.time()
            if got_valid:
                remaining = min(remaining, _QUAKE2_MASTER_GRACE)
            if remaining <= 0:
                break
            try:
                async with asyncio.timeout(remaining):
                    _, data, exc = await queue.get()
            except TimeoutError:
                break
            if exc is not None:
                first_error = first_error or exc
                break
            try:
                found, last = zandronum.parse_master_packet(data)
            except ProtocolError as err:
                first_error = first_error or err
                break
            servers.update(dict.fromkeys(found))
            got_valid = True
            if last:
                break
    finally:
        if transport is not None:
            transport.close()
    if not got_valid:
        raise first_error or TimeoutError(f"El master {host} no respondió")
    return list(servers)


async def _query_zandronum(address: ServerAddress, key: tuple[str, int],
                           transport: asyncio.DatagramTransport, router: _Router,
                           timeout: float, retries: int) -> ServerInfo | None:
    loop = asyncio.get_running_loop()
    first_sent: float | None = None
    for attempt in range(retries + 1):
        future: asyncio.Future[tuple[bytes, float]] = loop.create_future()

        def handler(data: bytes, now: float, future=future) -> None:
            if not future.done():
                future.set_result((data, now))

        router.handlers[key] = handler
        try:
            sent = _clock()
            if first_sent is None:
                first_sent = sent
            transport.sendto(zandronum.build_query(int(sent * 1000)), key)
            try:
                payload, received = await asyncio.wait_for(future, timeout)
            except TimeoutError:
                continue
        finally:
            router.handlers.pop(key, None)
        ping = round((received - (sent if attempt == 0 else first_sent)) * 1000)
        try:
            return zandronum.parse_response(payload, address, ping)
        except ProtocolError:
            return None
    return None


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


async def _read_ut2004_frame(reader: asyncio.StreamReader) -> bytes:
    size = int.from_bytes(await reader.readexactly(4), "little")
    if size > ut2004.MAX_FRAME:
        raise ProtocolError(f"Mensaje del master demasiado grande: {size}")
    return await reader.readexactly(size)


async def fetch_master_ut2004(host: str, port: int, timeout: float = 3.0) -> list[ServerAddress]:
    """TCP: challenge → respuesta con hashes ficticios fijos → APPROVED → VERIFIED → lista.
    Un rechazo (DENIED) sale como ProtocolError y no se reintenta con otros valores."""
    writer: asyncio.StreamWriter | None = None
    entries: list[bytes] = []
    try:
        async with asyncio.timeout(timeout):
            reader, writer = await asyncio.open_connection(host, port)
            await _read_ut2004_frame(reader)  # challenge: la respuesta es fija
            writer.write(ut2004.build_challenge_response())
            await writer.drain()
            ut2004.check_reply(await _read_ut2004_frame(reader), "APPROVED")
            writer.write(ut2004.VERIFY_REQUEST)
            await writer.drain()
            ut2004.check_reply(await _read_ut2004_frame(reader), "VERIFIED")
            writer.write(ut2004.LIST_REQUEST)
            await writer.drain()
            count = ut2004.parse_list_header(await _read_ut2004_frame(reader))
            for _ in range(count):
                entries.append(await _read_ut2004_frame(reader))
    except (OSError, asyncio.IncompleteReadError) as exc:
        # Corte a mitad de la lista (timeout, EOF o reset): usar lo recibido si sirve de algo.
        if not entries:
            if isinstance(exc, OSError):  # incluye TimeoutError
                raise
            raise ProtocolError(f"El master {host} cerró la conexión") from None
    finally:
        if writer is not None:
            writer.close()
            with contextlib.suppress(Exception):
                await writer.wait_closed()
    servers: dict[ServerAddress, None] = {}
    for body in entries:
        with contextlib.suppress(ProtocolError):
            servers[ut2004.parse_list_entry(body)] = None
    return list(servers)


# Valve limita las consultas al master: pocas páginas (~230 servidores cada una).
VALVE_MASTER_MAX_PAGES = 20
# Límite de S2C_CHALLENGE seguidos por intento (un servidor que siempre responde
# con challenge no debe provocar un bucle de envíos).
_VALVE_MAX_CHALLENGES = 2


async def fetch_master_valve(
    host: str, port: int, filter: str, timeout: float = 3.0,
    max_pages: int = VALVE_MASTER_MAX_PAGES,
) -> list[ServerAddress]:
    """Master Server Query de Valve: una página por ida y vuelta, cada una sembrada con
    la última dirección de la anterior, hasta 0.0.0.0:0 o `max_pages`.

    `timeout` es por página (incluye el DNS en la primera). Si una página posterior
    no llega (Valve limita), se devuelve lo ya recibido."""
    loop = asyncio.get_running_loop()
    queue: asyncio.Queue = asyncio.Queue()
    servers: dict[ServerAddress, None] = {}
    transport: asyncio.DatagramTransport | None = None
    seed: ServerAddress | None = None
    seen_seeds: set[ServerAddress] = set()
    try:
        for page in range(max_pages):
            try:
                async with asyncio.timeout(timeout):
                    if transport is None:
                        remote = await _resolve(host, port, socket.SOCK_DGRAM)
                        transport, _ = await loop.create_datagram_endpoint(
                            lambda: _QueueProtocol(queue, 0), remote_addr=remote)
                    transport.sendto(valve.build_master_query(seed, filter))
                    _, data, exc = await queue.get()
                if exc is not None:
                    raise exc
                found, seed = valve.parse_master_response(data)
            except _MASTER_ERRORS:
                if page == 0:
                    raise
                break  # página tardía o inválida: quedarse con lo recibido
            servers.update(dict.fromkeys(found))
            if seed is None or seed in seen_seeds:
                break
            seen_seeds.add(seed)
    finally:
        if transport is not None:
            transport.close()
    return list(servers)


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
    expected_gamedir: str = "",
    version_aliases: tuple[tuple[str, str, str], ...] = (),
    exclude_mods: tuple[str, ...] = (),
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
    if family == "quake2":
        return await _query_quake2(address, key, transport, router, timeout, retries,
                                   version_key, expected_gamedir, exclude_mods)
    if family == "ut2004":
        return await _query_ut2004(address, key, transport, router, timeout, retries)
    if family == "zandronum":
        return await _query_zandronum(address, key, transport, router, timeout, retries)
    if family == "valve":
        return await _query_valve(address, key, transport, router, timeout, retries,
                                  expected_gamedir)
    query = quake3.GETSTATUS if family == "quake3" else gamespy.STATUS_QUERY

    first_sent: float | None = None
    for attempt in range(retries + 1):
        future: asyncio.Future[tuple[Any, float]] = loop.create_future()
        if family == "quake3":
            def handler_q3(data: bytes, now: float, future=future) -> None:
                if not future.done():
                    future.set_result((data, now))

            handler = handler_q3
        else:
            assembler = gamespy.StatusAssembler()
            first: list[float] = []

            def handler_gs(data: bytes, now: float, future=future,
                           assembler=assembler, first=first) -> None:
                if future.done():
                    return
                first.append(now)
                try:
                    if assembler.feed(data):
                        future.set_result((assembler.fields(), first[0]))
                except ProtocolError as exc:
                    future.set_exception(exc)

            handler = handler_gs

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
                                                    version_key=version_key,
                                                    version_aliases=version_aliases,
                                                    exclude_mods=exclude_mods)
            return gamespy.parse_status(payload, address, ping,
                                        connect_port_fallback=connect_port_fallback,
                                        expected_gamename=expected_gamename)
        except ProtocolError:
            return None
    return None


async def _query_quake2(address: ServerAddress, key: tuple[str, int],
                        transport: asyncio.DatagramTransport, router: _Router,
                        timeout: float, retries: int, version_key: str = "version",
                        expected_gamedir: str = "",
                        exclude_mods: tuple[str, ...] = ()) -> ServerInfo | None:
    loop = asyncio.get_running_loop()
    first_sent: float | None = None
    for attempt in range(retries + 1):
        future: asyncio.Future[tuple[bytes, float]] = loop.create_future()

        def handler(data: bytes, now: float, future=future) -> None:
            if not future.done():
                future.set_result((data, now))

        router.handlers[key] = handler
        try:
            sent = _clock()
            if first_sent is None:
                first_sent = sent
            transport.sendto(quake2.STATUS_QUERY, key)
            try:
                payload, received = await asyncio.wait_for(future, timeout)
            except TimeoutError:
                continue
        finally:
            router.handlers.pop(key, None)
        ping = round((received - (sent if attempt == 0 else first_sent)) * 1000)
        try:
            return quake2.parse_status(payload, address, ping, version_key=version_key,
                                       expected_gamedir=expected_gamedir,
                                       exclude_mods=exclude_mods)
        except ProtocolError:
            return None
    return None


async def _query_ut2004(address: ServerAddress, key: tuple[str, int],
                        transport: asyncio.DatagramTransport, router: _Router,
                        timeout: float, retries: int) -> ServerInfo | None:
    """Pide información, reglas y jugadores a la vez; tras la información espera
    `_UT2004_QUERY_GRACE` sin datagramas nuevos (o `timeout`) por el resto."""
    loop = asyncio.get_running_loop()
    first_sent: float | None = None
    for attempt in range(retries + 1):
        packets: dict[int, list[bytes]] = {}
        info_at: asyncio.Future[float] = loop.create_future()
        arrived = asyncio.Event()

        def handler(data: bytes, now: float, packets=packets, info_at=info_at,
                    arrived=arrived) -> None:
            if len(data) < 5 or not data.startswith(ut2004.INFO_QUERY[:4]):
                return
            packets.setdefault(data[4], []).append(data)
            if data[4] == ut2004.INFO and not info_at.done():
                info_at.set_result(now)
            arrived.set()

        router.handlers[key] = handler
        try:
            sent = _clock()
            if first_sent is None:
                first_sent = sent
            for query in ut2004.QUERIES:
                transport.sendto(query, key)
            try:
                received = await asyncio.wait_for(info_at, timeout)
            except TimeoutError:
                continue
            stop = loop.time() + timeout
            while (remaining := stop - loop.time()) > 0:
                arrived.clear()
                try:
                    await asyncio.wait_for(arrived.wait(), min(remaining, _UT2004_QUERY_GRACE))
                except TimeoutError:
                    break
        finally:
            router.handlers.pop(key, None)
        ping = round((received - (sent if attempt == 0 else first_sent)) * 1000)
        try:
            info = ut2004.parse_info(packets[ut2004.INFO][0], address, ping)
        except ProtocolError:
            return None
        # Un datagrama de reglas o jugadores dañado no invalida el servidor.
        rules: dict[str, str] = {}
        for data in packets.get(ut2004.RULES, []):
            with contextlib.suppress(ProtocolError):
                ut2004.parse_rules(data, rules)
        players = []
        for data in packets.get(ut2004.PLAYERS, []):
            with contextlib.suppress(ProtocolError):
                players.extend(ut2004.parse_players(data))
        return ut2004.apply_details(info, rules, players)
    return None


def _valve_handler(future: asyncio.Future, transport: asyncio.DatagramTransport,
                   key: tuple[str, int], build_query: Callable[[bytes], bytes],
                   wanted: tuple[int, ...], last_sent: list[float],
                   on_other: Callable[[bytes], None] | None = None):
    """Manejador del router para una consulta A2S: reensambla multipaquete, responde
    a S2C_CHALLENGE reenviando la consulta (anota la hora en `last_sent`) y resuelve
    `future` con (mensaje, hora) al llegar un tipo de `wanted`. Los mensajes completos
    de otro tipo, o los que llegan con `future` ya resuelto, van a `on_other`."""
    assembler = valve.PacketAssembler()
    challenges = [0]

    def handler(data: bytes, now: float) -> None:
        if future.done():
            if on_other is not None and data.startswith(valve.OOB) and len(data) >= 5:
                on_other(data)
            return
        try:
            message = assembler.feed(data)
        except ProtocolError as exc:
            future.set_exception(exc)
            return
        if message is None or len(message) < 5:
            return
        challenge = valve.challenge_of(message)
        if challenge is not None:
            if challenges[0] < _VALVE_MAX_CHALLENGES:
                challenges[0] += 1
                last_sent[0] = _clock()
                transport.sendto(build_query(challenge), key)
            return
        if message[4] in wanted:
            future.set_result((message, now))
        elif on_other is not None:
            # Otro tipo, p. ej. la 0x49 que algunos HLDS mandan junto a la 0x6D.
            on_other(message)

    return handler


async def _query_valve(
    address: ServerAddress,
    key: tuple[str, int],
    transport: asyncio.DatagramTransport,
    router: _Router,
    timeout: float,
    retries: int,
    expected_gamedir: str = "",
) -> ServerInfo | None:
    """A2S_INFO (con reintentos y challenge) y luego A2S_PLAYER (un intento, opcional).

    El ping se mide en el intercambio final: desde el último envío (el reenvío con
    challenge, si lo hubo) hasta la respuesta INFO, para no sumar la ida y vuelta del
    challenge. En un reintento se mide desde el primer envío, como en las otras familias."""
    loop = asyncio.get_running_loop()
    first_sent: float | None = None
    result: tuple[bytes, float] | None = None
    last_sent = [0.0]
    # Algunos HLDS responden 0x6D y 0x49 a la misma consulta, en cualquier orden y casi
    # a la vez. Si gana la 0x6D (sin versión), la 0x49 que llegue después la completa.
    late_source: list[bytes] = []

    def keep_source(message: bytes) -> None:
        if message[4] == valve.INFO_SOURCE:
            late_source.append(message)

    attempt = 0
    for attempt in range(retries + 1):  # noqa: B007  (attempt se usa tras el bucle)
        future: asyncio.Future[tuple[bytes, float]] = loop.create_future()
        router.handlers[key] = _valve_handler(
            future, transport, key, valve.build_info_query,
            (valve.INFO_SOURCE, valve.INFO_GOLDSRC), last_sent, on_other=keep_source)
        try:
            last_sent[0] = _clock()
            if first_sent is None:
                first_sent = last_sent[0]
            transport.sendto(valve.A2S_INFO, key)
            try:
                result = await asyncio.wait_for(future, timeout)
            except TimeoutError:
                continue
            except ProtocolError:
                return None
            break
        finally:
            router.handlers.pop(key, None)
    if result is None:
        return None
    payload, received = result
    start = last_sent[0] if attempt == 0 or first_sent is None else first_sent
    try:
        info = valve.parse_info(payload, address, round((received - start) * 1000),
                                expected_gamedir)
    except ProtocolError:
        return None

    future = loop.create_future()
    router.handlers[key] = _valve_handler(
        future, transport, key, valve.build_player_query, (valve.PLAYER_RESPONSE,), [0.0],
        on_other=keep_source)
    try:
        transport.sendto(valve.build_player_query(None), key)
        players_payload, _ = await asyncio.wait_for(future, timeout)
        info.players = valve.parse_players(players_payload)
    except (TimeoutError, ProtocolError):
        pass  # sin lista de jugadores; el servidor sigue siendo válido
    finally:
        router.handlers.pop(key, None)
    if late_source and payload[4] == valve.INFO_GOLDSRC:
        try:
            source = valve.parse_info(late_source[0], address, info.ping_ms, expected_gamedir)
        except ProtocolError:
            pass
        else:
            source.players = info.players
            info = source
    return info


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
    expected_gamedir: str = "",
    version_aliases: tuple[tuple[str, str, str], ...] = (),
    exclude_mods: tuple[str, ...] = (),
) -> None:
    """Consulta servidores en paralelo; llama a `on_server` por cada respuesta válida
    y `on_done` por cada servidor cuya consulta terminó (respondiera o no).

    `connect_port_fallback` y `expected_gamename` (GameSpy), `version_key` (Quake3 y
    Quake II) y `expected_gamedir` (Valve y Quake II)
    salen del GameDef; ver `game_query_options`."""
    if family not in FAMILIES:
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
                                    expected_gamename, expected_gamedir, version_aliases,
                                    exclude_mods)
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


def game_query_options(game: GameDef) -> dict[str, Any]:
    """Opciones de `query_servers` que dependen del juego."""
    return {"connect_port_fallback": game.connect_port_fallback,
            "version_key": game.version_key,
            "version_aliases": game.version_aliases,
            "exclude_mods": game.status_exclude_mods,
            "expected_gamename": game.status_gamename,
            "expected_gamedir": (game.valve_gamedir if game.family == "valve"
                                 else game.status_gamedir)}


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


_STEAMWEB_MAX_BYTES = 16 * 1024 * 1024


async def fetch_master_steamweb(
    host: str, port: int, filter: str, key: str, timeout: float = 10.0
) -> list[ServerAddress]:
    """Lista de servidores por la API web de Steam (HTTPS). Los errores HTTP se traducen
    a SteamWebError sin incluir la URL (lleva la clave); los de red salen como OSError.
    La clave solo va a steamweb.OFFICIAL_HOST: otro host se rechaza sin enviar nada.
    La descarga corre en un hilo daemon: cancelar vuelve al instante y el hilo colgado
    no retiene el cierre (asyncio.to_thread sí, por el executor por defecto)."""
    if host.lower() != steamweb.OFFICIAL_HOST:
        raise steamweb.SteamWebError(tr("Solo se admite {host} como fuente de Steam",
                                        host=steamweb.OFFICIAL_HOST))
    url = steamweb.build_url(host, key, filter)

    def download() -> bytes:
        try:
            with urllib.request.urlopen(url, timeout=timeout) as response:
                return response.read(_STEAMWEB_MAX_BYTES)
        except urllib.error.HTTPError as exc:
            if exc.code in (401, 403):
                raise steamweb.SteamWebError(tr(
                    "La clave de API de Steam no es válida; revísala en Ajustes → General")
                ) from None
            if exc.code == 429:
                raise steamweb.SteamWebError(
                    tr("Steam limitó las consultas; prueba en unos minutos")) from None
            raise OSError(f"HTTP {exc.code}") from None
        except urllib.error.URLError as exc:
            raise OSError(type(exc.reason).__name__) from None

    loop = asyncio.get_running_loop()
    future: asyncio.Future[bytes] = loop.create_future()

    def settle(data: bytes | None, error: BaseException | None) -> None:
        if future.done():  # cancelado mientras descargaba: el resultado se descarta
            return
        if error is not None:
            future.set_exception(error)
        elif data is not None:
            future.set_result(data)

    def run() -> None:
        try:
            data, error = download(), None
        except Exception as exc:  # noqa: BLE001 - se entrega al bucle
            data, error = None, exc
        with contextlib.suppress(RuntimeError):  # bucle ya cerrado
            loop.call_soon_threadsafe(settle, data, error)

    threading.Thread(target=run, name="jase-steamweb", daemon=True).start()
    return steamweb.parse_server_list(await future)


async def _missing_steam_key():
    raise steamweb.SteamWebError(tr("Falta la clave de API de Steam; ponla en Ajustes → General"))


def fetch_master(game: GameDef, host: str, port: int, timeout: float, steam_key: str = "",
                 steam_include_empty: bool = False):
    """Corrutina que pide la lista al master según la familia del juego."""
    if game.steam_webapi:
        if not steam_key:
            return _missing_steam_key()
        # Por defecto solo servidores no vacíos (la lista completa es enorme).
        steam_filter = game.valve_master_filter or f"\\appid\\{game.valve_appid}"
        if not steam_include_empty:
            steam_filter += "\\empty\\1"
        return fetch_master_steamweb(host, port, steam_filter, steam_key, timeout=timeout)
    if game.family == "quake3":
        return fetch_master_quake3(host, port, game.protocols, timeout=timeout,
                                   gamename=game.master_gamename)
    if game.family == "quake2":
        return fetch_master_quake2(host, port, timeout=timeout)
    if game.family == "ut2004":
        return fetch_master_ut2004(host, port, timeout=timeout)
    if game.family == "zandronum":
        return fetch_master_zandronum(host, port, timeout=timeout)
    if game.family == "gamespy":
        if not game.gamespy_gamename or not game.gamespy_key:
            raise ValueError(f"{game.id}: faltan gamespy_gamename/gamespy_key")
        return fetch_master_gamespy(host, port, game.gamespy_gamename, game.gamespy_key,
                                    timeout=timeout)
    if game.family == "valve":
        if not game.valve_master_filter and not game.valve_appid:
            raise ValueError(f"{game.id}: falta valve_appid o valve_master_filter")
        return fetch_master_valve(
            host, port, game.valve_master_filter or f"\\appid\\{game.valve_appid}",
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
    steam_key: str = "",
    steam_include_empty: bool = False,
) -> ScanResult:
    """Consulta los masters en paralelo, deduplica y consulta cada servidor."""
    fetches = [fetch_master(game, h, p, master_timeout, steam_key, steam_include_empty)
               for h, p in masters]
    results = await asyncio.gather(*fetches, return_exceptions=True)

    addresses: dict[ServerAddress, None] = {}
    masters_ok = 0
    for (host, _), result in zip(masters, results, strict=True):
        if isinstance(result, steamweb.SteamWebError):
            on_status(str(result))
        elif isinstance(result, zandronum.MasterBusy):
            on_status(tr("El master {host} pide esperar unos segundos entre consultas",
                         host=host))
        elif isinstance(result, _MASTER_ERRORS):
            on_status(tr("Master {host} no responde", host=host))
        elif isinstance(result, BaseException):
            raise result
        else:
            masters_ok += 1
            addresses.update(dict.fromkeys(result))
    if not masters_ok:
        on_status(tr("Ningún master respondió"))
        return ScanResult(masters_ok=0, servers_listed=0, servers_responded=0)
    on_status(tr_n("{n} servidor en el master, consultando…",
                   "{n} servidores en el master, consultando…", len(addresses)))
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
