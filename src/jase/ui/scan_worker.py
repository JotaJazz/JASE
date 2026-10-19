"""Hilo que ejecuta el scanner asíncrono y reenvía resultados como señales Qt."""
from __future__ import annotations

import asyncio
import threading

from PySide6.QtCore import QThread, Signal

from jase import scanner
from jase.games import GameDef


class ScanWorker(QThread):
    server_found = Signal(object)
    status = Signal(str)
    progress = Signal(int, int)  # (consultados, total) en la fase de servidores

    def __init__(self, game: GameDef, masters: list[tuple[str, int]],
                 scan_fn=scanner.scan, parent=None) -> None:
        super().__init__(parent)
        self.game = game
        self.masters = list(masters)
        self.generation = 0
        self._scan_fn = scan_fn
        self._lock = threading.Lock()
        self._loop: asyncio.AbstractEventLoop | None = None
        self._task: asyncio.Task | None = None
        self._cancel_requested = False
        # True si scan_fn lanzó una excepción inesperada o ningún master respondió:
        # en ambos casos el resultado no debe reemplazar la última lista completa.
        self.failed = False
        self.result: scanner.ScanResult | None = None

    @property
    def cancel_requested(self) -> bool:
        return self._cancel_requested

    def cancel(self) -> None:
        with self._lock:
            self._cancel_requested = True
            loop, task = self._loop, self._task
        if loop is not None and task is not None:
            try:
                loop.call_soon_threadsafe(task.cancel)
            except RuntimeError:
                pass  # el bucle ya terminó

    async def _main(self) -> None:
        with self._lock:
            self._loop = asyncio.get_running_loop()
            self._task = asyncio.current_task()
            if self._cancel_requested:
                return
        result = await self._scan_fn(self.game, self.masters,
                                     self.server_found.emit, self.status.emit,
                                     on_progress=self.progress.emit)
        if isinstance(result, scanner.ScanResult):
            self.result = result
            if not result.ok:
                self.failed = True

    def run(self) -> None:
        try:
            asyncio.run(self._main())
        except asyncio.CancelledError:
            pass
        except Exception as exc:  # noqa: BLE001 - el hilo nunca debe morir en silencio
            self.failed = True
            self.status.emit(f"Error en el escaneo: {exc}")
        finally:
            with self._lock:
                self._loop = None
                self._task = None
