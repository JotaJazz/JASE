"""Revisa en segundo plano los favoritos y recientes para avisar de amigos en línea."""
from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import QObject, QTimer, Signal, Slot

from jase.games import GameDef
from jase.models import ServerAddress, ServerInfo
from jase.ui.scan_worker import QueryWorker


class FriendWatcher(QObject):
    """Cada INTERVAL_MS recorre los juegos (uno a la vez, una consulta en curso como máximo).

    `targets_fn()` devuelve [(GameDef, [ServerAddress, ...]), ...]; los juegos sin
    direcciones se omiten. Emite `server_seen(game_id, ServerInfo)` por cada respuesta.
    """

    server_seen = Signal(str, object)

    INTERVAL_MS = 120_000

    def __init__(self, targets_fn: Callable[[], list[tuple[GameDef, list[ServerAddress]]]],
                 query_fn, parent=None) -> None:
        super().__init__(parent)
        self._targets_fn = targets_fn
        self._query_fn = query_fn
        self._worker: QueryWorker | None = None
        self._queue: list[tuple[GameDef, list[ServerAddress]]] = []
        self._active = False
        self._timer = QTimer(self)
        self._timer.timeout.connect(self._tick)

    @property
    def active(self) -> bool:
        return self._active

    def current_worker(self) -> QueryWorker | None:
        """Última consulta lanzada que aún no terminó (también si ya se canceló)."""
        return self._worker

    def start(self) -> None:
        if self._active:
            return
        self._active = True
        self._timer.start(self.INTERVAL_MS)

    def stop(self) -> None:
        self._active = False
        self._timer.stop()
        self._queue = []
        if self._worker is not None:
            self._worker.cancel()  # se suelta en `finished`

    def _tick(self) -> None:
        if not self._active or self._worker is not None or self._queue:
            return  # la vuelta anterior sigue en curso
        self._queue = [(g, a) for g, a in self._targets_fn() if a]
        self._next()

    def _next(self) -> None:
        if not self._active or self._worker is not None or not self._queue:
            return
        game, addresses = self._queue.pop(0)
        worker = QueryWorker(game, addresses, query_fn=self._query_fn)
        worker.server_found.connect(self._on_found)
        worker.finished.connect(self._on_finished)
        self._worker = worker
        worker.start()

    @Slot(object)
    def _on_found(self, info: ServerInfo) -> None:
        worker = self.sender()
        if self._active and worker is self._worker and isinstance(worker, QueryWorker):
            self.server_seen.emit(worker.game.id, info)

    @Slot()
    def _on_finished(self) -> None:
        worker = self.sender()
        if isinstance(worker, QueryWorker):
            worker.wait()
            if worker is self._worker:
                self._worker = None
                self._next()
