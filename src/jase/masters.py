"""Masters: lista oficial (masters.json), personales (masters.custom.json) y lista efectiva.

Núcleo sin Qt. La lista oficial se valida de forma estricta; los archivos personales se
cargan de forma tolerante (entrada por entrada).
"""
from __future__ import annotations

import json
import re
import urllib.request
from dataclasses import dataclass, field
from importlib import resources
from pathlib import Path
from typing import TypeGuard

from jase import fetch_meta
from jase.config import Config
from jase.games import GAMES

Master = tuple[str, int]

SCHEMA = 1
MAX_BYTES = 256 * 1024
MAX_OFFICIAL_BYTES = MAX_BYTES
OFFICIAL_URL = "https://raw.githubusercontent.com/JotaJazz/JASE/master/masters.json"
CHECK_INTERVAL_S = 86400
FETCH_TIMEOUT_S = 10
META_FILENAME = "masters.meta.json"
OFFICIAL_FILENAME = "masters.json"
CUSTOM_FILENAME = "masters.custom.json"

_HOST_RE = re.compile(r"[A-Za-z0-9-]{1,63}(\.[A-Za-z0-9-]{1,63})*\.?")
_DATE_RE = re.compile(r"\d{4}-\d{2}-\d{2}")
RETRY_AFTER_FAILURE_S = 3600


def master_key(m: Master) -> str:
    return f"{m[0].casefold()}:{m[1]}"


def valid_host(host: object) -> TypeGuard[str]:
    return (isinstance(host, str) and len(host) <= 253
            and _HOST_RE.fullmatch(host) is not None)


def valid_port(port: object) -> TypeGuard[int]:
    return isinstance(port, int) and not isinstance(port, bool) and 1 <= port <= 65535


@dataclass(frozen=True)
class OfficialList:
    schema: int
    updated: str
    games: dict[str, tuple[Master, ...]]


def _parse_entries(entries: object, game_id: str) -> tuple[Master, ...]:
    if not isinstance(entries, list):
        raise ValueError(f"{game_id}: se esperaba una lista")
    result: dict[str, Master] = {}
    for e in entries:
        if not isinstance(e, dict):
            raise ValueError(f"{game_id}: entrada inválida")
        host, port = e.get("host"), e.get("port")
        if not valid_host(host):
            raise ValueError(f"{game_id}: host inválido")
        if not valid_port(port):
            raise ValueError(f"{game_id}: puerto inválido")
        result.setdefault(master_key((host, port)), (host, port))
    return tuple(result.values())


def parse_official(data: bytes) -> OfficialList:
    if len(data) > MAX_OFFICIAL_BYTES:
        raise ValueError("masters.json demasiado grande")
    try:
        raw = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise ValueError("masters.json no es JSON válido") from exc
    if not isinstance(raw, dict):
        raise ValueError("masters.json: se esperaba un objeto")
    schema = raw.get("schema")
    if schema != SCHEMA or isinstance(schema, bool):
        raise ValueError("masters.json: schema desconocido")
    updated = raw.get("updated")
    if not isinstance(updated, str) or _DATE_RE.fullmatch(updated) is None:
        raise ValueError("masters.json: 'updated' inválido")
    games_raw = raw.get("games")
    if not isinstance(games_raw, dict):
        raise ValueError("masters.json: falta 'games'")
    games: dict[str, tuple[Master, ...]] = {}
    for game_id, entries in games_raw.items():
        if game_id not in GAMES:
            continue  # ids desconocidos: se ignoran sin validar
        games[game_id] = _parse_entries(entries, game_id)
    # Salvaguarda: si ningún juego conocido queda con masters, se rechaza.
    if not any(games.get(gid, GAMES[gid].masters) for gid in GAMES):
        raise ValueError("masters.json: todos los juegos quedan sin masters")
    return OfficialList(schema, updated, games)


def _read_capped(path: Path) -> bytes:
    """Lee a lo sumo MAX_OFFICIAL_BYTES + 1 bytes (el excedente lo rechaza el parser)."""
    with open(path, "rb") as fh:
        return fh.read(MAX_OFFICIAL_BYTES + 1)


def _builtin_official() -> OfficialList:
    return OfficialList(SCHEMA, "", {gid: tuple(g.masters) for gid, g in GAMES.items()})


def bundled_official() -> OfficialList:
    data = resources.files("jase").joinpath("assets", OFFICIAL_FILENAME).read_bytes()
    return parse_official(data)


def load_official(cache_dir: Path) -> OfficialList:
    """La más nueva entre la caché válida y la copia empaquetada → masters de games.py.

    Una caché bajada antes de actualizar la app es más vieja que la copia empaquetada;
    el chequeo remoto es diario, así que hasta entonces se usa la empaquetada."""
    try:
        cached = parse_official(_read_capped(Path(cache_dir) / OFFICIAL_FILENAME))
    except (OSError, ValueError):
        cached = None
    try:
        bundled = bundled_official()
    except (OSError, ValueError):
        return cached or _builtin_official()
    # Fechas ISO (validadas por parse_official): se comparan como texto.
    return cached if cached is not None and cached.updated >= bundled.updated else bundled


def official_for(game_id: str, official: OfficialList) -> tuple[Master, ...]:
    if game_id in official.games:
        return official.games[game_id]
    game = GAMES.get(game_id)
    return tuple(game.masters) if game else ()


