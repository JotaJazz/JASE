"""Trabajo bloqueante en un hilo daemon con el resultado de vuelta por una señal de Qt."""
from __future__ import annotations

import threading
from collections.abc import Callable
from typing import Any


def run_daemon(name: str, request: Any, work: Callable[[], tuple], signal: Any,
               deliver_cancelled: bool = False) -> None:
    """Ejecuta `work()` en un hilo daemon (una descarga colgada no retiene el cierre) y
    emite `signal(request, *resultado)` salvo que `request.cancelled` (con
    `deliver_cancelled` se emite igual: quien escucha cierra su diálogo al recibirlo).
    `work` atrapa sus propios errores; emitir con la aplicación cerrándose no lanza."""

    def run() -> None:
        result = work()
        if request.cancelled and not deliver_cancelled:
            return
        try:
            signal.emit(request, *result)
        except RuntimeError:  # aplicación cerrándose
            pass

    threading.Thread(target=run, name=name, daemon=True).start()
