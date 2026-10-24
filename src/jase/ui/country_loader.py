"""Carga (y descarga si falta) la base de países en un hilo daemon."""
from __future__ import annotations

import threading
from dataclasses import dataclass, field
from typing import Any
from pathlib import Path

from PySide6.QtCore import QObject, Signal

from jase.i18n import tr


class CountryDBRelay(QObject):
    """Lleva el resultado al hilo de la UI (sin padre; ver MapImageRelay)."""

    loaded = Signal(object, object, object)  # (CountryDBRequest, CountryDB | None, error)


@dataclass(eq=False)
class CountryDBRequest:
    source: Any  # geo.CountrySource o equivalente con load(path, refresh)
    path: Path
    refresh: bool = False
    cancelled: bool = field(default=False)


_RELAY: CountryDBRelay | None = None


def shared_relay() -> CountryDBRelay:
    """Relay compartido; se crea en el hilo de la UI (afinidad)."""
    global _RELAY
    if _RELAY is None:
        _RELAY = CountryDBRelay()
    return _RELAY


def submit(request: CountryDBRequest) -> None:
    """Ejecuta `request.source.load` en un hilo daemon (una descarga colgada no
    impide cerrar la app) y emite `shared_relay().loaded` al terminar."""
    relay = shared_relay()

    def run() -> None:
        try:
            db, error = request.source.load(request.path, refresh=request.refresh)
        except Exception:  # la fuente no debería lanzar; por las dudas, sin países
            db, error = None, tr("No se pudo cargar la base de países")
        if request.cancelled:
            return
        try:
            relay.loaded.emit(request, db, error)
        except RuntimeError:  # aplicación cerrándose
            pass

    threading.Thread(target=run, name="jase-country-db", daemon=True).start()
