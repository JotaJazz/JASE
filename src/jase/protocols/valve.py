"""Protocolo Valve (GoldSrc/Source): Master Server Query y consultas A2S.

- Master (UDP): `0x31` + región + semilla "ip:puerto\\0" + filtro "\\0". La
  respuesta es `FF FF FF FF 66 0A` + entradas de 6 bytes (IP + puerto big-endian);
  se pide la página siguiente con la última dirección como semilla, hasta que la
  última sea 0.0.0.0:0.
- A2S_INFO: respuesta 0x49 (formato Source, también la usan los HLDS actuales) o
  0x6D (GoldSrc obsoleto). Si el servidor exige challenge responde S2C_CHALLENGE
  (0x41 + 4 bytes) y hay que repetir la consulta con esos 4 bytes al final.
- A2S_PLAYER (0x55): siempre con challenge (primero se pide con FF FF FF FF).
- Multipaquete (`FE FF FF FF`): GoldSrc lleva un byte con índice (nibble alto) y
  total (nibble bajo); Source lleva total, número y tamaño (y, si el id tiene el
  bit alto, datos comprimidos con bzip2: no soportado).
"""
from __future__ import annotations

import struct

from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import ProtocolError, strip_control

OOB = b"\xff\xff\xff\xff"
SPLIT = b"\xfe\xff\xff\xff"
MASTER_HEADER = OOB + b"\x66\x0a"
A2S_INFO = OOB + b"TSource Engine Query\x00"
REGION_ALL = 0xFF
DEFAULT_FILTER = "\\appid\\10"  # Counter-Strike 1.6

S2C_CHALLENGE = 0x41
INFO_SOURCE = 0x49
INFO_GOLDSRC = 0x6D
PLAYER_RESPONSE = 0x44

_SEED_START = "0.0.0.0:0"
_END = ServerAddress("0.0.0.0", 0)


def build_master_query(seed: ServerAddress | None, filter: str = DEFAULT_FILTER) -> bytes:
    start = str(seed) if seed is not None else _SEED_START
    return (bytes([0x31, REGION_ALL]) + start.encode("ascii") + b"\0"
            + filter.encode("latin-1") + b"\0")


def parse_master_response(data: bytes) -> tuple[list[ServerAddress], ServerAddress | None]:
    """(servidores, semilla de la página siguiente); semilla None = fin de la lista."""
    if not data.startswith(MASTER_HEADER):
        raise ProtocolError("Respuesta del master Valve inválida")
    servers: list[ServerAddress] = []
    body = data[len(MASTER_HEADER):]
    for i in range(0, len(body) - len(body) % 6, 6):
        entry = body[i:i + 6]
        servers.append(ServerAddress(".".join(str(b) for b in entry[:4]),
                                     int.from_bytes(entry[4:], "big")))
    if not servers or servers[-1] == _END:
        return [s for s in servers if s != _END], None
    return servers, servers[-1]


def build_info_query(challenge: bytes | None) -> bytes:
    return A2S_INFO + (challenge or b"")


def build_player_query(challenge: bytes | None) -> bytes:
    return OOB + b"U" + (challenge if challenge is not None else OOB)


def challenge_of(data: bytes) -> bytes | None:
    """Los 4 bytes del S2C_CHALLENGE, o None si `data` no es un challenge."""
    if len(data) >= 9 and data.startswith(OOB) and data[4] == S2C_CHALLENGE:
        return data[5:9]
    return None


class PacketAssembler:
    """Reensambla respuestas multipaquete (formatos GoldSrc y Source), desordenadas."""

    def __init__(self) -> None:
        self._fragments: list[bytes] = []

    def feed(self, data: bytes) -> bytes | None:
        """Devuelve el mensaje completo (empieza por FF FF FF FF) o None si faltan partes."""
        if data.startswith(OOB):
            return data
        if not data.startswith(SPLIT) or len(data) < 9:
            raise ProtocolError("Paquete Valve inválido")
        self._fragments.append(data)
        return self._assemble_goldsrc() or self._assemble_source()

    def _assemble_goldsrc(self) -> bytes | None:
        parts: dict[int, bytes] = {}
        total = self._fragments[0][8] & 0x0F
        for frag in self._fragments:
            if frag[8] & 0x0F != total:
                return None
            parts[frag[8] >> 4] = frag[9:]
        if total == 0 or set(parts) != set(range(total)) or not parts[0].startswith(OOB):
            return None
        return b"".join(parts[i] for i in range(total))

    def _assemble_source(self) -> bytes | None:
        parts: dict[int, bytes] = {}
        first = self._fragments[0]
        if len(first) < 12:
            return None
        total = first[8]
        for frag in self._fragments:
            if len(frag) < 12 or frag[8] != total:
                return None
            req_id = struct.unpack_from("<l", frag, 4)[0]
            if frag[9] == 0 and req_id < 0:
                raise ProtocolError("Respuesta comprimida (bzip2) no soportada")
            parts[frag[9]] = frag[12:]
        if total == 0 or set(parts) != set(range(total)) or not parts[0].startswith(OOB):
            return None
        return b"".join(parts[i] for i in range(total))


