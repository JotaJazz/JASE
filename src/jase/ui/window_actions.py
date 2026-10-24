"""Acciones, menús, barra de herramientas y menú contextual de MainWindow."""
from __future__ import annotations


from PySide6.QtCore import (QPoint, Qt)
from PySide6.QtGui import (
    QAction, QActionGroup, QKeySequence, QShortcut,
)
from PySide6.QtWidgets import (
    QMenu,
    QSizePolicy,
    QToolBar,
    QToolButton,
)

from jase.i18n import tr
from jase.ui import icons, theme


# Opciones del menú Tema: «Sistema» y luego los temas concretos.
THEME_CHOICES = ("system", *theme.THEME_IDS)


class ActionsMixin:
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: ui_actions, table, proxy, cfg, model, game_tree y los métodos de
    selección/escaneo.
    """

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

    def _setup_context_menu(self) -> None:
        self.table.setContextMenuPolicy(Qt.CustomContextMenu)
        self.table.customContextMenuRequested.connect(self._on_table_context_menu)

    def _setup_shortcuts(self) -> None:
        """Enter/Ctrl+C solo con la tabla enfocada; Ctrl+F en toda la ventana (F5, de la acción)."""
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
        """Menú de la fila seleccionada, con las acciones compartidas (sin selección, apagadas)."""
        # QMenu se busca en main_window en cada llamada (los tests lo parchean allí); import
        # diferido porque main_window importa este módulo.
        from jase.ui import main_window

        menu = main_window.QMenu(self)
        acts = self.ui_actions
        for key in ("connect", "copy", "refresh_selected"):
            menu.addAction(acts[key])
        menu.addSeparator()
        menu.addAction(acts["favorite"])
        return menu

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
