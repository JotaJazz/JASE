"""Búsqueda de juegos instalados (registro, Steam y carpetas). Sin Qt."""
from __future__ import annotations

import os
import re
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import PureWindowsPath

from jase.config import Config
from jase.games import GAMES, GameDef
from jase.paths import resolve_exe, resolve_game_dir

SOURCES = ("registro", "steam", "carpeta típica", "carpeta elegida")

ReadValue = Callable[[str, str], "str | None"]


@dataclass(frozen=True)
class Finding:
    game_id: str
    path: str
    source: str


def resolve_game_exe(game: GameDef, install_dir: str | None) -> str | None:
    """Exe del juego en `install_dir`, o None si la carpeta no es de este juego.

    Un hl.exe es de Half-Life; solo es CS 1.6 si además existe la carpeta del mod.
    Lo usan la búsqueda de juegos y el estado de Ajustes para que coincidan.
    """
    exe = resolve_exe(install_dir, game.exe_relpath, game.exe_candidates,
                      game.preferred_exes)
    if (exe is not None and game.family == "valve" and game.valve_gamedir
            and resolve_game_dir(install_dir, game.valve_gamedir) is None):
        return None
    return exe


def is_configured(game: GameDef, cfg: Config) -> bool:
    return resolve_game_exe(game, cfg.game_paths.get(game.id)) is not None


# Pares (subclave bajo HKLM\SOFTWARE, valor); se leen en las vistas de 32 y 64 bits.
REGISTRY_KEYS: dict[str, tuple[tuple[str, str], ...]] = {
    "cod2": (("Activision\\Call of Duty 2", "InstallPath"),),
    "cod1": (("Activision\\Call of Duty", "InstallPath"),),
    "coduo": (("Activision\\Call of Duty United Offensive", "InstallPath"),),
    "cod4": (("Activision\\Call of Duty 4", "InstallPath"),),
    "q3": (("Id\\Quake III Arena", "INSTALLPATH"),),
    "rtcw": (("Activision\\Return to Castle Wolfenstein", "InstallPath"),),
    "et": (("Activision\\Wolfenstein - Enemy Territory", "InstallPath"),),
    "jka": (("LucasArts\\Star Wars Jedi Knight Jedi Academy\\1.0", "Install Path"),),
    "sof2": (("Activision\\Soldier of Fortune II - Double Helix", "InstallPath"),),
    "cs16": (("Valve\\Half-Life", "InstallPath"),),
    "ut99": (("Unreal Technology\\Installed Apps\\UnrealTournament", "Folder"),),
    "ut2004": (("Unreal Technology\\Installed Apps\\UT2004", "Folder"),),
    "mohaa": (("EA GAMES\\MOHAA", "InstallPath"),),
    "mohaas": (("EA GAMES\\MOHAA", "InstallPath"),),
    "mohaab": (("EA GAMES\\MOHAA", "InstallPath"),),
    "bf1942": (("EA GAMES\\Battlefield 1942", "GAMEDIR"),),
}

STEAM_DIRS: dict[str, tuple[str, ...]] = {
    "cod1": ("Call of Duty",),
    "coduo": ("Call of Duty",),
    "cod2": ("Call of Duty 2",),
    "cod4": ("Call of Duty 4",),
    "q3": ("Quake 3 Arena",),
    "q2": ("Quake 2", "Quake II"),
    "rtcw": ("Return to Castle Wolfenstein",),
    "jka": ("Jedi Academy", "Star Wars Jedi Knight Jedi Academy"),
    "cs16": ("Half-Life",),
    "l4d": ("Left 4 Dead",),
    "l4d2": ("Left 4 Dead 2",),
    "css": ("Counter-Strike Source",),
    "tf2": ("Team Fortress 2",),
    "hldm": ("Half-Life",),
    "dod": ("Half-Life",),
    "tfc": ("Half-Life",),
    "ut99": ("Unreal Tournament",),
    "ut2004": ("Unreal Tournament 2004",),
}


def _winreg():
    try:
        import winreg
    except ImportError:
        return None
    return winreg


def _read_hklm(subkey: str, value_name: str) -> str | None:
    winreg = _winreg()
    if winreg is None:
        return None
    for view in (winreg.KEY_WOW64_32KEY, winreg.KEY_WOW64_64KEY):
        try:
            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE,
                                "SOFTWARE\\" + subkey, 0,
                                winreg.KEY_READ | view) as key:
                value, _ = winreg.QueryValueEx(key, value_name)
        except OSError:
            continue
        if isinstance(value, str) and value:
            return value
    return None


def registry_findings(read_value: ReadValue | None = None) -> list[Finding]:
    if read_value is None:
        if _winreg() is None:
            return []
        read_value = _read_hklm
    found: list[Finding] = []
    for game_id, keys in REGISTRY_KEYS.items():
        for subkey, value_name in keys:
            value = read_value(subkey, value_name)
            if value:
                found.append(Finding(game_id, value, "registro"))
                break
    return found


def steam_libraries(steam_root: str) -> list[str]:
    libs = [steam_root]
    vdf = os.path.join(steam_root, "steamapps", "libraryfolders.vdf")
    try:
        with open(vdf, encoding="utf-8", errors="replace") as fh:
            text = fh.read()
    except OSError:
        text = ""
    libs += [m.replace("\\\\", "\\") for m in re.findall(r'"path"\s+"(.+?)"', text)]
    seen: set[str] = set()
    unique: list[str] = []
    for lib in libs:
        key = os.path.normcase(os.path.normpath(lib))
        if key not in seen:
            seen.add(key)
            unique.append(lib)
    return unique


