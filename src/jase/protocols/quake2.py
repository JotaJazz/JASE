"""Protocolo Quake II: master (`query`) y `status` por UDP."""
from __future__ import annotations

from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import ProtocolError, strip_control

OOB = b"\xff\xff\xff\xff"
MASTER_QUERY = b"query\n\0"
STATUS_QUERY = OOB + b"status\n"

_MASTER_HEADER = OOB + b"servers "
_STATUS_HEADER = OOB + b"print\n"
_ENTRY_SIZE = 6  # 4 IP + 2 puerto big-endian


def parse_master_response(data: bytes) -> list[ServerAddress]:
    if not data.startswith(_MASTER_HEADER):
        raise ProtocolError("Respuesta del master inválida")
    body = data[len(_MASTER_HEADER):]
    servers: list[ServerAddress] = []
    for pos in range(0, len(body) - _ENTRY_SIZE + 1, _ENTRY_SIZE):
        port = int.from_bytes(body[pos + 4:pos + 6], "big")
        if port:
            servers.append(ServerAddress(".".join(str(b) for b in body[pos:pos + 4]), port))
    return servers


def short_version(version: str) -> str:
    """«3.20 x86_64 Linux» → «3.20»; «q2proded r1~277d2ef Mar 24 2026 …» → «q2proded r1~277d2ef»."""
    words = version.split()
    if not words:
        return ""
    return words[0] if words[0][0].isdigit() else " ".join(words[:2])


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
        name = strip_control(line[first + 1:last])
        return Player(name, int(score_s), int(ping_s), raw_name=name)
    except ValueError as exc:
        raise ProtocolError(f"Línea de jugador inválida: {line!r}") from exc


def parse_status(data: bytes, address: ServerAddress, ping_ms: int) -> ServerInfo:
    if not data.startswith(_STATUS_HEADER):
        raise ProtocolError("Falta la cabecera print")
    lines = data[len(_STATUS_HEADER):].decode("latin-1").split("\n")
    rules = _parse_infostring(lines[0])
    try:
        max_players = int(rules.get("maxclients", "0"))
        password = int(rules.get("needpass", "0") or 0) != 0
    except ValueError as exc:
        raise ProtocolError("maxclients/needpass inválido") from exc
    players = [_parse_player(ln) for ln in lines[1:] if ln.strip()]
    name = strip_control(rules.get("hostname", ""))
    return ServerInfo(
        address=address,
        connect_address=address,
        name=name,
        map=rules.get("mapname", ""),
        gametype=rules.get("gamename", ""),
        num_players=len(players),
        max_players=max_players,
        password=password,
        version=short_version(rules.get("version", "")),
        ping_ms=ping_ms,
        players=players,
        rules=rules,
        raw_name=name,
    )
