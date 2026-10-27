"""Diálogo «Agregar servidor…»: juego, IP y puerto escritos a mano o pegados (uno o varios)."""
from __future__ import annotations

import re
from collections.abc import Callable
from functools import partial

from PySide6.QtGui import QIntValidator
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPlainTextEdit,
    QPushButton,
    QVBoxLayout,
)

from jase import jase_url
from jase.games import GAMES, GameDef
from jase.i18n import tr, tr_n
from jase.masters import valid_host, valid_port
from jase.models import ServerAddress

MAX_ADDRESSES = 100  # por pegado: una lista de un clan, no un volcado de un master
# «host.con.puntos:puerto» suelto en cualquier texto (Discord, foros, «connect 1.2.3.4:28960»).
_ADDRESS_RE = re.compile(r"(?<![\w.:-])([A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+):(\d{1,5})(?![\w.:])")

# detect(dirección, al_terminar): al_terminar recibe [(id de juego, dirección de consulta)].
DetectFn = Callable[[ServerAddress, Callable[[list[tuple[str, ServerAddress]]], None]], None]


def split_host_port(text: str) -> tuple[str, int | None]:
    """`"1.2.3.4:28960"` → ("1.2.3.4", 28960); sin puerto válido, (texto limpio, None)."""
    text = text.strip()
    host, sep, port = text.rpartition(":")
    if sep and port.isascii() and port.isdigit() and valid_port(int(port)):
        return host.strip(), int(port)
    return text, None


def find_addresses(text: str) -> list[ServerAddress]:
    """Direcciones «host:puerto» de un texto cualquiera, sin repetir y como mucho
    MAX_ADDRESSES (el host lleva al menos un punto: IP o dominio)."""
    result: list[ServerAddress] = []
    for host, port in _ADDRESS_RE.findall(text):
        if not (valid_host(host) and host[0] not in ".-" and valid_port(int(port))):
            continue
        address = ServerAddress(host, int(port))
        if address not in result:
            result.append(address)
            if len(result) == MAX_ADDRESSES:
                break
    return result


def query_candidates(game: GameDef, port: int, host: str) -> list[ServerAddress]:
    """Direcciones a consultar: la escrita y, en GameSpy, la de consulta que corresponde si
    el usuario escribió el puerto de juego (el inverso de `connect_port_fallback`)."""
    result = [ServerAddress(host, port)]
    query_port = port - game.connect_port_fallback
    if game.connect_port_fallback and valid_port(query_port):
        result.append(ServerAddress(host, query_port))
    return result


