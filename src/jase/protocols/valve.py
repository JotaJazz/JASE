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


MAX_SPLIT_PACKETS = 16  # tope de partes por respuesta (y de fragmentos retenidos)


class PacketAssembler:
    """Reensambla respuestas multipaquete (formatos GoldSrc y Source), desordenadas.

    La cabecera no dice qué formato es, así que cada fragmento se interpreta de las dos
    formas y se guarda en un dict por índice (los duplicados sobrescriben). Solo se
    retienen fragmentos de un id de petición: un id nuevo descarta los anteriores.
    Un fragmento sin interpretación válida (o con más de MAX_SPLIT_PACKETS partes)
    lanza ProtocolError. Memoria y trabajo por paquete acotados."""

    def __init__(self) -> None:
        self._request_id: int | None = None
        self._reset()

    def _reset(self) -> None:
        # Por formato: total declarado (None = aún ninguno; False = descartado) y partes.
        self._totals: dict[str, int | None | bool] = {"goldsrc": None, "source": None}
        self._parts: dict[str, dict[int, bytes]] = {"goldsrc": {}, "source": {}}

    def feed(self, data: bytes) -> bytes | None:
        """Devuelve el mensaje completo (empieza por FF FF FF FF) o None si faltan partes."""
        if data.startswith(OOB):
            return data
        if not data.startswith(SPLIT) or len(data) < 9:
            raise ProtocolError("Paquete Valve inválido")
        request_id = struct.unpack_from("<l", data, 4)[0]
        if request_id != self._request_id:
            self._request_id = request_id
            self._reset()
        candidates = {"goldsrc": self._goldsrc(data), "source": self._source(data)}
        accepted = False
        error = "Fragmento Valve inválido"
        for fmt, parsed in candidates.items():
            if isinstance(parsed, str):
                error = parsed
                continue
            if parsed is None or self._totals[fmt] is False:
                continue
            total, index, payload = parsed
            if self._totals[fmt] is None:
                self._totals[fmt] = total
            elif self._totals[fmt] != total:
                self._totals[fmt] = False
                self._parts[fmt].clear()
                continue
            self._parts[fmt][index] = payload
            accepted = True
            if len(self._parts[fmt]) == total:
                return b"".join(self._parts[fmt][i] for i in range(total))
        if not accepted:
            raise ProtocolError(error)
        return None

    @staticmethod
    def _goldsrc(data: bytes) -> tuple[int, int, bytes] | None:
        total, index = data[8] & 0x0F, data[8] >> 4
        payload = data[9:]
        if total == 0 or index >= total or (index == 0 and not payload.startswith(OOB)):
            return None
        return total, index, payload

    @staticmethod
    def _source(data: bytes) -> tuple[int, int, bytes] | str | None:
        """(total, índice, datos), None si no encaja o un motivo de rechazo (str), que
        solo se usa si tampoco encaja como GoldSrc."""
        if len(data) < 12:
            return None
        total, index = data[8], data[9]
        if total == 0 or index >= total:
            return None
        if total > MAX_SPLIT_PACKETS:
            return f"Respuesta Valve en {total} partes (máx. {MAX_SPLIT_PACKETS})"
        if index == 0 and struct.unpack_from("<l", data, 4)[0] < 0:
            return "Respuesta comprimida (bzip2) no soportada"
        payload = data[12:]
        if index == 0 and not payload.startswith(OOB):
            return None
        return total, index, payload


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


def parse_info(data: bytes, address: ServerAddress, ping_ms: int,
               expected_gamedir: str = "") -> ServerInfo:
    """Respuesta A2S_INFO (0x49 o 0x6D) → ServerInfo. La dirección de conexión es la
    consultada salvo que el EDF (0x80) anuncie otro puerto de juego.

    `expected_gamedir`: si no está vacío y el servidor informa otra carpeta de juego,
    se rechaza (los masters de Valve mezclan juegos)."""
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
    connect = address
    if kind == INFO_SOURCE and r.pos < len(data):
        edf = r.byte()
        if edf & 0x80 and r.pos + 2 <= len(data):  # puerto de juego (el primer campo EDF)
            (game_port,) = r.unpack("<H")
            if game_port:
                connect = ServerAddress(address.host, game_port)
    if expected_gamedir and folder.casefold() != expected_gamedir.casefold():
        raise ProtocolError(f"Servidor de otro juego: {folder!r}")
    rules.update(gamedir=folder, game=game, bots=str(bots))
    return ServerInfo(
        address=address,
        connect_address=connect,
        name=strip_control(name),
        map=map_,
        gametype=game or folder,
        num_players=players,
        bots=min(bots, players),
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
