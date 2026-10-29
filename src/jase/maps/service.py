"""Imagen de un mapa: caché en disco → archivos del juego → GameTracker (opcional)."""
from __future__ import annotations

import contextlib
import io
import logging
import os
import re
import tempfile
import threading
from collections.abc import Callable
from pathlib import Path

from PIL import Image

from jase.games import GameDef
from jase.maps import cod, iwi, levelshots, unreal
from jase.net import default_fetch

log = logging.getLogger(__name__)

GAMETRACKER_URL = "https://image.gametracker.com/images/maps/160x120/{gt_id}/{map}.jpg"
# Tamaño máximo guardado en caché (la ficha lo muestra a 160×120; margen para HiDPI).
MAX_CACHED_SIZE = (480, 360)
# CoD estira la pantalla de carga (cuadrada) a toda la pantalla: se guarda en 4:3.
COD_LOADSCREEN_SIZE = (480, 360)

_UNSAFE_CHARS = re.compile(r'[\\/:*?"<>|\x00-\x1f]')
_WINDOWS_RESERVED = {"con", "prn", "aux", "nul",
                     *(f"com{i}" for i in range(1, 10)), *(f"lpt{i}" for i in range(1, 10))}

Fetch = Callable[[str], "bytes | None"]


def safe_map_filename(map_name: str) -> str | None:
    """Nombre de archivo (sin extensión) válido en Windows para `map_name`, o None."""
    name = _UNSAFE_CHARS.sub("", map_name).strip(" .").casefold()
    if not name:
        return None
    if name in _WINDOWS_RESERVED:
        name += "_"
    return name


def gametracker_map_name(map_name: str) -> str:
    # GameTracker guarda los mapas en minúsculas y con "_" en todo carácter que
    # no sea alfanumérico (MOHAA "obj/obj_team2", UT99 "DM-Deck16][").
    return re.sub(r"[^a-z0-9]", "_", map_name.casefold())


def gametracker_url(gt_id: str, map_name: str) -> str:
    return GAMETRACKER_URL.format(gt_id=gt_id, map=gametracker_map_name(map_name))


class MapImageService:
    """Devuelve la ruta a un PNG con la imagen del mapa, o None.

    `get` es bloqueante (lee archivos del juego y puede descargar): llamarlo
    desde un hilo de trabajo. Es seguro llamarlo desde varios hilos a la vez.
    Los "no encontrado" se recuerdan solo en memoria durante la sesión, para
    que un mapa instalado después aparezca al reiniciar.
    """

    def __init__(self, cache_dir: Path, fetch: Fetch | None = None) -> None:
        self.cache_dir = Path(cache_dir)
        self._fetch = fetch or default_fetch
        self._lock = threading.Lock()
        self._local_misses: set[tuple[str, str, str]] = set()
        self._download_misses: set[tuple[str, str]] = set()

    def cache_path(self, game: GameDef, map_name: str) -> Path | None:
        name = safe_map_filename(map_name)
        return self.cache_dir / game.id / f"{name}.png" if name else None

    def get(self, game: GameDef, install_dir: str | None, map_name: str,
            allow_download: bool) -> Path | None:
        path = self.cache_path(game, map_name)
        if path is None:
            return None
        if path.is_file():
            return path

        key = safe_map_filename(map_name) or ""  # no vacío: cache_path ya lo validó
        local_key = (game.id, install_dir or "", key)
        with self._lock:
            try_local = local_key not in self._local_misses
        if try_local:
            img = self._extract_local(game, install_dir, map_name)
            if img is not None:
                return self._store(img, path)
            with self._lock:
                self._local_misses.add(local_key)

        if not (allow_download and game.gametracker_id):
            return None
        download_key = (game.id, key)
        with self._lock:
            if download_key in self._download_misses:
                return None
        img = self._download(game.gametracker_id, map_name)
        if img is not None:
            return self._store(img, path)
        with self._lock:
            self._download_misses.add(download_key)
        return None

    # ------------------------------------------------------------ fuentes
    @staticmethod
    def _extract_local(game: GameDef, install_dir: str | None,
                       map_name: str) -> Image.Image | None:
        if not install_dir:
            return None
        try:
            if game.levelshot_dir:
                data = levelshots.find_levelshot(install_dir, game.levelshot_dir, map_name)
                if data is None:
                    return None
                with Image.open(io.BytesIO(data)) as src:
                    src.load()
                    return src.convert("RGB")
            if game.cod_loadscreens:
                data = cod.find_loadscreen(install_dir, map_name)
                if data is None:
                    return None
                return iwi.decode_iwi(data).convert("RGB").resize(
                    COD_LOADSCREEN_SIZE, Image.Resampling.LANCZOS)
            if game.family == "gamespy" and game.id == "ut99":
                unr = unreal.find_map_file(install_dir, map_name)
                return unreal.extract_screenshot(unr) if unr else None
        except Exception as exc:  # IwiError, archivo dañado o inesperado: sin imagen
            log.debug("Sin imagen local de %s/%s: %s", game.id, map_name, exc)
        return None

    def _download(self, gt_id: str, map_name: str) -> Image.Image | None:
        try:
            data = self._fetch(gametracker_url(gt_id, map_name))
            if not data:
                return None
            with Image.open(io.BytesIO(data)) as img:
                img.load()
                return img.convert("RGB")
        except Exception as exc:  # sin red, 404, no es una imagen…
            log.debug("Sin imagen de GameTracker para %s/%s: %s", gt_id, map_name, exc)
            return None

    # ------------------------------------------------------------- caché
    @staticmethod
    def _store(img: Image.Image, path: Path) -> Path | None:
        """Guarda como PNG de forma atómica; None si no se puede escribir."""
        tmp = None
        try:
            img = img.convert("RGB")
            img.thumbnail(MAX_CACHED_SIZE, Image.Resampling.LANCZOS)
            path.parent.mkdir(parents=True, exist_ok=True)
            fd, tmp = tempfile.mkstemp(prefix=f".{path.stem}.", suffix=".tmp", dir=path.parent)
            with os.fdopen(fd, "wb") as fh:
                img.save(fh, "PNG")
            os.replace(tmp, path)
            return path
        except Exception as exc:
            log.debug("No se pudo guardar %s: %s", path, exc)
            if tmp is not None:
                with contextlib.suppress(OSError):
                    os.unlink(tmp)
            # Windows: os.replace falla si el destino está abierto (p. ej. otro hilo
            # ya lo guardó y se está leyendo); ese archivo sirve igual.
            return path if path.is_file() else None
