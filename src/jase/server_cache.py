"""Listas de servidores del último escaneo completo, guardadas entre sesiones. Sin Qt.

Un archivo por juego (`<id>.json`) y un índice pequeño (`index.json`: servidores y fecha
de cada lista) para los contadores del árbol sin leer las listas. Carga tolerante:
un archivo dañado no es una lista y una entrada inválida se descarta sola.
"""
from __future__ import annotations

import contextlib
import json
import math
from pathlib import Path
from typing import Any

from jase.fetch_meta import atomic_write
from jase.games import GAMES
from jase.masters import valid_host, valid_port
from jase.models import Player, ServerAddress, ServerInfo
from jase.protocols import bounded_int

SCHEMA = 1
MAX_BYTES = 64 * 1024 * 1024  # una lista de Steam (10 000 servidores) ocupa unos 10 MB
INDEX = "index.json"


def _address_json(a: ServerAddress) -> str:
    return f"{a.host}:{a.port}"


def _to_json(info: ServerInfo) -> dict[str, Any]:
    return {"address": _address_json(info.address),
            "connect": _address_json(info.connect_address),
            "name": info.name, "map": info.map, "gametype": info.gametype,
            "num_players": info.num_players, "max_players": info.max_players,
            "password": info.password, "version": info.version, "ping_ms": info.ping_ms,
            "players": [[p.name, p.score, p.ping, p.raw_name] for p in info.players],
            "rules": info.rules, "raw_name": info.raw_name, "bots": info.bots,
            "deduced": info.connect_port_deduced}


def _address(text: object) -> ServerAddress:
    if not isinstance(text, str):
        raise ValueError("dirección")
    host, sep, port = text.rpartition(":")
    if not sep or not valid_host(host) or not port.isdigit() or not valid_port(int(port)):
        raise ValueError("dirección")
    return ServerAddress(host, int(port))


def _str(value: object) -> str:
    if not isinstance(value, str):
        raise ValueError("texto")
    return value


def _int(value: object) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError("número")
    return bounded_int(value)


def _from_json(raw: object) -> ServerInfo:
    """ValueError (o TypeError) si la entrada no tiene la forma esperada."""
    if not isinstance(raw, dict):
        raise ValueError("entrada")
    players = [Player(_str(n), _int(s), _int(p), raw_name=_str(r))
               for n, s, p, r in raw["players"]]
    rules = raw["rules"]
    if not isinstance(rules, dict) or not all(
            isinstance(k, str) and isinstance(v, str) for k, v in rules.items()):
        raise ValueError("reglas")
    if not isinstance(raw["password"], bool) or not isinstance(raw["deduced"], bool):
        raise ValueError("booleano")
    return ServerInfo(
        address=_address(raw["address"]), connect_address=_address(raw["connect"]),
        name=_str(raw["name"]), map=_str(raw["map"]), gametype=_str(raw["gametype"]),
        num_players=_int(raw["num_players"]), max_players=_int(raw["max_players"]),
        password=raw["password"], version=_str(raw["version"]), ping_ms=_int(raw["ping_ms"]),
        players=players, rules=dict(rules), raw_name=_str(raw["raw_name"]),
        bots=_int(raw["bots"]), connect_port_deduced=raw["deduced"])


def _valid_time(value: object) -> bool:
    """Instante Unix usable (json lee NaN, Infinity y 1e300; Qt no los convierte)."""
    return (isinstance(value, (int, float)) and not isinstance(value, bool)
            and math.isfinite(value) and 0 <= value < 2**33)


def _path(cache_dir: Path, game_id: str) -> Path | None:
    return Path(cache_dir) / f"{game_id}.json" if game_id in GAMES else None


def _read_json(path: Path) -> object:
    with open(path, "rb") as fh:
        data = fh.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError("demasiado grande")
    return json.loads(data)


def read_index(cache_dir: Path) -> dict[str, tuple[int, float]]:
    """{juego: (servidores, guardada en)}; vacío si falta o está dañado."""
    try:
        raw = _read_json(Path(cache_dir) / INDEX)
    except (OSError, ValueError, RecursionError):
        return {}
    if not isinstance(raw, dict):
        return {}
    index: dict[str, tuple[int, float]] = {}
    for game_id, entry in raw.items():
        if (game_id in GAMES and isinstance(entry, list) and len(entry) == 2
                and isinstance(entry[0], int) and not isinstance(entry[0], bool)
                and entry[0] >= 0 and _valid_time(entry[1])):
            index[game_id] = (entry[0], float(entry[1]))
    return index


def _write_index(cache_dir: Path, index: dict[str, tuple[int, float]]) -> None:
    data = {g: [n, t] for g, (n, t) in index.items()}
    atomic_write(Path(cache_dir) / INDEX, json.dumps(data).encode("utf-8"))


def save(cache_dir: Path, game_id: str, servers: list[ServerInfo], now: float) -> None:
    """Guarda la lista y su entrada del índice. Bloqueante: llamarla desde un hilo."""
    path = _path(cache_dir, game_id)
    if path is None:
        return
    data = {"schema": SCHEMA, "saved_at": now, "servers": [_to_json(s) for s in servers]}
    atomic_write(path, json.dumps(data, ensure_ascii=False, separators=(",", ":"))
                 .encode("utf-8"))
    index = read_index(cache_dir)
    index[game_id] = (len(servers), now)
    _write_index(cache_dir, index)


def load(cache_dir: Path, game_id: str) -> tuple[float, list[ServerInfo]] | None:
    """(guardada en, servidores) o None si no hay una lista válida. Bloqueante."""
    path = _path(cache_dir, game_id)
    if path is None:
        return None
    try:
        raw = _read_json(path)
    except (OSError, ValueError, RecursionError):
        return None
    if (not isinstance(raw, dict) or raw.get("schema") != SCHEMA
            or not _valid_time(raw.get("saved_at"))
            or not isinstance(raw.get("servers"), list)):
        return None
    servers = []
    for entry in raw["servers"]:
        with contextlib.suppress(ValueError, TypeError, KeyError):
            servers.append(_from_json(entry))
    return float(raw["saved_at"]), servers


def forget(cache_dir: Path, game_id: str) -> None:
    """Borra la lista guardada de un juego (p. ej. cambió el filtro de Steam)."""
    path = _path(cache_dir, game_id)
    if path is None:
        return
    with contextlib.suppress(OSError):
        path.unlink(missing_ok=True)
    index = read_index(cache_dir)
    if index.pop(game_id, None) is not None:
        with contextlib.suppress(OSError):
            _write_index(cache_dir, index)
