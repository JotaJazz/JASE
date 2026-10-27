"""Consola RCON (no modal) de un servidor: clave, comando con historial y salida."""
from __future__ import annotations

from PySide6.QtCore import QEvent, QObject, Qt, Signal
from PySide6.QtGui import QFontDatabase, QKeyEvent
from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QFormLayout,
    QHBoxLayout,
    QLineEdit,
    QPlainTextEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from jase.i18n import tr

HISTORY_MAX = 50


def error_text(code: str, detail: str = "") -> str:
    texts = {
        "timeout": tr("El servidor no respondió al RCON."),
        "bad_password": tr("Clave RCON incorrecta."),
        "no_challenge": tr("El servidor no dio un challenge de RCON (¿tiene RCON activado?)."),
        "connect": tr("No se pudo conectar al RCON (TCP) del servidor."),
        "invalid": tr("Respuesta RCON inválida."),
        "closed": tr("Se cortó la conexión RCON."),
        "unsupported": tr("Este juego no tiene RCON en JASE."),
    }
    text = texts.get(code) or tr("Error de red: {error}", error=detail or code)
    return f"{text} ({detail})" if detail and code in texts else text


class RconDialog(QDialog):
    """`command_requested(clave, comando, recordar)`: quien la abre manda el comando y
    llama a `show_answer`/`show_error`."""

    command_requested = Signal(str, str, bool)

    def __init__(self, server_name: str, address: str, saved_password: str | None,
                 parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle(tr("Consola RCON: {server}", server=server_name))
        self.resize(640, 440)
        self._history: list[str] = []
        self._history_pos = 0
        self.password_edit = QLineEdit(saved_password or "")
        self.password_edit.setEchoMode(QLineEdit.EchoMode.Password)
        self.remember_check = QCheckBox(tr("Recordar la clave"))
        self.remember_check.setToolTip(tr("Se guarda en el Administrador de credenciales"))
        self.remember_check.setChecked(saved_password is not None)
        self.output = QPlainTextEdit()
        self.output.setReadOnly(True)
        self.output.setFont(QFontDatabase.systemFont(QFontDatabase.SystemFont.FixedFont))
        self.output.setMaximumBlockCount(5000)
        self.command_edit = QLineEdit()
        self.command_edit.setPlaceholderText(tr("Comando (p. ej. status, map mp_toujane)"))
        self.command_edit.returnPressed.connect(self._send)
        self.command_edit.installEventFilter(self)
        self.send_button = QPushButton(tr("Enviar"))
        self.send_button.clicked.connect(self._send)
        form = QFormLayout()
        form.addRow(tr("Servidor:"), QLineEditReadOnly(address))
        password_row = QHBoxLayout()
        password_row.addWidget(self.password_edit, 1)
        password_row.addWidget(self.remember_check)
        form.addRow(tr("Clave RCON:"), password_row)
        command_row = QHBoxLayout()
        command_row.addWidget(self.command_edit, 1)
        command_row.addWidget(self.send_button)
        layout = QVBoxLayout(self)
        layout.addLayout(form)
        layout.addWidget(self.output, 1)
        layout.addLayout(command_row)
        (self.command_edit if saved_password else self.password_edit).setFocus()

    def _send(self) -> None:
        command = self.command_edit.text().strip()
        if not command or not self.send_button.isEnabled():
            return
        if not self._history or self._history[-1] != command:
            self._history = [*self._history, command][-HISTORY_MAX:]
        self._history_pos = len(self._history)
        self.command_edit.clear()
        self.output.appendPlainText(f"> {command}")
        self.set_busy(True)
        self.command_requested.emit(self.password_edit.text(), command,
                                    self.remember_check.isChecked())

    def set_busy(self, busy: bool) -> None:
        self.send_button.setEnabled(not busy)

    def show_answer(self, text: str) -> None:
        self.output.appendPlainText(text.rstrip("\n") or tr("(sin respuesta de texto)"))
        self.set_busy(False)

    def show_error(self, code: str, detail: str = "") -> None:
        self.output.appendPlainText("! " + error_text(code, detail))
        self.set_busy(False)

    def eventFilter(self, watched: QObject, event: QEvent) -> bool:  # noqa: N802 (Qt)
        """Flechas arriba/abajo en el comando: historial."""
        if watched is self.command_edit and isinstance(event, QKeyEvent) \
                and event.type() == QEvent.Type.KeyPress \
                and event.key() in (Qt.Key.Key_Up, Qt.Key.Key_Down) and self._history:
            step = -1 if event.key() == Qt.Key.Key_Up else 1
            self._history_pos = max(0, min(len(self._history), self._history_pos + step))
            self.command_edit.setText(self._history[self._history_pos]
                                      if self._history_pos < len(self._history) else "")
            return True
        return super().eventFilter(watched, event)


class QLineEditReadOnly(QLineEdit):
    def __init__(self, text: str) -> None:
        super().__init__(text)
        self.setReadOnly(True)