class _Reader:
    def __init__(self, data: bytes, pos: int) -> None:
        self.data = data
        self.pos = pos

    def byte(self) -> int:
        if self.pos >= len(self.data):
            raise ProtocolError("Respuesta Valve truncada")
        value = self.data[self.pos]
        self.pos += 1
        return value

    def unpack(self, fmt: str) -> tuple:
        size = struct.calcsize(fmt)
        if self.pos + size > len(self.data):
            raise ProtocolError("Respuesta Valve truncada")
        values = struct.unpack_from(fmt, self.data, self.pos)
        self.pos += size
        return values

    def string(self) -> str:
        end = self.data.find(b"\0", self.pos)
        if end < 0:
            raise ProtocolError("Cadena sin terminar en respuesta Valve")
        raw = self.data[self.pos:end]
        self.pos = end + 1
        try:
            return raw.decode("utf-8")
        except UnicodeDecodeError:
            return raw.decode("latin-1")


def parse_info(data: bytes, address: ServerAddress, ping_ms: int) -> ServerInfo:
    """Respuesta A2S_INFO (0x49 o 0x6D) → ServerInfo. La dirección de conexión es la
    consultada: en HLDS el puerto de juego es el mismo que el de consulta."""
    if len(data) < 6 or not data.startswith(OOB):
        raise ProtocolError("Respuesta A2S_INFO inválida")
    kind = data[4]
    r = _Reader(data, 5)
    rules: dict[str, str] = {}
    if kind == INFO_SOURCE:
        rules["protocol"] = str(r.byte())
        name, map_, folder, game = r.string(), r.string(), r.string(), r.string()
        (appid,) = r.unpack("<H")
        players, max_players, bots = r.byte(), r.byte(), r.byte()
        r.byte()  # tipo de servidor (d/l/p)
        r.byte()  # entorno (l/w/m/o)
        visibility = r.byte()
        r.byte()  # VAC
        version = r.string()
        rules["appid"] = str(appid)
    elif kind == INFO_GOLDSRC:
        r.string()  # dirección "ip:puerto" anunciada
        name, map_, folder, game = r.string(), r.string(), r.string(), r.string()
        players, max_players = r.byte(), r.byte()
        rules["protocol"] = str(r.byte())
        r.byte()  # tipo de servidor
        r.byte()  # entorno
        visibility = r.byte()
        if r.byte() == 1:  # mod de Half-Life: enlace, descarga, versión, tamaño…
            r.string()
            r.string()
            r.byte()
            r.unpack("<ll")
            r.byte()
            r.byte()
        r.byte()  # VAC
        bots = r.byte()
        version = ""
    else:
        raise ProtocolError(f"Tipo de respuesta A2S_INFO desconocido: {kind:#x}")
    rules.update(gamedir=folder, game=game, bots=str(bots))
    return ServerInfo(
        address=address,
        connect_address=address,
        name=strip_control(name),
        map=map_,
        gametype=game or folder,
        num_players=players,
        max_players=max_players,
        password=visibility == 1,
        version=version,
        ping_ms=ping_ms,
        rules=rules,
    )


def parse_players(data: bytes) -> list[Player]:
    """Respuesta A2S_PLAYER (0x44). El protocolo no da ping por jugador (queda en 0)."""
    if len(data) < 6 or not data.startswith(OOB) or data[4] != PLAYER_RESPONSE:
        raise ProtocolError("Respuesta A2S_PLAYER inválida")
    r = _Reader(data, 5)
    count = r.byte()
    players: list[Player] = []
    for _ in range(count):
        r.byte()  # índice
        name = r.string()
        score, _duration = r.unpack("<lf")
        players.append(Player(strip_control(name), score, 0))
    return players
