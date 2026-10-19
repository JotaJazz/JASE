"""Diálogos: contraseña de servidor y ajustes de rutas de juegos."""
from __future__ import annotations

from PySide6.QtCore import QDir
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from jase.config import Config
from jase.games import GAMES


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
    def __init__(self, cfg: Config, parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Ajustes")
        self.setMinimumWidth(520)
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
            form.addRow(game.name, row)
            self.path_edits[game.id] = edit

        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.button(QDialogButtonBox.Ok).setText("Guardar")
        buttons.button(QDialogButtonBox.Cancel).setText("Cancelar")
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addWidget(QLabel("Rutas de instalación de los juegos:"))
        layout.addLayout(form)
        layout.addWidget(buttons)

    def _browse(self, edit: QLineEdit, game_name: str) -> None:
        path = QFileDialog.getExistingDirectory(
            self, f"Carpeta de {game_name}", edit.text())
        if path:
            edit.setText(QDir.toNativeSeparators(path))

    def apply_to(self, cfg: Config) -> None:
        for game_id, edit in self.path_edits.items():
            path = edit.text().strip()
            if path:
                cfg.game_paths[game_id] = path
            else:
                cfg.game_paths.pop(game_id, None)
