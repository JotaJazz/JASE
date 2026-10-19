"""Diálogos: contraseña de servidor y ajustes (rutas de juegos, imágenes de mapas, países)."""
from __future__ import annotations

import os

from PySide6.QtCore import QDir, Qt, QTimer
from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from jase.config import Config
from jase.games import GAMES
from jase.paths import resolve_exe


class PasswordDialog(QDialog):
    def __init__(self, server_name: str, initial: str | None, parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Contraseña del servidor")
        self.forget_requested = False

        label = QLabel(f"El servidor «{server_name}» requiere contraseña:")
        label.setWordWrap(True)
        self.edit = QLineEdit(initial or "")
        self.edit.setEchoMode(QLineEdit.Password)
        self.edit.setPlaceholderText("Contraseña")

        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.button(QDialogButtonBox.Ok).setText("Conectar")
        buttons.button(QDialogButtonBox.Cancel).setText("Cancelar")
        self.forget_button = buttons.addButton("Olvidar contraseña",
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


class SettingsDialog(QDialog):
    def __init__(self, cfg: Config, parent=None, on_update_countries=None) -> None:
        """`on_update_countries`: callback del botón «Actualizar base de países»
        (None = botón deshabilitado, p. ej. sin base o con una descarga en curso)."""
        super().__init__(parent)
        self.setWindowTitle("Ajustes")
        self.setMinimumWidth(580)
        self.path_edits: dict[str, QLineEdit] = {}

        form = QFormLayout()
        for game in GAMES.values():
            edit = QLineEdit(cfg.game_paths.get(game.id, ""))
            edit.setPlaceholderText("Carpeta de instalación")
            browse = QPushButton("Examinar…")
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
            "Descargar imágenes de mapas desde internet (GameTracker)")
        self.download_maps_check.setObjectName("download_map_images")
        self.download_maps_check.setToolTip(
            "Si el mapa no está instalado, busca su imagen en image.gametracker.com")
        self.download_maps_check.setChecked(cfg.download_map_images)

        self.update_countries_button = QPushButton("Actualizar base de países")
        self.update_countries_button.setObjectName("update_country_db")
        self.update_countries_button.setToolTip(
            "Descarga de nuevo la base «IP to Country Lite» de DB-IP (db-ip.com)")
        self.update_countries_button.clicked.connect(self._request_country_update)
        self.update_countries_button.setEnabled(on_update_countries is not None)
        self._on_update_countries = on_update_countries
        self.countries_status = QLabel()
        self.countries_status.setObjectName("country_db_status")
        countries_row = QHBoxLayout()
        countries_row.addWidget(self.update_countries_button)
        countries_row.addWidget(self.countries_status, 1)

        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.button(QDialogButtonBox.Ok).setText("Guardar")
        buttons.button(QDialogButtonBox.Cancel).setText("Cancelar")
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addWidget(QLabel("Rutas de instalación de los juegos:"))
        # 11 juegos no entran en una pantalla de 768 px: el formulario se desplaza.
        form_widget = QWidget()
        form_widget.setLayout(form)
        scroll = QScrollArea()
        scroll.setObjectName("game_paths_scroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.NoFrame)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        scroll.setWidget(form_widget)
        scroll.setMinimumHeight(min(form_widget.sizeHint().height(), 420))
        layout.addWidget(scroll, 1)
        layout.addSpacing(8)
        layout.addWidget(self.download_maps_check)
        layout.addLayout(countries_row)
        layout.addWidget(buttons)

    def _request_country_update(self) -> None:
        if self._on_update_countries is None:
            return
        self.update_countries_button.setEnabled(False)
        self.countries_status.setText("Descargando en segundo plano…")
        self._on_update_countries()

    @staticmethod
    def _update_status(edit: QLineEdit, game, label: QLabel) -> None:
        found = resolve_exe(edit.text().strip(), game.exe_relpath,
                            game.exe_candidates, game.preferred_exes)
        if found:
            label.setText(f"\u2713 {os.path.basename(found)}")
        else:
            label.setText(f"\u26a0 No se encontró {game.exe_relpath}")

    def _browse(self, edit: QLineEdit, game_name: str) -> None:
        path = QFileDialog.getExistingDirectory(
            self, f"Carpeta de {game_name}", edit.text())
        if path:
            edit.setText(QDir.toNativeSeparators(path))

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
