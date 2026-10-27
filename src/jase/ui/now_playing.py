"""Diálogo no modal «Ahora se juega»: los servidores con gente de todos tus juegos."""
from __future__ import annotations

from collections.abc import Mapping

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QAbstractItemView,
    QDialog,
    QDialogButtonBox,
    QHeaderView,
    QLabel,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from jase.games import GAMES
from jase.i18n import tr
from jase.models import ServerAddress, ServerInfo

MAX_ROWS = 300
MIN_PEOPLE = 1  # un servidor solo con bots no cuenta


def busy_servers(lists: Mapping[str, list[ServerInfo]],
                 limit: int = MAX_ROWS) -> list[tuple[str, ServerInfo]]:
    """(juego, servidor) con al menos MIN_PEOPLE personas, de más a menos gente
    (a igualdad, menor ping)."""
    rows = [(game_id, info) for game_id, servers in lists.items() for info in servers
            if info.online and info.humans >= MIN_PEOPLE]
    rows.sort(key=lambda r: (-r[1].humans, r[1].ping_ms))
    return rows[:limit]


def _number(value: int) -> QTableWidgetItem:
    item = QTableWidgetItem()
    item.setData(Qt.ItemDataRole.DisplayRole, value)
    return item


class NowPlayingDialog(QDialog):
    """Tabla de servidores con gente; doble clic/Enter emite `server_chosen` (game_id,
    ServerAddress). «Actualizar» emite `refresh_requested`; quien lo abrió hace los
    escaneos y llama a `set_lists`/`set_status`."""

    server_chosen = Signal(str, object)
    refresh_requested = Signal()

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle(tr("Ahora se juega"))
        self.resize(760, 480)
        self._rows: list[tuple[str, ServerAddress]] = []
        self.table = QTableWidget(0, 5)
        self.table.setHorizontalHeaderLabels(
            [tr("Juego"), tr("Servidor"), tr("Jugadores"), tr("Mapa"), tr("Ping")])
        self.table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self.table.verticalHeader().setVisible(False)
        self.table.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        self.table.cellDoubleClicked.connect(self._choose_row)
        self.table.itemActivated.connect(lambda item: self._choose_row(item.row()))
        self.status = QLabel()
        self.status.setWordWrap(True)
        self.refresh_button = QPushButton(tr("Actualizar"))
        self.refresh_button.clicked.connect(self.refresh_requested)
        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Close)
        buttons.button(QDialogButtonBox.StandardButton.Close).setText(tr("Cerrar"))
        buttons.addButton(self.refresh_button, QDialogButtonBox.ButtonRole.ActionRole)
        buttons.rejected.connect(self.close)
        layout = QVBoxLayout(self)
        layout.addWidget(QLabel(tr("Servidores con gente en tus juegos instalados, de más a "
                                   "menos jugadores (sin contar bots). Doble clic para verlo.")))
        layout.addWidget(self.table, 1)
        layout.addWidget(self.status)
        layout.addWidget(buttons)

    def set_lists(self, lists: Mapping[str, list[ServerInfo]]) -> None:
        """Vuelve a llenar la tabla conservando la fila elegida."""
        selected = self._current()
        rows = busy_servers(lists)
        self.table.setRowCount(len(rows))
        self._rows = []
        for r, (game_id, info) in enumerate(rows):
            game = GAMES.get(game_id)
            players = (f"{info.humans}+{info.bots}/{info.max_players}" if info.bots
                       else f"{info.num_players}/{info.max_players}")
            name = QTableWidgetItem(game.short_name if game else game_id)
            name.setToolTip(game.name if game else game_id)
            name.setData(Qt.ItemDataRole.UserRole, r)
            server = QTableWidgetItem(info.name)
            server.setToolTip(info.name)
            for c, item in enumerate((name, server, QTableWidgetItem(players),
                                      QTableWidgetItem(info.map),
                                      _number(info.ping_ms))):
                self.table.setItem(r, c, item)
            self._rows.append((game_id, info.address))
        if selected in self._rows:
            self.table.selectRow(self._rows.index(selected))

    def set_status(self, text: str) -> None:
        self.status.setText(text)

    def set_busy(self, busy: bool) -> None:
        self.refresh_button.setEnabled(not busy)

    def _current(self) -> tuple[str, ServerAddress] | None:
        rows = self.table.selectionModel().selectedRows()
        return self._rows[rows[0].row()] if rows else None

    def _choose_row(self, row: int, _column: int = 0) -> None:
        if 0 <= row < len(self._rows):
            self.server_chosen.emit(*self._rows[row])
