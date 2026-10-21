"""Diálogos: contraseña de servidor y ajustes (rutas de juegos, imágenes de mapas, países)."""
from __future__ import annotations

import os
from pathlib import Path

from PySide6.QtCore import QDir, QRect, QSize, Qt, QTimer
from PySide6.QtGui import QColor, QIcon, QPainter, QPixmap
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QScrollArea,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from jase import masters as masters_core
from jase.config import Config
from jase.games import GAMES
from jase.detect import resolve_game_exe
from jase.i18n import tr
from jase.ui import theme
from jase.ui.masters_tab import MastersContext, MastersTab

# Muestras de color de cada tema en la pestaña «Apariencia».
SWATCH = 14
SWATCH_GAP = 2
SWATCH_TOKENS = ("bg", "surface", "accent", "selection")


def theme_swatch_icon(theme_id: str) -> QIcon:
    """Cuatro cuadros con los colores del tema; «Sistema» mezcla Oscuro y Claro."""
    if theme_id in theme.TOKENS:
        colors = [theme.TOKENS[theme_id][k] for k in SWATCH_TOKENS]
    else:
        dark, light = theme.TOKENS["dark"], theme.TOKENS["light"]
        colors = [dark["bg"], dark["accent"], light["bg"], light["accent"]]
    width = len(colors) * (SWATCH + SWATCH_GAP) - SWATCH_GAP
    pixmap = QPixmap(width, SWATCH)
    pixmap.fill(Qt.transparent)
    painter = QPainter(pixmap)
    for i, color in enumerate(colors):
        rect = QRect(i * (SWATCH + SWATCH_GAP), 0, SWATCH, SWATCH)
        painter.fillRect(rect, QColor(color))
        painter.setPen(QColor("#808080"))
        painter.drawRect(rect.adjusted(0, 0, -1, -1))
    painter.end()
    icon = QIcon(pixmap)
    # Mismos colores en todos los modos y estados (sin el tinte de selección).
    for mode in (QIcon.Selected, QIcon.Active):
        for state in (QIcon.Off, QIcon.On):
            icon.addPixmap(pixmap, mode, state)
    return icon


