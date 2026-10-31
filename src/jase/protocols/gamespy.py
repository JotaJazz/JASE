"""Protocolo GameSpy (UT99): master (puerto 28900) y consulta \\status\\."""
from __future__ import annotations

import re

from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import ProtocolError, bounded_int, strip_control

UT_GAMENAME = "ut"
UT_GAMEKEY = "Z5Nfb0"
STATUS_QUERY = b"\\status\\"

_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
_PLAYER_FIELD_RE = re.compile(r"^(.+)_(\d+)$")
_QUERYID_RE = re.compile(r"^\d+\.(\d+)$")


def parse_kv(data: bytes) -> list[tuple[str, str]]:
    parts = data.decode("latin-1").split("\\")
    if parts and parts[0] == "":
        parts = parts[1:]
    return [(parts[i], parts[i + 1]) for i in range(0, len(parts) - 1, 2)]


def parse_secure(data: bytes) -> str:
    for key, value in parse_kv(data):
        if key == "secure":
            return value
    raise ProtocolError("Falta el campo secure en la respuesta del master")


def gsmsalg(secure: str, key: str) -> str:
    k, src = key.encode(), bytearray(secure.encode())
    table = list(range(256))
    a = 0
    for i in range(256):
        a = (a + table[i] + k[i % len(k)]) & 0xFF
        table[a], table[i] = table[i], table[a]
    a = b = 0
    for i in range(len(src)):
        a = (a + src[i] + 1) & 0xFF
        x = table[a]
        b = (b + x) & 0xFF
        y = table[b]
        table[b], table[a] = x, y
        src[i] ^= table[(x + y) & 0xFF]
    out = []
    for i in range(0, len(src) - len(src) % 3, 3):
        t = (src[i] << 16) | (src[i + 1] << 8) | src[i + 2]
        out += [_ALPHABET[(t >> s) & 63] for s in (18, 12, 6, 0)]
    return "".join(out)


def build_validate(secure: str, gamename: str = UT_GAMENAME, gamekey: str = UT_GAMEKEY) -> bytes:
    response = gsmsalg(secure, gamekey)
    return f"\\gamename\\{gamename}\\enctype\\0\\validate\\{response}\\final\\".encode("latin-1")


def build_list_request(gamename: str = UT_GAMENAME) -> bytes:
    return f"\\list\\\\gamename\\{gamename}\\final\\".encode("latin-1")


def parse_master_list(data: bytes) -> list[ServerAddress]:
    servers: list[ServerAddress] = []
    for key, value in parse_kv(data):
        if key != "ip":
            continue
        host, sep, port = value.rpartition(":")
        if not sep or not host or not (port.isascii() and port.isdigit()):
            continue
        if not 1 <= int(port) <= 65535:
            continue
        servers.append(ServerAddress(host, int(port)))
    return servers


class StatusAssembler:
    """Reensambla las respuestas \\status\\ fragmentadas, posiblemente desordenadas."""

    def __init__(self) -> None:
        self._packets: dict[int, list[tuple[str, str]]] = {}
        self._total: int | None = None

    def feed(self, data: bytes) -> bool:
        pairs = parse_kv(data)
        number: int | None = None
        final = False
        for key, value in pairs:
            if key == "queryid":
                m = _QUERYID_RE.match(value)
                if m and int(m.group(1)) >= 1:
                    number = int(m.group(1))
            elif key == "final":
                final = True
        if number is None:
            raise ProtocolError("Paquete de estado sin queryid válido")
        self._packets[number] = pairs
        if final:
            self._total = number
        return self.complete

    @property
    def complete(self) -> bool:
        return self._total is not None and all(
            n in self._packets for n in range(1, self._total + 1)
        )

    def fields(self) -> dict[str, str]:
        result: dict[str, str] = {}
        for n in sorted(self._packets):
            for key, value in self._packets[n]:
                if key not in ("queryid", "final"):
                    result[key] = value
        return result


def _to_int(fields: dict[str, str], key: str, default: int = 0) -> int:
    try:
        return bounded_int(fields.get(key, default))
    except ValueError as exc:
        raise ProtocolError(f"Valor numérico inválido en {key}: {fields[key]!r}") from exc


def parse_status(fields: dict[str, str], address: ServerAddress, ping_ms: int,
                 connect_port_fallback: int = -1, expected_gamename: str = "",
                 version_key: str = "gamever",
                 version_aliases: tuple[tuple[str, str, str], ...] = ()) -> ServerInfo:
    """`connect_port_fallback`: desplazamiento sobre el puerto de consulta si falta
    `hostport` (UT99: el juego está en consulta − 1).

    `expected_gamename`: si no está vacío y el servidor envía otro `gamename`,
    ProtocolError (servidor de otro juego listado por el master).

    `version_key`: regla con la versión (si falta, `gamever`); `version_aliases` la traduce
    a la clave del selector (Tactical Ops: gametype «TO340» → «3.4»)."""
    gamename = fields.get("gamename", "")
    if expected_gamename and gamename and gamename.casefold() != expected_gamename.casefold():
        raise ProtocolError(f"Servidor de otro juego: gamename {gamename!r}")
    num_players = _to_int(fields, "numplayers")
    max_players = _to_int(fields, "maxplayers")
    hostport = _to_int(fields, "hostport", address.port + connect_port_fallback)
    deduced = "hostport" not in fields or not 1 <= hostport <= 65535
    if not 1 <= hostport <= 65535:
        hostport = address.port + connect_port_fallback

    indices = sorted(
        int(m.group(2))
        for key in fields
        if (m := _PLAYER_FIELD_RE.match(key)) and m.group(1) == "player"
    )
    players = [
        Player(
            strip_control(fields[f"player_{i}"]),
            _to_int(fields, f"frags_{i}"),
            _to_int(fields, f"ping_{i}"),
        )
        for i in indices
    ]
    rules = {
        k: v
        for k, v in fields.items()
        if k not in ("queryid", "final") and not _PLAYER_FIELD_RE.match(k)
    }
    version = fields.get(version_key) or fields.get("gamever", "")
    version = next((key for raw, key, _label in version_aliases if raw == version), version)
    return ServerInfo(
        address=address,
        connect_address=ServerAddress(address.host, hostport),
        connect_port_deduced=deduced,
        name=strip_control(fields.get("hostname", "")),
        map=fields.get("mapname", ""),
        gametype=fields.get("gametype", ""),
        num_players=num_players,
        max_players=max_players,
        password=fields.get("password", "").lower() in ("true", "1"),
        version=version,
        ping_ms=ping_ms,
        players=players,
        rules=rules,
    )
