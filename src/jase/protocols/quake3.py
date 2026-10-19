"""Protocolo Quake3 (CoD1/CoD2): consulta al master y getstatus."""
from __future__ import annotations

import re

from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import ProtocolError

OOB = b"\xff\xff\xff\xff"
GETSTATUS = OOB + b"getstatus"

_MASTER_HEADER = b"getserversResponse"
_STATUS_HEADER = "statusResponse"
_ENTRY_SIZE = 7  # "\" + 4 IP + 2 puerto big-endian
_COLOR_RE = re.compile(r"\^[0-9A-Za-z]")


def build_getservers(protocol: int) -> bytes:
    return OOB + f"getservers {protocol} full empty".encode("ascii")


def parse_getservers_response(data: bytes) -> tuple[list[ServerAddress], bool]:
    start = data.find(_MASTER_HEADER)
    if start < 0:
        raise ProtocolError("Respuesta del master inválida")
    body = data[start + len(_MASTER_HEADER):]
    servers: list[ServerAddress] = []
    pos = 0
    while len(body) - pos >= _ENTRY_SIZE:
        if body[pos] != 0x5C:
            break
        if body[pos + 1:pos + 5] == b"EOT\x00":
            return servers, True
        ip = ".".join(str(b) for b in body[pos + 1:pos + 5])
        port = int.from_bytes(body[pos + 5:pos + 7], "big")
        servers.append(ServerAddress(ip, port))
        pos += _ENTRY_SIZE
    return servers, False


def strip_colors(text: str) -> str:
    return _COLOR_RE.sub("", text)


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
        return Player(strip_colors(line[first + 1:last]), int(score_s), int(ping_s))
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
    )