@dataclass
class CustomMasters:
    added: dict[str, list[Master]] = field(default_factory=dict)
    disabled: dict[str, set[str]] = field(default_factory=dict)


def load_custom(path: Path) -> CustomMasters:
    custom = CustomMasters()
    try:
        raw = _read_capped(Path(path))
        if len(raw) > MAX_OFFICIAL_BYTES:
            return custom
        data = json.loads(raw.decode("utf-8"))
    except (OSError, ValueError, RecursionError):
        return custom
    if not isinstance(data, dict):
        return custom
    added = data.get("added")
    if isinstance(added, dict):
        for game_id, entries in added.items():
            if not isinstance(game_id, str) or not isinstance(entries, list):
                continue
            seen: dict[str, Master] = {}
            for e in entries:
                if not isinstance(e, dict):
                    continue
                host, port = e.get("host"), e.get("port")
                if valid_host(host) and valid_port(port):
                    seen.setdefault(master_key((host, port)), (host, port))
            if seen:
                custom.added[game_id] = list(seen.values())
    disabled = data.get("disabled")
    if isinstance(disabled, dict):
        for game_id, keys in disabled.items():
            if not isinstance(game_id, str) or not isinstance(keys, list):
                continue
            clean = {k for k in keys if isinstance(k, str) and k}
            if clean:
                custom.disabled[game_id] = clean
    return custom


def save_custom(custom: CustomMasters, path: Path) -> None:
    path = Path(path)
    data = {
        "schema": SCHEMA,
        "added": {gid: [{"host": h, "port": p} for h, p in ms]
                  for gid, ms in custom.added.items() if ms},
        "disabled": {gid: sorted(keys) for gid, keys in custom.disabled.items() if keys},
    }
    fetch_meta.atomic_write(path, json.dumps(data, indent=2, ensure_ascii=False).encode("utf-8"))


def migrate_config_masters(cfg: Config, custom: CustomMasters, official: OfficialList) -> bool:
    """Convierte `Config.masters` (reemplazo por juego) en added/disabled, una sola vez."""
    if not cfg.masters:
        return False
    for game_id, user_list in cfg.masters.items():
        if not user_list:
            continue
        official_list = official_for(game_id, official)
        official_keys = {master_key(m) for m in official_list}
        user_keys = {master_key(m) for m in user_list}
        added = custom.added.setdefault(game_id, [])
        have = {master_key(m) for m in added}
        for m in user_list:
            k = master_key(m)
            if not (valid_host(m[0]) and valid_port(m[1])):
                continue
            if k not in official_keys and k not in have:
                added.append((m[0], m[1]))
                have.add(k)
        gone = official_keys - user_keys
        if gone:
            custom.disabled.setdefault(game_id, set()).update(gone)
        if not added:
            del custom.added[game_id]
    cfg.masters = {}
    return True


def effective_masters(game_id: str, official: OfficialList,
                      custom: CustomMasters) -> list[Master]:
    disabled = custom.disabled.get(game_id, set())
    result: dict[str, Master] = {}
    for m in official_for(game_id, official):
        if master_key(m) not in disabled:
            result.setdefault(master_key(m), m)
    for m in custom.added.get(game_id, []):
        result.setdefault(master_key(m), m)
    return list(result.values())


# ---------------------------------------------------------------- actualización remota
def _default_fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "JASE"})
    with urllib.request.urlopen(req, timeout=FETCH_TIMEOUT_S) as resp:
        return resp.read(MAX_BYTES + 1)


default_fetch = _default_fetch


def fetch_official(fetch=_default_fetch) -> bytes:
    """Descarga masters.json; lanza ValueError si supera MAX_BYTES (u otro error de red)."""
    data = fetch(OFFICIAL_URL)
    if len(data) > MAX_BYTES:
        raise ValueError("masters.json remoto demasiado grande")
    return data


def should_check(meta_path: Path, now: float) -> bool:
    return fetch_meta.should_check(meta_path, now, CHECK_INTERVAL_S, RETRY_AFTER_FAILURE_S)


def _write_meta(cache_dir: Path, now: float, ok: bool) -> None:
    fetch_meta.write_meta(cache_dir / META_FILENAME, now, ok)


def check_official(cache_dir: Path, now: float,
                   fetch=_default_fetch) -> tuple[OfficialList | None, bool]:
    """Como `update_official`, pero indica también si la descarga fue correcta.

    Devuelve (nueva lista | None si no cambió o falló, ok)."""
    cache_dir = Path(cache_dir)
    try:
        data = fetch_official(fetch)
        new = parse_official(data)
    except Exception:  # red, tamaño o contenido inválido: no se reintenta en bucle
        _write_meta(cache_dir, now, False)
        return None, False
    current = load_official(cache_dir)
    try:
        fetch_meta.atomic_write(cache_dir / OFFICIAL_FILENAME, data)
    except OSError:
        _write_meta(cache_dir, now, False)
        return None, False
    _write_meta(cache_dir, now, True)
    changed = new.updated != current.updated or new.games != current.games
    return (new if changed else None), True


def update_official(cache_dir: Path, now: float, fetch=_default_fetch) -> OfficialList | None:
    """Descarga y valida la lista oficial. Devuelve la nueva solo si su contenido cambió."""
    return check_official(cache_dir, now, fetch)[0]
