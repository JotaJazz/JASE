"""Pestaña «Masters» de Ajustes: editor de masters oficiales y personales."""
from __future__ import annotations

import asyncio
import copy
import threading
import time
from collections.abc import Awaitable, Callable
from dataclasses import dataclass

from PySide6.QtCore import QObject, Qt, Signal
from PySide6.QtWidgets import (
    QAbstractItemView,
    QComboBox,
    QHBoxLayout,
    QHeaderView,
    QInputDialog,
    QLabel,
    QMessageBox,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from jase import masters, scanner
from jase.games import GAMES, GameDef
from jase.i18n import tr, tr_n

COL_ACTIVE, COL_MASTER, COL_ORIGIN, COL_PROBE = range(4)
PROBE_TIMEOUT_S = 5

ProbeFn = Callable[[GameDef, str, int], Awaitable[int]]


async def default_probe(game: GameDef, host: str, port: int) -> int:
    """Pide la lista al master y cuenta las direcciones recibidas."""
    return len(await scanner.fetch_master(game, host, port, timeout=PROBE_TIMEOUT_S))


@dataclass(eq=False)
class MastersContext:
    """Lo que Ajustes necesita para editar masters (lo arma la ventana principal)."""

    official: masters.OfficialList
    custom: masters.CustomMasters
    probe: ProbeFn | None = default_probe
    check_update: Callable[[], None] | None = None
    # La pestaña lo asigna; la ventana lo llama con ("updated" | "unchanged" | "failed", lista).
    outcome_listener: Callable[[str, masters.OfficialList | None], None] | None = None


class _ProbeRelay(QObject):
    """Lleva el resultado de una prueba al hilo de la UI (sin padre, como MapImageRelay)."""

    done = Signal(object, object)  # (token, int | None)


_RELAY: _ProbeRelay | None = None


def _relay() -> _ProbeRelay:
    global _RELAY
    if _RELAY is None:
        _RELAY = _ProbeRelay()
    return _RELAY


def _run_probe(probe: ProbeFn, token: tuple, game: GameDef, host: str, port: int) -> None:
    relay = _relay()

    def run() -> None:
        try:
            count: int | None = asyncio.run(probe(game, host, port))
        except Exception:
            count = None
        try:
            relay.done.emit(token, count)
        except RuntimeError:  # aplicación cerrándose
            pass

    threading.Thread(target=run, name="jase-master-probe", daemon=True).start()


def parse_master_text(text: str) -> masters.Master | None:
    """«host:puerto» → (host, puerto) con la misma validación que la lista oficial."""
    host, sep, port_text = text.strip().rpartition(":")
    if not sep or not port_text.isascii() or not port_text.isdigit():
        return None
    port = int(port_text)
    if not masters.valid_host(host) or not masters.valid_port(port):
        return None
    return host, port


class MastersTab(QWidget):
    probe_finished = Signal(bool)  # True si el resultado se aplicó, False si se descartó

    def __init__(self, ctx: MastersContext, parent=None) -> None:
        super().__init__(parent)
        self._ctx = ctx
        self.official = ctx.official
        self.custom = copy.deepcopy(ctx.custom)
        self._probe = ctx.probe
        self._probe_token: tuple | None = None
        self._probe_serial = 0
        self._owner = object()
        ctx.outcome_listener = self.show_update_result

        self.game_combo = QComboBox()
        self.game_combo.setObjectName("masters_game_combo")
        for game in GAMES.values():
            self.game_combo.addItem(game.name, game.id)

        self.table = QTableWidget(0, 4)
        self.table.setObjectName("masters_table")
        self.table.setHorizontalHeaderLabels(
            [tr("Activo"), tr("Master"), tr("Origen"), tr("Prueba")])
        self.table.verticalHeader().setVisible(False)
        self.table.setSelectionBehavior(QAbstractItemView.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SingleSelection)
        self.table.setEditTriggers(QAbstractItemView.NoEditTriggers)
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(COL_MASTER, QHeaderView.Stretch)
        for col in (COL_ACTIVE, COL_ORIGIN, COL_PROBE):
            header.setSectionResizeMode(col, QHeaderView.ResizeToContents)

        self.add_button = QPushButton(tr("Agregar…"))
        self.add_button.setObjectName("masters_add")
        self.remove_button = QPushButton(tr("Quitar"))
        self.remove_button.setObjectName("masters_remove")
        self.probe_button = QPushButton(tr("Probar"))
        self.probe_button.setObjectName("masters_probe")
        self.restore_button = QPushButton(tr("Restaurar oficiales"))
        self.restore_button.setObjectName("masters_restore")
        buttons = QHBoxLayout()
        for b in (self.add_button, self.remove_button, self.probe_button, self.restore_button):
            buttons.addWidget(b)
        buttons.addStretch(1)

        self.official_label = QLabel()
        self.official_label.setTextFormat(Qt.PlainText)  # `updated` viene de la red
        self.official_label.setObjectName("masters_official_date")
        self.check_button = QPushButton(tr("Buscar actualización"))
        self.check_button.setObjectName("masters_check_update")
        self.check_button.setEnabled(ctx.check_update is not None)
        self.update_status = QLabel()
        self.update_status.setObjectName("masters_update_status")
        self.update_status.setTextFormat(Qt.PlainText)
        self.update_status.setWordWrap(True)
        footer = QHBoxLayout()
        footer.addWidget(self.official_label)
        footer.addStretch(1)
        footer.addWidget(self.check_button)

        layout = QVBoxLayout(self)
        layout.addWidget(self.game_combo)
        layout.addWidget(self.table, 1)
        layout.addLayout(buttons)
        layout.addLayout(footer)
        layout.addWidget(self.update_status)

        self.game_combo.currentIndexChanged.connect(self._reload)
        self.table.itemChanged.connect(self._on_item_changed)
        self.table.itemSelectionChanged.connect(self._on_selection_changed)
        self.add_button.clicked.connect(self._prompt_add)
        self.remove_button.clicked.connect(self.remove_selected)
        self.probe_button.clicked.connect(self.probe_selected)
        self.restore_button.clicked.connect(self.restore_officials)
        self.check_button.clicked.connect(self._request_update)
        _relay().done.connect(self._on_probe_done)
        self._update_footer()
        self._reload()

    # ------------------------------------------------------------------ estado
    def game_id(self) -> str:
        return self.game_combo.currentData()

    def _selected_row(self) -> int:
        rows = self.table.selectionModel().selectedRows()
        return rows[0].row() if rows else -1

    def _row_info(self, row: int) -> tuple[masters.Master, bool] | None:
        item = self.table.item(row, COL_MASTER) if row >= 0 else None
        return item.data(Qt.UserRole) if item is not None else None

    def _cancel_probe(self) -> None:
        if self._probe_token is None:
            return
        self._probe_token = None  # el resultado en vuelo se descartará
        pending = tr("Probando…")
        for r in range(self.table.rowCount()):
            item = self.table.item(r, COL_PROBE)
            if item is not None and item.text() == pending:
                item.setText("")

    def _reload(self) -> None:
        self._cancel_probe()
        gid = self.game_id()
        disabled = self.custom.disabled.get(gid, set())
        rows = [(m, True) for m in masters.official_for(gid, self.official)]
        rows += [(m, False) for m in self.custom.added.get(gid, [])]
        self.table.blockSignals(True)
        self.table.setRowCount(len(rows))
        for r, (m, is_official) in enumerate(rows):
            active = QTableWidgetItem()
            if is_official:
                active.setFlags(Qt.ItemIsEnabled | Qt.ItemIsSelectable | Qt.ItemIsUserCheckable)
                active.setCheckState(
                    Qt.Unchecked if masters.master_key(m) in disabled else Qt.Checked)
            else:  # los personales siempre están activos: se quitan, no se desactivan
                active.setFlags(Qt.ItemIsEnabled | Qt.ItemIsSelectable)
                active.setToolTip(tr('Los masters personales se quitan con "Quitar"'))
            name = QTableWidgetItem(f"{m[0]}:{m[1]}")
            name.setData(Qt.UserRole, (m, is_official))
            origin = QTableWidgetItem(tr("Oficial") if is_official else tr("Personal"))
            self.table.setItem(r, COL_ACTIVE, active)
            self.table.setItem(r, COL_MASTER, name)
            self.table.setItem(r, COL_ORIGIN, origin)
            self.table.setItem(r, COL_PROBE, QTableWidgetItem(""))
        self.table.blockSignals(False)
        self._on_selection_changed()

    def _on_selection_changed(self) -> None:
        info = self._row_info(self._selected_row())
        self._cancel_probe()
        self.remove_button.setEnabled(info is not None and not info[1])
        self._refresh_probe_enabled()

    def _refresh_probe_enabled(self) -> None:
        info = self._row_info(self._selected_row())
        self.probe_button.setEnabled(
            info is not None and self._probe is not None and self._probe_token is None)

    def _on_item_changed(self, item: QTableWidgetItem) -> None:
        if item.column() != COL_ACTIVE:
            return
        info = self._row_info(item.row())
        if info is None or not info[1]:
            return
        gid, key = self.game_id(), masters.master_key(info[0])
        keys = self.custom.disabled.setdefault(gid, set())
        if item.checkState() == Qt.Checked:
            keys.discard(key)
        else:
            keys.add(key)
        if not keys:
            del self.custom.disabled[gid]

    def custom_masters(self) -> masters.CustomMasters:
        return self.custom

    # ------------------------------------------------------------------ acciones
    def add_master(self, text: str) -> str:
        """Agrega un master personal. Devuelve "ok", "invalid" o "duplicate"."""
        parsed = parse_master_text(text)
        if parsed is None:
            return "invalid"
        gid = self.game_id()
        key = masters.master_key(parsed)
        known = {masters.master_key(m) for m in masters.official_for(gid, self.official)}
        known |= {masters.master_key(m) for m in self.custom.added.get(gid, [])}
        if key in known:
            return "duplicate"
        self.custom.added.setdefault(gid, []).append(parsed)
        self._reload()
        self.table.selectRow(self.table.rowCount() - 1)
        return "ok"

    def _prompt_add(self) -> None:
        text, ok = QInputDialog.getText(
            self, tr("Agregar master"), tr("Dirección del master (host:puerto):"))
        if not ok:
            return
        result = self.add_master(text)
        if result == "invalid":
            QMessageBox.warning(self, tr("Agregar master"),
                                tr("Dirección no válida; usa host:puerto (puerto 1-65535)."))
        elif result == "duplicate":
            QMessageBox.warning(self, tr("Agregar master"),
                                tr("Ese master ya está en la lista."))

    def remove_selected(self) -> None:
        row = self._selected_row()
        info = self._row_info(row)
        if info is None or info[1]:
            return
        gid, key = self.game_id(), masters.master_key(info[0])
        remaining = [m for m in self.custom.added.get(gid, []) if masters.master_key(m) != key]
        if remaining:
            self.custom.added[gid] = remaining
        else:
            self.custom.added.pop(gid, None)
        self._reload()

    def restore_officials(self) -> None:
        self.custom.disabled.pop(self.game_id(), None)
        self._reload()

    def probe_selected(self) -> None:
        row = self._selected_row()
        info = self._row_info(row)
        if info is None or self._probe is None:
            return
        self._probe_serial += 1
        token = (self._owner, self.game_id(), masters.master_key(info[0]), self._probe_serial)
        self._probe_token = token
        self.table.item(row, COL_PROBE).setText(tr("Probando…"))
        self._refresh_probe_enabled()
        _run_probe(self._probe, token, GAMES[token[1]], *info[0])

    def _on_probe_done(self, token, count) -> None:
        if token[0] is not self._owner:
            return  # prueba de otra pestaña
        if token != self._probe_token:
            self.probe_finished.emit(False)
            return
        self._probe_token = None
        for r in range(self.table.rowCount()):
            info = self._row_info(r)
            if info is not None and masters.master_key(info[0]) == token[2]:
                self.table.item(r, COL_PROBE).setText(
                    tr("✗ no responde") if count is None
                    else tr_n("✓ 1 servidor", "✓ {n} servidores", count))
                break
        self._refresh_probe_enabled()
        self.probe_finished.emit(True)

    # ------------------------------------------------------------- lista oficial
    def _update_footer(self) -> None:
        self.official_label.setText(
            tr("Lista oficial: {date}", date=self.official.updated or "—"))

    def _request_update(self) -> None:
        if self._ctx.check_update is None:
            return
        self.update_status.setText(tr("Buscando actualización…"))
        self._ctx.check_update()

    def show_update_result(self, kind: str, official: masters.OfficialList | None = None) -> None:
        if kind == "updated" and official is not None:
            self.official = official
            self._update_footer()
            self._reload()
            date = official.updated or time.strftime("%Y-%m-%d")
            self.update_status.setText(tr("Lista de masters actualizada ({date})", date=date))
        elif kind == "unchanged":
            self.update_status.setText(tr("La lista de masters está al día"))
        else:
            self.update_status.setText(tr("No se pudo descargar la lista de masters"))
