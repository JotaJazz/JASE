"""Diálogo no modal para buscar jugadores en las listas cargadas en la sesión."""
from __future__ import annotations

from collections.abc import Callable, Mapping

from PySide6.QtCore import QTimer, Signal
from PySide6.QtWidgets import (
    QAbstractItemView, QDialog, QLabel, QLineEdit, QTableWidget, QTableWidgetItem,
    QVBoxLayout, QWidget,
)

from jase.games import GAMES
from jase.i18n import tr, tr_n
from jase.models import ServerInfo
from jase.player_search import PlayerHit, find_players

DEBOUNCE_MS = 250


class PlayerSearchDialog(QDialog):
    """Escribe un nombre y lista coincidencias; doble clic/Enter emite `server_chosen`
    (game_id, ServerAddress) y el diálogo sigue abierto."""

    server_chosen = Signal(str, object)

    def __init__(self, source: Callable[[], Mapping[str, list[ServerInfo]]],
                 parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._source = source
        self._hits: list[PlayerHit] = []
        self.setWindowTitle(tr("Buscar jugador…"))
        self.resize(640, 420)
        layout = QVBoxLayout(self)
        self.query_edit = QLineEdit()
        self.query_edit.setClearButtonEnabled(True)
        self.query_edit.setPlaceholderText(tr("Nombre del jugador"))
        layout.addWidget(self.query_edit)
        self.table = QTableWidget(0, 4)
        self.table.setHorizontalHeaderLabels(
            [tr("Jugador"), tr("Servidor"), tr("Juego"), tr("Mapa")])
        self.table.setEditTriggers(QAbstractItemView.NoEditTriggers)
        self.table.setSelectionBehavior(QAbstractItemView.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SingleSelection)
        self.table.verticalHeader().setVisible(False)
        self.table.horizontalHeader().setStretchLastSection(True)
        layout.addWidget(self.table, 1)
        self.note = QLabel()
        self.note.setWordWrap(True)
        layout.addWidget(self.note)

        self._timer = QTimer(self)
        self._timer.setSingleShot(True)
        self._timer.setInterval(DEBOUNCE_MS)
        self._timer.timeout.connect(self.refresh)
        self.query_edit.textChanged.connect(lambda _text: self._timer.start())
        self.query_edit.returnPressed.connect(self._choose_current)
        self.table.cellDoubleClicked.connect(self._choose_row)
        self.table.itemActivated.connect(lambda item: self._choose_row(item.row()))
        self.refresh()

    def showEvent(self, event) -> None:
        super().showEvent(event)
        self.refresh()  # las listas pueden haber cambiado desde la última vez
        self.query_edit.setFocus()
        self.query_edit.selectAll()

    def refresh(self) -> None:
        lists = self._source()
        self.note.setText(tr_n("Busca en el juego cargado en esta sesión",
                               "Busca en los {n} juegos cargados en esta sesión", len(lists)))
        self._hits = find_players(lists, self.query_edit.text())
        self.table.setRowCount(len(self._hits))
        for row, hit in enumerate(self._hits):
            cells = (hit.player.name, hit.server.name, GAMES[hit.game_id].short_name,
                     hit.server.map)
            for col, text in enumerate(cells):
                self.table.setItem(row, col, QTableWidgetItem(text))
        if self._hits:
            self.table.selectRow(0)

    def _choose_current(self) -> None:
        if self._timer.isActive():  # Enter antes del debounce: no elegir sobre resultados viejos
            self._timer.stop()
            self.refresh()
        row = self.table.currentRow()
        if row >= 0:
            self._choose_row(row)

    def _choose_row(self, row: int, _column: int = 0) -> None:
        if 0 <= row < len(self._hits):
            hit = self._hits[row]
            self.server_chosen.emit(hit.game_id, hit.server.address)
