"""Ventana principal estilo All-Seeing-Eye: juegos, servidores, jugadores y reglas."""
from __future__ import annotations

import base64
import binascii
import sys
import time
from importlib import metadata
from pathlib import Path

from keyring.errors import KeyringError
from PySide6.QtCore import QByteArray, QFileInfo, QItemSelection, QLocale, QPoint, Qt, QThread, QTimer, Slot
from PySide6.QtGui import (
    QAction, QActionGroup, QCloseEvent, QIcon, QKeySequence, QPixmap, QShortcut,
)
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
    QFileIconProvider,
    QListWidget,
    QMainWindow,
    QMenu,
    QMessageBox,
    QProgressBar,
    QSizePolicy,
    QSplitter,
    QTableView,
    QTableWidget,
    QTableWidgetItem,
    QToolBar,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from jase import detect, i18n, launcher, masters, passwords, scanner
from jase.browse import BASE_MOD, breakdown, mod_of
from jase.config import Config, save_config, version_for
from jase.games import ALL_VERSIONS, GAMES, GameDef
from jase.geo import DB_FILENAME, country_name
from jase.i18n import tr, tr_n
from jase.launcher import LaunchError
from jase.maps.service import MapImageService
from jase.models import ServerInfo
from jase.protocols.quake3 import strip_colors
from jase.ui import country_loader, favorites, icons, masters_loader, theme
from jase.ui.colored_text import COLOR_RUNS_ROLE, ColoredTextDelegate, color_runs
from jase.ui.country_loader import CountryDBRequest
from jase.ui.masters_loader import MastersUpdateRequest
from jase.ui.masters_tab import MastersContext
from jase.ui.exe_icon_loader import ExeIconLoader
from jase.ui.find_games import FindGamesDialog
from jase.ui.dialogs import PasswordDialog, SettingsDialog
from jase.ui.game_tree import GameTree
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

# Anchos iniciales (px) de las columnas de la tabla; "Servidor" ocupa el resto.
COLUMN_WIDTHS = {COL_LOCK: 28, COL_COUNTRY: 48, COL_PING: 70, COL_PLAYERS: 90, COL_MAP: 170,
                 COL_MODE: 150, COL_VERSION: 70}


# Opciones del menú Tema: «Sistema» y luego los temas concretos.
THEME_CHOICES = ("system", *theme.THEME_IDS)


def _max_ping_options() -> dict[str, int]:
    """Opciones del filtro "Ping máx." (texto mostrado → ms; 0 = sin límite)."""
    return {tr("Sin límite"): 0, "50": 50, "100": 100, "150": 150, "250": 250}


def _accepted(result) -> bool:
    return result in (QDialog.DialogCode.Accepted, QDialog.DialogCode.Accepted.value)


def system_locale_name() -> str:
    """Idioma de visualización del sistema (no el formato regional)."""
    locale = QLocale.system()
    languages = locale.uiLanguages()
    return languages[0] if languages else locale.name()


def shutdown_lingering_workers(timeout_ms: int = 3000) -> None:
    """Al salir de la app: espera a los workers rezagados; terminate() como último recurso."""
    deadline = time.monotonic() + timeout_ms / 1000
    for worker in list(_LINGERING):
        remaining = max(0, int((deadline - time.monotonic()) * 1000))
        if not worker.wait(remaining):
            worker.terminate()
            worker.wait()
        _LINGERING.discard(worker)


APP_NAME = "Jimmy's All Seeing Eye"
# Acción -> SVG de assets/icons (las que no están aquí no llevan icono).
ACTION_ICONS = {"refresh": "refresh", "stop": "stop", "refresh_selected": "refresh_one",
                "connect": "connect", "favorite": "favorite", "copy": "copy",
                "settings": "settings", "theme": "theme"}


def app_version() -> str:
    try:
        return metadata.version("jase")
    except metadata.PackageNotFoundError:
        return "dev"


def _platform_exe_icon(path: str) -> QIcon | None:
    """Icono del exe según el sistema; solo Windows lo tiene (fuera, None → badge)."""
    if sys.platform != "win32":
        return None
    return QFileIconProvider().icon(QFileInfo(path))


