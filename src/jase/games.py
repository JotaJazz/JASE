from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

ALL_VERSIONS = "Todas"


@dataclass(frozen=True)
class GameDef:
    id: str
    name: str
    # "valve" se acepta en el tipo; el escáner aún no lo implementa.
    family: Literal["quake3", "gamespy", "valve"]
    masters: tuple[tuple[str, int], ...]
    protocols: tuple[int, ...]
    known_versions: tuple[str, ...]
    default_version: str
    exe_relpath: str
    # Carpeta de GameTracker para imágenes de mapas; "" = no hay imágenes.
    gametracker_id: str = ""
    # Credenciales del master GameSpy (gamename y clave de validación).
    gamespy_gamename: str = ""
    gamespy_key: str = ""
    # Cómo se pasa la dirección al exe: "+connect" (q3) o "host:port?password=" (unreal).
    launch_style: Literal["q3", "unreal"] = "q3"
    # GameSpy: desplazamiento sobre el puerto de consulta cuando falta `hostport`.
    connect_port_fallback: int = 0
    # Quake3: cvar del statusResponse que lleva la versión.
    version_key: str = "shortversion"
    # Ruta de instalación propuesta en una config nueva ("" = ninguna).
    default_path: str = ""


GAMES: dict[str, GameDef] = {
    "cod2": GameDef(
        id="cod2",
        name="Call of Duty 2",
        family="quake3",
        masters=(("cod2master.activision.com", 20710),),
        protocols=(115, 117, 118),  # 1.0, 1.2, 1.3 (verificado 2026-10-18)
        known_versions=("1.0", "1.2", "1.3"),
        default_version="1.3",
        exe_relpath="CoD2MP_s.exe",
        gametracker_id="cod2",
        default_path=r"D:\Juegos\CoD2",
    ),
    "cod1": GameDef(
        id="cod1",
        name="Call of Duty",
        family="quake3",
        masters=(("codmaster.activision.com", 20510),),
        protocols=(1, 2, 4, 5, 6),  # 1.1, 1.2, 1.3, 1.4, 1.5 (verificado 2026-10-18)
        known_versions=("1.1", "1.2", "1.3", "1.4", "1.5"),
        default_version="1.5",
        exe_relpath="CoDMP.exe",
        gametracker_id="cod",
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
        gamespy_gamename="ut",
        gamespy_key="Z5Nfb0",
        launch_style="unreal",
        connect_port_fallback=-1,
        default_path=r"D:\Juegos\UnrealTournament",
    ),
}
