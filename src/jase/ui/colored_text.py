"""Nombres con códigos de color de Quake 3 / CoD (^0-^9) pintados por tramos."""
from __future__ import annotations

from PySide6.QtCore import QRect, Qt
from PySide6.QtGui import QColor, QPainter, QPalette
from PySide6.QtWidgets import QApplication, QStyle, QStyledItemDelegate, QStyleOptionViewItem

from jase.protocols import strip_control
from jase.protocols.quake3 import COLOR_RE
from jase.ui import theme

# Rol con la lista de tramos [(texto, QColor | None)]; None = color de texto normal.
COLOR_RUNS_ROLE = Qt.UserRole + 2

_ELLIPSIS = "…"


def color_runs(raw: str, mode: str) -> list[tuple[str, QColor | None]]:
    """Parte ``raw`` en tramos de color. El texto unido equivale a ``strip_colors(raw)``."""
    text = strip_control(raw)
    palette = theme.Q3_COLORS[mode]
    runs: list[tuple[str, QColor | None]] = []
    color: QColor | None = None
    pos = 0

    def push(chunk: str) -> None:
        if not chunk:
            return
        if runs and runs[-1][1] == color:
            runs[-1] = (runs[-1][0] + chunk, color)
        else:
            runs.append((chunk, color))

    for m in COLOR_RE.finditer(text):
        push(text[pos:m.start()])
        code = m.group()[-1]
        if code.isdigit():
            hex_color = palette.get(code)
            color = QColor(hex_color) if hex_color else None
        pos = m.end()
    push(text[pos:])
    return runs


def cell_text_color(color: QColor | None, opt: QStyleOptionViewItem) -> QColor:
    """Color con el que pintar texto propio de una celda (tramos ^N, ping).

    Sin color: el de texto de la vista (el de selección si la fila está seleccionada).
    En una fila seleccionada, un color que no se lee sobre la selección (p. ej. verde
    sobre el azul marino de Clásico) pasa al color de texto de la selección.
    """
    selected = bool(opt.state & QStyle.State_Selected)
    default = opt.palette.color(QPalette.HighlightedText if selected else QPalette.Text)
    if color is None:
        return default
    if selected:
        return theme.legible(color, opt.palette.color(QPalette.Highlight), default)
    return color


class ColoredTextDelegate(QStyledItemDelegate):
    """Dibuja los tramos de COLOR_RUNS_ROLE; sin tramos, pinta la celda normal."""

    def paint(self, painter: QPainter, option: QStyleOptionViewItem, index) -> None:
        runs = index.data(COLOR_RUNS_ROLE)
        if not runs:
            return super().paint(painter, option, index)
        opt = QStyleOptionViewItem(option)
        self.initStyleOption(opt, index)
        opt.text = ""
        style = opt.widget.style() if opt.widget else QApplication.style()
        style.drawControl(QStyle.CE_ItemViewItem, opt, painter, opt.widget)
        rect = style.subElementRect(QStyle.SE_ItemViewItemText, opt, opt.widget)
        fm = opt.fontMetrics
        fits = sum(fm.horizontalAdvance(t) for t, _ in runs) <= rect.width()
        limit = rect.right() - (0 if fits else fm.horizontalAdvance(_ELLIPSIS))

        painter.save()
        painter.setFont(opt.font)
        x = rect.left()
        for text, color in runs:
            painter.setPen(cell_text_color(color, opt))
            width = fm.horizontalAdvance(text)
            truncated = not fits and x + width > limit
            if truncated:
                while text and x + fm.horizontalAdvance(text) > limit:
                    text = text[:-1]
                text += _ELLIPSIS
                width = fm.horizontalAdvance(text)
            painter.drawText(QRect(x, rect.top(), width, rect.height()),
                             int(Qt.AlignVCenter | Qt.AlignLeft), text)
            x += width
            if truncated:
                break
        painter.restore()