class MainWindow(QMainWindow):
    # Plazo total para esperar a los workers al cerrar la ventana.
    CLOSE_WAIT_MS = 2000

    def __init__(self, cfg: Config, cfg_path: Path, scan_fn=scanner.scan,
                 launch_fn=launcher.launch, map_service=None,
                 query_fn=scanner.query_servers, geo=None, exe_icon_fn=None,
                 find_fn=detect.find_games, masters_fetch=None) -> None:
        super().__init__()
        self.cfg = cfg
        self.cfg_path = Path(cfg_path)
        self._scan_fn = scan_fn
        # Masters: lista oficial (caché/empaquetada) + personales; migra Config.masters.
        self.custom_masters_path = self.cfg_path.parent / masters.CUSTOM_FILENAME
        self._official = masters.load_official(self.cfg_path.parent / "cache")
        self._custom = masters.load_custom(self.custom_masters_path)
        saved_masters = {k: list(v) for k, v in cfg.masters.items()}
        if masters.migrate_config_masters(cfg, self._custom, self._official):
            try:
                masters.save_custom(self._custom, self.custom_masters_path)
            except OSError:
                # Sin archivo personal no se vacía la config (se migra de nuevo al
                # reiniciar); _custom migrado queda en memoria para esta sesión.
                cfg.masters = saved_masters
            else:
                try:
                    save_config(cfg, self.cfg_path)
                except OSError:
                    pass  # el archivo personal ya existe; la migración es idempotente
        # Actualización remota de masters; None = sin red (p. ej. tests).
        self._masters_fetch = masters_fetch
        self._masters_request: MastersUpdateRequest | None = None
        self._settings_masters: MastersContext | None = None  # Ajustes abierto
        self._launch_fn = launch_fn
        self._query_fn = query_fn  # para refrescar un servidor suelto
        self._find_fn = find_fn  # búsqueda de juegos instalados (diálogo «Buscar juegos»)
        # Caché junto a la config (%APPDATA%\JimmysASE\cache\maps en Windows).
        self._map_service = map_service or MapImageService(
            self.cfg_path.parent / "cache" / "maps")
        self._map_loader = shared_loader()
        self._map_loader.relay.loaded.connect(self._on_map_image_loaded)
        self._scan_stopped = False  # escaneo cancelado: el árbol cuenta las filas de la tabla
        self._card_key: tuple | None = None  # (juego, dirección, mapa) mostrado
        self._card_request: MapImageRequest | None = None  # petición vigente
        # Base de países (geo.CountrySource); None = sin columna País (p. ej. tests).
        self._geo = geo
        self.country_db_path = self.cfg_path.parent / "cache" / DB_FILENAME
        self._country_db = None
        self._country_request: CountryDBRequest | None = None
        # Icono del exe (hilo de UI); None = el de la plataforma (solo Windows tiene).
        self._exe_icon_fn = exe_icon_fn or _platform_exe_icon
        self._icon_generation = 0
        self._exe_icons: set[str] = set()  # juegos con icono de exe puesto
        self._exe_icon_loader = ExeIconLoader()
        self._exe_icon_loader.resolved.connect(self._on_exe_icon_resolved)

        self.game_ids: list[str] = self._sorted_game_ids()  # orden de las filas
        self.current_game: str | None = None
        self._cache: dict[str, list[ServerInfo]] = {}
        self._tree_prune_ok = False  # la lista de la tabla es completa: se pueden podar marcas
        self._generation = 0
        self._theme_id = "dark"  # tema efectivo (id concreto); lo fija apply_theme()
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

        self.setWindowTitle(APP_NAME)
        self.resize(1100, 700)
        self._build_ui()
        self._restore_ui_state()
        self._update_sidebar()
        self._update_actions()
        self._update_count_label()

        app = QApplication.instance()
        if app is not None:
            app.styleHints().colorSchemeChanged.connect(self._on_color_scheme_changed)
        self.apply_theme()  # también pone los iconos de juego (_refresh_game_icons)

        country_loader.shared_relay().loaded.connect(self._on_country_db_loaded)
        self._start_country_db(refresh=False)
        masters_loader.shared_relay().loaded.connect(self._on_masters_loaded)
        self.check_masters_update(force=False)

    # ------------------------------------------------------------------ UI
    def _build_ui(self) -> None:
        self.filter_edit = QLineEdit()
        self.filter_edit.setPlaceholderText(tr("Filtrar servidores…"))
        self.filter_edit.setClearButtonEnabled(True)
        self.filter_edit.textChanged.connect(self.proxy.set_text)
        self.filter_edit.textChanged.connect(self._after_list_changed)

        self.version_combo = QComboBox()
        self.version_combo.setToolTip(tr("Versión del juego"))
        self.version_combo.setMinimumContentsLength(6)
        self.version_combo.currentIndexChanged.connect(self._on_version_changed)

        # Sin scroll horizontal: nombre corto + conteo; si aun así no entra, se
        # recorta el medio y el conteo sigue visible (lo fija GameTree).
        self.game_tree = GameTree()
        self.game_tree.setObjectName("game_tree")
        self.game_tree.set_games(self.game_ids)
        self.game_tree.setMinimumWidth(190)
        # Clic en el juego ya seleccionado: sale de la vista Favoritos.
        self.game_tree.game_activated.connect(self.select_game)
        self.game_tree.marks_changed.connect(self._on_tree_marks_changed)
        # El desglose del juego en vivo se recalcula como mucho cada 300 ms.
        self._tree_timer = QTimer(self)
        self._tree_timer.setSingleShot(True)
        self._tree_timer.setInterval(300)
        self._tree_timer.timeout.connect(self._refresh_tree_breakdown)
        self.favorites_list = QListWidget()
        self.favorites_list.setObjectName("favorites_list")
        self.favorites_list.addItem(tr("★ Favoritos"))
        self.favorites_list.setToolTip(
            tr("Favoritos del juego seleccionado: se consultan directamente, sin master"))
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
        self.proxy.dataChanged.connect(lambda *_a: self._update_actions())
        self.table.doubleClicked.connect(lambda _index: self.connect_selected())
        self._setup_context_menu()

        self.players_table = self._make_detail_table([tr("Nombre"), tr("Puntos"), tr("Ping")])
        self.players_table.setItemDelegateForColumn(0, ColoredTextDelegate(self.players_table))
        self.rules_table = self._make_detail_table([tr("Variable"), tr("Valor")])
        details = self.details_splitter = QSplitter(Qt.Horizontal)
        details.addWidget(self.players_table)
        details.addWidget(self.rules_table)

        self.server_card = ServerCard()
        self.server_card.copy_requested.connect(
            lambda text: self.statusBar().showMessage(
                tr("Dirección copiada: {address}", address=text), 4000))
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
        layout.addWidget(body, 1)
        self.setCentralWidget(central)

        self.scanning_label = QLabel("")
        self.count_label = QLabel("")
        self.scan_progress = QProgressBar()
        self.scan_progress.setFormat(tr("Consultando %v / %m"))
        self.scan_progress.setTextVisible(True)
        self.scan_progress.setMaximumWidth(220)
        self.scan_progress.hide()
        self.statusBar().addPermanentWidget(self.scan_progress)
        self.statusBar().addPermanentWidget(self.scanning_label)
        self.statusBar().addPermanentWidget(self.count_label)
        for sig in (self.proxy.rowsInserted, self.proxy.rowsRemoved,
                    self.proxy.modelReset, self.proxy.layoutChanged):
            sig.connect(self._update_count_label)
        self._build_actions()
        self._build_menus()
        self._build_toolbar()
        self._setup_shortcuts()

    def _build_actions(self) -> None:
        """Acciones únicas que comparten menús, barra de iconos y menú contextual."""
        def make(key, text, slot=None, shortcut=None, checkable=False) -> QAction:
            action = QAction(text, self)
            if slot is not None:
                action.triggered.connect(lambda _checked=False: slot())
            if shortcut is not None:
                action.setShortcut(QKeySequence(shortcut))
            action.setCheckable(checkable)
            self.ui_actions[key] = action
            return action

        self.ui_actions: dict[str, QAction] = {}
        make("refresh", tr("Refrescar lista"), self.refresh, Qt.Key_F5)
        make("stop", tr("Detener"), self.stop_active)
        make("refresh_selected", tr("Refrescar este servidor"), self.refresh_selected)
        make("connect", tr("Conectar"), self.connect_selected)
        make("favorite", tr("Agregar a favoritos"), self.toggle_favorite_selected)
        # Sin atajo global: Ctrl+C es de la tabla (ver _setup_shortcuts).
        make("copy", tr("Copiar IP:puerto"), self.copy_selected_address)
        make("settings", tr("Ajustes"), self.open_settings)
        # Tema: submenú con las 11 opciones exclusivas (Ver → Tema y botón de la barra).
        self.theme_menu = QMenu(tr("Tema"), self)
        theme_group = QActionGroup(self)
        theme_group.setExclusive(True)
        self.theme_actions: dict[str, QAction] = {}
        for theme_id in THEME_CHOICES:
            action = QAction(theme.theme_display_name(theme_id), self)
            action.setData(theme_id)
            action.setCheckable(True)
            action.triggered.connect(lambda _checked=False, t=theme_id: self.set_theme(t))
            theme_group.addAction(action)
            self.theme_menu.addAction(action)
            self.theme_actions[theme_id] = action
        self.ui_actions["theme"] = self.theme_menu.menuAction()
        make("find_games", tr("Buscar juegos…"), self.find_games)
        make("clear_tree_marks", tr("Quitar filtros de mod/mapa"), self.clear_tree_marks)
        make("quit", tr("Salir"), self.close, QKeySequence.Quit)
        make("about", tr("Acerca de"), self.show_about)
        # Filtros rápidos: la casilla de la barra lateral y la acción se reflejan.
        for key, text, check in (("hide_empty", tr("Ocultar vacíos"), self.hide_empty_check),
                                 ("hide_full", tr("Ocultar llenos"), self.hide_full_check),
                                 ("hide_password", tr("Ocultar con contraseña"),
                                  self.hide_password_check)):
            action = make(key, text, checkable=True)
            action.setChecked(check.isChecked())
            action.toggled.connect(check.setChecked)
            check.toggled.connect(action.setChecked)
        # Ping máx.: un grupo exclusivo que refleja max_ping_combo en ambos sentidos.
        group = QActionGroup(self)
        group.setExclusive(True)
        self.ping_actions: list[QAction] = []
        for i in range(self.max_ping_combo.count()):
            action = QAction(self.max_ping_combo.itemText(i), self)
            action.setCheckable(True)
            action.setChecked(i == self.max_ping_combo.currentIndex())
            action.toggled.connect(
                lambda checked, i=i: checked and self.max_ping_combo.setCurrentIndex(i))
            group.addAction(action)
            self.ping_actions.append(action)
        self.max_ping_combo.currentIndexChanged.connect(
            lambda i: self.ping_actions[i].setChecked(True) if 0 <= i < len(
                self.ping_actions) else None)

    def _build_menus(self) -> None:
        acts = self.ui_actions
        bar = self.menuBar()
        menu = bar.addMenu(tr("Archivo"))
        menu.addAction(acts["find_games"])
        menu.addAction(acts["settings"])
        menu.addSeparator()
        menu.addAction(acts["quit"])
        bar.addMenu(tr("Ver")).addAction(acts["theme"])
        menu = bar.addMenu(tr("Filtro"))
        for key in ("hide_empty", "hide_full", "hide_password"):
            menu.addAction(acts[key])
        ping_menu = menu.addMenu(tr("Ping máx."))
        ping_menu.addActions(self.ping_actions)
        menu.addSeparator()
        menu.addAction(acts["clear_tree_marks"])
        menu = bar.addMenu(tr("Servidor"))
        for key in ("connect", "refresh", "stop", "refresh_selected", "copy", "favorite"):
            menu.addAction(acts[key])
        bar.addMenu(tr("Ayuda")).addAction(acts["about"])

    def _build_toolbar(self) -> None:
        acts = self.ui_actions
        bar = self.toolbar = QToolBar(tr("Herramientas"), self)
        bar.setObjectName("toolbar")
        bar.setMovable(False)
        bar.setFloatable(False)
        bar.setContextMenuPolicy(Qt.PreventContextMenu)
        bar.setIconSize(icons.ICON_SIZE)
        bar.setToolButtonStyle(Qt.ToolButtonIconOnly)
        groups = (("refresh", "stop", "refresh_selected"),
                  ("connect", "favorite", "copy"), ("settings", "theme"))
        for i, group in enumerate(groups):
            if i:
                bar.addSeparator()
            for key in group:
                bar.addAction(acts[key])
        self.filter_edit.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        bar.addWidget(self.filter_edit)
        bar.addWidget(self.version_combo)
        theme_button = bar.widgetForAction(acts["theme"])
        if isinstance(theme_button, QToolButton):
            theme_button.setPopupMode(QToolButton.InstantPopup)
        self.addToolBar(bar)

    def clear_tree_marks(self) -> None:
        if self.current_game is not None:
            self.game_tree.clear_marks(self.current_game)

    def stop_active(self) -> None:
        """Detiene el escaneo del master y la consulta de favoritos en curso."""
        self._stop_active_scan()
        self._cancel_favorites_query()
        self._update_actions()

    def show_about(self) -> None:
        QMessageBox.about(self, tr("Acerca de {name}", name=APP_NAME),
                          f"{APP_NAME}\n" + tr("Versión {version}", version=app_version()))

    def _setup_context_menu(self) -> None:
        self.table.setContextMenuPolicy(Qt.CustomContextMenu)
        self.table.customContextMenuRequested.connect(self._on_table_context_menu)

    def _setup_shortcuts(self) -> None:
        """Enter/Ctrl+C solo con la tabla enfocada; Ctrl+F en toda la ventana (F5 es de la acción)."""
        table_keys = ((Qt.Key_Return, self._connect_if_selected),
                      (Qt.Key_Enter, self._connect_if_selected),
                      (QKeySequence.Copy, self.copy_selected_address))
        for key, slot in table_keys:
            shortcut = QShortcut(QKeySequence(key), self.table)
            shortcut.setContext(Qt.WidgetShortcut)
            shortcut.activated.connect(slot)
        QShortcut(QKeySequence.Find, self).activated.connect(self._focus_filter)

    def _connect_if_selected(self) -> None:
        if self.selected_server() is not None:
            self.connect_selected()

    def _focus_filter(self) -> None:
        self.filter_edit.setFocus(Qt.ShortcutFocusReason)
        self.filter_edit.selectAll()

    def _build_context_menu(self) -> QMenu:
        """Menú de la fila seleccionada, con las acciones compartidas (se deshabilitan sin selección)."""
        menu = QMenu(self)
        acts = self.ui_actions
        for key in ("connect", "copy", "refresh_selected"):
            menu.addAction(acts[key])
        menu.addSeparator()
        menu.addAction(acts["favorite"])
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
        self.hide_empty_check = QCheckBox(tr("Ocultar vacíos"))
        self.hide_full_check = QCheckBox(tr("Ocultar llenos"))
        self.hide_password_check = QCheckBox(tr("Ocultar con contraseña"))
        for check, value in ((self.hide_empty_check, q.hide_empty),
                             (self.hide_full_check, q.hide_full),
                             (self.hide_password_check, q.hide_password)):
            check.setChecked(value)
            check.toggled.connect(self._on_quick_filters_changed)

        self.max_ping_combo = QComboBox()
        self.max_ping_combo.setToolTip(tr("Ocultar servidores con ping mayor"))
        for text, ms in _max_ping_options().items():
            self.max_ping_combo.addItem(text, ms)
        idx = self.max_ping_combo.findData(q.max_ping)
        if idx < 0:  # valor guardado fuera de la lista: se conserva tal cual
            self.max_ping_combo.addItem(str(q.max_ping), q.max_ping)
            idx = self.max_ping_combo.count() - 1
        self.max_ping_combo.setCurrentIndex(idx)
        self.max_ping_combo.currentIndexChanged.connect(self._on_quick_filters_changed)

        ping_row = QHBoxLayout()
        ping_row.setContentsMargins(0, 0, 0, 0)
        ping_row.addWidget(QLabel(tr("Ping máx.")))
        ping_row.addWidget(self.max_ping_combo, 1)

        panel = QWidget()
        layout = QVBoxLayout(panel)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(self.game_tree, 1)
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
        previous = self.current_game
        self.current_game = game_id
        self._tree_prune_ok = False
        self.game_tree.set_current_game(game_id)
        self._apply_tree_filter()

        self.model.clear()
        self.model.version_label = self.server_card.version_label = GAMES[game_id].version_label
        self._populate_version_combo()
        cached = self._cache.get(game_id)
        if cached is not None:
            for info in cached:
                self.model.add_or_update(info)
            self._add_observed_versions(cached)
            self._tree_prune_ok = True
            self.statusBar().showMessage(
                tr("{name}: lista guardada de la sesión", name=GAMES[game_id].name))
        else:
            self._start_scan()
        self._update_sidebar(only=game_id)  # desglose del árbol (con caché: poda marcas)
        if previous is not None and previous != game_id:
            self._set_tree_breakdown(previous)  # ya no es la tabla: pasa a su caché
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
        self._update_sidebar(only=self.current_game)  # marcas conservadas con 0 mientras escanea
        self._after_list_changed()

    def is_scanning(self) -> bool:
        return self._worker is not None

    def _game(self) -> GameDef | None:
        return GAMES.get(self.current_game) if self.current_game else None

    def _start_scan(self) -> None:
        game = self._game()
        if game is None:
            return
        active = masters.effective_masters(game.id, self._official, self._custom)
        if not active:
            self.scanning_label.setText("")
            self.statusBar().showMessage(tr(
                "{game}: no hay masters activos; revísalos en Ajustes → Masters",
                game=game.name))
            self._update_actions()
            return
        self._generation += 1
        self._tree_prune_ok = False
        self._scan_stopped = False
        worker = ScanWorker(game, active, scan_fn=self._scan_fn)
        worker.generation = self._generation
        worker.server_found.connect(self._on_server_found)
        worker.status.connect(self._on_status)
        worker.progress.connect(self._on_progress)
        worker.finished.connect(self._on_worker_finished)
        self._workers.add(worker)
        self._worker = worker
        self.scanning_label.setText(tr("Escaneando…"))
        self.statusBar().showMessage(tr("Consultando masters de {name}…", name=game.name))
        worker.start()
        self._update_actions()

    def _stop_active_scan(self) -> None:
        """Cancela el worker activo sin bloquear; sus señales quedan obsoletas."""
        self._tree_timer.stop()
        if self._worker is not None:
            self._worker.cancel()
            self._worker = None
            self._tree_prune_ok = False
            self._scan_stopped = True  # la tabla conserva lo que llegó
            self._generation += 1
            self.scanning_label.setText("")
            self.scan_progress.hide()
            self._update_actions()
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
        self._update_sidebar(only=self.current_game, tree=False)  # el resto no cambia
        if not self._tree_timer.isActive():
            self._tree_timer.start()
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
                self._tree_prune_ok = True
                if worker.result is not None:
                    self.statusBar().showMessage(
                        tr("{responded} de {listed} servidores respondieron",
                           responded=worker.result.servers_responded,
                           listed=worker.result.servers_listed))
            self._update_sidebar()
            self._update_actions()

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
        self.game_tree.blockSignals(True)
        try:
            self.game_tree.set_games(self.game_ids)
            self.game_tree.set_current_game(self.current_game)
        finally:
            self.game_tree.blockSignals(False)
        self._update_sidebar()

    def _refresh_game_icons(self) -> None:
        """Badge de inmediato para los juegos sin icono de exe y nueva resolución de los exes."""
        for game_id in GAMES:
            if game_id not in self._exe_icons:
                self.game_tree.set_game_icon(game_id, icons.badge_icon(game_id, self._theme_id))
        self._icon_generation += 1
        paths = {gid: p for gid, p in self.cfg.game_paths.items() if p and gid in GAMES}
        # Un juego sin ruta ya no tiene exe: su icono se descarta ahora mismo.
        for game_id in [g for g in self._exe_icons if g not in paths]:
            self._exe_icons.discard(game_id)
            self.game_tree.set_game_icon(game_id, icons.badge_icon(game_id, self._theme_id))
        if paths:
            self._exe_icon_loader.request(self._icon_generation, paths)

    @Slot(int, str, str)
    def _on_exe_icon_resolved(self, generation: int, game_id: str, exe: str) -> None:
        if generation != self._icon_generation or game_id not in GAMES:
            return  # resultado de una resolución anterior
        icon = self._exe_icon_fn(exe) if exe else None
        if icon is not None and not icon.isNull():
            self._exe_icons.add(game_id)
            self.game_tree.set_game_icon(game_id, icon)
        else:  # sin exe (o sin icono): vuelve el badge
            self._exe_icons.discard(game_id)
            self.game_tree.set_game_icon(game_id, icons.badge_icon(game_id, self._theme_id))

    def _tree_source(self, game_id: str) -> tuple[list[ServerInfo] | None, bool]:
        """(servidores del desglose, prune). Para el juego actual la fuente es la tabla y
        solo se poda (marcas de valores ausentes) con una lista completa: escaneo
        terminado sin error o lista restaurada de la caché."""
        if game_id == self.current_game and not self._favorites_view:
            return self.model.servers(), self._tree_prune_ok
        return self._cache.get(game_id), game_id != self.current_game

    def _set_tree_breakdown(self, game_id: str) -> None:
        servers, prune = self._tree_source(game_id)
        b = None if servers is None else breakdown(GAMES[game_id], servers)
        self.game_tree.set_breakdown(game_id, b, prune=prune)

    def _refresh_tree_breakdown(self) -> None:
        if self.current_game is not None:
            self._set_tree_breakdown(self.current_game)

    def _update_sidebar(self, only: str | None = None, tree: bool = True) -> None:
        """Textos del árbol de juegos; `only` = solo ese juego."""
        for game_id in self.game_ids:
            if only is not None and game_id != only:
                continue
            if game_id == self.current_game and self.is_scanning():
                count = str(self.model.rowCount())  # conteo en vivo
            elif game_id in self._cache:
                count = str(len(self._cache[game_id]))
            elif (game_id == self.current_game and self._scan_stopped
                  and not self._favorites_view and self.model.rowCount()):
                count = str(self.model.rowCount())  # escaneo detenido: lo que muestra la tabla
            else:
                count = "—"
            game = GAMES[game_id]
            self.game_tree.set_game_label(
                game_id, f"{game.short_name} · {count}", game.name)
            if tree and game_id == self.current_game:
                self._set_tree_breakdown(game_id)
        if only is not None:
            return
        text = tr("★ Favoritos")
        if self.current_game is not None:
            text += f" · {len(favorites.favorite_addresses(self.cfg, self.current_game))}"
        self.favorites_list.item(0).setText(text)

    def _tree_marks(self) -> tuple[frozenset[str], frozenset[str]]:
        if self.current_game is None or self._favorites_view:
            return frozenset(), frozenset()
        return self.game_tree.marks(self.current_game)

    def _apply_tree_filter(self) -> None:
        mods, maps = self._tree_marks()
        game = self._game() if (mods or maps) else None
        self.proxy.set_tree_filter(game, mods, maps)

    @Slot(str)
    def _on_tree_marks_changed(self, game_id: str) -> None:
        if game_id != self.current_game:
            return
        if self._favorites_view:
            self._update_count_label()  # las marcas no se aplican, pero se avisa
            return
        self._apply_tree_filter()
        self._after_list_changed()

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
        self._apply_tree_filter()  # en Favoritos las marcas no se aplican
        if bypassed:
            self.proxy.set_version(ALL_VERSIONS)  # al salir lo repone _populate_version_combo
        for widget in (self.hide_empty_check, self.hide_full_check,
                       self.hide_password_check, self.max_ping_combo, self.version_combo,
                       *(self.ui_actions[k] for k in ("hide_empty", "hide_full",
                                                      "hide_password"))):
            widget.setEnabled(not bypassed)
        for action in self.ping_actions:
            action.setEnabled(not bypassed)
        self.version_combo.setToolTip(
            tr("No aplica en Favoritos") if bypassed else tr("Versión del juego"))
        self._update_actions()
        self._update_count_label()

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
                tr("No hay favoritos de {name}: agrégalos con clic derecho "
                   "o con la estrella ☆ de la ficha", name=game.name))
            self._after_list_changed()
            return
        worker = QueryWorker(game, addresses, query_fn=self._query_fn)
        worker.server_found.connect(self._on_favorite_found)
        worker.finished.connect(self._on_favorites_finished)
        self._workers.add(worker)
        self._favorites_worker = worker
        self._favorites_answered = set()
        self.scanning_label.setText(tr("Consultando favoritos…"))
        self.statusBar().showMessage(
            tr_n("Consultando 1 favorito de {name}…", "Consultando {n} favoritos de {name}…",
                 len(addresses), name=game.name))
        worker.start()
        self._after_list_changed()  # también habilita «Detener»

    def _cancel_favorites_query(self) -> None:
        if self._favorites_worker is not None:
            self._favorites_worker.cancel()  # sigue en _workers hasta `finished`
            self._favorites_worker = None
            self.scanning_label.setText("")
            self._update_actions()

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
        self._update_actions()
        # Solo los que siguen siendo favoritos: uno quitado durante la consulta
        # no vuelve como "sin respuesta".
        current = [a for a in worker.addresses
                   if favorites.is_favorite(self.cfg, worker.game.id, a)]
        for address in current:
            if address not in self._favorites_answered:
                self.model.add_or_update(favorites.offline_server(address))
        online = sum(1 for a in current if a in self._favorites_answered)
        if worker.failed:
            self.statusBar().showMessage(
                tr("Error al consultar favoritos: {error}", error=worker.error))
        else:
            self.statusBar().showMessage(
                tr("Favoritos: {online} de {total} respondieron",
                   online=online, total=len(current)))
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
            message = tr("Quitado de favoritos: {address}", address=info.address)
            if self._favorites_view:
                self.model.remove(info.address)
        else:
            favorites.add_favorite(self.cfg, game.id, info.address)
            message = tr("Agregado a favoritos: {address}", address=info.address)
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
        for key in items:  # texto visible = etiqueta; el dato (clave) es lo que filtra
            self.version_combo.addItem(game.version_label(key), key)
        self.version_combo.setCurrentIndex(self.version_combo.findData(selected))
        self.version_combo.blockSignals(False)
        self.proxy.set_version(selected)

    def _add_observed_versions(self, servers: list[ServerInfo]) -> None:
        game = self._game()
        for info in servers:
            if info.version and self.version_combo.findData(info.version) < 0:
                label = game.version_label(info.version) if game else info.version
                self.version_combo.addItem(label, info.version)

    @Slot(int)
    def _on_version_changed(self, index: int) -> None:
        game = self._game()
        version = self.version_combo.itemData(index) if index >= 0 else None
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
        self._update_actions()
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
                name.setData(COLOR_RUNS_ROLE, color_runs(p.raw_name, self._theme_id))
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
        self.server_card.set_server(info, self._theme_id)
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
            self.statusBar().showMessage(tr("Descargando base de países…"))

    def country_db_busy(self) -> bool:
        return self._country_request is not None

    @Slot(object, object, object)
    def _on_country_db_loaded(self, request, db, error) -> None:
        if request is not self._country_request:
            return  # de otra ventana
        self._country_request = None
        if db is None:
            # Sin diálogos: columna vacía (o la base anterior) y aviso breve.
            self.statusBar().showMessage(error or tr("Base de países no disponible"), 8000)
            return
        self._country_db = db
        self.model.set_country_lookup(db.lookup)
        self._update_card(self.selected_server())
        if request.refresh:
            self.statusBar().showMessage(tr("Base de países actualizada"), 5000)

    # -------------------------------------------------------------- masters
    def check_masters_update(self, force: bool) -> bool:
        """Pide la lista oficial a GitHub (como mucho cada 24 h salvo `force`)."""
        if self._masters_fetch is None or self._masters_request is not None:
            return False
        cache_dir = self.cfg_path.parent / "cache"
        now = time.time()
        if not force and not masters.should_check(cache_dir / masters.META_FILENAME, now):
            return False
        self._masters_request = MastersUpdateRequest(cache_dir, now, self._masters_fetch,
                                                     force=force)
        masters_loader.submit(self._masters_request)
        return True

    @Slot(object, object, object)
    def _on_masters_loaded(self, request, official, error) -> None:
        if request is not self._masters_request:
            return  # de otra ventana o descartada
        self._masters_request = None
        if official is not None:
            self._official = official
            kind = "updated"
        else:
            kind = "failed" if error else "unchanged"
        listener = self._settings_masters.outcome_listener if self._settings_masters else None
        if listener is not None and (kind == "updated" or request.force):
            listener(kind, official)
        if kind == "updated":
            date = official.updated or time.strftime("%Y-%m-%d")
            self.statusBar().showMessage(
                tr("Lista de masters actualizada ({date})", date=date), 5000)
        elif request.force and listener is None:  # pedido a mano y Ajustes ya se cerró
            self.statusBar().showMessage(
                tr("La lista de masters está al día") if kind == "unchanged"
                else tr("No se pudo descargar la lista de masters"), 5000)

    def _force_masters_check(self) -> None:
        """«Buscar actualización» de Ajustes: el resultado vuelve por `outcome_listener`."""
        if self.check_masters_update(force=True) or self._masters_request is not None:
            return  # en curso: _on_masters_loaded informará
        ctx = self._settings_masters
        if ctx is not None and ctx.outcome_listener is not None:
            ctx.outcome_listener("failed", None)  # sin red disponible

    def _update_actions(self) -> None:
        game = self._game()
        # Los juegos con steam://connect no necesitan ruta.
        has_path = bool(game and (self.cfg.game_paths.get(game.id)
                                  or game.launch_style == "steam"))
        info = self.selected_server()
        acts = self.ui_actions
        # Un favorito sin respuesta solo tiene el puerto de consulta: no se lanza.
        acts["connect"].setEnabled(has_path and info is not None and info.online)
        acts["stop"].setEnabled(self.is_scanning() or self._favorites_worker is not None)
        for key in ("refresh_selected", "favorite", "copy"):
            acts[key].setEnabled(info is not None)
        acts["clear_tree_marks"].setEnabled(any(self._tree_marks()))
        acts["favorite"].setText(tr("Quitar de favoritos") if self._is_selected_favorite()
                                 else tr("Agregar a favoritos"))

    def _update_count_label(self, *_args) -> None:
        n = self.proxy.rowCount()
        text = tr_n("{n} servidor", "{n} servidores", n)
        if any(self._tree_marks()):
            text += tr(" · filtro de mod/mapa activo")
        elif self._favorites_view and self.current_game is not None \
                and any(self.game_tree.marks(self.current_game)):
            text += tr(" · filtro de mod/mapa (no aplica en Favoritos)")
        self.count_label.setText(text)

    # ------------------------------------------------- copiar / refrescar uno
    def copy_selected_address(self) -> None:
        info = self.selected_server()
        if info is None:
            return
        text = str(info.connect_address)
        QApplication.clipboard().setText(text)
        self.statusBar().showMessage(tr("Dirección copiada: {address}", address=text), 4000)

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
        self.statusBar().showMessage(tr("Consultando {address}…", address=info.address))
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
        self.statusBar().showMessage(
            tr("Servidor actualizado: {address}", address=info.address), 4000)

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
                self.statusBar().showMessage(tr("Error al consultar: {error}", error=worker.error))
            elif not worker.answered:
                self.statusBar().showMessage(tr("El servidor no respondió"))
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
        if not install_dir and game.launch_style != "steam":
            self._prompt_missing_path(game)
            return
        info = self.selected_server()
        if info is None:
            return
        if not info.online:
            self.statusBar().showMessage(tr("El servidor no responde; refresca para conectar"))
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
                    self.statusBar().showMessage(tr("Contraseña olvidada"))
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
                        self, tr("Contraseña"),
                        tr("No se pudo guardar la contraseña: {error}\n"
                           "Se usará solo para esta conexión.", error=exc))
        try:
            # Solo BF1942 usa el mod al lanzar; se pasa el valor tal como lo envía el
            # servidor (nombre de carpeta), no la clave normalizada de mod_of.
            mod = ""
            if (game.launch_style == "bf1942" and game.mod_key
                    and mod_of(game, info) != BASE_MOD):
                mod = info.rules.get(game.mod_key, "").strip()
            kwargs = {"mod": mod} if mod else {}
            self._launch_fn(game, install_dir, info.connect_address, password, **kwargs)
        except (LaunchError, OSError) as exc:
            self._message_with_settings(QMessageBox.Warning, tr("No se pudo conectar"),
                                        str(exc),
                                        tr("Revisa la ruta del juego en Ajustes."))
            return
        self.statusBar().showMessage(
            tr("Lanzando {name} → {address}", name=game.name, address=info.connect_address))

    def _prompt_missing_path(self, game: GameDef) -> None:
        self._message_with_settings(QMessageBox.Information, tr("Ruta no configurada"),
                                    tr("No hay ruta configurada para {name}.", name=game.name),
                                    tr("Configúrala en Ajustes para poder conectarte."))

    def _message_with_settings(self, icon, title: str, text: str, informative: str) -> None:
        """Mensaje con botón «Abrir ajustes» que abre el diálogo de rutas."""
        box = QMessageBox(self)
        box.setIcon(icon)
        box.setWindowTitle(title)
        box.setText(text)
        box.setInformativeText(informative)
        open_button = box.addButton(tr("Abrir ajustes"), QMessageBox.AcceptRole)
        box.addButton(tr("Cancelar"), QMessageBox.RejectRole)
        box.exec()
        clicked = box.clickedButton()
        box.deleteLater()
        if clicked is open_button:
            self.open_settings()

    def _pick_found_games(self) -> dict[str, str]:
        dialog = FindGamesDialog(self.cfg, self, find_fn=self._find_fn)
        try:
            return dialog.selected_paths() if _accepted(dialog.exec()) else {}
        finally:
            dialog.deleteLater()

    def find_games(self) -> None:
        """Busca juegos instalados y guarda solo las rutas que el usuario marcó."""
        chosen = self._pick_found_games()
        if not chosen:
            return
        self.cfg.game_paths.update(chosen)
        self._after_game_paths_changed()

    def _after_game_paths_changed(self) -> None:
        """Guarda y refresca todo lo que depende de las rutas de los juegos."""
        self._save_config()
        # Rutas cambiadas: volver a pedir la imagen de la ficha.
        self._card_key = None
        self._update_card(self.selected_server())
        self._resort_sidebar()
        self._refresh_game_icons()
        self._update_actions()

    def offer_find_games(self) -> None:
        """Primer arranque sin juegos configurados: ofrece buscarlos (una sola vez)."""
        if self.cfg.find_games_offered:
            return
        self.cfg.find_games_offered = True
        self._save_config()
        if any(detect.is_configured(g, self.cfg) for g in GAMES.values()):
            return
        box = QMessageBox(self)
        box.setIcon(QMessageBox.Question)
        box.setWindowTitle(tr("Buscar juegos"))
        box.setText(tr("No hay juegos configurados. ¿Buscar juegos instalados?"))
        search_button = box.addButton(tr("Buscar juegos"), QMessageBox.AcceptRole)
        box.addButton(tr("Ahora no"), QMessageBox.RejectRole)
        box.exec()
        clicked = box.clickedButton()
        box.deleteLater()
        if clicked is search_button:
            self.find_games()

    def open_settings(self) -> None:
        can_update = self._geo is not None and not self.country_db_busy()
        ctx = MastersContext(self._official, self._custom,
                             check_update=self._force_masters_check)
        self._settings_masters = ctx
        dialog = SettingsDialog(self.cfg, self,
                                on_update_countries=self.update_country_db if can_update else None,
                                on_find_games=self._pick_found_games, masters=ctx)
        try:
            if _accepted(dialog.exec()):
                masters_changed = self._apply_custom_masters(dialog)
                previous_language = self.cfg.language
                previous_theme = self.cfg.theme
                dialog.apply_to(self.cfg)
                self._after_game_paths_changed()
                if self.cfg.theme != previous_theme:
                    self.apply_theme()
                system = system_locale_name()
                if (i18n.resolve_language(self.cfg.language, system)
                        != i18n.resolve_language(previous_language, system)):
                    self._notify_restart_for_language()
                if masters_changed and self.current_game is not None \
                        and not self._favorites_view:
                    self.refresh()  # los masters cambiaron: relistar el juego actual
        finally:
            self._settings_masters = None
            dialog.deleteLater()

    def _apply_custom_masters(self, dialog) -> bool:
        """Aplica los masters editados; devuelve si cambiaron."""
        custom = getattr(dialog, "custom_masters", lambda: None)()
        if custom is None or custom == self._custom:
            return False
        try:
            masters.save_custom(custom, self.custom_masters_path)
        except OSError as exc:
            self.statusBar().showMessage(
                tr("No se pudo guardar la configuración: {error}", error=exc))
        else:
            if self.cfg.masters:  # migración pendiente: ya no debe repetirse
                self.cfg.masters = {}
                try:
                    save_config(self.cfg, self.cfg_path)
                except OSError:
                    pass
        self._custom = custom  # en esta sesión rige lo editado aunque falle el guardado
        return True

    def _notify_restart_for_language(self) -> None:
        box = QMessageBox(self)
        box.setIcon(QMessageBox.Information)
        box.setWindowTitle(tr("Idioma"))
        box.setText(tr("El idioma cambiará al reiniciar JASE."))
        box.addButton(tr("Entendido"), QMessageBox.AcceptRole)
        box.exec()
        box.deleteLater()

    # ---------------------------------------------------------------- tema
    def set_theme(self, theme_id: str) -> None:
        """Elige el tema (id de THEME_CHOICES), lo guarda y lo aplica en caliente."""
        self.cfg.theme = theme_id if theme_id in THEME_CHOICES else "system"
        self._save_config()
        self.apply_theme()

    def apply_theme(self) -> None:
        app = QApplication.instance()
        scheme = app.styleHints().colorScheme() if app else Qt.ColorScheme.Unknown
        theme_id = theme.resolve_theme(self.cfg.theme, scheme)
        if app is not None:
            app.setStyleSheet(theme.build_stylesheet(theme_id))
        self._theme_id = theme_id
        self._fit_favorites_list()
        self.model.set_mode(theme_id)
        self.occupancy_delegate.set_mode(theme_id)
        self.table.viewport().update()
        self._show_details(self.selected_server())
        self._refresh_game_icons()
        for key, name in ACTION_ICONS.items():
            self.ui_actions[key].setIcon(icons.tool_icon(name, theme_id))
        choice = self.cfg.theme if self.cfg.theme in self.theme_actions else "system"
        self.theme_actions[choice].setChecked(True)
        self.ui_actions["theme"].setToolTip(
            tr("Tema: {name}", name=theme.theme_display_name(choice)))

    @Slot(Qt.ColorScheme)
    def _on_color_scheme_changed(self, _scheme) -> None:
        if self.cfg.theme == "system":
            self.apply_theme()

    # ---------------------------------------------------------------- otros
    def _save_config(self) -> None:
        try:
            save_config(self.cfg, self.cfg_path)
        except OSError as exc:
            self.statusBar().showMessage(
                tr("No se pudo guardar la configuración: {error}", error=exc))

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
        self._tree_timer.stop()
        self._save_ui_state()
        self._worker = None
        self._query_worker = None
        self._favorites_worker = None
        self._generation += 1
        self._icon_generation += 1  # los iconos que lleguen tarde se descartan
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
        if self._masters_request is not None:
            self._masters_request.cancelled = True
            self._masters_request = None
        super().closeEvent(event)
