"""Tema y estado persistente de la ventana (geometría, divisiones, columnas, config)."""
from __future__ import annotations

import base64
import binascii

from PySide6.QtCore import QByteArray, Qt
from PySide6.QtWidgets import QApplication

from jase.ui._mixin_base import MixinBase
from jase.config import save_config
from jase.i18n import tr
from jase.ui import icons, theme
from jase.ui.server_model import COLUMNS
from jase.ui.window_actions import THEME_CHOICES

# Acción -> SVG de assets/icons (las que no están aquí no llevan icono).
ACTION_ICONS = {"refresh": "refresh", "stop": "stop", "refresh_selected": "refresh_one",
                "connect": "connect", "favorite": "favorite", "copy": "copy",
                "settings": "settings", "theme": "theme"}


class StateMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, cfg_path, model, table, body_splitter, right_splitter,
    details_splitter, occupancy_delegate, ui_actions, theme_actions, current_game, _theme_id,
    statusBar, selected_server, _show_details, _fit_favorites_list y _refresh_game_icons.
    """

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
