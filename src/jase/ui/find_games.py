"""Diálogo «Buscar juegos»: la búsqueda corre en un hilo daemon (nunca en la UI)."""
from __future__ import annotations

import os
import threading
from collections.abc import Callable
from dataclasses import dataclass, field

from PySide6.QtCore import QObject, Qt, Signal
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from jase import detect
from jase.config import Config
from jase.detect import Finding
from jase.games import GAMES
from jase.i18n import tr, tr_n


def _source_label(source: str) -> str:
    """Origen del hallazgo para mostrar (``detect.SOURCES`` conserva el español para ordenar)."""
    if source == "registro":
        return tr("registro")
    if source == "steam":
        return tr("steam")
    if source == "carpeta típica":
        return tr("carpeta típica")
    if source == "carpeta elegida":
        return tr("carpeta elegida")
    return source


class FindGamesRelay(QObject):
    """Lleva el resultado al hilo de la UI (sin padre; ver CountryDBRelay)."""

    found = Signal(object, object)  # (FindRequest, dict[str, list[Finding]])


@dataclass(eq=False)
class FindRequest:
    extra_root: str | None = None
    cancelled: bool = field(default=False)


_RELAY: FindGamesRelay | None = None


def shared_relay() -> FindGamesRelay:
    """Relay compartido; se crea en el hilo de la UI (afinidad)."""
    global _RELAY
    if _RELAY is None:
        _RELAY = FindGamesRelay()
    return _RELAY


def submit(request: FindRequest, find_fn: Callable) -> None:
    """Ejecuta `find_fn` en un hilo daemon y emite `shared_relay().found` al terminar
    (nada si la petición se canceló)."""
    relay = shared_relay()

    def run() -> None:
        try:
            result = find_fn(steam_roots=detect.default_steam_roots(),
                             drives=detect.default_drives(),
                             extra_root=request.extra_root,
                             cancelled=lambda: request.cancelled)
        except Exception:  # búsqueda best-effort: un fallo equivale a no encontrar nada
            result = {}
        if request.cancelled:
            return
        try:
            relay.found.emit(request, result)
        except RuntimeError:  # aplicación cerrándose
            pass

    threading.Thread(target=run, name="jase-find-games", daemon=True).start()


class FindGamesDialog(QDialog):
    def __init__(self, cfg: Config, parent=None, find_fn=detect.find_games) -> None:
        super().__init__(parent)
        self.setWindowTitle(tr("Buscar juegos"))
        self.setMinimumSize(640, 420)
        self._cfg = cfg
        self._find_fn = find_fn
        self._request: FindRequest | None = None
        self.rows: dict[str, tuple[QCheckBox, QComboBox, QLabel]] = {}

        self.status_label = QLabel()
        self.status_label.setObjectName("find_status")
        self.cancel_search_button = QPushButton(tr("Cancelar búsqueda"))
        self.cancel_search_button.setObjectName("find_cancel_search")
        self.cancel_search_button.clicked.connect(self._cancel_search)
        self.folder_button = QPushButton(tr("Buscar en carpeta…"))
        self.folder_button.setObjectName("find_in_folder")
        self.folder_button.clicked.connect(self._choose_folder)

        top = QHBoxLayout()
        top.addWidget(self.status_label, 1)
        top.addWidget(self.cancel_search_button)
        top.addWidget(self.folder_button)

        self._grid = QGridLayout()
        self._grid.setColumnStretch(2, 1)
        body = QWidget()
        body_layout = QVBoxLayout(body)
        body_layout.addLayout(self._grid)
        body_layout.addStretch(1)
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.Shape.NoFrame)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        scroll.setWidget(body)

        std = QDialogButtonBox.StandardButton
        buttons = QDialogButtonBox(std.Ok | std.Cancel)
        buttons.button(QDialogButtonBox.StandardButton.Ok).setText(tr("Aplicar"))
        buttons.button(QDialogButtonBox.StandardButton.Cancel).setText(tr("Cancelar"))
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addLayout(top)
        layout.addWidget(scroll, 1)
        layout.addWidget(buttons)

        shared_relay().found.connect(self._on_found)
        self._start(None)

    # ------------------------------------------------------------ búsqueda
    def _start(self, extra_root: str | None) -> None:
        if self._request is not None:
            self._request.cancelled = True
        self._clear_rows()
        self._request = FindRequest(extra_root)
        self.status_label.setText(tr("Buscando…"))
        self.cancel_search_button.setEnabled(True)
        submit(self._request, self._find_fn)

    def _cancel_search(self) -> None:
        if self._request is not None:
            self._request.cancelled = True
        self.status_label.setText(tr("Búsqueda cancelada"))
        self.cancel_search_button.setEnabled(False)

    def _choose_folder(self) -> None:
        folder = QFileDialog.getExistingDirectory(self, tr("Buscar juegos en la carpeta"))
        if folder:
            self._start(os.path.normpath(folder))  # sin separadores mezclados

    def done(self, result: int) -> None:
        if self._request is not None:
            self._request.cancelled = True
        try:
            shared_relay().found.disconnect(self._on_found)
        except (RuntimeError, TypeError):
            pass
        super().done(result)

    # ------------------------------------------------------------- filas
    def _clear_rows(self) -> None:
        while (item := self._grid.takeAt(0)) is not None:
            widget = item.widget()
            if widget is not None:
                widget.deleteLater()
        self.rows.clear()

    def _on_found(self, request: FindRequest, found) -> None:
        if request is not self._request or request.cancelled:
            return  # búsqueda anterior o cancelada
        self.cancel_search_button.setEnabled(False)
        self._clear_rows()
        for row, game_id in enumerate(g for g in GAMES if g in found):
            self._add_row(row, GAMES[game_id], found[game_id])
        self.status_label.setText(
            tr_n("1 juego encontrado", "{n} juegos encontrados", len(self.rows)) if self.rows
            else tr("No se encontraron juegos"))

    def _add_row(self, row: int, game, findings: list[Finding]) -> None:
        if detect.is_configured(game, self._cfg):
            status, checked = tr("ya configurado"), False
        elif self._cfg.game_paths.get(game.id):
            status, checked = tr("ruta guardada no encontrada"), True
        else:
            status, checked = tr("nuevo"), True
        check = QCheckBox()
        check.setChecked(checked)
        combo = QComboBox()
        for f in findings:
            combo.addItem(f"{f.path} ({_source_label(f.source)})", f.path)
        label = QLabel(status)
        self._grid.addWidget(check, row, 0)
        self._grid.addWidget(QLabel(game.name), row, 1)
        self._grid.addWidget(combo, row, 2)
        self._grid.addWidget(label, row, 3)
        self.rows[game.id] = (check, combo, label)

    def selected_paths(self) -> dict[str, str]:
        return {gid: combo.currentData()
                for gid, (check, combo, _) in self.rows.items() if check.isChecked()}
