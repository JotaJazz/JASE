from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

ALL_VERSIONS = "Todas"


@dataclass(frozen=True)
class GameDef:
    id: str
    name: str
    family: Literal["quake3", "gamespy", "valve"]
    masters: tuple[tuple[str, int], ...]
    protocols: tuple[int, ...]
    known_versions: tuple[str, ...]
    default_version: str
    exe_relpath: str
    # Ejecutables alternativos, probados en orden si no está `exe_relpath`
    # (p. ej. los lanzadores de OpenMoHAA).
    exe_candidates: tuple[str, ...] = ()
    # Ejecutables que, si están en la misma carpeta donde se resolvió el exe,
    # se usan en su lugar (una carpeta de OpenMoHAA conserva MOHAA.exe).
    preferred_exes: tuple[str, ...] = ()
    # Carpeta de GameTracker para imágenes de mapas; "" = no hay imágenes.
    gametracker_id: str = ""
    # Credenciales del master GameSpy (gamename y clave de validación).
    gamespy_gamename: str = ""
    gamespy_key: str = ""
    # GameSpy: si no está vacío, se descartan los servidores cuyo \status\ trae
    # otro `gamename` (los masters de MOHAA mezclan AA, Spearhead y Breakthrough).
    status_gamename: str = ""
    # Cómo se pasa la dirección al exe: "+connect" (q3), "host:port?password=" (unreal)
    # "-game <valve_gamedir> +connect" (valve, hl.exe) o "+restart 1 +joinServer" (bf1942).
    launch_style: Literal["q3", "unreal", "valve", "bf1942"] = "q3"
    # GameSpy: desplazamiento sobre el puerto de consulta cuando falta `hostport`.
    connect_port_fallback: int = 0
    # Quake3: cvar del statusResponse que lleva la versión.
    version_key: str = "shortversion"
    # Quake3: (prefijo, clave, etiqueta) para agrupar versiones crudas en una sola entrada
    # del selector (CoD2x anuncia "1.4.6.8", "1.4.4.6"… → clave "1.4", «1.4 (CoD2x)»).
    # El valor crudo sigue en `ServerInfo.rules`.
    version_aliases: tuple[tuple[str, str, str], ...] = ()
    # Valve: appid del filtro del master (\\appid\\N) y carpeta del mod para -game.
    valve_appid: int = 0
    valve_gamedir: str = ""
    # Valve: filtro crudo del master; vacío = \\appid\\{valve_appid}. Con `valve_gamedir`
    # se descartan además los servidores que informan otra carpeta de juego.
    valve_master_filter: str = ""
    # Valve: la lista sale de la API web de Steam (necesita clave) en vez del master UDP.
    steam_webapi: bool = False
    # Ruta de instalación propuesta en una config nueva ("" = ninguna).
    default_path: str = ""
    # Nombre para la barra lateral ("" = `name`); el completo va en el tooltip.
    short_name: str = ""
    # Regla del servidor que trae el mod ("" = juego sin mods) y los valores que
    # cuentan como «(base)» (comparados con casefold).
    mod_key: str = ""
    base_mods: tuple[str, ...] = ()
    # Iniciales (2 caracteres) del icono dibujado cuando no hay icono del exe.
    badge: str = ""

    def version_label(self, key: str) -> str:
        """Texto del selector de versión para la clave `key`."""
        for _prefix, alias_key, label in self.version_aliases:
            if alias_key == key:
                return label
        return key

    def __post_init__(self) -> None:
        if not self.short_name:
            object.__setattr__(self, "short_name", self.name)


