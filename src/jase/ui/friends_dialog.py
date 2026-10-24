"""Diálogo «Amigos»: lista de nombres de jugador a vigilar."""
from __future__ import annotations

from PySide6.QtWidgets import (QCheckBox, QDialog, QDialogButtonBox, QHBoxLayout, QLineEdit,
                               QListWidget, QPushButton, QVBoxLayout, QWidget)

from jase import friends
from jase.i18n import tr


class FriendsDialog(QDialog):
    def __init__(self, current: list[str], watch: bool = False,
                 parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle(tr("Amigos"))
        self.resize(360, 420)
        self.list = QListWidget()
        self.list.addItems(current)
        self.watch_check = QCheckBox(
            tr("Revisar favoritos y recientes cada 2 min, también con la ventana en la bandeja"))
        self.watch_check.setChecked(watch)
        self.edit = QLineEdit()
        self.edit.setPlaceholderText(tr("Nombre del jugador"))
        self.add_button = QPushButton(tr("Agregar"))
        self.remove_button = QPushButton(tr("Quitar"))
        self.edit.returnPressed.connect(self._add)
        self.add_button.clicked.connect(self._add)
        self.remove_button.clicked.connect(self._remove)
        row = QHBoxLayout()
        row.addWidget(self.edit, 1)
        row.addWidget(self.add_button)
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        layout = QVBoxLayout(self)
        layout.addWidget(self.list)
        layout.addLayout(row)
        layout.addWidget(self.remove_button)
        layout.addWidget(self.watch_check)
        layout.addWidget(buttons)

    def watch(self) -> bool:
        return self.watch_check.isChecked()

    def names(self) -> list[str]:
        return friends.parse([self.list.item(i).text() for i in range(self.list.count())])

    def _add(self) -> None:
        text = self.edit.text().strip()
        self.edit.clear()
        if text:
            self.list.addItem(text)
            # parse() deduplica y recorta: se reescribe la lista con el resultado.
            clean = self.names()
            self.list.clear()
            self.list.addItems(clean)

    def _remove(self) -> None:
        for item in self.list.selectedItems():
            self.list.takeItem(self.list.row(item))
