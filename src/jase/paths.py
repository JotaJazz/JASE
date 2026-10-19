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


def resolve_exe(install_dir: str | None, exe_relpath: str) -> str | None:
    """Ruta nativa del ejecutable del juego, o None si no se encuentra.

    Orden: install_dir/exe, install_dir/<subcarpeta>/exe, padre/exe y, si
    install_dir ya es la carpeta del exe (p. ej. "System"), install_dir/nombre.
    """
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
