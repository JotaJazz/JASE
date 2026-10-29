"""Mods, mapas y países de los servidores de un juego, para las hojas del árbol (sin Qt)."""
from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from collections.abc import Callable, Iterable

from .games import GameDef
from .models import ServerInfo
from .protocols.quake3 import strip_colors

BASE_MOD = "(base)"
TOP_MAPS = 10
_MODS_PREFIX = "mods/"


def mod_of(game: GameDef, info: ServerInfo) -> str:
    if not game.mod_key:
        return BASE_MOD
    value = strip_colors(info.rules.get(game.mod_key, "")).strip()
    if value.casefold().startswith(_MODS_PREFIX):
        value = value[len(_MODS_PREFIX):]
    value = value.casefold().strip()
    if not value or value in {b.casefold() for b in game.base_mods}:
        return BASE_MOD
    return value


def map_of(info: ServerInfo) -> str:
    return info.map.strip().casefold()


@dataclass(frozen=True)
class Breakdown:
    mods: tuple[tuple[str, int], ...]
    maps: tuple[tuple[str, int], ...]
    other_maps: int
    countries: tuple[tuple[str, int], ...] = ()  # código ISO en minúsculas


def _ranked(counter: Counter[str]) -> list[tuple[str, int]]:
    return sorted(counter.items(), key=lambda kv: (-kv[1], kv[0]))


def breakdown(game: GameDef, servers: Iterable[ServerInfo],
              country_of: Callable[[ServerInfo], str | None] | None = None) -> Breakdown:
    """`country_of`: país de un servidor (None = desconocido, no se cuenta)."""
    mods: Counter[str] = Counter()
    maps: Counter[str] = Counter()
    countries: Counter[str] = Counter()
    for info in servers:
        mods[mod_of(game, info)] += 1
        maps[map_of(info)] += 1
        if country_of is not None and (code := country_of(info)):
            countries[code] += 1
    ranked_maps = _ranked(maps)
    shown_mods = () if set(mods) <= {BASE_MOD} else tuple(_ranked(mods))
    return Breakdown(
        mods=shown_mods,
        maps=tuple(ranked_maps[:TOP_MAPS]),
        other_maps=sum(n for _, n in ranked_maps[TOP_MAPS:]),
        countries=tuple(_ranked(countries)),
    )


def matches(game: GameDef, info: ServerInfo, mods: frozenset[str],
            maps: frozenset[str]) -> bool:
    return ((not mods or mod_of(game, info) in mods)
            and (not maps or map_of(info) in maps))
