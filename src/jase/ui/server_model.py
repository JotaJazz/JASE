from __future__ import annotations

from PySide6.QtCore import (
    QAbstractTableModel,
    QModelIndex,
    QSortFilterProxyModel,
    Qt,
)
from PySide6.QtGui import QPainter
from PySide6.QtWidgets import QStyle, QStyledItemDelegate, QStyleOptionViewItem

from jase.games import ALL_VERSIONS
from jase.models import ServerAddress, ServerInfo
from jase.ui import theme
from jase.ui.colored_text import COLOR_RUNS_ROLE, color_runs

COLUMNS = ["", "Servidor", "Ping", "Jugadores", "Mapa", "Modo", "Versión"]
SORT_ROLE = Qt.UserRole + 1

_COL_LOCK, _COL_NAME, _COL_PING, _COL_PLAYERS, _COL_MAP, _COL_MODE, _COL_VERSION = range(7)


class ServerTableModel(QAbstractTableModel):
    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self._rows: list[ServerInfo] = []
        self._index: dict[ServerAddress, int] = {}
        self._mode = "dark"

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return 0 if parent.isValid() else len(self._rows)

    def columnCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return 0 if parent.isValid() else len(COLUMNS)

    def headerData(self, section, orientation, role=Qt.DisplayRole):
        if orientation == Qt.Horizontal and role == Qt.DisplayRole:
            return COLUMNS[section]
        return None

    def data(self, index: QModelIndex, role: int = Qt.DisplayRole):
        if not index.isValid() or not (0 <= index.row() < len(self._rows)):
            return None
        info = self._rows[index.row()]
        col = index.column()
        if role == Qt.DisplayRole:
            return self._display(info, col)
        if role == SORT_ROLE:
            if col == _COL_PING:
                return info.ping_ms
            if col == _COL_PLAYERS:
                return info.num_players
            return str(self._display(info, col)).lower()
        if role == Qt.ForegroundRole and col == _COL_PING:
            return theme.ping_color(info.ping_ms, self._mode)
        if role == COLOR_RUNS_ROLE and col == _COL_NAME and info.raw_name:
            return color_runs(info.raw_name, self._mode)
        if role == Qt.ToolTipRole and col in (_COL_NAME, _COL_MAP, _COL_MODE):
            return self._display(info, col)
        return None

    @staticmethod
    def _display(info: ServerInfo, col: int):
        if col == _COL_LOCK:
            return "🔒" if info.password else ""
        if col == _COL_NAME:
            return info.name
        if col == _COL_PING:
            return info.ping_ms
        if col == _COL_PLAYERS:
            return f"{info.num_players}/{info.max_players}"
        if col == _COL_MAP:
            return info.map
        if col == _COL_MODE:
            return info.gametype
        return info.version

    def add_or_update(self, info: ServerInfo) -> None:
        row = self._index.get(info.address)
        if row is None:
            row = len(self._rows)
            self.beginInsertRows(QModelIndex(), row, row)
            self._rows.append(info)
            self._index[info.address] = row
            self.endInsertRows()
        else:
            self._rows[row] = info
            self.dataChanged.emit(self.index(row, 0), self.index(row, len(COLUMNS) - 1))

    def clear(self) -> None:
        self.beginResetModel()
        self._rows.clear()
        self._index.clear()
        self.endResetModel()

    def server_at(self, row: int) -> ServerInfo:
        return self._rows[row]

    def servers(self) -> list[ServerInfo]:
        return list(self._rows)

    def set_mode(self, mode: str) -> None:
        self._mode = mode
        if self._rows:
            self.dataChanged.emit(
                self.index(0, _COL_NAME),
                self.index(len(self._rows) - 1, _COL_PING),
                [Qt.ForegroundRole, COLOR_RUNS_ROLE],
            )


class PingDelegate(QStyledItemDelegate):
    """Pinta el ping con su color también en la fila seleccionada.

    La hoja de estilos aplica ``selection-color`` a todo el texto seleccionado
    y taparía el ForegroundRole, así que el texto se dibuja aparte.
    """

    def paint(self, painter: QPainter, option: QStyleOptionViewItem, index) -> None:
        opt = QStyleOptionViewItem(option)
        self.initStyleOption(opt, index)
        text, opt.text = opt.text, ""
        style = opt.widget.style() if opt.widget else None
        if style is None:
            return super().paint(painter, option, index)
        style.drawControl(QStyle.CE_ItemViewItem, opt, painter, opt.widget)
        rect = style.subElementRect(QStyle.SE_ItemViewItemText, opt, opt.widget)
        color = index.data(Qt.ForegroundRole)
        painter.save()
        if color is not None:
            painter.setPen(color)
        painter.drawText(rect, int(Qt.AlignVCenter | Qt.AlignLeft), text)
        painter.restore()


class ServerFilterProxy(QSortFilterProxyModel):
    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self._text = ""
        self._version = ALL_VERSIONS
        self.setSortRole(SORT_ROLE)
        self.setDynamicSortFilter(True)

    def set_text(self, text: str) -> None:
        self.beginFilterChange()
        self._text = text.strip().lower()
        self.endFilterChange()

    def set_version(self, version: str) -> None:
        self.beginFilterChange()
        self._version = version
        self.endFilterChange()

    def filterAcceptsRow(self, source_row: int, source_parent: QModelIndex) -> bool:
        info = self.sourceModel().server_at(source_row)
        if self._version != ALL_VERSIONS and info.version != self._version:
            return False
        if self._text:
            haystack = f"{info.name}\n{info.map}\n{info.gametype}".lower()
            return self._text in haystack
        return True
