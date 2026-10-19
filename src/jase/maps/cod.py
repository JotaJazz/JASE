"""Pantallas de carga de mapas de CoD1/CoD2 desde los .iwd/.pk3 del juego."""
from __future__ import annotations

import logging
import os
import zipfile

from jase.paths import _child, resolve_game_dir

log = logging.getLogger(__name__)

ARCHIVE_EXTS = (".iwd", ".pk3")
MAX_ENTRY_SIZE = 64 * 1024 * 1024


def _archives(folder: str) -> list[str]:
    """Archivos .iwd/.pk3 de `folder` en orden de carga (alfabético)."""
    try:
        with os.scandir(folder) as it:
            names = [
                e.path
                for e in it
                if e.name.casefold().endswith(ARCHIVE_EXTS) and e.is_file()
            ]
    except OSError:
        return []
    return sorted(names, key=lambda p: os.path.basename(p).casefold())


def _search_order(game_dir: str) -> list[str]:
    """main/ primero y luego cada mods/<mod>/: los posteriores pisan a los anteriores."""
    order: list[str] = []
    main = _child(game_dir, "main")
    if main and os.path.isdir(main):
        order += _archives(main)
    mods = _child(game_dir, "mods")
    if mods and os.path.isdir(mods):
        try:
            with os.scandir(mods) as it:
                mod_dirs = [e.path for e in it if e.is_dir()]
        except OSError:
            mod_dirs = []
        for mod in sorted(mod_dirs, key=lambda p: os.path.basename(p).casefold()):
            order += _archives(mod)
    return order


def _read_entry(archive: str, wanted: str) -> bytes | None:
    try:
        with zipfile.ZipFile(archive) as z:
            for info in z.infolist():
                if info.filename.casefold() != wanted or info.is_dir():
                    continue
                if info.file_size > MAX_ENTRY_SIZE:
                    return None
                return z.read(info)
    except Exception as exc:  # zip dañado, ilegible, cifrado, etc.: se ignora
        log.debug("Archivo ignorado %s: %s", archive, exc)
    return None


def find_loadscreen(install_dir: str | None, map_name: str) -> bytes | None:
    """Bytes de images/loadscreen_<map>.iwi, o None si no se encuentra.

    Si varios archivos lo contienen gana el último en orden de carga
    (semántica de Quake 3). Los archivos que no son zip válidos se ignoran.
    """
    if not map_name or "/" in map_name or "\\" in map_name:
        return None
    game_dir = resolve_game_dir(install_dir, "main")
    if game_dir is None:
        return None
    wanted = f"images/loadscreen_{map_name}.iwi".casefold()
    for archive in reversed(_search_order(game_dir)):
        data = _read_entry(archive, wanted)
        if data is not None:
            return data
    return None
