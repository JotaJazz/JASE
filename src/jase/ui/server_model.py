from __future__ import annotations

from collections.abc import Callable
from typing import cast

from PySide6.QtCore import (
    QAbstractTableModel,
    QModelIndex,
    QPersistentModelIndex,
    QRect,
    QSize,
    QSortFilterProxyModel,
    Qt,
)
from PySide6.QtGui import QColor, QLinearGradient, QPainter, QPalette
from PySide6.QtWidgets import QStyle, QStyledItemDelegate, QStyleOptionViewItem

from jase.browse import matches
from jase.games import ALL_VERSIONS, GameDef
from jase.geo import country_name
from jase.i18n import tr, tr_n
from jase.models import QuickFilters, ServerAddress, ServerInfo
from jase.ui import theme
from jase.ui.colored_text import COLOR_RUNS_ROLE, cell_text_color, color_runs, plain_tip
from jase.ui.flags import flag_icon

COLUMNS = ["", "País", "Servidor", "Ping", "Jugadores", "Mapa", "Modo", "Versión"]
SORT_ROLE = Qt.ItemDataRole.UserRole + 1
OCCUPANCY_ROLE = Qt.ItemDataRole.UserRole + 2  # columna Jugadores: fracción ocupada en [0, 1]

# Índices de columna públicos: usarlos siempre en vez de números literales.
(COL_LOCK, COL_COUNTRY, COL_NAME, COL_PING, COL_PLAYERS, COL_MAP, COL_MODE,
 COL_VERSION) = range(len(COLUMNS))
# Valor de orden del ping de una fila offline: queda al final en orden ascendente.
OFFLINE_PING_SORT = 2**31 - 1
# Tamaño de la bandera en la columna País (4:3); la vista usa el mismo iconSize.
FLAG_SIZE = QSize(20, 15)


def column_title(section: int) -> str:
    """Cabecera traducida de la columna; se resuelve al pedirla (el idioma se fija después)."""
    titles = {COL_COUNTRY: tr("País"), COL_NAME: tr("Servidor"), COL_PING: tr("Ping"),
              COL_PLAYERS: tr("Jugadores"), COL_MAP: tr("Mapa"), COL_MODE: tr("Modo"),
              COL_VERSION: tr("Versión")}
    return titles.get(section, "")


def occupancy_ratio(num_players: int, max_players: int) -> float:
    """Fracción ocupada en [0, 1]; 0 si el máximo es desconocido (<= 0)."""
    if max_players <= 0:
        return 0.0
    return max(0.0, min(1.0, num_players / max_players))


def occupancy_color(mode: str) -> QColor:
    color = QColor(theme.TOKENS[mode]["occupancy"])
    color.setAlphaF(theme.OCCUPANCY_ALPHA)
    return color


