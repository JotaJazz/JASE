"""Recolección de basura solo en el hilo de la UI.

Con el recolector automático, un ciclo de objetos que incluye wrappers de Qt puede
liberarse en cualquier hilo que esté reservando memoria (p. ej. el bucle asyncio de un
worker) y destruir así un QObject fuera de su hilo: en Windows es una «access
violation». Aquí se apaga el automático y se recolecta con un QTimer del hilo de la UI,
con los mismos umbrales por generación que usaría Python.
"""
from __future__ import annotations

import gc

from PySide6.QtCore import QObject, QTimer

INTERVAL_MS = 500


def collect_if_due() -> None:
    """Recolecta las generaciones que superaron su umbral (llamar en el hilo de la UI)."""
    counts, thresholds = gc.get_count(), gc.get_threshold()
    if counts[0] <= thresholds[0]:
        return
    generation = 0
    if counts[1] > thresholds[1]:
        generation = 2 if counts[2] > thresholds[2] else 1
    gc.collect(generation)


class MainThreadGC(QObject):
    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        gc.disable()
        self._timer = QTimer(self)
        self._timer.setInterval(INTERVAL_MS)
        self._timer.timeout.connect(collect_if_due)
        self._timer.start()