GAMES: dict[str, GameDef] = {
    "cod2": GameDef(
        id="cod2",
        badge="C2",
        mod_key="fs_game",
        name="Call of Duty 2",
        family="quake3",
        masters=(("cod2master.activision.com", 20710),),
        # 1.0, 1.2, 1.3 (verificado 2026-10-18); 119/120 = CoD2x 1.4.x (2026-10-19)
        protocols=(115, 117, 118, 119, 120),
        known_versions=("1.0", "1.2", "1.3", "1.4"),
        version_aliases=(("1.4", "1.4", "1.4 (CoD2x)"),),
        default_version="1.3",
        exe_relpath="CoD2MP_s.exe",
        gametracker_id="cod2",
        default_path=r"D:\Juegos\CoD2",
    ),
    "cod1": GameDef(
        id="cod1",
        badge="C1",
        mod_key="fs_game",
        name="Call of Duty",
        family="quake3",
        masters=(("codmaster.activision.com", 20510),),
        protocols=(1, 2, 4, 5, 6),  # 1.1, 1.2, 1.3, 1.4, 1.5 (verificado 2026-10-18)
        known_versions=("1.1", "1.2", "1.3", "1.4", "1.5"),
        default_version="1.5",
        exe_relpath="CoDMP.exe",
        gametracker_id="cod",
    ),
    # --- Familia Quake 3 (CoD:UO y otros): masters sondeados el 2026-10-19. ---
    "coduo": GameDef(
        id="coduo",
        badge="UO",
        mod_key="fs_game",
        name="Call of Duty: United Offensive",
        short_name="CoD: United Offensive",
        family="quake3",
        masters=(("coduomaster.activision.com", 20610),),
        # 21 = 1.41, 22 = 1.51; 24 también anuncia shortversion 1.51 (servidores
        # "Activision TDM 24/7").
        protocols=(21, 22, 24),
        known_versions=("1.41", "1.51"),
        default_version="1.51",
        exe_relpath="CoDUOMP.exe",
        gametracker_id="uo",
    ),
    "cod4": GameDef(
        id="cod4",
        badge="C4",
        mod_key="fs_game",
        name="Call of Duty 4: Modern Warfare",
        short_name="CoD4",
        family="quake3",
        masters=(("cod4master.activision.com", 20810),),
        # 6 = 1.7 (la gran mayoría, casi todo CoD4X con shortversion "-"), 7 = 1.8,
        # 5 = 1.6, 1 y 8 = servidores que anuncian 1.0/1.xu.
        protocols=(1, 5, 6, 7, 8),
        known_versions=("1.0", "1.6", "1.7", "1.8"),
        default_version=ALL_VERSIONS,
        exe_relpath="iw3mp.exe",
        gametracker_id="cod4",
    ),
    "q3": GameDef(
        id="q3",
        badge="Q3",
        mod_key="gamename",
        base_mods=("baseq3",),
        name="Quake III Arena",
        family="quake3",
        masters=(("master.ioquake3.org", 27950), ("master.quake3arena.com", 27950),
                 ("master.maverickservers.com", 27950)),
        protocols=(68,),  # 1.32 (quake3.exe); 67 = 1.31 y 71 = ioq3/OpenArena, fuera
        known_versions=("Q3 1.32e", "ioq3 1.36", "CNQ3 1.53"),
        default_version=ALL_VERSIONS,
        exe_relpath="quake3.exe",
        gametracker_id="q3",
        version_key="version",  # normalizado con quake3.short_engine_version
    ),
    "rtcw": GameDef(
        id="rtcw",
        badge="RW",
        mod_key="gamename",
        base_mods=("main",),
        name="Return to Castle Wolfenstein",
        short_name="RtCW",
        family="quake3",
        masters=(("wolfmaster.idsoftware.com", 27950),),
        protocols=(57, 60),  # 60 = 1.41 (WolfMP.exe); 57 = 1.0/1.33 y mods
        known_versions=("Wolf 1.0", "Wolf 1.41", "Wolf 1.41b", "iortcw 1.51c", "iortcw 1.51d"),
        default_version=ALL_VERSIONS,
        exe_relpath="WolfMP.exe",
        version_key="version",
    ),
    "et": GameDef(
        id="et",
        badge="ET",
        mod_key="gamename",
        base_mods=("etmain",),
        name="Wolfenstein: Enemy Territory",
        short_name="Wolfenstein: ET",
        family="quake3",
        masters=(("etmaster.idsoftware.com", 27950), ("master.etlegacy.com", 27950)),
        protocols=(84,),  # 2.60b y ET: Legacy
        known_versions=("ET Legacy v2.83.2", "ET Legacy v2.86.2"),
        default_version=ALL_VERSIONS,
        exe_relpath="ET.exe",
        gametracker_id="et",
        version_key="version",
    ),
    # --- Raven (Quake 3): masters sondeados el 2026-10-19. ---
    # JKA no envía fs_game en \getstatus: el mod solo viene en `gamename`.
    "jka": GameDef(
        id="jka",
        badge="JK",
        mod_key="gamename",
        base_mods=("basejka",),
        name="Star Wars Jedi Knight: Jedi Academy",
        short_name="Jedi Academy",
        family="quake3",
        masters=(("master.jkhub.org", 29060),),
        protocols=(26,),  # 26 = 1.01; 25 (1.00) casi no tiene servidores
        known_versions=(),
        default_version=ALL_VERSIONS,
        exe_relpath="GameData\\jamp.exe",
        exe_candidates=("jamp.exe",),
        version_key="version",
    ),
    # El cvar `version` varía (SOF2MP V1.02a, SoF2Plus 1.00_g…): la versión sale del
    # protocolo (2002 = 1.00, 2004 = 1.03), ver quake3.PROTOCOL_VERSIONS.
    "sof2": GameDef(
        id="sof2",
        badge="S2",
        mod_key="fs_game",
        name="Soldier of Fortune II",
        short_name="SoF2",
        family="quake3",
        masters=(("master.1fxmod.org", 20110),),
        protocols=(2002, 2004),
        known_versions=("1.00", "1.03"),
        default_version=ALL_VERSIONS,
        exe_relpath="SoF2MP.exe",
        gametracker_id="sof2",
        version_key="protocol",
    ),
    "ut99": GameDef(
        id="ut99",
        badge="UT",
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
    # --- Medal of Honor (GameSpy): masters sondeados el 2026-10-19. ---
    # El master lista el puerto de consulta (12300); el \status\ trae `hostport`
    # (normalmente consulta − 97, 12203). MOHAA no envía `password` en \status\.
    "mohaa": GameDef(
        id="mohaa",
        badge="MA",
        name="Medal of Honor: Allied Assault",
        short_name="MOHAA: Allied Assault",
        family="gamespy",
        masters=(("master.333networks.com", 28900), ("master.openspy.net", 28900)),
        protocols=(),
        known_versions=("1.11",),  # OpenMoHAA anuncia "1.12+0.8x.y" (se añade al verlo)
        default_version=ALL_VERSIONS,
        exe_relpath="MOHAA.exe",
        exe_candidates=("launch_openmohaa_base.exe", "openmohaa.exe"),
        preferred_exes=("launch_openmohaa_base.exe",),
        gametracker_id="mohaa",
        gamespy_gamename="mohaa",
        gamespy_key="M5Fdwc",
        status_gamename="mohaa",
        launch_style="q3",
        connect_port_fallback=-97,
        default_path=r"D:\Juegos\MOHAA_STOCK",
    ),
    "mohaas": GameDef(
        id="mohaas",
        badge="SH",
        name="Medal of Honor: Spearhead",
        short_name="MOHAA: Spearhead",
        family="gamespy",
        masters=(("master.333networks.com", 28900), ("master.openspy.net", 28900)),
        protocols=(),
        known_versions=("2.15",),
        default_version=ALL_VERSIONS,
        exe_relpath="moh_spearhead.exe",
        # Solo el lanzador: openmohaa.exe sin "+set com_target_game 1" arrancaría AA.
        exe_candidates=("launch_openmohaa_spearhead.exe",),
        preferred_exes=("launch_openmohaa_spearhead.exe",),
        gametracker_id="sh",
        gamespy_gamename="mohaas",
        gamespy_key="h2P1c9",
        status_gamename="mohaas",
        launch_style="q3",
        connect_port_fallback=-97,
    ),
    "mohaab": GameDef(
        id="mohaab",
        badge="BT",
        name="Medal of Honor: Breakthrough",
        short_name="MOHAA: Breakthrough",
        family="gamespy",
        masters=(("master.333networks.com", 28900), ("master.openspy.net", 28900)),
        protocols=(),
        known_versions=("2.40",),
        default_version=ALL_VERSIONS,
        exe_relpath="moh_breakthrough.exe",
        exe_candidates=("launch_openmohaa_breakthrough.exe",),
        preferred_exes=("launch_openmohaa_breakthrough.exe",),
        gametracker_id="bt",
        gamespy_gamename="mohaab",
        gamespy_key="y32FDc",
        status_gamename="mohaab",
        launch_style="q3",
        connect_port_fallback=-97,
    ),
    # --- Counter-Strike 1.6 (Valve): master sondeado el 2026-10-19. ---
    # El master mezcla juegos de GoldSrc; el filtro \\gamedir\\cstrike lo acota y
    # el A2S_INFO confirma la carpeta. Las versiones varían (HLDS 0x49 trae
    # "1.1.2.7/Stdio", 0x6D no trae ninguna), así que no hay lista fija.
    "cs16": GameDef(
        id="cs16",
        badge="CS",
        name="Counter-Strike 1.6",
        short_name="CS 1.6",
        family="valve",
        masters=(("ms.cs16.net", 27010),),
        protocols=(),
        known_versions=(),
        default_version=ALL_VERSIONS,
        exe_relpath="hl.exe",
        gametracker_id="cs",
        launch_style="valve",
        valve_appid=10,
        valve_gamedir="cstrike",
        valve_master_filter="\\gamedir\\cstrike",
    ),
    # --- Battlefield 1942 (GameSpy): master sondeado el 2026-10-19. ---
    # El master lista el puerto de consulta (23000+); el juego va en `hostport` (14567).
    "bf1942": GameDef(
        id="bf1942",
        badge="BF",
        name="Battlefield 1942",
        short_name="BF1942",
        family="gamespy",
        masters=(("master.openspy.net", 28900),),
        protocols=(),
        known_versions=("v1.61", "v1.612"),
        default_version=ALL_VERSIONS,
        exe_relpath="BF1942.exe",
        gametracker_id="bf1942",
        gamespy_gamename="bfield1942",
        gamespy_key="HpWx9z",
        status_gamename="bfield1942",
        launch_style="bf1942",
        # Los servidores anuncian el mod (carpeta) en `gameId`; "bf1942" es el juego base.
        mod_key="gameId",
        base_mods=("bf1942",),
        # hostport - puerto de consulta fue -8433 en 2 de 3 capturas.
        connect_port_fallback=-8433,
    ),
}