class ServerTableModel(QAbstractTableModel):
    def __init__(self, parent=None) -> None:
        self.version_label = lambda version: version  # el juego actual lo reemplaza
        super().__init__(parent)
        self._rows: list[ServerInfo] = []
        self._index: dict[ServerAddress, int] = {}
        self._mode = "dark"
        self._country_lookup: Callable[[str], str | None] | None = None
        self._countries: dict[str, str | None] = {}  # host → código (caché)
        self._notes: dict[ServerAddress, str] = {}  # notas de favoritos del juego actual

    def rowCount(self, parent: QModelIndex | QPersistentModelIndex = QModelIndex()) -> int:
        return 0 if parent.isValid() else len(self._rows)

    def columnCount(self, parent: QModelIndex | QPersistentModelIndex = QModelIndex()) -> int:
        return 0 if parent.isValid() else len(COLUMNS)

    def headerData(self, section, orientation, role=Qt.ItemDataRole.DisplayRole):
        if orientation == Qt.Orientation.Horizontal and role == Qt.ItemDataRole.DisplayRole:
            return column_title(section)
        if (orientation == Qt.Orientation.Horizontal and role == Qt.ItemDataRole.ToolTipRole
                and section == COL_COUNTRY):
            return tr("País por IP: IP to Country Lite by DB-IP (db-ip.com), CC BY 4.0")
        return None

    def data(self, index: QModelIndex | QPersistentModelIndex,
             role: int = Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or not (0 <= index.row() < len(self._rows)):
            return None
        info = self._rows[index.row()]
        col = index.column()
        if role == Qt.ItemDataRole.DisplayRole:
            return self._display(info, col)
        if col == COL_COUNTRY:
            return self._country_data(info, role)
        if role == Qt.ItemDataRole.ForegroundRole and not info.online:
            return QColor(theme.TOKENS[self._mode]["muted"])
        if role == SORT_ROLE:
            if col == COL_PING:
                return info.ping_ms if info.online else OFFLINE_PING_SORT
            if col == COL_PLAYERS:  # personas primero; a igualdad, cuenta el total
                return info.humans * 1024 + min(info.num_players, 1023)
            return str(self._display(info, col)).lower()
        if role == OCCUPANCY_ROLE and col == COL_PLAYERS:
            return occupancy_ratio(info.num_players, info.max_players)
        if role == Qt.ItemDataRole.ForegroundRole and col == COL_PING:
            return theme.ping_color(info.ping_ms, self._mode)
        if role == COLOR_RUNS_ROLE and col == COL_NAME and info.raw_name:
            return color_runs(info.raw_name, self._mode)
        if role == Qt.ItemDataRole.ToolTipRole and col == COL_NAME:
            note = self._notes.get(info.address)
            if note or not info.online:
                head = (tr("{name}: el favorito no respondió", name=info.name)
                        if not info.online else info.name)
                return plain_tip(f"{head}\n{note}" if note else head)
        if role == Qt.ItemDataRole.ToolTipRole and col == COL_PLAYERS and info.bots:
            return tr_n("{n} jugador", "{n} jugadores", info.humans) + " · " + tr_n(
                "{n} bot", "{n} bots", info.bots)
        if role == Qt.ItemDataRole.ToolTipRole and col in (COL_NAME, COL_MAP, COL_MODE):
            return plain_tip(str(self._display(info, col)))
        return None

    def set_notes(self, notes: dict[ServerAddress, str]) -> None:
        """Notas de favoritos (se muestran en el tooltip de la columna Servidor)."""
        self._notes = dict(notes)
        if self._rows:
            self.dataChanged.emit(self.index(0, COL_NAME),
                                  self.index(len(self._rows) - 1, COL_NAME),
                                  [Qt.ItemDataRole.ToolTipRole])

    def _country_data(self, info: ServerInfo, role: int):
        code = self.country_code(info)
        if role == Qt.ItemDataRole.DecorationRole:
            return flag_icon(code, FLAG_SIZE) if code else None
        if role == Qt.ItemDataRole.ToolTipRole:
            return country_name(code) if code else None
        if role == SORT_ROLE:
            return country_name(code).lower() if code else ""
        if role == Qt.ItemDataRole.TextAlignmentRole:
            return int(Qt.AlignmentFlag.AlignCenter)
        return None

    def country_code(self, info: ServerInfo) -> str | None:
        """Código ISO (minúsculas) del servidor según la base cargada, o None."""
        if self._country_lookup is None:
            return None
        host = info.address.host
        if host not in self._countries:
            try:
                code = self._country_lookup(host)
            except Exception:  # base dañada o función de búsqueda rota: sin país
                code = None
            self._countries[host] = code.lower() if isinstance(code, str) and code else None
        return self._countries[host]

    def set_country_lookup(self, fn: Callable[[str], str | None] | None) -> None:
        """Función IP → código de país; re-pinta (y re-filtra) la columna País."""
        self._country_lookup = fn
        self._countries.clear()
        if self._rows:
            self.dataChanged.emit(self.index(0, COL_COUNTRY),
                                  self.index(len(self._rows) - 1, COL_COUNTRY))

    def _display(self, info: ServerInfo, col: int):
        if col == COL_LOCK:
            return "🔒" if info.password else ""
        if col == COL_COUNTRY:
            return ""  # solo bandera (DecorationRole)
        if col == COL_NAME:
            return info.name if info.online else info.name + tr(" — sin respuesta")
        if col == COL_PING:
            return info.ping_ms if info.online else "—"
        if col == COL_PLAYERS:
            if not info.online:
                return "—"
            if info.bots:  # «2+6/16»: 2 personas y 6 bots
                return f"{info.humans}+{info.bots}/{info.max_players}"
            return f"{info.num_players}/{info.max_players}"
        if col == COL_MAP:
            return info.map
        if col == COL_MODE:
            return info.gametype
        return self.version_label(info.version)

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

    def remove(self, address: ServerAddress) -> bool:
        """Quita la fila de `address`; False si no estaba."""
        row = self._index.get(address)
        if row is None:
            return False
        self.beginRemoveRows(QModelIndex(), row, row)
        del self._rows[row]
        self._index = {info.address: i for i, info in enumerate(self._rows)}
        self.endRemoveRows()
        return True

    def clear(self) -> None:
        self.beginResetModel()
        self._rows.clear()
        self._index.clear()
        self.endResetModel()

    def row_of(self, address: ServerAddress) -> int:
        """Fila de `address` en el modelo fuente; -1 si no está."""
        return self._index.get(address, -1)

    def server_at(self, row: int) -> ServerInfo:
        return self._rows[row]

    def servers(self) -> list[ServerInfo]:
        return list(self._rows)

    def set_mode(self, mode: str) -> None:
        self._mode = mode
        if self._rows:
            # Todas las columnas: las filas offline usan el color atenuado en cada una.
            self.dataChanged.emit(
                self.index(0, 0),
                self.index(len(self._rows) - 1, len(COLUMNS) - 1),
                [Qt.ItemDataRole.ForegroundRole, COLOR_RUNS_ROLE],
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
        style.drawControl(QStyle.ControlElement.CE_ItemViewItem, opt, painter, opt.widget)
        rect = style.subElementRect(QStyle.SubElement.SE_ItemViewItemText, opt, opt.widget)
        color = index.data(Qt.ItemDataRole.ForegroundRole)
        painter.save()
        if color is not None:
            painter.setPen(cell_text_color(color, opt))
        align = Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft
        painter.drawText(rect, int(align), text)
        painter.restore()


class OccupancyDelegate(QStyledItemDelegate):
    """Columna Jugadores: barra proporcional a num/max detrás del texto "n/max"."""

    RADIUS = 3
    BLOCK, BLOCK_GAP = 6, 2  # bloques de 98/XP/GoldSrc/Terminal

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self.mode = "dark"

    def set_mode(self, mode: str) -> None:
        self.mode = mode

    def paint(self, painter: QPainter, option: QStyleOptionViewItem, index) -> None:
        opt = QStyleOptionViewItem(option)
        self.initStyleOption(opt, index)
        text, opt.text = opt.text, ""
        style = opt.widget.style() if opt.widget else None
        if style is None:
            return super().paint(painter, option, index)
        style.drawControl(QStyle.ControlElement.CE_ItemViewItem, opt, painter, opt.widget)

        ratio = index.data(OCCUPANCY_ROLE) or 0.0
        rect = style.subElementRect(QStyle.SubElement.SE_ItemViewItemText, opt, opt.widget)
        selected = bool(opt.state & QStyle.StateFlag.State_Selected)
        role = QPalette.ColorRole.HighlightedText if selected else QPalette.ColorRole.Text
        align = int(Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft)
        painter.save()
        if theme.STYLE.get(self.mode) in ("win98", "luna", "vgui", "terminal"):
            self._paint_blocks(painter, opt, rect, text, ratio, opt.palette.color(role))
            painter.restore()
            return
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        if ratio > 0:
            bar = QRect(opt.rect)
            bar.setWidth(max(1, round(opt.rect.width() * ratio)))
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(occupancy_color(self.mode))
            painter.drawRoundedRect(bar, self.RADIUS, self.RADIUS)
        painter.setPen(opt.palette.color(role))
        painter.drawText(rect, align, text)
        painter.restore()

    def _paint_blocks(self, painter: QPainter, opt: QStyleOptionViewItem, rect: QRect,
                      text: str, ratio: float, ink: QColor) -> None:
        """98/XP/GoldSrc/Terminal: "n/max" y, a su derecha, una barra de progreso por bloques."""
        t = theme.TOKENS[self.mode]
        align = int(Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft)
        painter.setPen(ink)
        painter.drawText(rect, align, text)
        fm = opt.fontMetrics
        left = rect.left() + max(fm.horizontalAdvance(text), fm.horizontalAdvance("00/00")) + 6
        frame = QRect(left, opt.rect.top() + 4, opt.rect.right() - 3 - left,
                      opt.rect.height() - 8)
        if frame.width() < 12 or frame.height() < 6:
            return
        painter.fillRect(frame, QColor(t["surface"]))
        if theme.STYLE[self.mode] in ("luna", "terminal"):  # marco de línea fina
            painter.setPen(QColor(t["progress_border"]))
            painter.drawRect(frame.adjusted(0, 0, -1, -1))
        else:  # bisel hundido de 1 px (98 y GoldSrc)
            painter.setPen(QColor(t["border"]))
            painter.drawLine(frame.topLeft(), frame.topRight())
            painter.drawLine(frame.topLeft(), frame.bottomLeft())
            painter.setPen(QColor(t["bevel_inner"]))
            painter.drawLine(frame.bottomLeft(), frame.bottomRight())
            painter.drawLine(frame.topRight(), frame.bottomRight())
        inner = frame.adjusted(2, 2, -2, -2)
        fill = QLinearGradient(inner.topLeft(), inner.bottomLeft())  # verde con brillo en XP
        fill.setColorAt(0, QColor(t["progress_light"]))
        fill.setColorAt(0.45, QColor(t["progress"]))
        fill.setColorAt(1, QColor(t["progress"]))
        end = inner.left() + round(inner.width() * ratio)
        for x in range(inner.left(), end, self.BLOCK + self.BLOCK_GAP):
            painter.fillRect(QRect(x, inner.top(), min(self.BLOCK, end - x), inner.height()),
                             fill)


class ServerFilterProxy(QSortFilterProxyModel):
    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self._text = ""
        self._version = ALL_VERSIONS
        self._quick = QuickFilters()
        self._tree_game: GameDef | None = None
        self._tree_mods: frozenset[str] = frozenset()
        self._tree_maps: frozenset[str] = frozenset()
        self._tree_countries: frozenset[str] = frozenset()
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

    def set_quick_filters(self, f: QuickFilters) -> None:
        self.beginFilterChange()
        self._quick = f
        self.endFilterChange()

    def set_tree_filter(self, game: GameDef | None, mods: frozenset[str],
                        maps: frozenset[str],
                        countries: frozenset[str] = frozenset()) -> None:
        """Marcas del árbol; mods y mapas solo cuentan con `game`, los países siempre."""
        self.beginFilterChange()
        self._tree_game = game
        self._tree_mods = mods
        self._tree_maps = maps
        self._tree_countries = countries
        self.endFilterChange()

    def _passes_quick(self, info: ServerInfo, source: ServerTableModel) -> bool:
        q = self._quick
        if q.hide_empty and info.num_players == 0:
            return False
        if q.hide_bots and info.num_players > 0 and info.humans == 0:
            return False
        if q.country and source.country_code(info) != q.country:
            return False
        if q.hide_full and info.max_players > 0 and info.num_players >= info.max_players:
            return False
        if q.hide_password and info.password:
            return False
        return not (q.max_ping > 0 and info.ping_ms > q.max_ping)

    def filterAcceptsRow(self, source_row: int,
                         source_parent: QModelIndex | QPersistentModelIndex) -> bool:
        source = cast(ServerTableModel, self.sourceModel())
        info = source.server_at(source_row)
        if not self._passes_quick(info, source):
            return False
        if self._version != ALL_VERSIONS and info.version != self._version:
            return False
        if (self._tree_game is not None and (self._tree_mods or self._tree_maps)
                and not matches(self._tree_game, info, self._tree_mods, self._tree_maps)):
            return False
        if self._tree_countries and source.country_code(info) not in self._tree_countries:
            return False
        if self._text:
            code = source.country_code(info) or ""
            haystack = f"{info.name}\n{info.map}\n{info.gametype}\n{code}".lower()
            return self._text in haystack
        return True
