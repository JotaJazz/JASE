"""Actualización de la lista oficial de masters en un hilo daemon."""
from __future__ import annotations

import threading
from dataclasses import dataclass, field
from pathlib import Path

from PySide6.QtCore import QObject, Signal

from jase import masters


class MastersRelay(QObject):
    """Lleva el resultado al hilo de la UI (sin padre; ver MapImageRelay)."""

    loaded = Signal(object, object, object)  # (MastersUpdateRequest, OfficialList | None, error)


@dataclass(eq=False)
class MastersUpdateRequest:
    cache_dir: Path
    now: float
    fetch: object  # callable(url) -> bytes
    cancelled: bool = field(default=False)
    force: bool = False  # pedido a mano desde Ajustes: el resultado siempre se informa


_RELAY: MastersRelay | None = None


def shared_relay() -> MastersRelay:
    """Relay compartido; se crea en el hilo de la UI (afinidad)."""
    global _RELAY
    if _RELAY is None:
        _RELAY = MastersRelay()
    return _RELAY


def submit(request: MastersUpdateRequest) -> None:
    relay = shared_relay()

    def run() -> None:
        try:
            official, ok = masters.check_official(request.cache_dir, request.now,
                                                  request.fetch)
            error = None if ok else "download failed"
        except Exception as exc:  # update_official no debería lanzar
            official, error = None, str(exc)
        if request.cancelled:
            return
        try:
            relay.loaded.emit(request, official, error)
        except RuntimeError:  # aplicación cerrándose
            pass

    threading.Thread(target=run, name="jase-masters-update", daemon=True).start()
