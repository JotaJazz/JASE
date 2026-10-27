"""Protocolo de lanzadores de Zandronum (Doom multijugador): master y consulta. Sin red.

Todo paquete va con el Huffman de `huffman` (o sin comprimir, con 0xFF delante). Enteros
en little-endian; cadenas terminadas en \\0. Campos verificados contra sv_master.cpp de
Zandronum y servidores reales (2026-10-27).
"""
from __future__ import annotations

import struct

from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import ProtocolError, huffman, strip_control

MASTER_CHALLENGE = 5660028
MASTER_PROTOCOL = 2
SERVER_CHALLENGE = 199
SERVER_ACCEPTED = 5660023
SERVER_IGNORING = 5660024  # se consultó hace muy poco
SERVER_BANNED = 5660025

# Respuestas del master.
MSC_BEGINSERVERLISTPART = 6
MSC_SERVERBLOCK = 8
MSC_ENDSERVERLISTPART = 7
MSC_ENDSERVERLIST = 2
MASTER_ERRORS = {3: "IP bloqueada en el master", 4: "consulta ignorada (demasiado seguida)",
                 5: "versión de protocolo antigua"}
MASTER_IGNORING = 4  # el master pide unos 10 s entre consultas


class MasterBusy(ProtocolError):
    """El master ignoró la consulta por llegar demasiado pronto tras la anterior."""

# Campos pedidos al servidor (SQF_*), en el orden en que llegan.
SQF_NAME = 0x1
SQF_MAPNAME = 0x8
SQF_MAXCLIENTS = 0x10
SQF_MAXPLAYERS = 0x20
SQF_PWADS = 0x40
SQF_GAMETYPE = 0x80
SQF_GAMENAME = 0x100
SQF_IWAD = 0x200
SQF_FORCEPASSWORD = 0x400
SQF_FORCEJOINPASSWORD = 0x800
SQF_NUMPLAYERS = 0x80000
SQF_PLAYERDATA = 0x100000
QUERY_FLAGS = (SQF_NAME | SQF_MAPNAME | SQF_MAXCLIENTS | SQF_MAXPLAYERS | SQF_PWADS
               | SQF_GAMETYPE | SQF_GAMENAME | SQF_IWAD | SQF_FORCEPASSWORD
               | SQF_FORCEJOINPASSWORD | SQF_NUMPLAYERS | SQF_PLAYERDATA)

# Modos de juego (GAMEMODE_e) y los que llevan equipos (el jugador trae un byte más).
GAMEMODES = ("cooperative", "survival", "invasion", "deathmatch", "teamplay", "duel",
             "terminator", "lastmanstanding", "teamlms", "possession", "teampossession",
             "teamgame", "ctf", "oneflagctf", "skulltag", "domination")
TEAM_MODES = {4, 8, 10, 11, 12, 13, 14, 15}

MASTER_REQUEST = huffman.encode_raw(struct.pack("<iH", MASTER_CHALLENGE, MASTER_PROTOCOL))


def build_query(time_ms: int = 0) -> bytes:
    return huffman.encode_raw(struct.pack("<iIi", SERVER_CHALLENGE, QUERY_FLAGS,
                                          time_ms & 0x7FFFFFFF))


def parse_master_packet(data: bytes) -> tuple[list[ServerAddress], bool]:
    """(direcciones del paquete, ¿era el último?)."""
    body = huffman.decode(data)
    if len(body) < 4:
        raise ProtocolError("Respuesta del master demasiado corta")
    (code,) = struct.unpack_from("<i", body, 0)
    if code == MASTER_IGNORING:
        raise MasterBusy(MASTER_ERRORS[code])
    if code in MASTER_ERRORS:
        raise ProtocolError(MASTER_ERRORS[code])
    if code != MSC_BEGINSERVERLISTPART or len(body) < 6:
        raise ProtocolError(f"Respuesta del master inesperada: {code}")
    pos = 5  # código + número de paquete
    found: list[ServerAddress] = []
    try:
        while True:
            command = body[pos]
            pos += 1
            if command == MSC_ENDSERVERLISTPART:
                return found, False
            if command == MSC_ENDSERVERLIST:
                return found, True
            if command != MSC_SERVERBLOCK:
                raise ProtocolError(f"Bloque desconocido en el master: {command}")
            while True:
                count = body[pos]
                pos += 1
                if count == 0:
                    break
                host = ".".join(str(b) for b in body[pos:pos + 4])
                pos += 4
                for _ in range(count):
                    (port,) = struct.unpack_from("<H", body, pos)
                    pos += 2
                    if port:
                        found.append(ServerAddress(host, port))
    except (IndexError, struct.error) as exc:
        raise ProtocolError("Lista del master truncada") from exc


