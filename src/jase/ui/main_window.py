"""Ventana principal estilo All-Seeing-Eye: juegos, servidores, jugadores y reglas."""
from __future__ import annotations

import base64
import binascii
import time
from pathlib import Path

from keyring.errors import KeyringError
from PySide6.QtCore import QByteArray, QItemSelection, QPoint, Qt, QThread, Slot
from PySide6.QtGui import QCloseEvent, QKeySequence, QPixmap, QShortcut
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QCheckBox,
    QComboBox,
    QDialog,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QListWidget,
    QMainWindow,
    QMenu,
    QMessageBox,
    QProgressBar,
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
from jase.geo import DB_FILENAME, country_name
from jase.launcher import LaunchError
from jase.maps.service import MapImageService
from jase.models import ServerInfo
from jase.protocols.quake3 import strip_colors
from jase.ui import country_loader, favorites, theme
from jase.ui.colored_text import COLOR_RUNS_ROLE, ColoredTextDelegate, color_runs
from jase.ui.country_loader import CountryDBRequest
from jase.ui.dialogs import PasswordDialog, SettingsDialog
from jase.ui.map_loader import MapImageRequest, shared_loader
from jase.ui.scan_worker import QueryWorker, ScanWorker
from jase.ui.server_card import ServerCard
from jase.ui.server_model import (
    COL_COUNTRY,
    COL_LOCK,
    COL_MAP,
    COL_MODE,
    COL_NAME,
    COL_PING,
    COL_PLAYERS,
    COL_VERSION,
    COLUMNS,
    FLAG_SIZE,
    OccupancyDelegate,
    PingDelegate,
    QuickFilters,
    ServerFilterProxy,
    ServerTableModel,
)

# Workers que no terminaron dentro del plazo de cierre: se mantienen referenciados
# para que el QThread no se destruya mientras corre (ver shutdown_lingering_workers).
_LINGERING: set[QThread] = set()

_NEXT_THEME = {"system": "dark", "dark": "light", "light": "system"}
_THEME_NAMES = {"system": "Sistema", "dark": "Oscuro", "light": "Claro"}
# Anchos iniciales (px) de las columnas de la tabla; "Servidor" ocupa el resto.
COLUMN_WIDTHS = {COL_LOCK: 28, COL_COUNTRY: 40, COL_PING: 70, COL_PLAYERS: 90, COL_MAP: 170,
                 COL_MODE: 150, COL_VERSION: 70}
# Opciones del filtro "Ping máx." (texto mostrado → ms; 0 = sin límite).
MAX_PING_OPTIONS = {"Sin límite": 0, "50": 50, "100": 100, "150": 150, "250": 250}


def _accepted(result) -> bool:
    return result in (QDialog.DialogCode.Accepted, QDialog.DialogCode.Accepted.value)


def shutdown_lingering_workers(timeout_ms: int = 3000) -> None:
    """Al salir de la app: espera a los workers rezagados; terminate() como último recurso."""
    deadline = time.monotonic() + timeout_ms / 1000
    for worker in list(_LINGERING):
        remaining = max(0, int((deadline - time.monotonic()) * 1000))
        if not worker.wait(remaining):
            worker.terminate()
            worker.wait()
        _LINGERING.discard(worker)


OFFLINE_CONNECT_MESSAGE = "El servidor no responde; refresca para conectar"


