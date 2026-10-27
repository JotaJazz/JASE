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

from jase.ui._mixin_base import MixinBase
from jase.i18n import tr, tr_n
from jase.ui import icons, theme


# Opciones del menú Tema: «Sistema» y luego los temas concretos.
THEME_CHOICES = ("system", *theme.THEME_IDS)


class ActionsMixin(MixinBase):
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
        make("watch_slot", tr("Avisar cuando haya sitio"), self.toggle_slot_watch)
        make("party_alert", tr("Avisar cuando se arme partida…"), self.edit_party_alert)
        make("favorite", tr("Agregar a favoritos"), self.toggle_favorite_selected)
        make("edit_note", tr("Editar nota…"), self.edit_note_selected)
        make("remove_recent", tr("Quitar de recientes"), self.remove_recent_selected)
        # Sin atajo global: Ctrl+C es de la tabla (ver _setup_shortcuts).
        make("copy", tr("Copiar IP:puerto"), self.copy_selected_address)
        make("copy_link", tr("Copiar enlace jase://"), self.copy_selected_link)
        make("add_server", tr("Agregar servidor…"), self.add_server_manually, "Ctrl+N")
        make("settings", tr("Ajustes"), self.open_settings)
        # Tema: submenú con las 12 opciones exclusivas (Ver → Tema y botón de la barra) y,
        # al final, el color del fósforo de Terminal.
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
        self.theme_menu.addSeparator()
        self.terminal_color_menu = self.theme_menu.addMenu(tr("Color del fósforo"))
        color_group = QActionGroup(self)
        color_group.setExclusive(True)
        self.terminal_color_actions: dict[str, QAction] = {}
        for color in theme.TERMINAL_COLORS:
            action = QAction(theme.terminal_color_name(color), self)
            action.setData(color)
            action.setCheckable(True)
            action.triggered.connect(lambda _checked=False, c=color: self.set_terminal_color(c))
            color_group.addAction(action)
            self.terminal_color_menu.addAction(action)
            self.terminal_color_actions[color] = action
        self.ui_actions["theme"] = self.theme_menu.menuAction()
        self._build_group_widgets()
        make("find_games", tr("Buscar juegos…"), self.find_games)
        make("export_favorites", tr("Exportar favoritos…"), self.export_favorites_to_file)
        make("import_favorites", tr("Importar favoritos…"), self.import_favorites_from_file)
        make("copy_favorites", tr("Copiar favoritos como texto"), self.copy_favorites_as_text)
        make("find_player", tr("Buscar jugador…"), self.find_player, "Ctrl+Shift+F")
        make("friends", tr("Amigos…"), self.edit_friends, "Ctrl+Shift+A")
        make("clear_tree_marks", tr("Quitar filtros de mod/mapa"), self.clear_tree_marks)
        make("quit", tr("Salir"), self._quit_app, QKeySequence.Quit)
        make("about", tr("Acerca de"), self.show_about)
        # Filtros rápidos: la casilla de la barra lateral y la acción se reflejan.
        for key, text, check in (("hide_empty", tr("Ocultar vacíos"), self.hide_empty_check),
                                 ("hide_full", tr("Ocultar llenos"), self.hide_full_check),
                                 ("hide_password", tr("Ocultar con contraseña"),
                                  self.hide_password_check),
                                 ("hide_bots", tr("Ocultar solo bots"), self.hide_bots_check)):
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
        menu.addSeparator()
        menu.addAction(acts["export_favorites"])
        menu.addAction(acts["import_favorites"])
        menu.addAction(acts["copy_favorites"])
        menu.addSeparator()
        menu.addAction(acts["settings"])
        menu.addSeparator()
        menu.addAction(acts["quit"])
        bar.addMenu(tr("Ver")).addAction(acts["theme"])
        menu = bar.addMenu(tr("Filtro"))
        for key in ("hide_empty", "hide_full", "hide_password", "hide_bots"):
            menu.addAction(acts[key])
        ping_menu = menu.addMenu(tr("Ping máx."))
        ping_menu.addActions(self.ping_actions)
        menu.addSeparator()
        menu.addAction(acts["clear_tree_marks"])
        menu = bar.addMenu(tr("Servidor"))
        for key in ("connect", "refresh", "stop", "refresh_selected", "copy", "favorite",
                    "edit_note", "watch_slot", "party_alert"):
            menu.addAction(acts[key])
            if key == "edit_note":
                menu.addMenu(self.group_menu)
        menu.addSeparator()
        menu.addAction(acts["add_server"])
        menu.addAction(acts["find_player"])
        menu.addAction(acts["friends"])
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
        self.group_combo_action = bar.addWidget(self.group_combo)  # solo en Favoritos
        self.group_combo_action.setVisible(False)
        theme_button = bar.widgetForAction(acts["theme"])
        if isinstance(theme_button, QToolButton):
            theme_button.setPopupMode(QToolButton.InstantPopup)
        self.addToolBar(bar)

    def _setup_context_menu(self) -> None:
        self.table.setContextMenuPolicy(Qt.CustomContextMenu)
        self.table.customContextMenuRequested.connect(self._on_table_context_menu)
        self.favorites_list.setContextMenuPolicy(Qt.CustomContextMenu)
        self.favorites_list.customContextMenuRequested.connect(self._on_direct_context_menu)

    def _setup_shortcuts(self) -> None:
        """Enter/Ctrl+C/Ctrl+V solo con la tabla enfocada; Ctrl+F en toda la ventana (F5, de
        la acción). Ctrl+V abre «Agregar servidor» con las direcciones copiadas."""
        table_keys = ((Qt.Key_Return, self._connect_if_selected),
                      (Qt.Key_Enter, self._connect_if_selected),
                      (QKeySequence.Copy, self.copy_selected_address),
                      (QKeySequence.Paste, self.paste_servers))
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
        for key in ("connect", "copy", "copy_link", "refresh_selected"):
            menu.addAction(acts[key])
        menu.addSeparator()
        menu.addAction(acts["favorite"])
        menu.addAction(acts["edit_note"])
        menu.addAction(self.group_menu.menuAction())
        menu.addAction(acts["watch_slot"])
        menu.addAction(acts["party_alert"])
        if self.in_recent_view():
            menu.addAction(acts["remove_recent"])  # solo existe en la vista Recientes
        menu.addSeparator()
        menu.addAction(acts["add_server"])  # en Favoritos entra como favorito
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
        acts["remove_recent"].setVisible(self.in_recent_view())
        for key in ("refresh_selected", "favorite", "copy", "copy_link", "remove_recent",
                    "party_alert"):
            acts[key].setEnabled(info is not None)
        full = (info is not None and info.online and info.max_players > 0
                and info.num_players >= info.max_players)
        acts["edit_note"].setEnabled(self._is_selected_favorite())
        self.group_menu.menuAction().setEnabled(self._is_selected_favorite())
        acts["watch_slot"].setEnabled(full or self.is_watching(info))
        acts["watch_slot"].setText(tr("Dejar de vigilar") if self.is_watching(info)
                                   else tr("Avisar cuando haya sitio"))
        threshold = (self.party_threshold(game.id, info.address)
                     if game is not None and info is not None else 0)
        acts["party_alert"].setText(
            tr_n("Avisar de partida: {n} jugador…", "Avisar de partida: {n} jugadores…", threshold)
            if threshold else tr("Avisar cuando se arme partida…"))
        acts["clear_tree_marks"].setEnabled(any(self._tree_marks()))
        acts["favorite"].setText(tr("Quitar de favoritos") if self._is_selected_favorite()
                                 else tr("Agregar a favoritos"))
