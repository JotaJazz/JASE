"""Ficha del servidor seleccionado: imagen del mapa y datos principales."""
from __future__ import annotations

import html

from PySide6.QtCore import QSize, Qt, Signal
from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import (
    QApplication,
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QSizePolicy,
    QStackedLayout,
    QVBoxLayout,
    QWidget,
)

from jase.models import ServerAddress, ServerInfo
from jase.ui import theme
from jase.ui.colored_text import color_runs
from jase.ui.flags import flag_pixmap

MAP_IMAGE_SIZE = QSize(160, 120)
CARD_FLAG_SIZE = QSize(24, 18)


class ServerCard(QFrame):
    copy_requested = Signal(str)
    # Clic en la estrella: la ventana agrega/quita y luego llama a set_favorite().
    favorite_toggle_requested = Signal()

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self.setObjectName("server_card")
        self._info: ServerInfo | None = None
        self._identity: tuple[ServerAddress, str] | None = None

        self.empty_label = QLabel("Selecciona un servidor")
        self.empty_label.setObjectName("card_empty")
        self.empty_label.setAlignment(Qt.AlignCenter)

        self.map_label = QLabel()
        self.map_label.setObjectName("map_image")
        self.map_label.setFixedSize(MAP_IMAGE_SIZE)
        self.map_label.setAlignment(Qt.AlignCenter)
        self.map_label.setWordWrap(True)

        self.name_label = QLabel()
        self.name_label.setObjectName("card_name")
        self.name_label.setTextFormat(Qt.RichText)
        self.name_label.setSizePolicy(QSizePolicy.Ignored, QSizePolicy.Preferred)
        font = self.name_label.font()
        font.setBold(True)
        font.setPointSizeF(font.pointSizeF() * 1.15)
        self.name_label.setFont(font)

        self.lock_label = QLabel("🔒 Contraseña")
        self.lock_label.setObjectName("card_lock")
        self.lock_label.setToolTip("El servidor requiere contraseña")

        self.favorite_button = QPushButton("☆")
        self.favorite_button.setObjectName("favorite_toggle")
        self.favorite_button.setFlat(True)
        self.favorite_button.clicked.connect(self.favorite_toggle_requested.emit)
        self.set_favorite(False)

        self.address_label = QLabel()
        self.address_label.setObjectName("card_address")
        self.address_label.setTextInteractionFlags(Qt.TextSelectableByMouse)
        self.copy_button = QPushButton("Copiar")
        self.copy_button.setObjectName("copy_address")
        self.copy_button.setToolTip("Copiar IP:puerto al portapapeles")
        self.copy_button.clicked.connect(self._copy_address)

        self.country_flag = QLabel()
        self.country_flag.setObjectName("card_country_flag")
        self.country_flag.setFixedSize(CARD_FLAG_SIZE)
        self.country_flag.hide()
        self.country_label = QLabel()
        self.country_label.setObjectName("card_country")
        self.country_label.hide()  # visible cuando la base de países conoce la IP

        self.map_value = self._value("card_map")
        self.mode_value = self._value("card_mode")
        self.players_value = self._value("card_players")
        self.ping_value = self._value("card_ping")
        self.version_value = self._value("card_version")

        name_row = QHBoxLayout()
        name_row.setContentsMargins(0, 0, 0, 0)
        name_row.addWidget(self.name_label, 1)
        name_row.addWidget(self.lock_label)
        name_row.addWidget(self.favorite_button)

        address_row = QHBoxLayout()
        address_row.setContentsMargins(0, 0, 0, 0)
        address_row.addWidget(self.address_label)
        address_row.addWidget(self.copy_button)
        address_row.addSpacing(12)
        address_row.addWidget(self.country_flag)
        address_row.addWidget(self.country_label)
        address_row.addStretch(1)

        grid = QGridLayout()
        grid.setContentsMargins(0, 0, 0, 0)
        grid.setHorizontalSpacing(10)
        grid.setVerticalSpacing(4)
        fields = (("Mapa", self.map_value, 0, 0), ("Modo", self.mode_value, 0, 2),
                  ("Jugadores", self.players_value, 1, 0), ("Ping", self.ping_value, 1, 2),
                  ("Versión", self.version_value, 2, 0))
        for caption, value, row, col in fields:
            cap = QLabel(caption)
            cap.setProperty("role", "caption")
            grid.addWidget(cap, row, col)
            grid.addWidget(value, row, col + 1)
        grid.setColumnMinimumWidth(1, 200)
        grid.setColumnStretch(3, 1)

        text_col = QVBoxLayout()
        text_col.setContentsMargins(0, 0, 0, 0)
        text_col.setSpacing(4)
        text_col.addLayout(name_row)
        text_col.addLayout(address_row)
        text_col.addLayout(grid)
        text_col.addStretch(1)

        details = QWidget()
        details.setObjectName("card_details")
        details_layout = QHBoxLayout(details)
        details_layout.setContentsMargins(0, 0, 0, 0)
        details_layout.setSpacing(12)
        details_layout.addWidget(self.map_label, 0, Qt.AlignTop)
        details_layout.addLayout(text_col, 1)

        self._stack = QStackedLayout()
        self._stack.addWidget(self.empty_label)
        self._stack.addWidget(details)
        outer = QVBoxLayout(self)
        outer.setContentsMargins(8, 8, 8, 8)
        outer.addLayout(self._stack)
        self.setFixedHeight(MAP_IMAGE_SIZE.height() + 18)
        self.set_server(None, "dark")

    @staticmethod
    def _value(name: str) -> QLabel:
        label = QLabel()
        label.setObjectName(name)
        label.setSizePolicy(QSizePolicy.Ignored, QSizePolicy.Preferred)
        return label

    # ------------------------------------------------------------ API
    def set_server(self, info: ServerInfo | None, mode: str) -> None:
        """Muestra `info` (None = estado vacío). Conserva la imagen si es el mismo
        servidor y mapa; si cambió, vuelve al marcador hasta que llegue la nueva."""
        self._info = info
        identity = (info.address, info.map) if info else None
        if identity != self._identity:
            self._identity = identity
            self.set_map_image(None)
        if info is None:
            self._stack.setCurrentWidget(self.empty_label)
            return
        self._stack.setCurrentIndex(1)
        self.name_label.setText(self._name_html(info, mode))
        self.name_label.setToolTip(info.name)
        self.address_label.setText(str(info.connect_address))
        self.version_value.setText(info.version or "—")
        self.lock_label.setVisible(info.password)
        if not info.online:  # favorito sin respuesta: sin datos que mostrar
            for label in (self.map_value, self.mode_value, self.players_value):
                label.setText("—")
            self.ping_value.setText("Sin respuesta")
            self.ping_value.setStyleSheet(f"color: {theme.TOKENS[mode]['muted']};")
            return
        self.map_value.setText(info.map)
        self.mode_value.setText(info.gametype)
        self.players_value.setText(f"{info.num_players} / {info.max_players}")
        self.ping_value.setText(f"{info.ping_ms} ms")
        self.ping_value.setStyleSheet(
            f"color: {theme.ping_color(info.ping_ms, mode).name()}; font-weight: 600;")

    def set_favorite(self, favorite: bool) -> None:
        """Estrella llena (★) si el servidor mostrado es favorito del juego."""
        button = self.favorite_button
        button.setText("★" if favorite else "☆")
        button.setToolTip("Quitar de favoritos" if favorite else "Agregar a favoritos")
        button.setProperty("favorite", favorite)
        button.style().unpolish(button)
        button.style().polish(button)

    def set_map_image(self, pixmap: QPixmap | None) -> None:
        if pixmap is None or pixmap.isNull():
            self.map_label.setPixmap(QPixmap())
            self.map_label.setText(self._info.map if self._info else "")
            self.map_label.setProperty("placeholder", True)
        else:
            self.map_label.setText("")
            self.map_label.setPixmap(pixmap.scaled(
                MAP_IMAGE_SIZE, Qt.KeepAspectRatio, Qt.SmoothTransformation))
            self.map_label.setProperty("placeholder", False)
        # Reaplica el estilo para que el selector [placeholder] se reevalúe.
        self.map_label.style().unpolish(self.map_label)
        self.map_label.style().polish(self.map_label)

    def set_country(self, code: str | None, name: str | None) -> None:
        if not name:
            self.country_flag.hide()
            self.country_label.hide()
            return
        pixmap = flag_pixmap(code, CARD_FLAG_SIZE)
        if pixmap is None:
            self.country_flag.hide()
        else:
            self.country_flag.setPixmap(pixmap)
            self.country_flag.show()
        self.country_label.setText(name)
        self.country_label.show()

    # ---------------------------------------------------------- interno
    @staticmethod
    def _name_html(info: ServerInfo, mode: str) -> str:
        default = theme.TOKENS[mode]["text"]
        if not info.raw_name:
            return f'<span style="color:{default}">{html.escape(info.name)}</span>'
        return "".join(
            f'<span style="color:{(color.name() if color else default)}">'
            f"{html.escape(text)}</span>"
            for text, color in color_runs(info.raw_name, mode))

    def _copy_address(self) -> None:
        if self._info is None:
            return
        text = str(self._info.connect_address)
        clipboard = QApplication.clipboard()
        if clipboard is not None:
            clipboard.setText(text)
        self.copy_requested.emit(text)
