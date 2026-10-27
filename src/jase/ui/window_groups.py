"""Grupos de favoritos («Clan», «LAN», «Probar»...): se asignan con el clic derecho y, en la
vista ★ Favoritos, un combo de la barra muestra uno solo (y solo se consultan esos)."""
from __future__ import annotations

from PySide6.QtWidgets import QComboBox, QInputDialog, QLineEdit, QMenu

from jase.i18n import tr
from jase.models import ServerAddress
from jase.ui import favorites
from jase.ui._mixin_base import MixinBase

ALL_GROUPS = "\x00all"  # dato del combo para «Todos»; "" es «Sin grupo»


class GroupsMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, model, current_game, selected_server(), in_favorites_view(),
    _is_selected_favorite(), _query_direct_list(), _save_config(), _after_list_changed().
    """

    def _build_group_widgets(self) -> None:
        self.group_combo = QComboBox()
        self.group_combo.setObjectName("group_combo")
        self.group_combo.setToolTip(tr("Grupo de favoritos que se muestra"))
        self.group_combo.setMinimumContentsLength(8)
        self.group_combo.currentIndexChanged.connect(self._on_group_changed)
        self.group_menu = QMenu(tr("Grupo"), self)
        self.group_menu.aboutToShow.connect(self._rebuild_group_menu)

    def current_group(self) -> str | None:
        """Grupo elegido en la vista Favoritos; None = todos, "" = sin grupo."""
        if not self.in_favorites_view():
            return None
        data = self.group_combo.currentData()
        return None if data in (None, ALL_GROUPS) else data

    def group_filter(self, game_id: str,
                     addresses: list[ServerAddress]) -> list[ServerAddress]:
        group = self.current_group()
        if group is None:
            return addresses
        return [a for a in addresses if favorites.get_group(self.cfg, game_id, a) == group]

    def _sync_group_combo(self) -> None:
        """Rellena el combo con los grupos del juego; solo se ve en Favoritos con grupos."""
        names = favorites.group_names(self.cfg, self.current_game or "")
        combo = self.group_combo
        selected = combo.currentData()
        combo.blockSignals(True)
        try:
            combo.clear()
            combo.addItem(tr("Todos los grupos"), ALL_GROUPS)
            for name in names:
                combo.addItem(name, name)
            combo.addItem(tr("Sin grupo"), "")
            combo.setCurrentIndex(max(0, combo.findData(selected)))
        finally:
            combo.blockSignals(False)
        self.group_combo_action.setVisible(self.in_favorites_view() and bool(names))
        if combo.currentData() != selected and selected is not None and self.in_favorites_view():
            self._query_direct_list()  # el grupo a la vista se quedó vacío: vuelve a «Todos»

    def _on_group_changed(self, _index: int) -> None:
        if self.in_favorites_view():
            self._query_direct_list()

    def _rebuild_group_menu(self) -> None:
        menu = self.group_menu
        menu.clear()
        info, game_id = self.selected_server(), self.current_game
        if info is None or game_id is None or not self._is_selected_favorite():
            return
        current = favorites.get_group(self.cfg, game_id, info.address)
        for name in favorites.group_names(self.cfg, game_id):
            action = menu.addAction(name)
            action.setCheckable(True)
            action.setChecked(name == current)
            action.triggered.connect(lambda _c=False, n=name: self.move_selected_to_group(n))
        none = menu.addAction(tr("Sin grupo"))
        none.setCheckable(True)
        none.setChecked(not current)
        none.triggered.connect(lambda _c=False: self.move_selected_to_group(""))
        menu.addSeparator()
        menu.addAction(tr("Nuevo grupo…")).triggered.connect(
            lambda _c=False: self.new_group_for_selected())

    def new_group_for_selected(self) -> None:
        name, ok = QInputDialog.getText(self, tr("Nuevo grupo…"), tr("Nombre del grupo:"),
                                        QLineEdit.EchoMode.Normal)
        if ok and favorites.clean_group(name):
            self.move_selected_to_group(name)

    def move_selected_to_group(self, name: str) -> None:
        info, game_id = self.selected_server(), self.current_game
        if info is None or game_id is None or not self._is_selected_favorite():
            return
        favorites.set_group(self.cfg, game_id, info.address, name)
        self._save_config()
        group = self.current_group()
        if group is not None and favorites.clean_group(name) != group:
            self.model.remove(info.address)  # ya no es del grupo que se muestra
        self._sync_group_combo()
        self._after_list_changed()
        clean = favorites.clean_group(name)
        self.statusBar().showMessage(
            tr("{server} pasó al grupo «{group}»", server=info.name, group=clean) if clean
            else tr("{server} quedó sin grupo", server=info.name), 4000)
