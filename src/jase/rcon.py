"""Consola RCON: mandar comandos de administrador a un servidor (sin Qt).

Tres variantes según el juego (`kind`):
- "quake": Quake 3 y Quake II, UDP: «\\xff\\xff\\xff\\xffrcon <clave> <comando>», respuesta en
  uno o más «print\\n…».
- "goldsrc": Half-Life 1 (CS 1.6, DoD, TFC, HL DM), UDP: primero «challenge rcon», luego
  «rcon <challenge> "<clave>" <comando>»; la respuesta llega como «l<texto>».
- "source": Source (CS: Source, TF2, L4D, Garry's Mod), TCP en el puerto del juego con
  paquetes [tamaño][id][tipo][cuerpo\\0\\0]: AUTH (3) y luego EXECCOMMAND (2).
"""
from __future__ import annotations

import asyncio
import re
import socket
import struct
from collections.abc import Awaitable, Callable

from jase.games import GameDef
from jase.models import ServerAddress

OOB = b"\xff\xff\xff\xff"
GRACE_S = 0.4  # tras la primera respuesta UDP, cuánto se espera por más trozos
MAX_COMMAND = 1000
_COLOR_RE = re.compile(r"\^[0-9a-zA-Z]")


class RconError(Exception):
    """Fallo del RCON; `code` es uno de ERRORS (la UI lo traduce) y `detail`, el del
    sistema si lo hay."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(f"{code}: {detail}" if detail else code)
        self.code, self.detail = code, detail


ERRORS = ("timeout", "bad_password", "no_challenge", "connect", "invalid", "closed",
          "unsupported", "network")


def kind_of(game: GameDef) -> str | None:
    if game.family in ("quake3", "quake2"):
        return "quake"
    if game.family == "valve":
        return "source" if game.launch_style == "steam" else "goldsrc"
    return None


def clean_command(command: str) -> str:
    """Una sola línea (un salto de línea colaría otro comando) y con tope de largo."""
    return " ".join(command.splitlines()).strip()[:MAX_COMMAND]


# ---------------------------------------------------------------------- UDP
class _UdpCollector(asyncio.DatagramProtocol):
    def __init__(self) -> None:
        self.packets: asyncio.Queue[bytes] = asyncio.Queue()

    def datagram_received(self, data: bytes, addr) -> None:
        self.packets.put_nowait(data)

    def error_received(self, exc: Exception) -> None:
        pass


async def _udp(address: ServerAddress, timeout: float,
               body: Callable[[asyncio.DatagramTransport, _UdpCollector], Awaitable[str]]) -> str:
    loop = asyncio.get_running_loop()
    try:
        transport, proto = await loop.create_datagram_endpoint(
            _UdpCollector, remote_addr=(address.host, address.port), family=socket.AF_INET)
    except OSError as exc:
        raise RconError("network", str(exc)) from exc
    try:
        return await body(transport, proto)
    finally:
        transport.close()


async def _first(proto: _UdpCollector, timeout: float) -> bytes:
    try:
        return await asyncio.wait_for(proto.packets.get(), timeout)
    except TimeoutError as exc:
        raise RconError("timeout") from exc


async def _rest(proto: _UdpCollector) -> list[bytes]:
    more = []
    while True:
        try:
            more.append(await asyncio.wait_for(proto.packets.get(), GRACE_S))
        except TimeoutError:
            return more


async def quake(address: ServerAddress, password: str, command: str,
                timeout: float = 3.0) -> str:
    async def body(transport, proto) -> str:
        transport.sendto(OOB + f"rcon {password} {command}".encode("latin-1", "replace"))
        packets = [await _first(proto, timeout), *await _rest(proto)]
        text = "".join(p[4:].decode("latin-1").removeprefix("print\n")
                       for p in packets if p.startswith(OOB))
        text = _COLOR_RE.sub("", text)
        if "bad rconpassword" in text.casefold() or "invalid password" in text.casefold():
            raise RconError("bad_password")
        return text
    return await _udp(address, timeout, body)


async def goldsrc(address: ServerAddress, password: str, command: str,
                  timeout: float = 3.0) -> str:
    async def body(transport, proto) -> str:
        transport.sendto(OOB + b"challenge rcon\n")
        reply = (await _first(proto, timeout))[4:].decode("latin-1")
        match = re.match(r"challenge rcon (\d+)", reply)
        if not match:
            raise RconError("no_challenge")
        transport.sendto(OOB + f'rcon {match.group(1)} "{password}" {command}\n'
                         .encode("latin-1", "replace"))
        packets = [await _first(proto, timeout), *await _rest(proto)]
        text = "".join(p[5:].decode("latin-1") for p in packets
                       if p.startswith(OOB) and p[4:5] == b"l")
        if text.strip().casefold().startswith("bad rcon_password"):
            raise RconError("bad_password")
        return text
    return await _udp(address, timeout, body)


# ---------------------------------------------------------------------- Source (TCP)
SERVERDATA_AUTH = 3
SERVERDATA_AUTH_RESPONSE = 2
SERVERDATA_EXECCOMMAND = 2
SERVERDATA_RESPONSE_VALUE = 0
MAX_PACKET = 4096 + 14


def source_packet(request_id: int, kind: int, body: str) -> bytes:
    payload = struct.pack("<ii", request_id, kind) + body.encode("utf-8") + b"\0\0"
    return struct.pack("<i", len(payload)) + payload


async def _read_packet(reader: asyncio.StreamReader) -> tuple[int, int, str]:
    (size,) = struct.unpack("<i", await reader.readexactly(4))
    if not 10 <= size <= MAX_PACKET:
        raise RconError("invalid")
    data = await reader.readexactly(size)
    request_id, kind = struct.unpack_from("<ii", data)
    return request_id, kind, data[8:].split(b"\0", 1)[0].decode("utf-8", "replace")


async def source(address: ServerAddress, password: str, command: str,
                 timeout: float = 5.0) -> str:
    try:
        async with asyncio.timeout(timeout):
            reader, writer = await asyncio.open_connection(address.host, address.port)
    except (OSError, TimeoutError) as exc:
        raise RconError("connect", str(exc)) from exc
    try:
        async with asyncio.timeout(timeout):
            writer.write(source_packet(1, SERVERDATA_AUTH, password))
            await writer.drain()
            while True:  # puede llegar antes un RESPONSE_VALUE vacío
                request_id, kind, _ = await _read_packet(reader)
                if kind == SERVERDATA_AUTH_RESPONSE:
                    break
            if request_id == -1:
                raise RconError("bad_password")
            # El comando y, detrás, un paquete vacío: su eco marca el final de la respuesta
            # (las largas llegan en varios paquetes).
            writer.write(source_packet(2, SERVERDATA_EXECCOMMAND, command)
                         + source_packet(3, SERVERDATA_RESPONSE_VALUE, ""))
            await writer.drain()
            parts = []
            while True:
                request_id, kind, body = await _read_packet(reader)
                if request_id == 3:
                    break
                if request_id == 2:
                    parts.append(body)
            return "".join(parts)
    except TimeoutError as exc:
        raise RconError("timeout") from exc
    except (OSError, asyncio.IncompleteReadError, struct.error) as exc:
        raise RconError("closed") from exc
    finally:
        writer.close()


async def send(game: GameDef, address: ServerAddress, password: str, command: str) -> str:
    """Manda `command` y devuelve la respuesta; RconError si no se puede."""
    kind = kind_of(game)
    command = clean_command(command)
    if kind is None:
        raise RconError("unsupported")
    if not command:
        return ""
    if kind == "quake":
        return await quake(address, password, command)
    if kind == "goldsrc":
        return await goldsrc(address, password, command)
    return await source(address, password, command)