def steam_findings(steam_roots: list[str]) -> list[Finding]:
    found: list[Finding] = []
    for root in steam_roots:
        for lib in steam_libraries(root):
            common = os.path.join(lib, "steamapps", "common")
            for game_id, names in STEAM_DIRS.items():
                for name in names:
                    path = os.path.join(common, name)
                    if os.path.isdir(path):
                        found.append(Finding(game_id, path, "steam"))
    return found


def typical_roots(drives: list[str]) -> list[str]:
    roots: list[str] = []
    for drive in drives:
        pf = [os.path.join(drive, "Program Files (x86)"),
              os.path.join(drive, "Program Files")]
        roots += pf
        roots += [os.path.join(drive, name)
                  for name in ("Juegos", "Games", "GOG Games")]
        publishers = ("Activision", "EA Games", "id Software", "GT Interactive", "LucasArts")
        roots += [os.path.join(base, pub) for base in pf for pub in publishers]
    return roots


def _exe_markers() -> list[tuple[str, tuple[str, ...], str]]:
    """(nombre del exe en minúsculas, carpetas que lo preceden, id de juego)."""
    markers = []
    for game in GAMES.values():
        for relpath in (game.exe_relpath, *game.exe_candidates):
            parts = PureWindowsPath(relpath).parts
            markers.append((parts[-1].casefold(),
                            tuple(p.casefold() for p in parts[:-1]), game.id))
    return markers


def _install_dir(directory: str, prefix: tuple[str, ...]) -> str | None:
    """Sube por `prefix` (p. ej. ("system",)) desde la carpeta del exe."""
    current = directory
    for name in reversed(prefix):
        if os.path.basename(current).casefold() != name:
            return None
        current = os.path.dirname(current)
    return current


def scan_folder(root: str, source: str, max_depth: int,
                cancelled: Callable[[], bool]) -> list[Finding]:
    markers = _exe_markers()
    found: list[Finding] = []
    stack = [(root, 0)]
    while stack:
        if cancelled():
            break
        path, depth = stack.pop()
        names: set[str] = set()
        subdirs: list[str] = []
        try:
            with os.scandir(path) as it:
                for entry in it:
                    try:
                        if entry.is_dir(follow_symlinks=False):
                            subdirs.append(entry.path)
                        else:
                            names.add(entry.name.casefold())
                    except OSError:
                        continue
        except OSError:
            continue
        for exe, prefix, game_id in markers:
            if exe in names:
                install = _install_dir(path, prefix)
                if install:
                    found.append(Finding(game_id, install, source))
        if depth < max_depth:
            stack.extend((d, depth + 1) for d in sorted(subdirs, reverse=True))
    return _drop_nested(found)


def _drop_nested(found: list[Finding]) -> list[Finding]:
    """Quita el hallazgo de un juego que cuelga de otro hallazgo del mismo juego
    (jamp.exe en GameData\\ cuenta como el candidato suelto y como GameData\\jamp.exe)."""
    def norm(path: str) -> str:
        return os.path.normcase(os.path.normpath(path))

    def nested(inner: Finding) -> bool:
        return any(o.game_id == inner.game_id and norm(inner.path).startswith(norm(o.path) + os.sep)
                   for o in found)

    return [f for f in found if not nested(f)]


def default_steam_roots() -> list[str]:
    roots: list[str] = []
    winreg = _winreg()
    if winreg is not None:
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Software\\Valve\\Steam") as key:
                value, _ = winreg.QueryValueEx(key, "SteamPath")
            if isinstance(value, str) and value:
                roots.append(os.path.normpath(value))
        except OSError:
            pass
    default = "C:\\Program Files (x86)\\Steam"
    if os.path.isdir(default) and os.path.normcase(default) not in {
            os.path.normcase(r) for r in roots}:
        roots.append(default)
    return roots


def default_drives() -> list[str]:
    if _winreg() is None:
        return []
    return [f"{letter}:\\" for letter in "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
            if os.path.isdir(f"{letter}:\\")]


def find_games(*, read_value: ReadValue | None = None, steam_roots: list[str],
               drives: list[str], extra_root: str | None = None,
               cancelled: Callable[[], bool] = lambda: False) -> dict[str, list[Finding]]:
    raw = registry_findings(read_value) + steam_findings(steam_roots)
    for root in typical_roots(drives):
        if cancelled():
            break
        raw += scan_folder(root, "carpeta típica", 2, cancelled)
    if extra_root and not cancelled():
        raw += scan_folder(extra_root, "carpeta elegida", 6, cancelled)

    best: dict[tuple[str, str], Finding] = {}
    for f in raw:
        game = GAMES.get(f.game_id)
        if game is None or resolve_game_exe(game, f.path) is None:
            continue
        key = (f.game_id, os.path.normcase(os.path.normpath(f.path)))
        old = best.get(key)
        if old is None or SOURCES.index(f.source) < SOURCES.index(old.source):
            best[key] = f
    result: dict[str, list[Finding]] = {}
    for f in best.values():
        result.setdefault(f.game_id, []).append(f)
    for items in result.values():
        items.sort(key=lambda f: SOURCES.index(f.source))
    return result
