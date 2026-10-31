"""Protocolo de Unreal Tournament 2004: master (TCP) y consulta nativa (UDP).

Master (TCP 28902), según el servicio `utmaster` de OpenSpy
(github.com/openspy/openspy-core, code/utmaster/server: UTPeer.cpp,
commands/handle_challenge.cpp y commands/handle_serverlist.cpp) y las respuestas reales
de utmaster.openspy.net y ut2004master.333networks.com (2026-10-20):

- Cada mensaje va precedido de su longitud (int32 little-endian). Las cadenas son FString
  de Unreal: longitud en índice compacto (contando el NUL final); negativa = UTF-16LE.
- Servidor → challenge (FString). Cliente → hash de CD key, hash de respuesta, cliente
  ("UT2K4CLIENT"), versión (int32), plataforma (byte), idioma (FString) y tres int32 + un
  byte de hardware (a cero). Servidor → "APPROVED" (+ int32). Cliente → un mensaje de
  verificación (su contenido no se comprueba). Servidor → "VERIFIED".
- Cliente → petición de lista (byte 0 + número de filtros 0). Servidor → int32 con el
  número de servidores + byte, y un mensaje por servidor: IP (4 bytes), puerto de juego y
  puerto de consulta (uint16 LE), nombre, mapa, tipo de juego, jugadores, máximo, flags…

Los hashes que se envían son FICTICIOS y fijos (`FAKE_CDKEY_HASH`, `FAKE_RESPONSE_HASH`):
no se derivan de ninguna CD key. Si un master los rechaza, no se intenta nada más.

Consulta (UDP, puerto de consulta = juego + 1): `80 00 00 00 <tipo>` con 0 = información,
1 = reglas y 2 = jugadores; las respuestas repiten la cabecera y pueden ocupar varios
datagramas (reglas y jugadores). Las cadenas usan el mismo FString.
"""
from __future__ import annotations

import re

from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import ProtocolError, strip_control

# Ficticios: 32 dígitos hex fijos, no salen de ninguna CD key real.
FAKE_CDKEY_HASH = "0" * 32
FAKE_RESPONSE_HASH = "0" * 32
CLIENT_NAME = "UT2K4CLIENT"
CLIENT_VERSION = 3369
_PLATFORM = 3  # el master resta 3 (0 = Windows)
_LANGUAGE = "int"
MAX_FRAME = 64 * 1024

_QUERY_HEADER = b"\x80\x00\x00\x00"
INFO_QUERY = _QUERY_HEADER + b"\x00"
RULES_QUERY = _QUERY_HEADER + b"\x01"
PLAYERS_QUERY = _QUERY_HEADER + b"\x02"
QUERIES = (INFO_QUERY, RULES_QUERY, PLAYERS_QUERY)
INFO, RULES, PLAYERS = 0, 1, 2

_COLOUR_RE = re.compile(rb"\x1b[\s\S]{3}")  # ESC + R G B


def frame(body: bytes) -> bytes:
    return len(body).to_bytes(4, "little") + body


def compact_int(value: int) -> bytes:
    """Índice compacto de Unreal (bit 0x80 del primer byte = negativo)."""
    sign = 0x80 if value < 0 else 0
    value = abs(value)
    first = value & 0x3F
    value >>= 6
    out = bytearray([sign | first | (0x40 if value else 0)])
    while value:
        byte = value & 0x7F
        value >>= 7
        out.append(byte | (0x80 if value else 0))
    return bytes(out)


def read_compact_int(data: bytes, pos: int) -> tuple[int, int]:
    try:
        first = data[pos]
        pos += 1
        value = first & 0x3F
        shift = 6
        more = first & 0x40
        while more:
            if shift > 27:
                raise ProtocolError("Índice compacto demasiado largo")
            byte = data[pos]
            pos += 1
            value |= (byte & 0x7F) << shift
            shift += 7
            more = byte & 0x80
    except IndexError as exc:
        raise ProtocolError("Índice compacto truncado") from exc
    return (-value if first & 0x80 else value), pos


def _read_raw(data: bytes, pos: int) -> tuple[bytes, bool, int]:
    """Bytes de un FString sin el NUL final; el bool indica UTF-16LE."""
    length, pos = read_compact_int(data, pos)
    wide = length < 0
    size = -2 * length if wide else length
    end = pos + size
    if end > len(data):
        raise ProtocolError("Cadena truncada")
    raw = data[pos:end]
    terminator = b"\0\0" if wide else b"\0"
    if raw.endswith(terminator):
        raw = raw[:-len(terminator)]
    return raw, wide, end


def read_fstring(data: bytes, pos: int) -> tuple[str, int]:
    raw, wide, pos = _read_raw(data, pos)
    return raw.decode("utf-16-le" if wide else "latin-1", "replace"), pos


def _read_text(data: bytes, pos: int) -> tuple[str, int]:
    """FString para mostrar: sin códigos de color (ESC R G B) ni caracteres de control."""
    raw, wide, pos = _read_raw(data, pos)
    if wide:
        text = re.sub(r"\x1b[\s\S]{3}", "", raw.decode("utf-16-le", "replace"))
    else:
        raw = _COLOUR_RE.sub(b"", raw)
        try:  # hay servidores con nombres en UTF-8 dentro de cadenas de 8 bits
            text = raw.decode("utf-8")
        except UnicodeDecodeError:
            text = raw.decode("latin-1")
    return strip_control(text).strip(), pos


