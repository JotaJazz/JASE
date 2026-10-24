"""Levelshots de los .pk3 de los juegos de la familia Quake 3 (no CoD)."""
from __future__ import annotations

from jase.maps.cod import archives_in, read_entry
from jase.paths import _child, resolve_game_dir

LEVELSHOT_EXTS = (".jpg", ".tga", ".png")


def valid_map_path(map_name: str) -> bool:
    """Ruta de mapa segura: segmentos con `/`, sin vacíos, `.`, `..`, `\\` ni `:`."""
    if not map_name or "\\" in map_name or ":" in map_name:
        return False
    return all(seg not in ("", ".", "..") for seg in map_name.split("/"))


def find_levelshot(install_dir: str | None, base_dir: str, map_name: str) -> bytes | None:
    """Bytes de levelshots/<map>.<ext> en los pk3 de <juego>/<base_dir>, o None.

    Gana el último pk3 en orden de carga; los que no son zip válidos se ignoran.
    """
    if not valid_map_path(map_name):
        return None
    game_dir = resolve_game_dir(install_dir, base_dir)
    if game_dir is None:
        return None
    folder = _child(game_dir, base_dir)
    if not folder:
        return None
    wanted = [f"levelshots/{map_name}{ext}".casefold() for ext in LEVELSHOT_EXTS]
    for archive in reversed(archives_in(folder)):
        if not archive.casefold().endswith(".pk3"):
            continue
        for name in wanted:
            data = read_entry(archive, name)
            if data is not None:
                return data
    return None