class PasswordDialog(QDialog):
    def __init__(self, server_name: str, initial: str | None, parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle(tr("Contraseña del servidor"))
        self.forget_requested = False

        label = QLabel(tr("El servidor «{name}» requiere contraseña:", name=server_name))
        label.setWordWrap(True)
        self.edit = QLineEdit(initial or "")
        self.edit.setEchoMode(QLineEdit.Password)
        self.edit.setPlaceholderText(tr("Contraseña"))

        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.button(QDialogButtonBox.Ok).setText(tr("Conectar"))
        buttons.button(QDialogButtonBox.Cancel).setText(tr("Cancelar"))
        self.forget_button = buttons.addButton(tr("Olvidar contraseña"),
                                               QDialogButtonBox.DestructiveRole)
        self.forget_button.setVisible(bool(initial))
        self.forget_button.clicked.connect(self._forget)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addWidget(label)
        layout.addWidget(self.edit)
        layout.addWidget(buttons)

    def _forget(self) -> None:
        self.forget_requested = True
        self.reject()

    def password(self) -> str:
        return self.edit.text()


# Sin carpeta de caché: load_official cae a la lista empaquetada.
_NO_CACHE = Path(os.devnull) / "cache"
STEAM_KEY_URL = "https://steamcommunity.com/dev/apikey"


class SettingsDialog(QDialog):
    def __init__(self, cfg: Config, parent=None, on_update_countries=None,
                 on_find_games=None, masters: MastersContext | None = None,
                 steam_key: str = "") -> None:
        """`on_update_countries`: callback del botón «Actualizar base de países»
        (None = botón deshabilitado, p. ej. sin base o con una descarga en curso).
        `on_find_games`: abre «Buscar juegos» y devuelve {id: ruta} (vacío si se cancela).
        `masters`: datos para la pestaña «Masters» (None = lista empaquetada, sin cambios
        de red ni pruebas)."""
        super().__init__(parent)
        self.setWindowTitle(tr("Ajustes"))
        self.setMinimumWidth(580)
        self.path_edits: dict[str, QLineEdit] = {}

        form = QFormLayout()
        for game in GAMES.values():
            edit = QLineEdit(cfg.game_paths.get(game.id, ""))
            edit.setPlaceholderText(tr("Carpeta de instalación"))
            browse = QPushButton(tr("Examinar…"))
            browse.clicked.connect(lambda _=False, e=edit, g=game: self._browse(e, g.name))
            row = QWidget()
            row_layout = QHBoxLayout(row)
            row_layout.setContentsMargins(0, 0, 0, 0)
            row_layout.addWidget(edit, 1)
            row_layout.addWidget(browse)
            status = QLabel()
            status.setObjectName(f"path_status_{game.id}")
            cell = QWidget()
            cell_layout = QVBoxLayout(cell)
            cell_layout.setContentsMargins(0, 0, 0, 0)
            cell_layout.addWidget(row)
            cell_layout.addWidget(status)
            form.addRow(game.name, cell)
            self.path_edits[game.id] = edit
            timer = QTimer(self)
            timer.setSingleShot(True)
            timer.setInterval(400)
            timer.timeout.connect(
                lambda e=edit, g=game, s=status: self._update_status(e, g, s))
            edit.textChanged.connect(lambda _t, t=timer: t.start())
            self._update_status(edit, game, status)

        self.download_maps_check = QCheckBox(
            tr("Descargar imágenes de mapas desde internet (GameTracker)"))
        self.download_maps_check.setObjectName("download_map_images")
        self.download_maps_check.setToolTip(
            tr("Si el mapa no está instalado, busca su imagen en image.gametracker.com"))
        self.download_maps_check.setChecked(cfg.download_map_images)

        self.language_combo = QComboBox()
        self.language_combo.setObjectName("language_combo")
        # Los idiomas se muestran en su propio idioma; solo «Automático» se traduce.
        for label, code in ((tr("Automático"), "auto"), ("Español", "es"),
                            ("English", "en")):
            self.language_combo.addItem(label, code)
        index = self.language_combo.findData(cfg.language)
        self.language_combo.setCurrentIndex(max(index, 0))
        language_row = QFormLayout()
        language_row.addRow(tr("Idioma"), self.language_combo)

        self.find_games_button = QPushButton(tr("Buscar juegos…"))
        self.find_games_button.setObjectName("settings_find_games")
        self.find_games_button.clicked.connect(self._request_find_games)
        self.find_games_button.setEnabled(on_find_games is not None)
        self._on_find_games = on_find_games

        self.update_countries_button = QPushButton(tr("Actualizar base de países"))
        self.update_countries_button.setObjectName("update_country_db")
        self.update_countries_button.setToolTip(
            tr("Descarga de nuevo la base «IP to Country Lite» de DB-IP (db-ip.com)"))
        self.update_countries_button.clicked.connect(self._request_country_update)
        self.update_countries_button.setEnabled(on_update_countries is not None)
        self._on_update_countries = on_update_countries
        self.countries_status = QLabel()
        self.countries_status.setObjectName("country_db_status")
        countries_row = QHBoxLayout()
        countries_row.addWidget(self.update_countries_button)
        countries_row.addWidget(self.countries_status, 1)

        self.steam_key_edit = QLineEdit(steam_key)
        self.steam_key_edit.setObjectName("steam_key_edit")
        self.steam_key_edit.setEchoMode(QLineEdit.Password)
        self.steam_key_link = QLabel(tr(
            "Consíguela en {link} (cualquier dominio sirve)",
            link=f'<a href="{STEAM_KEY_URL}">steamcommunity.com/dev/apikey</a>'))
        self.steam_key_link.setObjectName("steam_key_link")
        self.steam_key_link.setTextFormat(Qt.RichText)
        self.steam_key_link.setOpenExternalLinks(True)
        steam_row = QFormLayout()
        steam_row.addRow(tr("Clave de API de Steam"), self.steam_key_edit)
        steam_row.addRow("", self.steam_key_link)

        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.button(QDialogButtonBox.Ok).setText(tr("Guardar"))
        buttons.button(QDialogButtonBox.Cancel).setText(tr("Cancelar"))
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        if masters is None:
            masters = MastersContext(masters_core.load_official(_NO_CACHE),
                                     masters_core.CustomMasters(), probe=None)
        self.masters_tab = MastersTab(masters)

        # Pestaña «Juegos»: rutas (11 juegos no entran en 768 px: el formulario se desplaza).
        games_page = QWidget()
        games_layout = QVBoxLayout(games_page)
        games_layout.addWidget(QLabel(tr("Rutas de instalación de los juegos:")))
        form_widget = QWidget()
        form_widget.setLayout(form)
        scroll = QScrollArea()
        scroll.setObjectName("game_paths_scroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.NoFrame)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        scroll.setWidget(form_widget)
        scroll.setMinimumHeight(min(form_widget.sizeHint().height(), 420))
        games_layout.addWidget(scroll, 1)
        games_layout.addWidget(self.find_games_button, 0, Qt.AlignLeft)

        general_page = QWidget()
        general_layout = QVBoxLayout(general_page)
        general_layout.addWidget(self.download_maps_check)
        general_layout.addLayout(language_row)
        general_layout.addLayout(countries_row)
        general_layout.addLayout(steam_row)
        general_layout.addStretch(1)

        self.tabs = QTabWidget()
        self.tabs.setObjectName("settings_tabs")
        self.tabs.addTab(games_page, tr("Juegos"))
        self.tabs.addTab(self.masters_tab, tr("Masters"))
        self.tabs.addTab(general_page, tr("General"))
        self.tabs.addTab(self._build_appearance_page(cfg), tr("Apariencia"))

        layout = QVBoxLayout(self)
        layout.addWidget(self.tabs, 1)
        layout.addWidget(buttons)

    def steam_key(self) -> str:
        return self.steam_key_edit.text().strip()

    def _build_appearance_page(self, cfg: Config) -> QWidget:
        """Lista de los temas (Sistema + 10) con sus muestras; se aplica al guardar."""
        self.theme_list = QListWidget()
        self.theme_list.setObjectName("theme_list")
        self.theme_list.setIconSize(QSize(4 * (SWATCH + SWATCH_GAP) - SWATCH_GAP, SWATCH))
        for theme_id in ("system", *theme.THEME_IDS):
            item = QListWidgetItem(theme_swatch_icon(theme_id),
                                   theme.theme_display_name(theme_id))
            item.setData(Qt.UserRole, theme_id)
            self.theme_list.addItem(item)
            if theme_id == cfg.theme:
                self.theme_list.setCurrentItem(item)
        if self.theme_list.currentItem() is None:
            self.theme_list.setCurrentRow(0)
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.addWidget(QLabel(tr("Tema de la interfaz:")))
        layout.addWidget(self.theme_list, 1)
        return page

    def _request_find_games(self) -> None:
        if self._on_find_games is None:
            return
        # Solo rellena los campos; se guardan con «Guardar».
        for game_id, path in self._on_find_games().items():
            if game_id in self.path_edits:
                self.path_edits[game_id].setText(path)

    def _request_country_update(self) -> None:
        if self._on_update_countries is None:
            return
        self.update_countries_button.setEnabled(False)
        self.countries_status.setText(tr("Descargando en segundo plano…"))
        self._on_update_countries()

    @staticmethod
    def _update_status(edit: QLineEdit, game, label: QLabel) -> None:
        found = resolve_game_exe(game, edit.text().strip())
        if found:
            label.setText(f"\u2713 {os.path.basename(found)}")
        else:
            wanted = (tr("{exe} con la carpeta {folder}", exe=game.exe_relpath,
                         folder=game.valve_gamedir)
                      if game.family == "valve" and game.valve_gamedir
                      else game.exe_relpath)
            label.setText("\u26a0 " + tr("No se encontró {wanted}", wanted=wanted))

    def _browse(self, edit: QLineEdit, game_name: str) -> None:
        path = QFileDialog.getExistingDirectory(
            self, tr("Carpeta de {name}", name=game_name), edit.text())
        if path:
            edit.setText(QDir.toNativeSeparators(path))

    def custom_masters(self) -> masters_core.CustomMasters:
        return self.masters_tab.custom_masters()

    def apply_to(self, cfg: Config) -> None:
        for game_id, edit in self.path_edits.items():
            path = edit.text().strip()
            if path or GAMES[game_id].default_path:
                # Vaciar un juego con ruta por defecto la guarda como "" para que
                # load_config no la vuelva a proponer.
                cfg.game_paths[game_id] = path
            else:
                cfg.game_paths.pop(game_id, None)
        cfg.download_map_images = self.download_maps_check.isChecked()
        cfg.language = self.language_combo.currentData()
        item = self.theme_list.currentItem()
        if item is not None:
            cfg.theme = item.data(Qt.UserRole)
