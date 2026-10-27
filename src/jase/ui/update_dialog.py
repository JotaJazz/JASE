"""Actualizar JASE desde la app: diálogo de elección y descarga del instalador en un hilo."""
from __future__ import annotations

import threading
import zipfile
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

from PySide6.QtCore import QObject, Signal
from PySide6.QtWidgets import QMessageBox, QWidget

from jase import app_update, portable_update
from jase.i18n import tr

INSTALL, PAGE = "install", "page"


BAD_ZIP = "bad_zip"  # el zip portátil no se pudo extraer o no trae JASE.exe


def download_error_text(code: str) -> str:
    """Texto traducido para un código DOWNLOAD_* de app_update."""
    texts = {
        app_update.DOWNLOAD_SIZE: lambda: tr("el archivo descargado está incompleto"),
        app_update.DOWNLOAD_TOO_MUCH: lambda: tr("el archivo es más grande de lo anunciado"),
        app_update.DOWNLOAD_HASH: lambda: tr("la suma SHA-256 no coincide"),
        app_update.DOWNLOAD_INSECURE: lambda: tr("la descarga no usa una conexión segura (https)"),
        app_update.DOWNLOAD_CANCELLED: lambda: tr("descarga cancelada"),
        BAD_ZIP: lambda: tr("el zip descargado no es una versión portátil válida de JASE"),
    }
    text = texts.get(code)
    return text() if text is not None else tr("error de red o de disco")


def ask_action(parent: QWidget | None, release: app_update.Release) -> str | None:
    """«Descargar e instalar» → INSTALL, «Ver la página» → PAGE, «Cancelar» → None."""
    box = QMessageBox(parent)
    box.setIcon(QMessageBox.Icon.Question)
    box.setWindowTitle(tr("Nueva versión {version} disponible", version=release.version))
    box.setText(tr("Hay una versión nueva de JASE ({version}).", version=release.version))
    install = box.addButton(tr("Descargar e instalar"), QMessageBox.ButtonRole.AcceptRole)
    page = box.addButton(tr("Ver la página"), QMessageBox.ButtonRole.ActionRole)
    box.addButton(tr("Cancelar"), QMessageBox.ButtonRole.RejectRole)
    box.exec()
    clicked = box.clickedButton()
    return INSTALL if clicked is install else PAGE if clicked is page else None


class DownloadRelay(QObject):
    """Lleva progreso y resultado al hilo de la UI (sin padre; ver UpdateRelay)."""

    progress = Signal(object, int, int)  # (DownloadRequest, hecho, total)
    finished = Signal(object, object, str)  # (DownloadRequest, Path | None, código DOWNLOAD_*)


@dataclass(eq=False)
class DownloadRequest:
    asset: app_update.Asset
    dest_dir: Path
    fetch_stream: Callable | None = None
    cancelled: bool = field(default=False)
    # Zip portátil: tras verificarlo se extrae aquí y `finished` trae la carpeta nueva.
    extract_to: Path | None = None


_RELAY: DownloadRelay | None = None


def shared_relay() -> DownloadRelay:
    """Relay compartido; se crea en el hilo de la UI (afinidad)."""
    global _RELAY
    if _RELAY is None:
        _RELAY = DownloadRelay()
    return _RELAY


def submit(request: DownloadRequest) -> None:
    relay = shared_relay()

    def emit(signal, *args) -> None:
        try:
            signal.emit(request, *args)
        except RuntimeError:  # aplicación cerrándose
            pass

    def run() -> None:
        path: Path | None = None
        error = ""
        try:
            path = app_update.download_installer(
                request.asset, request.dest_dir,
                request.fetch_stream or app_update.default_fetch_stream,
                lambda done, total: emit(relay.progress, done, total),
                lambda: request.cancelled)
            if request.extract_to is not None and not request.cancelled:
                try:
                    path = portable_update.extract_release(path, request.extract_to)
                except (portable_update.PortableUpdateError, zipfile.BadZipFile, OSError):
                    path, error = None, BAD_ZIP
        except app_update.DownloadError as e:  # incluye cancelación y verificación
            error = e.code
        except Exception:  # p. ej. no se pudo crear la carpeta de destino
            error = app_update.DOWNLOAD_NETWORK
        emit(relay.finished, path, error)

    threading.Thread(target=run, name="jase-app-download", daemon=True).start()
