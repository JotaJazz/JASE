"""Búsqueda de versión nueva de JASE en un hilo daemon."""
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

from PySide6.QtCore import QObject, Signal

from jase import app_update
from jase.ui.daemon import run_daemon


class UpdateRelay(QObject):
    """Lleva el resultado al hilo de la UI (sin padre; ver MapImageRelay)."""

    loaded = Signal(object, object)  # (UpdateRequest, Release | None)


@dataclass(eq=False)
class UpdateRequest:
    cache_dir: Path
    now: float
    current: str
    fetch: object  # callable(url) -> bytes
    cancelled: bool = field(default=False)
    manual: bool = False  # Ayuda → Buscar actualizaciones: se informa también «al día»
    ok: bool = False  # se pudo consultar GitHub (lo fija el hilo)


_RELAY: UpdateRelay | None = None


def shared_relay() -> UpdateRelay:
    """Relay compartido; se crea en el hilo de la UI (afinidad)."""
    global _RELAY
    if _RELAY is None:
        _RELAY = UpdateRelay()
    return _RELAY


def submit(request: UpdateRequest) -> None:
    def work() -> tuple:
        try:
            release, request.ok = app_update.check_detailed(
                request.cache_dir, request.now, request.current, request.fetch)
        except Exception:  # check no debería lanzar
            release = None
        return (release,)

    run_daemon("jase-app-update", request, work, shared_relay().loaded)
