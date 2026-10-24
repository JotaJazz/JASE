"""Vigila un servidor lleno y avisa cuando queda un hueco."""
from __future__ import annotations

from PySide6.QtCore import QObject, QTimer, Signal, Slot

from jase.games import GameDef
from jase.models import ServerAddress, ServerInfo
from jase.ui.scan_worker import QueryWorker


def has_free_slot(info: ServerInfo) -> bool:
    return info.online and info.max_players > 0 and info.num_players < info.max_players


class SlotWatcher(QObject):
    """Consulta el servidor cada INTERVAL_MS (una consulta a la vez) hasta que haya sitio."""

    server_seen = Signal(object)  # ServerInfo: cada respuesta de la consulta en curso
    slot_free = Signal(object)  # ServerInfo
    expired = Signal()

    INTERVAL_MS = 10_000
    MAX_MS = 30 * 60_000

    def __init__(self, game: GameDef, address: ServerAddress, query_fn, parent=None) -> None:
        super().__init__(parent)
        self.game = game
        self.address = address
        self._query_fn = query_fn
        self._worker: QueryWorker | None = None
        self._active = False
        self._timer = QTimer(self)
        self._timer.timeout.connect(self._tick)
        self._expiry = QTimer(self)
        self._expiry.setSingleShot(True)
        self._expiry.timeout.connect(self._on_expiry)

    @property
    def active(self) -> bool:
        return self._active

    def current_worker(self) -> QueryWorker | None:
        """Última consulta lanzada que aún no terminó (también si ya se canceló)."""
        return self._worker

    def start(self) -> None:
        self._active = True
        self._timer.start(self.INTERVAL_MS)
        self._expiry.start(self.MAX_MS)

    def stop(self) -> None:
        self._active = False
        self._timer.stop()
        self._expiry.stop()
        if self._worker is not None:
            self._worker.cancel()  # se suelta en `finished`

    def _tick(self) -> None:
        if not self._active or self._worker is not None:
            return  # la consulta anterior sigue en curso
        worker = QueryWorker(self.game, [self.address], query_fn=self._query_fn)
        worker.server_found.connect(self._on_found)
        worker.finished.connect(self._on_finished)
        self._worker = worker
        worker.start()

    @Slot(object)
    def _on_found(self, info: ServerInfo) -> None:
        if not self._active or self.sender() is not self._worker:
            return
        self.server_seen.emit(info)
        if self._active and has_free_slot(info):
            self.stop()
            self.slot_free.emit(info)

    @Slot()
    def _on_finished(self) -> None:
        worker = self.sender()
        if isinstance(worker, QueryWorker):
            worker.wait()
            if worker is self._worker:
                self._worker = None

    def _on_expiry(self) -> None:
        if self._active:
            self.stop()
            self.expired.emit()
