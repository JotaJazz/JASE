"""Árbol de juegos: cada juego con sus carpetas «Mods» y «Mapas» de hojas marcables."""
from __future__ import annotations

from PySide6.QtCore import QModelIndex, QPersistentModelIndex, QPointF, QRect, Qt, Signal
from PySide6.QtGui import QColor, QIcon, QLinearGradient, QPainter, QPolygonF
from PySide6.QtWidgets import QAbstractItemView, QTreeWidget, QTreeWidgetItem

from ..browse import BASE_MOD, Breakdown
from jase.i18n import tr
from jase.ui import theme

ROLE_KEY = Qt.ItemDataRole.UserRole        # valor de la hoja (mod o mapa)
ROLE_GAME = Qt.ItemDataRole.UserRole + 1   # id del juego dueño de cualquier nodo
ROLE_KIND = Qt.ItemDataRole.UserRole + 2   # "game" | "mods" | "maps" | "leaf" | "other"



def _child(item: QTreeWidgetItem, index: int) -> QTreeWidgetItem:
    child = item.child(index)
    assert child is not None  # el índice viene de childCount()
    return child


def _folders() -> tuple[tuple[str, str], ...]:
    return (("mods", tr("Mods")), ("maps", tr("Mapas")))


class GameTree(QTreeWidget):
    game_activated = Signal(str)
    marks_changed = Signal(str)

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self.setHeaderHidden(True)
        self.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.setTextElideMode(Qt.TextElideMode.ElideMiddle)
        self.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self._ids: list[str] = []
        self._items: dict[str, QTreeWidgetItem] = {}
        self._labels: dict[str, tuple[str, str]] = {}
        self._icons: dict[str, QIcon] = {}
        self._breakdown: dict[str, Breakdown | None] = {}
        self._marks: dict[str, dict[str, set[str]]] = {}
        self._expanded: set[tuple[str, str]] = set()
        self._current: str | None = None
        self._busy = False  # evita reentrada de itemChanged en cambios programáticos
        self._programmatic = False  # el cambio de ítem actual no viene del usuario
        self._mode: str | None = None  # tema actual: las ramas se dibujan con sus colores
        self.itemClicked.connect(self._on_clicked)
        self.itemActivated.connect(self._on_clicked)  # Enter / doble clic
        self.itemChanged.connect(self._on_item_changed)
        self.currentItemChanged.connect(self._on_current_changed)  # flechas del teclado

    # --- API ---
    def game_ids(self) -> list[str]:
        return list(self._ids)

    def game_item(self, game_id: str) -> QTreeWidgetItem:
        return self._items[game_id]

    def marks(self, game_id: str) -> tuple[frozenset[str], frozenset[str]]:
        m = self._marks.get(game_id, {})
        return frozenset(m.get("mods", ())), frozenset(m.get("maps", ()))

    def set_games(self, game_ids: list[str]) -> None:
        self._snapshot_expansion()
        self._busy = True
        try:
            self.clear()
            self._ids = list(game_ids)
            self._items = {}
            for gid in self._ids:
                item = QTreeWidgetItem(self)
                item.setData(0, ROLE_GAME, gid)
                item.setData(0, ROLE_KIND, "game")
                self._items[gid] = item
                self._apply_label(gid)
                if gid in self._icons:
                    item.setIcon(0, self._icons[gid])
                self._render(gid)
                item.setExpanded((gid, "game") in self._expanded)
        finally:
            self._busy = False
        if self._current in self._items:
            self._set_current_item(self._items[self._current])

    def set_game_label(self, game_id: str, text: str, tooltip: str) -> None:
        self._labels[game_id] = (text, tooltip)
        self._busy = True
        try:
            self._apply_label(game_id)
        finally:
            self._busy = False

    def set_game_icon(self, game_id: str, icon: QIcon) -> None:
        self._icons[game_id] = icon
        if game_id in self._items:
            self._items[game_id].setIcon(0, icon)

    def set_mode(self, mode: str) -> None:
        """Tema actual: 98/XP con líneas punteadas y +/−; los planos, flechas del tema."""
        self._mode = mode if mode in theme.TOKENS else None
        self.viewport().update()

    def drawBranches(self, painter: QPainter, rect: QRect,
                     index: QModelIndex | QPersistentModelIndex) -> None:
        if self._mode is None:
            super().drawBranches(painter, rect, index)
            return
        t = theme.TOKENS[self._mode]
        if theme.STYLE[self._mode] == "flat":
            self._flat_arrow(painter, t, rect, index)
            return
        painter.save()
        painter.fillRect(rect, QColor(t["surface"]))
        indent = self.indentation()
        mid_y = rect.top() + rect.height() // 2
        model = index.model()
        # Columnas de los ancestros: línea vertical si al ancestro le sigue un hermano.
        chain = []
        node = index
        while node.isValid():
            chain.append(node)
            node = node.parent()
        chain.reverse()  # raíz … ítem
        right = rect.right() + 1
        dots = QColor(t["tree_line"])
        for depth, node in enumerate(chain):
            x = right - (len(chain) - depth) * indent + indent // 2
            has_next = node.row() + 1 < model.rowCount(node.parent())
            if depth < len(chain) - 1:
                if has_next:
                    self._dotted(painter, dots, x, rect.top(), x, rect.bottom())
                continue
            first_root = not node.parent().isValid() and node.row() == 0
            top = mid_y if first_root else rect.top()
            bottom = rect.bottom() if has_next else mid_y
            self._dotted(painter, dots, x, top, x, bottom)
            self._dotted(painter, dots, x, mid_y, x + indent // 2, mid_y)
            if model.hasChildren(node):
                self._branch_box(painter, t, x, mid_y, self.isExpanded(node))
        painter.restore()

    def _flat_arrow(self, painter: QPainter, t: dict[str, str], rect: QRect,
                    index: QModelIndex | QPersistentModelIndex) -> None:
        """Temas planos: solo la flecha (▸ cerrado, ▾ abierto) con el color atenuado.

        Antes tapa con el fondo del árbol la selección que Qt pinta bajo la rama.
        """
        painter.fillRect(rect, QColor(t["sidebar"]))
        if not index.model().hasChildren(index):
            return
        indent = self.indentation()
        cx = rect.right() + 1 - indent // 2
        cy = rect.top() + rect.height() // 2
        if self.isExpanded(index):
            points = [QPointF(cx - 4, cy - 2), QPointF(cx + 4, cy - 2), QPointF(cx, cy + 3)]
        else:
            points = [QPointF(cx - 2, cy - 4), QPointF(cx - 2, cy + 4), QPointF(cx + 3, cy)]
        painter.save()
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(t["muted"]))
        painter.drawPolygon(QPolygonF(points))
        painter.restore()

    @staticmethod
    def _dotted(painter: QPainter, color: QColor, x1: int, y1: int, x2: int, y2: int) -> None:
        """Línea de puntos alternos alineada a la rejilla de píxeles (como en 98)."""
        painter.setPen(color)
        for x in range(x1, x2 + 1):
            for y in range(y1, y2 + 1):
                if (x + y) % 2 == 0:
                    painter.drawPoint(x, y)

    @staticmethod
    def _branch_box(painter: QPainter, t: dict[str, str], cx: int, cy: int,
                    expanded: bool) -> None:
        """Cuadro de 9 px con − (abierto) o + (cerrado); en XP, con degradé."""
        box = QRect(cx - 4, cy - 4, 8, 8)  # drawRect suma 1 px: 9×9
        fill = QLinearGradient(box.topLeft(), box.bottomRight())
        fill.setColorAt(0, QColor(t["surface"]))
        fill.setColorAt(1, QColor(t["tree_box_bottom"]))
        painter.setPen(QColor(t["tree_box"]))
        painter.setBrush(fill)
        painter.drawRect(box)
        painter.setPen(QColor(t["text"]))
        painter.drawLine(cx - 2, cy, cx + 2, cy)
        if not expanded:
            painter.drawLine(cx, cy - 2, cx, cy + 2)

    def set_breakdown(self, game_id: str, b: Breakdown | None, prune: bool = True) -> None:
        """`None` quita las carpetas pero nunca las marcas. `prune=False` conserva las marcas
        de valores ausentes como hojas con 0 (p. ej. mientras se refresca la lista)."""
        marks = self._marks.setdefault(game_id, {"mods": set(), "maps": set()})
        dropped = False
        if b is not None:
            fields = {}
            for kind in ("mods", "maps"):
                shown = list(getattr(b, kind))
                gone = marks[kind] - {v for v, _ in shown}
                if prune:
                    if gone:
                        marks[kind] -= gone
                        dropped = True
                else:
                    shown += [(v, 0) for v in sorted(gone)]
                fields[kind] = tuple(shown)
            b = Breakdown(fields["mods"], fields["maps"], b.other_maps)
        self._breakdown[game_id] = b
        if game_id in self._items:
            self._snapshot_expansion()
            self._busy = True
            try:
                self._render(game_id)
            finally:
                self._busy = False
        if dropped:
            self.marks_changed.emit(game_id)

    def set_current_game(self, game_id: str | None) -> None:
        self._current = game_id
        self._set_current_item(self._items.get(game_id) if game_id is not None else None)

    def clear_marks(self, game_id: str) -> None:
        marks = self._marks.get(game_id)
        if marks:
            marks["mods"].clear()
            marks["maps"].clear()
        if game_id in self._items:
            self._busy = True
            try:
                self._render(game_id)
            finally:
                self._busy = False
        self.marks_changed.emit(game_id)

    # --- internos ---
    def _set_current_item(self, item: QTreeWidgetItem | None) -> None:
        self._programmatic = True
        try:
            self.setCurrentItem(item)  # type: ignore[arg-type]  # None limpia la selección
        finally:
            self._programmatic = False

    def _on_current_changed(self, item: QTreeWidgetItem | None, _previous) -> None:
        if self._programmatic or self._busy or item is None:
            return
        if item.data(0, ROLE_KIND) == "game":  # mover el foco a un juego lo selecciona
            self._on_clicked(item)

    def _apply_label(self, gid: str) -> None:
        item = self._items.get(gid)
        if item is None:
            return
        text, tip = self._labels.get(gid, (gid, ""))
        item.setText(0, text)
        item.setToolTip(0, tip)

    def _snapshot_expansion(self) -> None:
        for gid, item in self._items.items():
            self._note(gid, "game", item.isExpanded())
            for i in range(item.childCount()):
                child = _child(item, i)
                kind = child.data(0, ROLE_KIND)
                if kind in ("mods", "maps"):
                    self._note(gid, kind, child.isExpanded())

    def _note(self, gid: str, kind: str, expanded: bool) -> None:
        (self._expanded.add if expanded else self._expanded.discard)((gid, kind))

    def _layout(self, gid: str) -> list[tuple[str, list[str], int]]:
        """(carpeta, claves de hojas, otros) esperados para el breakdown actual."""
        b = self._breakdown.get(gid)
        if b is None:
            return []
        out = []
        if b.mods:
            out.append(("mods", [v for v, _ in b.mods], 0))
        out.append(("maps", [v for v, _ in b.maps], b.other_maps))
        return out

    def _current_shape(self, gid: str) -> list[tuple[str, list[str], int]]:
        shape = []
        for i in range(self._items[gid].childCount()):
            folder = _child(self._items[gid], i)
            keys = [_child(folder, j).data(0, ROLE_KEY) for j in range(folder.childCount())
                    if _child(folder, j).data(0, ROLE_KIND) == "leaf"]
            has_other = any(_child(folder, j).data(0, ROLE_KIND) == "other"
                            for j in range(folder.childCount()))
            shape.append((folder.data(0, ROLE_KIND), keys, 1 if has_other else 0))
        return shape

    def _render(self, gid: str) -> None:
        """Deja los hijos del juego según breakdown y marcas; en sitio si las claves no cambian."""
        item = self._items[gid]
        b = self._breakdown.get(gid)
        if b is None:
            item.takeChildren()
            return
        layout = self._layout(gid)
        same = ([(k, keys, 1 if o > 0 else 0) for k, keys, o in layout]
                == self._current_shape(gid))
        marks = self._marks.get(gid, {"mods": set(), "maps": set()})
        if not same:
            item.takeChildren()
            for kind, label in _folders():
                if not any(k == kind for k, _, _ in layout):
                    continue
                folder = QTreeWidgetItem(item, [label])
                folder.setData(0, ROLE_GAME, gid)
                folder.setData(0, ROLE_KIND, kind)
                for value, _n in getattr(b, kind):
                    leaf = QTreeWidgetItem(folder)
                    leaf.setFlags((leaf.flags() | Qt.ItemFlag.ItemIsUserCheckable)
                                  & ~Qt.ItemFlag.ItemIsAutoTristate)
                    leaf.setData(0, ROLE_KEY, value)
                    leaf.setData(0, ROLE_GAME, gid)
                    leaf.setData(0, ROLE_KIND, "leaf")
                if kind == "maps" and b.other_maps > 0:
                    other = QTreeWidgetItem(folder)
                    other.setFlags(other.flags() & ~Qt.ItemFlag.ItemIsUserCheckable)
                    other.setData(0, ROLE_GAME, gid)
                    other.setData(0, ROLE_KIND, "other")
                folder.setExpanded((gid, kind) in self._expanded)
        # textos y casillas (también cambian los conteos sin cambiar las claves)
        for i in range(item.childCount()):
            folder = _child(item, i)
            kind = folder.data(0, ROLE_KIND)
            counts = dict(getattr(b, kind))
            for j in range(folder.childCount()):
                leaf = _child(folder, j)
                if leaf.data(0, ROLE_KIND) == "other":
                    leaf.setText(0, f"{tr('otros…')} · {b.other_maps}")
                    continue
                value = leaf.data(0, ROLE_KEY)
                shown = tr("(base)") if kind == "mods" and value == BASE_MOD else value
                leaf.setText(0, f"{shown} · {counts[value]}")
                leaf.setCheckState(
                    0, Qt.CheckState.Checked if value in marks[kind] else Qt.CheckState.Unchecked)

    def _on_clicked(self, item: QTreeWidgetItem, _col: int = 0) -> None:
        gid = item.data(0, ROLE_GAME)
        if gid is None:
            return
        # nodos internos solo cambian de juego si pertenecen a otro
        if item.data(0, ROLE_KIND) == "game" or gid != self._current:
            self._current = gid
            self.game_activated.emit(gid)

    def _on_item_changed(self, item: QTreeWidgetItem, _col: int) -> None:
        if self._busy or item.data(0, ROLE_KIND) != "leaf":
            return
        gid = item.data(0, ROLE_GAME)
        parent = item.parent()
        if parent is None:
            return
        kind = parent.data(0, ROLE_KIND)
        marks = self._marks.setdefault(gid, {"mods": set(), "maps": set()})
        value = item.data(0, ROLE_KEY)
        if item.checkState(0) == Qt.CheckState.Checked:
            marks[kind].add(value)
        else:
            marks[kind].discard(value)
        if gid != self._current:
            self._current = gid
            self.game_activated.emit(gid)
        self.marks_changed.emit(gid)
