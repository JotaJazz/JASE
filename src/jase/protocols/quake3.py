"""Protocolo Quake3 (CoD1/CoD2): consulta al master y getstatus."""
from __future__ import annotations

import re

from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import ProtocolError, strip_control

OOB = b"\xff\xff\xff\xff"
GETSTATUS = OOB + b"getstatus"

_MASTER_HEADER = b"getserversResponse"
_STATUS_HEADER = "statusResponse"
_ENTRY_SIZE = 7  # "\" + 4 IP + 2 puerto big-endian
COLOR_RE = re.compile(r"\^[0-9A-Za-z]")


def build_getservers(protocol: int) -> bytes:
    return OOB + f"getservers {protocol} full empty".encode("ascii")


def parse_getservers_response(data: bytes) -> tuple[list[ServerAddress], bool]:
    """Devuelve (servidores, es_el_último_paquete).

    Formato real de los masters de CoD1/CoD2: tras la cabecera viene "\\n\\0" y luego
    entradas "\\" + IP + puerto. Cada paquete intermedio termina en "\\EOT" (sin
    relleno) y el último en "\\EOF". También se acepta "\\EOT\\0\\0\\0" (Quake3
    clásico) como final.
    """
    start = data.find(_MASTER_HEADER)
    if start < 0:
        raise ProtocolError("Respuesta del master inválida")
    body = data[start + len(_MASTER_HEADER):]
    first = body.find(b"\\")
    if first < 0:
        return [], False
    servers: list[ServerAddress] = []
    pos = first
    while len(body) - pos >= 4:
        if body[pos] != 0x5C:
            break
        tail = body[pos + 1:pos + 4]
        rest = body[pos + 4:]
        if tail in (b"EOT", b"EOF") and (rest == b"" or rest[:3] == b"\0\0\0"):
            return servers, tail == b"EOF" or rest[:3] == b"\0\0\0"
        if len(body) - pos < _ENTRY_SIZE:
            break
        ip = ".".join(str(b) for b in body[pos + 1:pos + 5])
        port = int.from_bytes(body[pos + 5:pos + 7], "big")
        servers.append(ServerAddress(ip, port))
        pos += _ENTRY_SIZE
    return servers, False


def strip_colors(text: str) -> str:
    """Quita códigos de color ^X y caracteres de control (antes, por si separan ^ y X)."""
    return COLOR_RE.sub("", strip_control(text))


def _parse_infostring(line: str) -> dict[str, str]:
    parts = line.split("\\")
    if parts and parts[0] == "":
        parts = parts[1:]
    return {parts[i]: parts[i + 1] for i in range(0, len(parts) - 1, 2)}


def _parse_player(line: str) -> Player:
    first = line.find('"')
    last = line.rfind('"')
    if first < 0 or last <= first:
        raise ProtocolError(f"Línea de jugador inválida: {line!r}")
    try:
        score_s, ping_s = line[:first].split()
        raw = line[first + 1:last]
        return Player(strip_colors(raw), int(score_s), int(ping_s), raw_name=strip_control(raw))
    except ValueError as exc:
        raise ProtocolError(f"Línea de jugador inválida: {line!r}") from exc


def parse_status_response(data: bytes, address: ServerAddress, ping_ms: int) -> ServerInfo:
    text = data.decode("latin-1")
    if not text.startswith(OOB.decode("latin-1") + _STATUS_HEADER):
        raise ProtocolError("Falta statusResponse")
    lines = text.split("\n")
    if len(lines) < 2:
        raise ProtocolError("statusResponse sin infostring")
    rules = _parse_infostring(lines[1])
    try:
        max_players = int(rules.get("sv_maxclients", "0"))
    except ValueError as exc:
        raise ProtocolError("sv_maxclients inválido") from exc
    players = [_parse_player(ln) for ln in lines[2:] if ln.strip()]
    return ServerInfo(
        address=address,
        connect_address=address,
        name=strip_colors(rules.get("sv_hostname", "")),
        map=rules.get("mapname", ""),
        gametype=rules.get("g_gametype", ""),
        num_players=len(players),
        max_players=max_players,
        password=rules.get("pswrd") == "1" or rules.get("g_needpass") == "1",
        version=rules.get("shortversion", ""),
        ping_ms=ping_ms,
        players=players,
        rules=rules,
        raw_name=strip_control(rules.get("sv_hostname", "")),
    )