class _Reader:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.pos = 0

    def byte(self) -> int:
        if self.pos >= len(self.data):
            raise ProtocolError("Respuesta truncada")
        value = self.data[self.pos]
        self.pos += 1
        return value

    def unpack(self, fmt: str) -> tuple:
        try:
            values = struct.unpack_from(fmt, self.data, self.pos)
        except struct.error as exc:
            raise ProtocolError("Respuesta truncada") from exc
        self.pos += struct.calcsize(fmt)
        return values

    def string(self) -> str:
        end = self.data.find(b"\0", self.pos)
        if end < 0:
            raise ProtocolError("Cadena sin terminar")
        text = self.data[self.pos:end].decode("latin-1")
        self.pos = end + 1
        return text


def strip_colors(text: str) -> str:
    """Quita los códigos de color de Doom (\\x1c + letra o \\x1c[nombre])."""
    out, i = [], 0
    while i < len(text):
        if text[i] == "\x1c":
            if i + 1 < len(text) and text[i + 1] == "[":
                end = text.find("]", i + 2)
                i = end + 1 if end >= 0 else len(text)
            else:
                i += 2
            continue
        out.append(text[i])
        i += 1
    return strip_control("".join(out))


def _players(r: _Reader, count: int, teams: bool) -> tuple[list[Player], int]:
    players, bots = [], 0
    for _ in range(count):
        name = strip_colors(r.string())
        score, ping = r.unpack("<hh")
        _spectator, bot = r.byte(), r.byte()
        if teams:
            r.byte()
        r.byte()  # minutos en el servidor
        players.append(Player(name, score, ping))
        bots += 1 if bot else 0
    return players, bots


def parse_response(data: bytes, address: ServerAddress, ping_ms: int) -> ServerInfo:
    body = huffman.decode(data)
    r = _Reader(body)
    (code,) = r.unpack("<i")
    if code != SERVER_ACCEPTED:
        raise ProtocolError(f"El servidor no respondió la consulta ({code})")
    r.unpack("<i")  # nuestro tiempo, devuelto
    version = r.string()
    (flags,) = r.unpack("<I")
    if flags & ~QUERY_FLAGS:
        raise ProtocolError(f"Campos no pedidos en la respuesta: {flags:#x}")
    rules: dict[str, str] = {"version": version}
    name = strip_colors(r.string()) if flags & SQF_NAME else ""
    map_ = r.string() if flags & SQF_MAPNAME else ""
    max_clients = r.byte() if flags & SQF_MAXCLIENTS else 0
    max_players = r.byte() if flags & SQF_MAXPLAYERS else max_clients
    if flags & SQF_PWADS:
        pwads = [r.string() for _ in range(r.byte())]
        rules["pwads"] = " ".join(pwads)
    mode = -1
    if flags & SQF_GAMETYPE:
        mode, instagib, buckshot = r.byte(), r.byte(), r.byte()
        rules.update(instagib=str(instagib), buckshot=str(buckshot))
    if flags & SQF_GAMENAME:
        rules["game"] = r.string()
    if flags & SQF_IWAD:
        rules["iwad"] = r.string()
    password = False
    if flags & SQF_FORCEPASSWORD:
        password = bool(r.byte())
    if flags & SQF_FORCEJOINPASSWORD:
        rules["forcejoinpassword"] = str(r.byte())
    count = r.byte() if flags & SQF_NUMPLAYERS else 0
    players: list[Player] = []
    bots = 0
    if flags & SQF_PLAYERDATA and count:
        start = r.pos
        players, bots = _players(r, count, mode in TEAM_MODES)
        if r.pos != len(body):  # modo desconocido: se prueba con/sin byte de equipo
            r.pos = start
            players, bots = _players(r, count, mode not in TEAM_MODES)
            if r.pos != len(body):
                raise ProtocolError("Datos de jugadores inconsistentes")
    gametype = GAMEMODES[mode] if 0 <= mode < len(GAMEMODES) else ""
    # Versión filtrable: «3.2.1-r230709 on Linux…» → «3.2.1».
    short = version.split(" ", 1)[0].split("-", 1)[0]
    return ServerInfo(
        address=address,
        connect_address=address,
        name=name,
        map=map_,
        gametype=gametype,
        num_players=count,
        max_players=max_players,
        password=password,
        version=short,
        ping_ms=ping_ms,
        players=players,
        rules=rules,
        raw_name=name,
        bots=bots,
    )
