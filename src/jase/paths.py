from __future__ import annotations

import os
from pathlib import PureWindowsPath


def _child(base: str, name: str) -> str | None:
    """Entrada de `base` cuyo nombre coincide con `name` ignorando mayúsculas."""
    wanted = name.casefold()
    try:
        with os.scandir(base) as it:
            for entry in it:
                if entry.name.casefold() == wanted:
                    return entry.path
    except OSError:
        return None
    return None


def _find_file(base: str, parts: tuple[str, ...]) -> str | None:
    current = base
    for i, part in enumerate(parts):
        found = _child(current, part)
        if found is None:
            return None
        if i < len(parts) - 1 and not os.path.isdir(found):
            return None
        current = found
    return current if os.path.isfile(current) else None


def resolve_exe(install_dir: str | None, exe_relpath: str,
                candidates: tuple[str, ...] = (),
                preferred: tuple[str, ...] = ()) -> str | None:
    """Ruta nativa del ejecutable del juego, o None si no se encuentra.

    Orden: install_dir/exe, install_dir/<subcarpeta>/exe, padre/exe y, si
    install_dir ya es la carpeta del exe (p. ej. "System"), install_dir/nombre.
    Si `exe_relpath` no aparece, se prueba cada uno de `candidates` igual, en orden.
    Una vez resuelto, si en esa MISMA carpeta hay alguno de `preferred` (en orden),
    se devuelve ese en su lugar (p. ej. el lanzador de OpenMoHAA junto a MOHAA.exe).
    """
    for relpath in (exe_relpath, *candidates):
        found = _resolve_one(install_dir, relpath)
        if found:
            folder = os.path.dirname(found)
            for name in preferred:
                better = _find_file(folder, (name,))
                if better:
                    return better
            return found
    return None


def _resolve_one(install_dir: str | None, exe_relpath: str) -> str | None:
    if not install_dir:
        return None
    parts = PureWindowsPath(exe_relpath).parts
    if not parts or not os.path.isdir(install_dir):
        return None

    found = _find_file(install_dir, parts)
    if found:
        return found
    try:
        with os.scandir(install_dir) as it:
            subdirs = sorted(e.path for e in it if e.is_dir())
    except OSError:
        subdirs = []
    for sub in subdirs:
        found = _find_file(sub, parts)
        if found:
            return found
    parent = os.path.dirname(os.path.normpath(install_dir))
    if parent and parent != os.path.normpath(install_dir):
        found = _find_file(parent, parts)
        if found:
            return found
    last_dir = os.path.basename(os.path.normpath(install_dir)).casefold()
    if len(parts) > 1 and last_dir == parts[-2].casefold():
        return _find_file(install_dir, parts[-1:])
    return None


def resolve_game_dir(install_dir: str | None, marker: str) -> str | None:
    """Carpeta del juego que contiene la subcarpeta `marker` (p. ej. "main").

    Orden: install_dir, sus subcarpetas directas (alfabético) y su padre.
    La comparación de nombres ignora mayúsculas. None si no se encuentra.
    """
    if not install_dir or not marker or not os.path.isdir(install_dir):
        return None

    def has_marker(base: str) -> bool:
        found = _child(base, marker)
        return found is not None and os.path.isdir(found)

    if has_marker(install_dir):
        return install_dir
    try:
        with os.scandir(install_dir) as it:
            subdirs = sorted(e.path for e in it if e.is_dir())
    except OSError:
        subdirs = []
    for sub in subdirs:
        if has_marker(sub):
            return sub
    parent = os.path.dirname(os.path.normpath(install_dir))
    if parent and parent != os.path.normpath(install_dir) and has_marker(parent):
        return parent
    return None
