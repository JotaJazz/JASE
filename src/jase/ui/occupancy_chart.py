"""Gráfico de ocupación por hora (24 barras) para la ficha de favoritos y recientes."""
from __future__ import annotations

from PySide6.QtCore import QRectF, Qt
from PySide6.QtGui import QColor, QPainter
from PySide6.QtWidgets import QWidget

from jase.i18n import tr
from jase.ui import theme

CHART_HEIGHT = 48


class OccupancyChart(QWidget):
    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self.setObjectName("occupancy_chart")
        self.setFixedHeight(CHART_HEIGHT)
        self._values: list[float | None] = [None] * 24
        self._max_players = 0
        self._current_hour = -1
        self._mode = "dark"

    def set_data(self, values: list[float | None], max_players: int, current_hour: int,
                 mode: str) -> None:
        self._values = list(values)[:24] + [None] * (24 - len(values))
        self._max_players = max_players
        self._current_hour = current_hour
        self._mode = mode
        self.setToolTip("" if not self.has_data() else tr("Jugadores promedio por hora"))
        self.update()

    def has_data(self) -> bool:
        return any(v is not None for v in self._values)

    def bar_heights(self, full_height: float) -> list[float]:
        """Altura de cada barra (0 sin datos), proporcional a promedio / máx. jugadores."""
        top = max(self._max_players, max((v for v in self._values if v is not None), default=0))
        if top <= 0:
            return [0.0] * 24
        return [0.0 if v is None else min(1.0, v / top) * full_height for v in self._values]

    def paintEvent(self, event) -> None:
        tokens = theme.TOKENS[self._mode]
        painter = QPainter(self)
        try:
            rect = self.rect()
            if not self.has_data():
                painter.setPen(QColor(tokens["muted"]))
                painter.drawText(rect, Qt.AlignmentFlag.AlignCenter, tr("Sin historial todavía"))
                return
            slot = rect.width() / 24
            base = rect.height() - 2
            painter.setPen(Qt.PenStyle.NoPen)
            normal = QColor(tokens["occupancy"])
            normal.setAlphaF(theme.OCCUPANCY_ALPHA)
            current = QColor(tokens["accent"])
            for hour, height in enumerate(self.bar_heights(base - 2)):
                color = current if hour == self._current_hour else normal
                if height <= 0:
                    painter.setBrush(QColor(tokens["border"]))
                    height = 1.0
                else:
                    painter.setBrush(color)
                painter.drawRect(QRectF(hour * slot + 1, base - height, max(1.0, slot - 2), height))
        finally:
            painter.end()
