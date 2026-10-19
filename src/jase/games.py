from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

ALL_VERSIONS = "Todas"


@dataclass(frozen=True)
class GameDef:
    id: str
    name: str
    family: Literal["quake3", "gamespy"]
    masters: tuple[tuple[str, int], ...]
    protocols: tuple[int, ...]
    known_versions: tuple[str, ...]
    default_version: str
    exe_relpath: str


GAMES: dict[str, GameDef] = {
    "cod2": GameDef(
        id="cod2",
        name="Call of Duty 2",
        family="quake3",
        masters=(("cod2master.activision.com", 20710),),
        protocols=(115, 117, 118),
        known_versions=("1.0", "1.2", "1.3"),
        default_version="1.3",
        exe_relpath="CoD2MP_s.exe",
    ),
    "cod1": GameDef(
        id="cod1",
        name="Call of Duty",
        family="quake3",
        masters=(("codmaster.activision.com", 20510),),
        protocols=(1, 2, 4, 5, 6),  # verificar en Task 8
        known_versions=("1.1", "1.2", "1.3", "1.4", "1.5"),
        default_version="1.5",
        exe_relpath="CoDMP.exe",
    ),
    "ut99": GameDef(
        id="ut99",
        name="Unreal Tournament",
        family="gamespy",
        masters=(("master.333networks.com", 28900), ("master.openspy.net", 28900)),
        protocols=(),
        known_versions=("436", "451", "469"),
        default_version=ALL_VERSIONS,
        exe_relpath="System\\UnrealTournament.exe",
    ),
}