def fstring(text: str) -> bytes:
    raw = text.encode("latin-1") + b"\0"
    return compact_int(len(raw)) + raw


def _int32(data: bytes, pos: int, signed: bool = True) -> tuple[int, int]:
    if pos + 4 > len(data):
        raise ProtocolError("Entero truncado")
    return int.from_bytes(data[pos:pos + 4], "little", signed=signed), pos + 4


# ------------------------------------------------------------------ master
def build_challenge_response() -> bytes:
    body = (fstring(FAKE_CDKEY_HASH) + fstring(FAKE_RESPONSE_HASH) + fstring(CLIENT_NAME)
            + CLIENT_VERSION.to_bytes(4, "little") + bytes([_PLATFORM]) + fstring(_LANGUAGE)
            + b"\0" * 12 + b"\0")
    return frame(body)


# Tras APPROVED el cliente manda un mensaje que el master no comprueba (ficticio, fijo).
VERIFY_REQUEST = frame(fstring(FAKE_RESPONSE_HASH))
# Petición de lista: tipo 0 (lista de servidores) y 0 filtros.
LIST_REQUEST = frame(b"\x00\x00")


def check_reply(body: bytes, expected: str) -> None:
    """Exige que el mensaje empiece con el FString `expected` (APPROVED, VERIFIED)."""
    got, _ = read_fstring(body, 0)
    if got != expected:
        raise ProtocolError(f"El master respondió {got[:32]!r} en vez de {expected}")


def parse_list_header(body: bytes) -> int:
    count, _ = _int32(body, 0, signed=False)
    return count


def parse_list_entry(body: bytes) -> ServerAddress:
    """Dirección de consulta (IP + puerto de consulta) de un registro de la lista."""
    if len(body) < 8:
        raise ProtocolError("Registro del master truncado")
    host = ".".join(str(b) for b in body[:4])
    query_port = int.from_bytes(body[6:8], "little")
    if not query_port:
        raise ProtocolError("Registro del master sin puerto de consulta")
    return ServerAddress(host, query_port)


# ------------------------------------------------------------------ consulta
def _check_header(data: bytes, kind: int) -> int:
    if len(data) < 5 or not data.startswith(_QUERY_HEADER) or data[4] != kind:
        raise ProtocolError(f"Cabecera de respuesta {kind} inválida")
    return 5


def parse_info(data: bytes, address: ServerAddress, ping_ms: int) -> ServerInfo:
    pos = _check_header(data, INFO)
    _, pos = _int32(data, pos)  # id del servidor
    _, pos = read_fstring(data, pos)  # IP (suele ir vacía)
    game_port, pos = _int32(data, pos)
    _, pos = _int32(data, pos)  # puerto de consulta
    name, pos = _read_text(data, pos)
    map_name, pos = _read_text(data, pos)
    gametype, pos = _read_text(data, pos)
    num_players, pos = _int32(data, pos)
    max_players, pos = _int32(data, pos)
    port = game_port if 0 < game_port < 65536 else address.port - 1
    return ServerInfo(
        address=address,
        connect_address=ServerAddress(address.host, port),
        name=name,
        map=map_name,
        gametype=gametype,
        num_players=max(num_players, 0),
        max_players=max(max_players, 0),
        password=False,  # va en las reglas (GamePassword); ver apply_details
        version="",
        ping_ms=ping_ms,
    )


def parse_rules(data: bytes, rules: dict[str, str] | None = None) -> dict[str, str]:
    """Pares clave/valor (añadidos a `rules` si se pasa, para juntar varios datagramas);
    las claves repetidas (Mutator) se unen con ", "."""
    pos = _check_header(data, RULES)
    rules = {} if rules is None else rules
    while pos < len(data):
        key, pos = _read_text(data, pos)
        value, pos = _read_text(data, pos)
        rules[key] = f"{rules[key]}, {value}" if key in rules else value
    return rules


def parse_players(data: bytes) -> list[Player]:
    """Jugadores de un datagrama; se omiten las filas de id 0 (puntuación de equipos)."""
    pos = _check_header(data, PLAYERS)
    players: list[Player] = []
    while pos < len(data):
        player_id, pos = _int32(data, pos)
        name, pos = _read_text(data, pos)
        ping, pos = _int32(data, pos)
        score, pos = _int32(data, pos)
        _, pos = _int32(data, pos)  # id de estadísticas + bits de equipo
        if player_id != 0:
            players.append(Player(name, score, ping))
    return players


def apply_details(info: ServerInfo, rules: dict[str, str], players: list[Player]) -> ServerInfo:
    """Completa `info` con reglas (contraseña, versión) y jugadores."""
    info.rules = rules
    info.players = players
    info.password = rules.get("GamePassword", "").casefold() == "true"
    info.version = rules.get("ServerVersion", "")
    return info
