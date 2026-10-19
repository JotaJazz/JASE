"""Ventana principal estilo All-Seeing-Eye: juegos, servidores, jugadores y reglas."""
from __future__ import annotations

from pathlib import Path

from keyring.errors import KeyringError
from PySide6.QtCore import QItemSelection, Qt, Slot
from PySide6.QtGui import QCloseEvent
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QComboBox,
    QDialog,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QListWidget,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QSplitter,
    QTableView,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from jase import launcher, passwords, scanner
from jase.config import Config, masters_for, save_config, version_for
from jase.games import ALL_VERSIONS, GAMES, GameDef
from jase.launcher import LaunchError
from jase.models import ServerInfo
from jase.ui import theme
from jase.ui.dialogs import PasswordDialog, SettingsDialog
from jase.ui.scan_worker import ScanWorker
from jase.ui.server_model import ServerFilterProxy, ServerTableModel

_NEXT_THEME = {"system": "dark", "dark": "light", "light": "system"}
_THEME_NAMES = {"system": "Sistema", "dark": "Oscuro", "light": "Claro"}


def _accepted(result) -> bool:
    return result in (QDialog.DialogCode.Accepted, QDialog.DialogCode.Accepted.value)


class MainWindow(QMainWindow):
    def __init__(self, cfg: Config, cfg_path: Path, scan_fn=scanner.scan,
                 launch_fn=launcher.launch) -> None:
        super().__init__()
        self.cfg = cfg
        self.cfg_path = Path(cfg_path)
        self._scan_fn = scan_fn
        self._launch_fn = launch_fn

        self.game_ids: list[str] = list(GAMES)
        self.current_game: str | None = None
        self._cache: dict[str, list[ServerInfo]] = {}
        self._generation = 0
        self._worker: ScanWorker | None = None
        # Workers vivos (incluidos los cancelados) hasta su señal `finished`.
        self._workers: set[ScanWorker] = set()

        self.model = ServerTableModel(self)
        self.proxy = ServerFilterProxy(self)
        self.proxy.setSourceModel(self.model)

        self.setWindowTitle("Jimmy's All Seeing Eye")
        self.resize(1100, 700)
        self._build_ui()
        self._update_sidebar()
        self._update_connect_enabled()
        self._update_count_label()

        app = QApplication.instance()
        if app is not None:
            app.styleHints().colorSchemeChanged.connect(self._on_color_scheme_changed)
        self.apply_theme()

    # ------------------------------------------------------------------ UI
    def _build_ui(self) -> None:
        title = QLabel("👁 Jimmy's All Seeing Eye")
        title.setObjectName("title")
        font = title.font()
        font.setBold(True)
        font.setPointSizeF(font.pointSizeF() * 1.15)
        title.setFont(font)

        self.filter_edit = QLineEdit()
        self.filter_edit.setPlaceholderText("Filtrar servidores…")
        self.filter_edit.setClearButtonEnabled(True)
        self.filter_edit.textChanged.connect(self.proxy.set_text)

        self.version_combo = QComboBox()
        self.version_combo.setToolTip("Versión del juego")
        self.version_combo.setMinimumContentsLength(6)
        self.version_combo.currentTextChanged.connect(self._on_version_changed)

        self.refresh_button = QPushButton("⟳ Refrescar")
        self.refresh_button.clicked.connect(self.refresh)

        self.connect_button = QPushButton("▶ Conectar")
        self.connect_button.setObjectName("connect")
        self.connect_button.clicked.connect(self.connect_selected)

        self.theme_button = QPushButton()
        self.theme_button.clicked.connect(self.cycle_theme)

        self.settings_button = QPushButton("⚙ Ajustes")
        self.settings_button.clicked.connect(self.open_settings)

        top = QHBoxLayout()
        top.addWidget(title)
        top.addSpacing(12)
        top.addWidget(self.filter_edit, 1)
        top.addWidget(self.version_combo)
        top.addWidget(self.refresh_button)
        top.addWidget(self.connect_button)
        top.addWidget(self.theme_button)
        top.addWidget(self.settings_button)

        self.sidebar = QListWidget()
        for game_id in self.game_ids:
            self.sidebar.addItem(GAMES[game_id].name)
        self.sidebar.setMinimumWidth(190)
        self.sidebar.currentRowChanged.connect(self._on_sidebar_row)

        self.table = QTableView()
        self.table.setModel(self.proxy)
        self.table.setSortingEnabled(True)
        self.table.sortByColumn(2, Qt.AscendingOrder)
        self.table.setSelectionBehavior(QAbstractItemView.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SingleSelection)
        self.table.setEditTriggers(QAbstractItemView.NoEditTriggers)
        self.table.verticalHeader().setVisible(False)
        self.table.setShowGrid(False)
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(QHeaderView.Interactive)
        header.setSectionResizeMode(1, QHeaderView.Stretch)
        header.resizeSection(0, 28)
        self.table.selectionModel().selectionChanged.connect(self._on_selection_changed)
        self.table.doubleClicked.connect(lambda _index: self.connect_selected())

        self.players_table = self._make_detail_table(["Nombre", "Puntos", "Ping"])
        self.rules_table = self._make_detail_table(["Variable", "Valor"])
        details = QSplitter(Qt.Horizontal)
        details.addWidget(self.players_table)
        details.addWidget(self.rules_table)

        right = QSplitter(Qt.Vertical)
        right.addWidget(self.table)
        right.addWidget(details)
        right.setStretchFactor(0, 3)
        right.setStretchFactor(1, 2)

        body = QSplitter(Qt.Horizontal)
        body.addWidget(self.sidebar)
        body.addWidget(right)
        body.setStretchFactor(1, 1)
        body.setSizes([200, 900])

        central = QWidget()
        layout = QVBoxLayout(central)
        layout.setContentsMargins(10, 10, 10, 6)
        layout.addLayout(top)
        layout.addWidget(body, 1)
        self.setCentralWidget(central)

        self.scanning_label = QLabel("")
        self.count_label = QLabel("")
        self.statusBar().addPermanentWidget(self.scanning_label)
        self.statusBar().addPermanentWidget(self.count_label)
        for sig in (self.proxy.rowsInserted, self.proxy.rowsRemoved,
                    self.proxy.modelReset, self.proxy.layoutChanged):
            sig.connect(self._update_count_label)

    @staticmethod
    def _make_detail_table(headers: list[str]) -> QTableWidget:
        t = QTableWidget(0, len(headers))
        t.setHorizontalHeaderLabels(headers)
        t.setEditTriggers(QAbstractItemView.NoEditTriggers)
        t.setSelectionBehavior(QAbstractItemView.SelectRows)
        t.verticalHeader().setVisible(False)
        t.setShowGrid(False)
        t.horizontalHeader().setSectionResizeMode(0, QHeaderView.Stretch)
        return t

    # --------------------------------------------------------------- juegos
    def select_game(self, game_id: str) -> None:
        if game_id not in GAMES or game_id == self.current_game:
            return
        self._stop_active_scan()
        self.current_game = game_id
        self.sidebar.blockSignals(True)
        self.sidebar.setCurrentRow(self.game_ids.index(game_id))
        self.sidebar.blockSignals(False)

        self.model.clear()
        self._populate_version_combo()
        cached = self._cache.get(game_id)
        if cached is not None:
            for info in cached:
                self.model.add_or_update(info)
            self._add_observed_versions(cached)
            self.statusBar().showMessage(
                f"{GAMES[game_id].name}: lista guardada de la sesión")
        else:
            self._start_scan()
        self._after_list_changed()

    def refresh(self) -> None:
        if self.current_game is None:
            return
        self._stop_active_scan()
        self._cache.pop(self.current_game, None)
        self.model.clear()
        self._start_scan()
        self._after_list_changed()

    def is_scanning(self) -> bool:
        return self._worker is not None

    def _game(self) -> GameDef | None:
        return GAMES.get(self.current_game) if self.current_game else None

    def _start_scan(self) -> None:
        game = self._game()
        if game is None:
            return
        self._generation += 1
        worker = ScanWorker(game, masters_for(game, self.cfg), scan_fn=self._scan_fn)
        worker.generation = self._generation
        worker.server_found.connect(self._on_server_found)
        worker.status.connect(self._on_status)
        worker.finished.connect(self._on_worker_finished)
        self._workers.add(worker)
        self._worker = worker
        self.scanning_label.setText("Escaneando…")
        self.statusBar().showMessage(f"Consultando masters de {game.name}…")
        worker.start()

    def _stop_active_scan(self) -> None:
        """Cancela el worker activo sin bloquear; sus señales quedan obsoletas."""
        if self._worker is not None:
            self._worker.cancel()
            self._worker = None
            self._generation += 1
            self.scanning_label.setText("")
            # Un escaneo interrumpido no se guarda: al volver se escanea de nuevo.
            if self.current_game is not None:
                self._cache.pop(self.current_game, None)
        self._update_sidebar()

    def _is_current(self, worker) -> bool:
        return (worker is not None and worker is self._worker
                and worker.generation == self._generation)

    @Slot(object)
    def _on_server_found(self, info: ServerInfo) -> None:
        if not self._is_current(self.sender()):
            return
        self.model.add_or_update(info)
        self._add_observed_versions([info])
        self._update_sidebar()
        selected = self.selected_server()
        if selected is not None and selected.address == info.address:
            self._show_details(info)

    @Slot(str)
    def _on_status(self, message: str) -> None:
        if self._is_current(self.sender()):
            self.statusBar().showMessage(message)

    @Slot()
    def _on_worker_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, ScanWorker):
            return
        worker.wait()  # ya terminó run(); asegura que el hilo esté cerrado
        current = self._is_current(worker)
        self._workers.discard(worker)
        if current:
            self._worker = None
            self.scanning_label.setText("")
            if self.current_game is not None:
                self._cache[self.current_game] = self.model.servers()
            self._update_sidebar()

    def _update_sidebar(self) -> None:
        for row, game_id in enumerate(self.game_ids):
            if game_id == self.current_game and (self.is_scanning() or game_id in self._cache):
                count = str(self.model.rowCount())
            elif game_id in self._cache:
                count = str(len(self._cache[game_id]))
            else:
                count = "—"
            self.sidebar.item(row).setText(f"{GAMES[game_id].name} · {count}")

    @Slot(int)
    def _on_sidebar_row(self, row: int) -> None:
        if 0 <= row < len(self.game_ids):
            self.select_game(self.game_ids[row])

    # ------------------------------------------------------------ versiones
    def _populate_version_combo(self) -> None:
        game = self._game()
        if game is None:
            return
        selected = version_for(game, self.cfg)
        items = [ALL_VERSIONS, *game.known_versions]
        if selected not in items:
            items.append(selected)
        self.version_combo.blockSignals(True)
        self.version_combo.clear()
        self.version_combo.addItems(items)
        self.version_combo.setCurrentText(selected)
        self.version_combo.blockSignals(False)
        self.proxy.set_version(selected)

    def _add_observed_versions(self, servers: list[ServerInfo]) -> None:
        for info in servers:
            if info.version and self.version_combo.findText(info.version) < 0:
                self.version_combo.addItem(info.version)

    @Slot(str)
    def _on_version_changed(self, version: str) -> None:
        game = self._game()
        if game is None or not version:
            return
        self.proxy.set_version(version)
        self.cfg.version_filter[game.id] = version
        self._save_config()
        self._after_list_changed()

    # ------------------------------------------------------------ selección
    def selected_server(self) -> ServerInfo | None:
        rows = self.table.selectionModel().selectedRows()
        if not rows:
            return None
        source = self.proxy.mapToSource(rows[0])
        if not source.isValid():
            return None
        return self.model.server_at(source.row())

    @Slot(QItemSelection, QItemSelection)
    def _on_selection_changed(self, *_args) -> None:
        self._after_list_changed()

    def _after_list_changed(self) -> None:
        self._show_details(self.selected_server())
        self._update_connect_enabled()
        self._update_count_label()

    def _show_details(self, info: ServerInfo | None) -> None:
        players = info.players if info else []
        rules = sorted(info.rules.items()) if info else []
        self.players_table.setSortingEnabled(False)
        self.players_table.setRowCount(len(players))
        for r, p in enumerate(players):
            self.players_table.setItem(r, 0, QTableWidgetItem(p.name))
            for c, value in ((1, p.score), (2, p.ping)):
                item = QTableWidgetItem()
                item.setData(Qt.DisplayRole, value)
                self.players_table.setItem(r, c, item)
        self.players_table.setSortingEnabled(True)
        self.rules_table.setRowCount(len(rules))
        for r, (k, v) in enumerate(rules):
            self.rules_table.setItem(r, 0, QTableWidgetItem(k))
            self.rules_table.setItem(r, 1, QTableWidgetItem(v))

    def _update_connect_enabled(self) -> None:
        game = self._game()
        has_path = bool(game and self.cfg.game_paths.get(game.id))
        self.connect_button.setEnabled(has_path and self.selected_server() is not None)

    def _update_count_label(self, *_args) -> None:
        n = self.proxy.rowCount()
        self.count_label.setText(f"{n} servidor" if n == 1 else f"{n} servidores")

    # ------------------------------------------------------------- conectar
    def connect_selected(self) -> None:
        game = self._game()
        if game is None:
            return
        install_dir = self.cfg.game_paths.get(game.id)
        if not install_dir:
            self._prompt_missing_path(game)
            return
        info = self.selected_server()
        if info is None:
            return
        password: str | None = None
        if info.password:
            dialog = PasswordDialog(info.name, passwords.get_password(game.id, info.address), self)
            try:
                result = dialog.exec()
                forget = dialog.forget_requested
                typed = dialog.password()
            finally:
                dialog.deleteLater()
            if not _accepted(result):
                if forget:
                    passwords.forget_password(game.id, info.address)
                    self.statusBar().showMessage("Contraseña olvidada")
                return
            password = typed or None
            if password:
                try:
                    passwords.set_password(game.id, info.address, password)
                except KeyringError as exc:
                    QMessageBox.warning(
                        self, "Contraseña",
                        f"No se pudo guardar la contraseña: {exc}\n"
                        "Se usará solo para esta conexión.")
        try:
            self._launch_fn(game, install_dir, info.connect_address, password)
        except (LaunchError, OSError) as exc:
            QMessageBox.warning(self, "No se pudo conectar", str(exc))
            return
        self.statusBar().showMessage(f"Lanzando {game.name} → {info.connect_address}")

    def _prompt_missing_path(self, game: GameDef) -> None:
        box = QMessageBox(self)
        box.setIcon(QMessageBox.Information)
        box.setWindowTitle("Ruta no configurada")
        box.setText(f"No hay ruta configurada para {game.name}.")
        box.setInformativeText("Configúrala en Ajustes para poder conectarte.")
        open_button = box.addButton("Abrir ajustes", QMessageBox.AcceptRole)
        box.addButton("Cancelar", QMessageBox.RejectRole)
        box.exec()
        clicked = box.clickedButton()
        box.deleteLater()
        if clicked is open_button:
            self.open_settings()

    def open_settings(self) -> None:
        dialog = SettingsDialog(self.cfg, self)
        try:
            if _accepted(dialog.exec()):
                dialog.apply_to(self.cfg)
                self._save_config()
        finally:
            dialog.deleteLater()
        self._update_connect_enabled()

    # ---------------------------------------------------------------- tema
    def cycle_theme(self) -> None:
        self.cfg.theme = _NEXT_THEME.get(self.cfg.theme, "system")
        self._save_config()
        self.apply_theme()

    def apply_theme(self) -> None:
        app = QApplication.instance()
        scheme = app.styleHints().colorScheme() if app else Qt.ColorScheme.Unknown
        mode = theme.resolve_mode(self.cfg.theme, scheme)
        if app is not None:
            app.setStyleSheet(theme.build_stylesheet(mode))
        self.model.set_mode(mode)
        self.theme_button.setText("☾" if mode == "dark" else "☀")
        self.theme_button.setToolTip(
            f"Tema: {_THEME_NAMES.get(self.cfg.theme, 'Sistema')} (clic para cambiar)")

    @Slot(Qt.ColorScheme)
    def _on_color_scheme_changed(self, _scheme) -> None:
        if self.cfg.theme == "system":
            self.apply_theme()

    # ---------------------------------------------------------------- otros
    def _save_config(self) -> None:
        try:
            save_config(self.cfg, self.cfg_path)
        except OSError as exc:
            self.statusBar().showMessage(f"No se pudo guardar la configuración: {exc}")

    def closeEvent(self, event: QCloseEvent) -> None:
        self._worker = None
        self._generation += 1
        workers = list(self._workers)
        for worker in workers:
            worker.cancel()
        for worker in workers:
            worker.wait()
        self._workers.clear()
        super().closeEvent(event)
