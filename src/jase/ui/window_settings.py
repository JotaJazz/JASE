"""Ajustes, búsqueda de juegos y avisos de ruta de MainWindow."""
from __future__ import annotations


from keyring.errors import KeyringError
from PySide6.QtWidgets import (
    QDialog,
    QMessageBox,
)

from jase import detect, i18n, masters, passwords
from jase.config import save_config
from jase.games import GAMES, GameDef
from jase.i18n import tr
from jase.ui.masters_tab import MastersContext
from jase.ui.find_games import FindGamesDialog



def _accepted(result) -> bool:
    return result in (QDialog.DialogCode.Accepted, QDialog.DialogCode.Accepted.value)


class SettingsMixin:
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, cfg_path, game_tree, _official/_custom, _settings_masters y
    _save_config.
    """

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
                             check_update=self._force_masters_check,
                             steam_key=self._steam_key)
        self._settings_masters = ctx
        # SettingsDialog y system_locale_name se buscan en main_window en cada llamada (los tests
        # los parchean allí); import diferido porque main_window importa este módulo.
        from jase.ui import main_window

        dialog = main_window.SettingsDialog(self.cfg, self,
                                on_update_countries=self.update_country_db if can_update else None,
                                on_find_games=self._pick_found_games, masters=ctx,
                                steam_key=self._steam_key)
        try:
            if _accepted(dialog.exec()):
                masters_changed = self._apply_custom_masters(dialog)
                previous_language = self.cfg.language
                previous_theme = self.cfg.theme
                dialog.apply_to(self.cfg)
                self._apply_auto_refresh()
                key_changed = self._apply_steam_key(dialog.steam_key())
                self._after_game_paths_changed()
                if self.cfg.theme != previous_theme:
                    self.apply_theme()
                system = main_window.system_locale_name()
                if (i18n.resolve_language(self.cfg.language, system)
                        != i18n.resolve_language(previous_language, system)):
                    self._notify_restart_for_language()
                game = self._game()
                if (masters_changed or (key_changed and game is not None
                                        and game.steam_webapi)) \
                        and self.current_game is not None and not self._in_direct_view():
                    self.refresh()  # los masters cambiaron: relistar el juego actual
        finally:
            self._settings_masters = None
            dialog.deleteLater()

    def _apply_steam_key(self, key: str) -> bool:
        """Guarda la clave de Steam en el llavero; devuelve si cambió."""
        if key == self._steam_key:
            return False
        try:
            passwords.set_steam_key(key)
        except KeyringError as exc:
            QMessageBox.warning(
                self, tr("Clave de API de Steam"),
                tr("No se pudo guardar la clave de Steam: {error}\n"
                   "Se usará solo en esta sesión.", error=exc))
        self._steam_key = key  # rige en esta sesión aunque falle el guardado
        return True

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
