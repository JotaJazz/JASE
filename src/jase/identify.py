"""¿Qué juego es este servidor? Lógica pura (sin red ni Qt) para «Detectar juego».

Se consulta la dirección con cada familia de protocolo (`probe_plan`) y cada respuesta
se cruza con los `GameDef` (`identify`): en Valve y GameSpy el servidor dice su juego
(appid/carpeta, `gamename`); en Quake 3 varios juegos comparten protocolo y se
desempata con pistas del `gamename` y la versión (sacadas de las capturas de
`tests/fixtures`).
"""
from __future__ import annotations

from jase.games import GAMES, GameDef
from jase.masters import valid_port
from jase.models import ServerAddress, ServerInfo
from jase.scanner import FAMILIES

# Zandronum ignora unos 10 s todo lo que llegue de una IP tras cualquier paquete: su
# consulta sale primero y las demás familias, un poco después (`probe_delay`).
FIRST_FAMILIES = ("zandronum",)
STAGGER_S = 0.25


def probe_delay(family: str) -> float:
    return 0.0 if family in FIRST_FAMILIES else STAGGER_S

# Quake 3: textos (en minúsculas) que, en `gamename` o en la versión, delatan el juego.
QUAKE3_HINTS: dict[str, tuple[str, ...]] = {
    "cod2": ("call of duty 2",),
    "cod4": ("call of duty 4", "cod4"),
    "coduo": ("united offensive",),
    "cod1": ("call of duty",),  # el último: «call of duty 2/4» ya se reconocen antes
    "urt": ("q3ut", "q3urt", " urt "),
    "smokinguns": ("smokinguns",),
    "oa": ("baseoa", "+oa"),
    "jka": ("jamp",),
    "jk2": ("jk2mp",),
    "sof2": ("sof2",),
    "tremulous": ("tremulous",),
    "rtcw": ("wolf ", "iortcw"),  # iortcw no siempre envía `protocol`
    "et": ("et legacy", "etlegacy", "et 2."),
    "q3": ("baseq3",),
}
# Carpeta del juego → juego, para servidores que responden por otra familia (Warsow 2.1
# contesta también A2S de Valve con carpeta «basewsw»).
FOLDER_HINTS = {"basewsw": "warsow", "basewf": "warfork"}


def probe_plan(address: ServerAddress) -> list[tuple[str, list[ServerAddress]]]:
    """(familia, direcciones) a consultar. En GameSpy y UT2004 el puerto de consulta
    suele ser otro que el de juego: se prueban también los desplazamientos conocidos."""
    host, port = address.host, address.port
    offsets = {"gamespy": sorted({-g.connect_port_fallback for g in GAMES.values()
                                  if g.family == "gamespy" and g.connect_port_fallback}),
               "ut2004": [1]}
    plan = []
    for family in FAMILIES:
        ports = [port] + [port + d for d in offsets.get(family, ()) if valid_port(port + d)]
        plan.append((family, [ServerAddress(host, p) for p in ports]))
    return plan


def _family_games(family: str) -> list[GameDef]:
    return [g for g in GAMES.values() if g.family == family]


def _quake3(info: ServerInfo) -> list[str]:
    rules = {k.casefold(): v for k, v in info.rules.items()}
    gamename = rules.get("gamename", "").casefold()
    text = f" {gamename} {rules.get('version', '').casefold()} "
    try:
        protocol = int(rules.get("protocol", ""))
    except ValueError:
        protocol = None
    games = _family_games("quake3")
    by_master = [g.id for g in games if g.master_gamename
                 and g.master_gamename.casefold() == gamename]
    if by_master:  # Xonotic, Warsow, Warfork: el gamename es el del master
        return by_master
    hinted = [gid for gid, hints in QUAKE3_HINTS.items()
              if any(h in text for h in hints)]
    if "cod1" in hinted and gamename != "call of duty":
        hinted.remove("cod1")  # «call of duty» sale también en CoD2/CoD4
    by_protocol = [g.id for g in games if protocol in g.protocols
                   and gamename not in {m.casefold() for m in g.status_exclude_mods}]
    both = [gid for gid in hinted if gid in by_protocol]
    return both or hinted[:1] or by_protocol


def _gamespy(info: ServerInfo) -> list[str]:
    gamename = info.rules.get("gamename", "").casefold()
    return [g.id for g in _family_games("gamespy")
            if gamename in {g.status_gamename.casefold(), g.gamespy_gamename.casefold()} - {""}
            and g.lists_server(info.rules)]


def _valve(info: ServerInfo) -> list[str]:
    games = _family_games("valve")
    appid = info.rules.get("appid", "")
    by_appid = [g.id for g in games if appid and str(g.valve_appid) == appid]
    if by_appid:
        return by_appid
    gamedir = info.rules.get("gamedir", "").casefold()
    by_gamedir = [g.id for g in games if gamedir and g.valve_gamedir.casefold() == gamedir]
    return by_gamedir or ([FOLDER_HINTS[gamedir]] if gamedir in FOLDER_HINTS else [])


def _quake2(info: ServerInfo) -> list[str]:
    folder = (info.rules.get("gamedir") or info.rules.get("game", "")).casefold()
    special = [g.id for g in _family_games("quake2")
               if g.status_gamedir and g.status_gamedir.casefold() == folder]
    return special or [g.id for g in _family_games("quake2") if not g.status_gamedir]


def identify(family: str, info: ServerInfo) -> list[str]:
    """Ids de juego que encajan con la respuesta, del más probable al menos."""
    if family == "quake3":
        return _quake3(info)
    if family == "gamespy":
        return _gamespy(info)
    if family == "valve":
        return _valve(info)
    if family == "quake2":
        return _quake2(info)
    if family in ("ut2004", "zandronum"):  # un solo juego por familia
        return [g.id for g in _family_games(family)]
    return []


def detected_games(responses: list[tuple[str, ServerInfo]]) -> list[tuple[str, ServerAddress]]:
    """(juego, dirección de consulta) de todas las respuestas, sin repetir juego."""
    result: dict[str, ServerAddress] = {}
    for family, info in responses:
        for game_id in identify(family, info):
            result.setdefault(game_id, info.address)
    return list(result.items())
