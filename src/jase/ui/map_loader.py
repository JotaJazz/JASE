"""Carga de imágenes de mapas en hilos daemon y relay hacia el hilo de la UI."""
from __future__ import annotations

import queue
import threading
from dataclasses import dataclass, field

from PySide6.QtCore import QObject, Signal


class MapImageRelay(QObject):
    """Lleva el resultado de una petición de imagen al hilo de la UI.

    Sin padre: vive mientras el cargador (módulo) lo referencie. Las ventanas
    conectan su slot; Qt corta la conexión cuando una ventana se destruye, así
    que emitir después es inofensivo.
    """

    loaded = Signal(object, object)  # (MapImageRequest, Path | None)


@dataclass(eq=False)
class MapImageRequest:
    service: object
    args: tuple  # (game, install_dir, map_name, allow_download)
    cancelled: bool = field(default=False)


class MapImageLoader:
    """Hilos daemon que llaman a `service.get` (bloqueante).

    Daemon a propósito: una descarga colgada (p. ej. getaddrinfo lento) nunca
    impide que el proceso termine. Concurrencia acotada a WORKERS hilos.
    """

    WORKERS = 2

    def __init__(self) -> None:
        self.relay = MapImageRelay()
        self._queue: queue.SimpleQueue[MapImageRequest] = queue.SimpleQueue()
        for i in range(self.WORKERS):
            threading.Thread(target=self._run, name=f"jase-map-image-{i}", daemon=True).start()

    def submit(self, request: MapImageRequest) -> None:
        self._queue.put(request)

    def _run(self) -> None:
        while True:
            request = self._queue.get()
            if request.cancelled:
                continue  # selección ya cambiada antes de empezar
            try:
                path = request.service.get(*request.args)
            except Exception:  # el servicio no debería fallar; por las dudas, sin imagen
                path = None
            if request.cancelled:
                continue
            try:
                self.relay.loaded.emit(request, path)
            except RuntimeError:  # aplicación cerrándose
                pass


_SHARED_LOADER: MapImageLoader | None = None


def shared_loader() -> MapImageLoader:
    """Cargador compartido; se crea en el hilo de la UI (afinidad del relay)."""
    global _SHARED_LOADER
    if _SHARED_LOADER is None:
        _SHARED_LOADER = MapImageLoader()
    return _SHARED_LOADER