class AddServerDialog(QDialog):
    """`text`: contenido inicial (p. ej. el portapapeles); `detect`: activa «Detectar»."""

    def __init__(self, game_ids: list[str], current: str | None, favorite: bool,
                 text: str = "", detect: DetectFn | None = None, parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle(tr("Agregar servidor"))
        self._detect_fn = detect
        self._detect_token = 0
        self.game_combo = QComboBox()
        for game_id in game_ids:
            self.game_combo.addItem(GAMES[game_id].name, game_id)
        if current in game_ids:
            self.game_combo.setCurrentIndex(game_ids.index(current))
        self.detect_button = QPushButton(tr("Detectar"))
        self.detect_button.setToolTip(
            tr("Consulta la dirección con cada protocolo y elige el juego que responde"))
        self.detect_button.setVisible(detect is not None)
        self.detect_button.clicked.connect(self._detect)
        self.detect_label = QLabel()
        self.detect_label.setWordWrap(True)
        self.detect_label.hide()
        self._host_text = ""
        self.host_edit = QLineEdit()
        self.host_edit.setPlaceholderText(tr("IP o nombre (se puede pegar IP:puerto)"))
        self.host_edit.textChanged.connect(self._on_host_changed)
        self.host_edit.editingFinished.connect(
            lambda: self._apply_text(self.host_edit.text()))
        self.port_edit = QLineEdit()
        self.port_edit.setValidator(QIntValidator(1, 65535, self.port_edit))
        self.port_edit.setPlaceholderText(tr("Puerto"))
        self.port_edit.textChanged.connect(self._update_ok)
        self.list_edit = QPlainTextEdit()
        self.list_edit.setPlaceholderText(
            tr("Pega aquí varias direcciones IP:puerto (una por línea o mezcladas con texto)"))
        self.list_edit.textChanged.connect(self._update_ok)
        # Botón normal (no checkable): pulsado, en los temas se vería como el principal.
        self._multi = False
        self.multi_button = QPushButton(tr("Varias direcciones…"))
        self.multi_button.clicked.connect(lambda: self._set_multi(not self._multi))
        self.favorite_check = QCheckBox(tr("Agregar a favoritos"))
        self.favorite_check.setChecked(favorite)

        game_row = QHBoxLayout()
        game_row.addWidget(self.game_combo, 1)
        game_row.addWidget(self.detect_button)
        self.form = QFormLayout()
        self.form.addRow(tr("Juego:"), game_row)
        self.form.addRow(tr("IP:"), self.host_edit)
        self.form.addRow(tr("Puerto:"), self.port_edit)
        self.form.addRow(tr("Direcciones:"), self.list_edit)

        std = QDialogButtonBox.StandardButton
        self.buttons = QDialogButtonBox(std.Ok | std.Cancel)
        self.buttons.button(std.Cancel).setText(tr("Cancelar"))
        self.buttons.addButton(self.multi_button, QDialogButtonBox.ButtonRole.ResetRole)
        self.buttons.accepted.connect(self.accept)
        self.buttons.rejected.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addLayout(self.form)
        layout.addWidget(self.detect_label)
        layout.addWidget(self.favorite_check)
        layout.addWidget(self.buttons)
        self.setMinimumWidth(420)
        self._set_multi(False)
        if text:
            self._apply_text(text)

    # ----------------------------------------------------------- texto pegado
    def _on_host_changed(self, text: str) -> None:
        """Al pegar (entra más de un carácter de golpe) reparte el texto en sus casillas;
        quien escribe tecla a tecla lo ve separado al salir de la casilla."""
        pasted = len(text) - len(self._host_text) > 1
        self._host_text = text
        if pasted:
            self._apply_text(text)
        self._update_ok()

    def _apply_text(self, text: str) -> None:
        """Varias direcciones → modo lista; un enlace jase:// (elige también el juego),
        «IP:puerto» o texto con una sola dirección → casillas de IP y puerto."""
        found = find_addresses(text)
        if len(found) > 1:
            self._set_host("")
            self.list_edit.setPlainText(text.strip())
            self._set_multi(True)
            return
        parsed = jase_url.parse(text.strip())
        port: int | None
        if parsed is not None:
            game_id, address = parsed
            index = self.game_combo.findData(game_id)
            if index >= 0:
                self.game_combo.setCurrentIndex(index)
            host, port = address.host, address.port
        elif found:
            host, port = found[0].host, found[0].port
        else:
            host, port = split_host_port(text)
        if port is not None:
            self.port_edit.setText(str(port))
        if host != text:
            self._set_host(host)
        self._update_ok()

    def _set_host(self, host: str) -> None:
        self._host_text = host
        self.host_edit.blockSignals(True)  # sin volver a entrar en _on_host_changed
        self.host_edit.setText(host)
        self.host_edit.blockSignals(False)

    def _set_multi(self, multi: bool) -> None:
        self._multi = multi
        if multi and not self.list_edit.toPlainText().strip():
            single = self.result_address()
            if single is not None:  # lo ya escrito pasa a ser la primera línea
                self.list_edit.setPlainText(f"{single}\n")
        for widget in (self.host_edit, self.port_edit):
            self.form.setRowVisible(widget, not multi)
        self.form.setRowVisible(self.list_edit, multi)
        self.multi_button.setText(tr("Una dirección") if multi else tr("Varias direcciones…"))
        self._update_ok()

    # ---------------------------------------------------------------- detectar
    def _detect(self) -> None:
        address = self.result_address()
        if self._detect_fn is None or address is None:
            return
        self._detect_token += 1
        token = self._detect_token
        self.detect_button.setEnabled(False)
        self.detect_label.setText(tr("Detectando el juego de {address}…", address=address))
        self.detect_label.show()
        self._detect_fn(address, partial(self._on_detected, token, address))

    def _on_detected(self, token: int, address: ServerAddress,
                     games: list[tuple[str, ServerAddress]]) -> None:
        if token != self._detect_token:
            return  # se pidió otra detección después
        self._update_ok()
        games = [(gid, a) for gid, a in games if self.game_combo.findData(gid) >= 0]
        if not games:
            self.detect_label.setText(
                tr("{address} no respondió como ningún juego conocido", address=address))
            return
        game_id, query_address = games[0]
        self.game_combo.setCurrentIndex(self.game_combo.findData(game_id))
        if query_address.host == address.host:
            self.port_edit.setText(str(query_address.port))  # p. ej. el de consulta de GameSpy
        if len(games) == 1:
            self.detect_label.setText(tr("Detectado: {game}", game=GAMES[game_id].name))
        else:
            self.detect_label.setText(tr("Puede ser: {games}", games=", ".join(
                GAMES[gid].name for gid, _a in games)))

    # --------------------------------------------------------------- resultado
    def _update_ok(self, *_args) -> None:
        ok = self.buttons.button(QDialogButtonBox.StandardButton.Ok)
        n = len(self.addresses())
        ok.setEnabled(n > 0)
        ok.setText(tr_n("Agregar 1 servidor", "Agregar {n} servidores", n)
                   if self.is_multi() and n else tr("Agregar"))
        self.detect_button.setEnabled(not self.is_multi() and n == 1)

    def is_multi(self) -> bool:
        return self._multi

    def game_id(self) -> str:
        return self.game_combo.currentData()

    def result_address(self) -> ServerAddress | None:
        """Dirección escrita; «IP:puerto» aún sin separar en la casilla de IP también vale."""
        host, inline_port = split_host_port(self.host_edit.text())
        port = self.port_edit.text()
        if inline_port is None and not port.isdigit():
            return None
        number = inline_port if inline_port is not None else int(port)
        if not valid_host(host) or host[0] in ".-" or not valid_port(number):
            return None
        return ServerAddress(host, number)

    def addresses(self) -> list[ServerAddress]:
        """Lo que se agregará: las direcciones de la lista o la de las casillas."""
        if self.is_multi():
            return find_addresses(self.list_edit.toPlainText())
        single = self.result_address()
        return [single] if single is not None else []

    def favorite(self) -> bool:
        return self.favorite_check.isChecked()
