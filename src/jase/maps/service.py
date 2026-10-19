"""Imagen de un mapa: caché en disco → archivos del juego → GameTracker (opcional)."""
from __future__ import annotations

import contextlib
import io
import logging
import os
import re
import tempfile
import threading
import time
import urllib.parse
import urllib.request
from collections.abc import Callable
from pathlib import Path

from PIL import Image

from jase.games import GameDef
from jase.maps import cod, iwi, unreal

log = logging.getLogger(__name__)

GAMETRACKER_URL = "https://image.gametracker.com/images/maps/160x120/{gt_id}/{map}.jpg"
FETCH_TIMEOUT = 5  # por operación de socket
FETCH_DEADLINE = 8  # total para leer la respuesta
FETCH_CHUNK = 64 * 1024
USER_AGENT = "JASE"
MAX_DOWNLOAD_BYTES = 2 * 1024 * 1024
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


def gametracker_url(gt_id: str, map_name: str) -> str:
    # GameTracker guarda los mapas con carpeta (MOHAA: "obj/obj_team2") con "_".
    name = map_name.casefold().replace("/", "_")
    return GAMETRACKER_URL.format(gt_id=gt_id, map=urllib.parse.quote(name))


def default_fetch(url: str) -> bytes | None:
    """GET con timeout y User-Agent "JASE"; cualquier error (404, sin red…) → None."""
    deadline = time.monotonic() + FETCH_DEADLINE
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    chunks: list[bytes] = []
    total = 0
    try:
        with urllib.request.urlopen(req, timeout=FETCH_TIMEOUT) as resp:
            while True:
                if time.monotonic() > deadline:
                    log.debug("Descarga demasiado lenta %s", url)
                    return None
                chunk = resp.read(FETCH_CHUNK)
                if not chunk:
                    break
                total += len(chunk)
                if total > MAX_DOWNLOAD_BYTES:
                    return None
                chunks.append(chunk)
    except Exception as exc:  # HTTPError, URLError, timeout, etc.
        log.debug("Descarga fallida %s: %s", url, exc)
        return None
    return b"".join(chunks)


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

        key = safe_map_filename(map_name)
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
            if game.family == "quake3":
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