class MainWindow(QMainWindow):
    # Plazo total para esperar a los workers al cerrar la ventana.
    CLOSE_WAIT_MS = 2000

    def __init__(self, cfg: Config, cfg_path: Path, scan_fn=scanner.scan,
                 launch_fn=launcher.launch, map_service=None,
                 query_fn=scanner.query_servers, geo=None) -> None:
        super().__init__()
        self.cfg = cfg
        self.cfg_path = Path(cfg_path)
        self._scan_fn = scan_fn
        self._launch_fn = launch_fn
        self._query_fn = query_fn  # para refrescar un servidor suelto
        # Caché junto a la config (%APPDATA%\JimmysASE\cache\maps en Windows).
        self._map_service = map_service or MapImageService(
            self.cfg_path.parent / "cache" / "maps")
        self._map_loader = shared_loader()
        self._map_loader.relay.loaded.connect(self._on_map_image_loaded)
        self._card_key: tuple | None = None  # (juego, dirección, mapa) mostrado
        self._card_request: MapImageRequest | None = None  # petición vigente
        # Base de países (geo.CountrySource); None = sin columna País (p. ej. tests).
        self._geo = geo
        self.country_db_path = self.cfg_path.parent / "cache" / DB_FILENAME
        self._country_db = None
        self._country_request: CountryDBRequest | None = None

        self.game_ids: list[str] = self._sorted_game_ids()  # orden de las filas
        self.current_game: str | None = None
        self._cache: dict[str, list[ServerInfo]] = {}
        self._generation = 0
        self._mode = "dark"  # tema efectivo; lo fija apply_theme()
        self._worker: ScanWorker | None = None
        # Workers vivos (escaneo y consulta, incluidos los cancelados) hasta `finished`.
        self._workers: set[QThread] = set()
        self._query_worker: QueryWorker | None = None  # «Refrescar este servidor» vigente
        # Vista «★ Favoritos» del juego actual (no cambia current_game) y su consulta.
        self._favorites_view = False
        self._favorites_worker: QueryWorker | None = None
        self._favorites_answered: set = set()  # direcciones que respondieron (consulta vigente)

        self.model = ServerTableModel(self)
        self.proxy = ServerFilterProxy(self)
        self.proxy.setSourceModel(self.model)
        self.proxy.set_quick_filters(cfg.quick_filters)

        self.setWindowTitle("Jimmy's All Seeing Eye")
        self.resize(1100, 700)
        self._build_ui()
        self._restore_ui_state()
        self._update_sidebar()
        self._update_connect_enabled()
        self._update_count_label()

        app = QApplication.instance()
        if app is not None:
            app.styleHints().colorSchemeChanged.connect(self._on_color_scheme_changed)
        self.apply_theme()

        country_loader.shared_relay().loaded.connect(self._on_country_db_loaded)
        self._start_country_db(refresh=False)

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
        self.filter_edit.textChanged.connect(self._after_list_changed)

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
        self._fill_sidebar()
        self.sidebar.setMinimumWidth(190)
        # Sin scroll horizontal: nombre corto + conteo; si aun así no entra, se
        # recorta el medio y el conteo sigue visible.
        self.sidebar.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.sidebar.setTextElideMode(Qt.ElideMiddle)
        self.sidebar.currentRowChanged.connect(self._on_sidebar_row)
        # Clic en el juego ya seleccionado: sale de la vista Favoritos.
        self.sidebar.itemClicked.connect(self._on_sidebar_clicked)
        self.sidebar.itemActivated.connect(self._on_sidebar_clicked)  # Enter / doble clic
        self.favorites_list = QListWidget()
        self.favorites_list.setObjectName("favorites_list")
        self.favorites_list.addItem("★ Favoritos")
        self.favorites_list.setToolTip(
            "Favoritos del juego seleccionado: se consultan directamente, sin master")
        self.favorites_list.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.favorites_list.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.favorites_list.itemClicked.connect(lambda _item: self.show_favorites())
        self.favorites_list.itemActivated.connect(lambda _item: self.show_favorites())
        sidebar_panel = self._build_quick_filters()

        self.table = QTableView()
        self.table.setModel(self.proxy)
        self.table.setSortingEnabled(True)
        self.table.sortByColumn(COL_PING, Qt.AscendingOrder)
        self.table.setSelectionBehavior(QAbstractItemView.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SingleSelection)
        self.table.setEditTriggers(QAbstractItemView.NoEditTriggers)
        self.table.verticalHeader().setVisible(False)
        self.table.setShowGrid(False)
        self.table.setIconSize(FLAG_SIZE)
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(QHeaderView.Interactive)
        header.setSectionResizeMode(COL_NAME, QHeaderView.Stretch)
        for col, width in COLUMN_WIDTHS.items():
            header.resizeSection(col, width)
        self._no_wrap(self.table)
        self.table.setItemDelegateForColumn(COL_NAME, ColoredTextDelegate(self.table))
        self.table.setItemDelegateForColumn(COL_PING, PingDelegate(self.table))
        self.occupancy_delegate = OccupancyDelegate(self.table)
        self.table.setItemDelegateForColumn(COL_PLAYERS, self.occupancy_delegate)
        self.table.selectionModel().selectionChanged.connect(self._on_selection_changed)
        # Un favorito offline que pasa a responder (o al revés) cambia «Conectar».
        self.proxy.dataChanged.connect(lambda *_a: self._update_connect_enabled())
        self.table.doubleClicked.connect(lambda _index: self.connect_selected())
        self._setup_context_menu()

        self.players_table = self._make_detail_table(["Nombre", "Puntos", "Ping"])
        self.players_table.setItemDelegateForColumn(0, ColoredTextDelegate(self.players_table))
        self.rules_table = self._make_detail_table(["Variable", "Valor"])
        details = self.details_splitter = QSplitter(Qt.Horizontal)
        details.addWidget(self.players_table)
        details.addWidget(self.rules_table)

        self.server_card = ServerCard()
        self.server_card.copy_requested.connect(
            lambda text: self.statusBar().showMessage(f"Dirección copiada: {text}", 4000))
        self.server_card.favorite_toggle_requested.connect(self.toggle_favorite_selected)
        lower = QWidget()
        lower_layout = QVBoxLayout(lower)
        lower_layout.setContentsMargins(0, 0, 0, 0)
        lower_layout.setSpacing(6)
        lower_layout.addWidget(self.server_card)
        lower_layout.addWidget(details, 1)

        right = self.right_splitter = QSplitter(Qt.Vertical)
        right.addWidget(self.table)
        right.addWidget(lower)
        right.setStretchFactor(0, 3)
        right.setStretchFactor(1, 2)

        body = self.body_splitter = QSplitter(Qt.Horizontal)
        body.addWidget(sidebar_panel)
        body.addWidget(right)
        body.setStretchFactor(1, 1)
        body.setSizes([230, 900])

        central = QWidget()
        layout = QVBoxLayout(central)
        layout.setContentsMargins(10, 10, 10, 6)
        layout.addLayout(top)
        layout.addWidget(body, 1)
        self.setCentralWidget(central)

        self.scanning_label = QLabel("")
        self.count_label = QLabel("")
        self.scan_progress = QProgressBar()
        self.scan_progress.setFormat("Consultando %v / %m")
        self.scan_progress.setTextVisible(True)
        self.scan_progress.setMaximumWidth(220)
        self.scan_progress.hide()
        self.statusBar().addPermanentWidget(self.scan_progress)
        self.statusBar().addPermanentWidget(self.scanning_label)
        self.statusBar().addPermanentWidget(self.count_label)
        for sig in (self.proxy.rowsInserted, self.proxy.rowsRemoved,
                    self.proxy.modelReset, self.proxy.layoutChanged):
            sig.connect(self._update_count_label)
        self._setup_shortcuts()

    def _setup_context_menu(self) -> None:
        self.table.setContextMenuPolicy(Qt.CustomContextMenu)
        self.table.customContextMenuRequested.connect(self._on_table_context_menu)

    def _setup_shortcuts(self) -> None:
        """Enter/Ctrl+C solo con la tabla enfocada; Ctrl+F y F5 en toda la ventana."""
        table_keys = ((Qt.Key_Return, self._connect_if_selected),
                      (Qt.Key_Enter, self._connect_if_selected),
                      (QKeySequence.Copy, self.copy_selected_address))
        for key, slot in table_keys:
            shortcut = QShortcut(QKeySequence(key), self.table)
            shortcut.setContext(Qt.WidgetShortcut)
            shortcut.activated.connect(slot)
        QShortcut(QKeySequence.Find, self).activated.connect(self._focus_filter)
        QShortcut(QKeySequence(Qt.Key_F5), self).activated.connect(self.refresh)

    def _connect_if_selected(self) -> None:
        if self.selected_server() is not None:
            self.connect_selected()

    def _focus_filter(self) -> None:
        self.filter_edit.setFocus(Qt.ShortcutFocusReason)
        self.filter_edit.selectAll()

    def _build_context_menu(self) -> QMenu:
        """Menú de la fila seleccionada; acciones deshabilitadas si no hay selección."""
        menu = QMenu(self)
        has_server = self.selected_server() is not None
        for text, slot in (("Conectar", self.connect_selected),
                           ("Copiar IP:puerto", self.copy_selected_address),
                           ("Refrescar este servidor", self.refresh_selected)):
            action = menu.addAction(text)
            action.setEnabled(has_server)
            action.triggered.connect(slot)
        menu.addSeparator()
        text = "Quitar de favoritos" if self._is_selected_favorite() else "Agregar a favoritos"
        action = menu.addAction(text)
        action.setEnabled(has_server)
        action.triggered.connect(self.toggle_favorite_selected)
        return menu

    @Slot(QPoint)
    def _on_table_context_menu(self, pos: QPoint) -> None:
        index = self.table.indexAt(pos)
        if index.isValid():
            self.table.selectRow(index.row())
        else:
            self.table.clearSelection()  # espacio vacío: menú con acciones deshabilitadas
        menu = self._build_context_menu()
        try:
            menu.exec(self.table.viewport().mapToGlobal(pos))
        finally:
            menu.deleteLater()

    def _build_quick_filters(self) -> QWidget:
        """Lista de juegos y, debajo, los filtros rápidos (reflejan la config)."""
        q = self.cfg.quick_filters
        self.hide_empty_check = QCheckBox("Ocultar vacíos")
        self.hide_full_check = QCheckBox("Ocultar llenos")
        self.hide_password_check = QCheckBox("Ocultar con contraseña")
        for check, value in ((self.hide_empty_check, q.hide_empty),
                             (self.hide_full_check, q.hide_full),
                             (self.hide_password_check, q.hide_password)):
            check.setChecked(value)
            check.toggled.connect(self._on_quick_filters_changed)

        self.max_ping_combo = QComboBox()
        self.max_ping_combo.setToolTip("Ocultar servidores con ping mayor")
        for text, ms in MAX_PING_OPTIONS.items():
            self.max_ping_combo.addItem(text, ms)
        idx = self.max_ping_combo.findData(q.max_ping)
        if idx < 0:  # valor guardado fuera de la lista: se conserva tal cual
            self.max_ping_combo.addItem(str(q.max_ping), q.max_ping)
            idx = self.max_ping_combo.count() - 1
        self.max_ping_combo.setCurrentIndex(idx)
        self.max_ping_combo.currentIndexChanged.connect(self._on_quick_filters_changed)

        ping_row = QHBoxLayout()
        ping_row.setContentsMargins(0, 0, 0, 0)
        ping_row.addWidget(QLabel("Ping máx."))
        ping_row.addWidget(self.max_ping_combo, 1)

        panel = QWidget()
        layout = QVBoxLayout(panel)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(self.sidebar, 1)
        layout.addWidget(self.favorites_list)
        layout.addWidget(self.hide_empty_check)
        layout.addWidget(self.hide_full_check)
        layout.addWidget(self.hide_password_check)
        layout.addLayout(ping_row)
        return panel

    def _on_quick_filters_changed(self, *_args) -> None:
        self.cfg.quick_filters = QuickFilters(
            hide_empty=self.hide_empty_check.isChecked(),
            hide_full=self.hide_full_check.isChecked(),
            hide_password=self.hide_password_check.isChecked(),
            max_ping=int(self.max_ping_combo.currentData() or 0),
        )
        self.proxy.set_quick_filters(self.cfg.quick_filters)
        self._save_config()
        self._after_list_changed()

    @staticmethod
    def _make_detail_table(headers: list[str]) -> QTableWidget:
        t = QTableWidget(0, len(headers))
        t.setHorizontalHeaderLabels(headers)
        t.setEditTriggers(QAbstractItemView.NoEditTriggers)
        t.setSelectionBehavior(QAbstractItemView.SelectRows)
        t.verticalHeader().setVisible(False)
        t.setShowGrid(False)
        t.horizontalHeader().setSectionResizeMode(0, QHeaderView.Stretch)
        MainWindow._no_wrap(t)
        return t

    @staticmethod
    def _no_wrap(view: QAbstractItemView) -> None:
        # Una línea por fila: los nombres largos se recortan con "…" (tooltip completo).
        view.setWordWrap(False)
        view.setTextElideMode(Qt.ElideRight)
        view.verticalHeader().setSectionResizeMode(QHeaderView.Fixed)
        view.verticalHeader().setDefaultSectionSize(28)

    # --------------------------------------------------------------- juegos
    def select_game(self, game_id: str) -> None:
        """Muestra la lista normal del juego; desde la vista Favoritos también
        con el juego actual (vuelve a su lista guardada o escanea)."""
        if game_id not in GAMES or (game_id == self.current_game
                                    and not self._favorites_view):
            return
        self._stop_active_scan()
        self._cancel_query()
        self._leave_favorites_view()
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
        if self._favorites_view:
            self._query_favorites()  # F5 en Favoritos: reconsultar, sin master
            return
        self._stop_active_scan()
        self._cancel_query()
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
        worker.progress.connect(self._on_progress)
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
            self.scan_progress.hide()
            # Un escaneo interrumpido no se guarda; se conserva el último completo
            # (si existe) y, si no hay ninguno, al volver se escanea de nuevo.
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

    @Slot(int, int)
    def _on_progress(self, done: int, total: int) -> None:
        if not self._is_current(self.sender()):
            return
        self.scan_progress.setRange(0, total)
        self.scan_progress.setValue(done)
        self.scan_progress.show()

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
            self.scan_progress.hide()
            # Solo un escaneo completo y sin error reemplaza la lista guardada.
            if self.current_game is not None and not worker.failed:
                self._cache[self.current_game] = self.model.servers()
                if worker.result is not None:
                    self.statusBar().showMessage(
                        f"{worker.result.servers_responded} de "
                        f"{worker.result.servers_listed} servidores respondieron")
            self._update_sidebar()

    def _sorted_game_ids(self) -> list[str]:
        """Juegos con ruta configurada primero; dentro de cada grupo, orden de GAMES."""
        paths = self.cfg.game_paths
        return sorted(GAMES, key=lambda gid: not paths.get(gid))

    def _resort_sidebar(self) -> None:
        """Reordena las filas si cambiaron las rutas, conservando el juego actual."""
        new_ids = self._sorted_game_ids()
        if new_ids == self.game_ids:
            return
        self.game_ids = new_ids
        self.sidebar.blockSignals(True)
        try:
            self.sidebar.clear()
            self._fill_sidebar()
            if self.current_game in self.game_ids:
                self.sidebar.setCurrentRow(self.game_ids.index(self.current_game))
        finally:
            self.sidebar.blockSignals(False)
        self._update_sidebar()

    def _fill_sidebar(self) -> None:
        """Una fila por juego (texto final en _update_sidebar; nombre completo en el tooltip)."""
        for game_id in self.game_ids:
            game = GAMES[game_id]
            self.sidebar.addItem(game.short_name)
            self.sidebar.item(self.sidebar.count() - 1).setToolTip(game.name)

    def _update_sidebar(self) -> None:
        for row, game_id in enumerate(self.game_ids):
            if game_id == self.current_game and self.is_scanning():
                count = str(self.model.rowCount())  # conteo en vivo
            elif game_id in self._cache:
                count = str(len(self._cache[game_id]))
            else:
                count = "—"
            self.sidebar.item(row).setText(f"{GAMES[game_id].short_name} · {count}")
        text = "★ Favoritos"
        if self.current_game is not None:
            text += f" · {len(favorites.favorite_addresses(self.cfg, self.current_game))}"
        self.favorites_list.item(0).setText(text)

    @Slot(int)
    def _on_sidebar_row(self, row: int) -> None:
        if 0 <= row < len(self.game_ids):
            self.select_game(self.game_ids[row])

    def _on_sidebar_clicked(self, item) -> None:
        self._on_sidebar_row(self.sidebar.row(item))

    # ------------------------------------------------------------ favoritos
    def in_favorites_view(self) -> bool:
        return self._favorites_view

    def show_favorites(self) -> None:
        """Vista «★ Favoritos» del juego actual: consulta directa, sin master.

        Los filtros rápidos y el de versión no se aplican (un favorito offline no
        tiene jugadores, ping ni versión); el filtro de texto sí. Se restauran al
        volver a un juego desde la barra lateral."""
        if self._game() is None or self._favorites_view:
            return
        self._stop_active_scan()
        self._cancel_query()
        self._favorites_view = True
        self._set_filters_bypassed(True)
        self._set_favorites_item_selected(True)
        self._query_favorites()

    def _leave_favorites_view(self) -> None:
        """Sale de la vista; quien llama (select_game) repone la lista y la versión."""
        if not self._favorites_view:
            return
        self._favorites_view = False
        self._cancel_favorites_query()
        self._set_filters_bypassed(False)
        self._set_favorites_item_selected(False)

    def _set_filters_bypassed(self, bypassed: bool) -> None:
        self.proxy.set_quick_filters(QuickFilters() if bypassed else self.cfg.quick_filters)
        if bypassed:
            self.proxy.set_version(ALL_VERSIONS)  # al salir lo repone _populate_version_combo
        for widget in (self.hide_empty_check, self.hide_full_check,
                       self.hide_password_check, self.max_ping_combo, self.version_combo):
            widget.setEnabled(not bypassed)

    def _fit_favorites_list(self) -> None:
        """Alto justo para su única fila (depende del padding de la hoja de estilos)."""
        lst = self.favorites_list
        lst.ensurePolished()
        lst.setFixedHeight(lst.sizeHintForRow(0) + 2 * lst.frameWidth() + 2)

    def _set_favorites_item_selected(self, selected: bool) -> None:
        self.favorites_list.blockSignals(True)
        try:
            self.favorites_list.setCurrentRow(0 if selected else -1)
            if not selected:
                self.favorites_list.clearSelection()
        finally:
            self.favorites_list.blockSignals(False)

    def _query_favorites(self) -> None:
        """(Re)consulta los favoritos; los que no respondan quedan como offline."""
        game = self._game()
        if game is None:
            return
        self._cancel_favorites_query()
        self._cancel_query()
        self.model.clear()
        addresses = favorites.favorite_addresses(self.cfg, game.id)
        if not addresses:
            self.statusBar().showMessage(
                f"No hay favoritos de {game.name}: agrégalos con clic derecho "
                "o con la estrella ☆ de la ficha")
            self._after_list_changed()
            return
        worker = QueryWorker(game, addresses, query_fn=self._query_fn)
        worker.server_found.connect(self._on_favorite_found)
        worker.finished.connect(self._on_favorites_finished)
        self._workers.add(worker)
        self._favorites_worker = worker
        self._favorites_answered = set()
        self.scanning_label.setText("Consultando favoritos…")
        self.statusBar().showMessage(f"Consultando {len(addresses)} favoritos de {game.name}…")
        worker.start()
        self._after_list_changed()

    def _cancel_favorites_query(self) -> None:
        if self._favorites_worker is not None:
            self._favorites_worker.cancel()  # sigue en _workers hasta `finished`
            self._favorites_worker = None
            self.scanning_label.setText("")

    @Slot(object)
    def _on_favorite_found(self, info: ServerInfo) -> None:
        worker = self.sender()
        if worker is not self._favorites_worker:
            return
        self._favorites_answered.add(info.address)
        if not favorites.is_favorite(self.cfg, worker.game.id, info.address):
            return  # se quitó de favoritos mientras se consultaba
        self.model.add_or_update(info)
        selected = self.selected_server()
        if selected is not None and selected.address == info.address:
            self._show_details(info)

    @Slot()
    def _on_favorites_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, QueryWorker):
            return
        worker.wait()
        self._workers.discard(worker)
        if worker is not self._favorites_worker:
            return  # cancelada: se salió de la vista o se volvió a consultar
        self._favorites_worker = None
        self.scanning_label.setText("")
        # Solo los que siguen siendo favoritos: uno quitado durante la consulta
        # no vuelve como "sin respuesta".
        current = [a for a in worker.addresses
                   if favorites.is_favorite(self.cfg, worker.game.id, a)]
        for address in current:
            if address not in self._favorites_answered:
                self.model.add_or_update(favorites.offline_server(address))
        online = sum(1 for a in current if a in self._favorites_answered)
        if worker.failed:
            self.statusBar().showMessage(f"Error al consultar favoritos: {worker.error}")
        else:
            self.statusBar().showMessage(f"Favoritos: {online} de {len(current)} respondieron")
        self._after_list_changed()

    def _is_selected_favorite(self) -> bool:
        game, info = self._game(), self.selected_server()
        return bool(game and info and favorites.is_favorite(self.cfg, game.id, info.address))

    def toggle_favorite_selected(self) -> None:
        """Agrega o quita el servidor seleccionado de los favoritos del juego.

        En la vista normal no consulta nada; en Favoritos, quitar borra la fila."""
        game, info = self._game(), self.selected_server()
        if game is None or info is None:
            return
        if favorites.is_favorite(self.cfg, game.id, info.address):
            favorites.remove_favorite(self.cfg, game.id, info.address)
            message = f"Quitado de favoritos: {info.address}"
            if self._favorites_view:
                self.model.remove(info.address)
        else:
            favorites.add_favorite(self.cfg, game.id, info.address)
            message = f"Agregado a favoritos: {info.address}"
        self._save_config()
        self._update_sidebar()
        self._after_list_changed()
        self.statusBar().showMessage(message, 4000)

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

    def _after_list_changed(self, *_args) -> None:
        self._show_details(self.selected_server())
        self._update_connect_enabled()
        self._update_count_label()

    def _show_details(self, info: ServerInfo | None) -> None:
        self._update_card(info)
        players = info.players if info else []
        rules = sorted(info.rules.items()) if info else []
        self.players_table.setSortingEnabled(False)
        self.players_table.setRowCount(len(players))
        for r, p in enumerate(players):
            name = QTableWidgetItem(p.name)
            if p.raw_name:
                name.setData(COLOR_RUNS_ROLE, color_runs(p.raw_name, self._mode))
            self.players_table.setItem(r, 0, name)
            for c, value in ((1, p.score), (2, p.ping)):
                item = QTableWidgetItem()
                item.setData(Qt.DisplayRole, value)
                self.players_table.setItem(r, c, item)
        self.players_table.setSortingEnabled(True)
        self.rules_table.setRowCount(len(rules))
        for r, (k, v) in enumerate(rules):
            value = QTableWidgetItem(strip_colors(v))
            value.setToolTip(value.text())
            self.rules_table.setItem(r, 0, QTableWidgetItem(k))
            self.rules_table.setItem(r, 1, value)

    def _update_card(self, info: ServerInfo | None) -> None:
        """Datos de la ficha al instante; la imagen del mapa se pide a un hilo."""
        self.server_card.set_server(info, self._mode)
        self.server_card.set_favorite(self._is_selected_favorite())
        code = self.model.country_code(info) if info else None
        self.server_card.set_country(code, country_name(code) if code else None)
        game = self._game()
        key = (game.id, info.address, info.map) if info and game else None
        if key == self._card_key:
            return  # mismo servidor y mapa: la imagen ya está o viene en camino
        self._card_key = key
        self._cancel_card_request()
        if key is None or not info.map or not info.online:
            return
        self._card_request = MapImageRequest(self._map_service, (
            game, self.cfg.game_paths.get(game.id), info.map, self.cfg.download_map_images))
        self._map_loader.submit(self._card_request)

    def _cancel_card_request(self) -> None:
        if self._card_request is not None:
            self._card_request.cancelled = True  # si no empezó, el hilo la salta
            self._card_request = None

    @Slot(object, object)
    def _on_map_image_loaded(self, request, path) -> None:
        if request is not self._card_request:
            return  # de otra ventana o de un servidor que ya no está seleccionado
        pixmap = QPixmap(str(path)) if path is not None else None
        self.server_card.set_map_image(pixmap if pixmap and not pixmap.isNull() else None)

    # --------------------------------------------------------------- países
    def _start_country_db(self, refresh: bool) -> bool:
        """Carga (o con `refresh`, descarga de nuevo) la base en un hilo daemon."""
        if self._geo is None or self._country_request is not None:
            return False
        self._country_request = CountryDBRequest(self._geo, self.country_db_path, refresh)
        country_loader.submit(self._country_request)
        return True

    def update_country_db(self) -> None:
        """Botón de Ajustes: vuelve a descargar la base; la actual sigue en uso."""
        if self._start_country_db(refresh=True):
            self.statusBar().showMessage("Descargando base de países…")

    def country_db_busy(self) -> bool:
        return self._country_request is not None

    @Slot(object, object, object)
    def _on_country_db_loaded(self, request, db, error) -> None:
        if request is not self._country_request:
            return  # de otra ventana
        self._country_request = None
        if db is None:
            # Sin diálogos: columna vacía (o la base anterior) y aviso breve.
            self.statusBar().showMessage(error or "Base de países no disponible", 8000)
            return
        self._country_db = db
        self.model.set_country_lookup(db.lookup)
        self._update_card(self.selected_server())
        if request.refresh:
            self.statusBar().showMessage("Base de países actualizada", 5000)

    def _update_connect_enabled(self) -> None:
        game = self._game()
        has_path = bool(game and self.cfg.game_paths.get(game.id))
        info = self.selected_server()
        # Un favorito sin respuesta solo tiene el puerto de consulta: no se lanza.
        self.connect_button.setEnabled(has_path and info is not None and info.online)

    def _update_count_label(self, *_args) -> None:
        n = self.proxy.rowCount()
        self.count_label.setText(f"{n} servidor" if n == 1 else f"{n} servidores")

    # ------------------------------------------------- copiar / refrescar uno
    def copy_selected_address(self) -> None:
        info = self.selected_server()
        if info is None:
            return
        text = str(info.connect_address)
        QApplication.clipboard().setText(text)
        self.statusBar().showMessage(f"Dirección copiada: {text}", 4000)

    def refresh_selected(self) -> None:
        """Vuelve a consultar el servidor seleccionado (dirección de consulta)."""
        game = self._game()
        info = self.selected_server()
        if game is None or info is None:
            return
        self._cancel_query()
        worker = QueryWorker(game, [info.address], query_fn=self._query_fn)
        worker.server_found.connect(self._on_query_server_found)
        worker.finished.connect(self._on_query_finished)
        self._workers.add(worker)
        self._query_worker = worker
        self.statusBar().showMessage(f"Consultando {info.address}…")
        worker.start()

    def _cancel_query(self) -> None:
        if self._query_worker is not None:
            self._query_worker.cancel()  # sigue en _workers hasta `finished`
            self._query_worker = None

    def _is_current_query(self, worker) -> bool:
        return (worker is not None and worker is self._query_worker
                and worker.game.id == self.current_game)

    @Slot(object)
    def _on_query_server_found(self, info: ServerInfo) -> None:
        if not self._is_current_query(self.sender()):
            return
        if self._favorites_view:
            game = self._game()
            if not favorites.is_favorite(self.cfg, game.id, info.address):
                return  # se quitó de favoritos mientras se consultaba
        else:
            self._add_observed_versions([info])
        self.model.add_or_update(info)
        if (self.current_game in self._cache and not self.is_scanning()
                and not self._favorites_view):
            self._cache[self.current_game] = self.model.servers()
        selected = self.selected_server()
        if selected is not None and selected.address == info.address:
            self._show_details(info)
        self.statusBar().showMessage(f"Servidor actualizado: {info.address}", 4000)

    @Slot()
    def _on_query_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, QueryWorker):
            return
        worker.wait()
        self._workers.discard(worker)
        if self._is_current_query(worker):
            self._query_worker = None
            if worker.failed:
                self.statusBar().showMessage(f"Error al consultar: {worker.error}")
            elif not worker.answered:
                self.statusBar().showMessage("El servidor no respondió")
                address = worker.addresses[0]
                if self._favorites_view and favorites.is_favorite(
                        self.cfg, worker.game.id, address):
                    # En Favoritos, un favorito que dejó de responder pasa a offline.
                    self.model.add_or_update(favorites.offline_server(address))
                    self._after_list_changed()

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
        if not info.online:
            self.statusBar().showMessage(OFFLINE_CONNECT_MESSAGE)
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
            if password is None:
                # Enviar vacío equivale a olvidar la guardada y conectar sin contraseña.
                passwords.forget_password(game.id, info.address)
            else:
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
            self._message_with_settings(QMessageBox.Warning, "No se pudo conectar", str(exc),
                                        "Revisa la ruta del juego en Ajustes.")
            return
        self.statusBar().showMessage(f"Lanzando {game.name} → {info.connect_address}")

    def _prompt_missing_path(self, game: GameDef) -> None:
        self._message_with_settings(QMessageBox.Information, "Ruta no configurada",
                                    f"No hay ruta configurada para {game.name}.",
                                    "Configúrala en Ajustes para poder conectarte.")

    def _message_with_settings(self, icon, title: str, text: str, informative: str) -> None:
        """Mensaje con botón «Abrir ajustes» que abre el diálogo de rutas."""
        box = QMessageBox(self)
        box.setIcon(icon)
        box.setWindowTitle(title)
        box.setText(text)
        box.setInformativeText(informative)
        open_button = box.addButton("Abrir ajustes", QMessageBox.AcceptRole)
        box.addButton("Cancelar", QMessageBox.RejectRole)
        box.exec()
        clicked = box.clickedButton()
        box.deleteLater()
        if clicked is open_button:
            self.open_settings()

    def open_settings(self) -> None:
        can_update = self._geo is not None and not self.country_db_busy()
        dialog = SettingsDialog(self.cfg, self,
                                on_update_countries=self.update_country_db if can_update else None)
        try:
            if _accepted(dialog.exec()):
                dialog.apply_to(self.cfg)
                self._save_config()
                # Rutas o descarga cambiadas: volver a pedir la imagen de la ficha.
                self._card_key = None
                self._update_card(self.selected_server())
                self._resort_sidebar()
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
        self._mode = mode
        self._fit_favorites_list()
        self.model.set_mode(mode)
        self.occupancy_delegate.set_mode(mode)
        self.table.viewport().update()
        self._show_details(self.selected_server())
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

    def _ui_state_targets(self) -> dict:
        """Clave en Config.ui_state → (saveState, restoreState) del widget."""
        header = self.table.horizontalHeader()
        targets = {"geometry": (self.saveGeometry, self.restoreGeometry),
                   "table_header": (header.saveState, header.restoreState)}
        for key, splitter in (("body_splitter", self.body_splitter),
                              ("right_splitter", self.right_splitter),
                              ("details_splitter", self.details_splitter)):
            targets[key] = (splitter.saveState, splitter.restoreState)
        return targets

    def _restore_ui_state(self) -> None:
        """Geometría, divisiones y columnas guardadas; un valor inválido se ignora."""
        # Un estado de cabecera de otra versión (otro número de columnas) se
        # aplicaría con anchos corridos: se descarta y quedan los valores por defecto.
        header_ok = self.cfg.ui_state.get("table_header_cols") == str(len(COLUMNS))
        for key, (_save, restore) in self._ui_state_targets().items():
            value = self.cfg.ui_state.get(key)
            if not value or (key == "table_header" and not header_ok):
                continue
            try:
                raw = base64.b64decode(value, validate=True)
            except (binascii.Error, ValueError):
                continue
            restore(QByteArray(raw))  # Qt valida el formato; si no, devuelve False
        header = self.table.horizontalHeader()
        # restoreState solo mueve el indicador: ordenar de verdad por él.
        self.table.sortByColumn(header.sortIndicatorSection(), header.sortIndicatorOrder())

    def _save_ui_state(self) -> None:
        state = dict(self.cfg.ui_state)
        for key, (save, _restore) in self._ui_state_targets().items():
            state[key] = base64.b64encode(bytes(save().data())).decode("ascii")
        state["table_header_cols"] = str(len(COLUMNS))
        if self.current_game is not None:
            state["last_game"] = self.current_game
        self.cfg.ui_state = state
        self._save_config()

    def closeEvent(self, event: QCloseEvent) -> None:
        self._save_ui_state()
        self._worker = None
        self._query_worker = None
        self._favorites_worker = None
        self._generation += 1
        workers = list(self._workers)
        for worker in workers:
            worker.cancel()
        deadline = time.monotonic() + self.CLOSE_WAIT_MS / 1000
        for worker in workers:
            remaining = max(0, int((deadline - time.monotonic()) * 1000))
            if not worker.wait(remaining):
                # p. ej. un getaddrinfo lento: no congelar el cierre; el hilo sigue
                # referenciado a nivel de módulo hasta shutdown_lingering_workers().
                _LINGERING.add(worker)
        self._workers.clear()
        # Sin esperar: la petición en curso termina en su hilo daemon y se ignora.
        self._card_key = None
        self._cancel_card_request()
        if self._country_request is not None:
            self._country_request.cancelled = True  # el hilo daemon termina solo
            self._country_request = None
        super().closeEvent(event)
