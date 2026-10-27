"""Copiar, refrescar un servidor y conectarse a él (con contraseña y lanzamiento del juego)."""
from __future__ import annotations

from keyring.errors import KeyringError
from PySide6.QtWidgets import QApplication, QMessageBox

from jase.ui._mixin_base import MixinBase
from jase import launcher, passwords, recent
from jase.browse import BASE_MOD, mod_of
from jase.games import GameDef
from jase.i18n import tr
from jase.launcher import LaunchError
from jase.models import ServerInfo
from jase.ui import favorites
from jase.ui.dialogs import PasswordDialog
from jase.ui.scan_worker import QueryWorker
from jase.ui.window_settings import _accepted


class ConnectMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, model, statusBar, selected_server, current_game, _cache, _workers,
    _query_worker, _query_fn, _launch_fn, _game, is_scanning, _in_direct_view, _in_direct_list,
    _add_observed_versions, _show_details, _after_list_changed, _prompt_missing_path,
    _message_with_settings, _save_config y _update_sidebar.
    """

    _query_worker: QueryWorker | None

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
        return (isinstance(worker, QueryWorker) and worker is self._query_worker
                and worker.game.id == self.current_game)

    def _on_query_server_found(self, info: ServerInfo) -> None:
        if not self._is_current_query(self.sender()):
            return
        if self._in_direct_view():
            game = self._game()
            if not self._in_direct_list(game.id, info.address):
                return  # se quitó de la lista mientras se consultaba
        else:
            self._add_observed_versions([info])
        self.model.add_or_update(info)
        self._on_server_response(self.current_game, info)
        if (self.current_game in self._cache and not self.is_scanning()
                and not self._in_direct_view()):
            self._cache[self.current_game] = self.model.servers()
        selected = self.selected_server()
        if selected is not None and selected.address == info.address:
            self._show_details(info)
        self.statusBar().showMessage(
            tr("Servidor actualizado: {address}", address=info.address), 4000)

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
                if self._in_direct_view() and self._in_direct_list(
                        worker.game.id, address):
                    # En Favoritos/Recientes, un servidor que dejó de responder pasa a offline.
                    self.model.add_or_update(favorites.offline_server(address))
                    self._after_list_changed()

    # ------------------------------------------------------------- conectar
    def connect_selected(self) -> None:
        game = self._game()
        info = self.selected_server()
        if game is not None and info is not None:
            self.connect_to(game, info)
        elif game is not None and not self.cfg.game_paths.get(game.id) \
                and game.launch_style != "steam":
            self._prompt_missing_path(game)  # sin ruta se avisa aunque no haya selección

    def connect_to(self, game: GameDef, info: ServerInfo) -> bool:
        """Lanza `game` hacia `info`; True si se lanzó (False si no)."""
        install_dir = self.cfg.game_paths.get(game.id)
        if not install_dir and game.launch_style != "steam":
            self._prompt_missing_path(game)
            return False
        if not info.online:
            self.statusBar().showMessage(tr("El servidor no responde; refresca para conectar"))
            return False
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
                return False
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
            # BF1942 (carpeta del mod) y Zandronum (IWAD) lo usan al lanzar; se pasa el
            # valor tal como lo envía el servidor, no la clave normalizada de mod_of.
            mod = ""
            if (game.launch_style in ("bf1942", "zandronum") and game.mod_key
                    and mod_of(game, info) != BASE_MOD):
                mod = info.rules.get(game.mod_key, "").strip()
            kwargs: dict = {"mod": mod} if mod else {}
            wrapper = launcher.wine_wrapper(self.cfg.wine_command)
            if wrapper and game.launch_style != "steam":
                kwargs["wrapper"] = wrapper
            self._launch_fn(game, install_dir, info.connect_address, password, **kwargs)
        except (LaunchError, OSError) as exc:
            if game.launch_style == "steam":  # Steam no usa la ruta del juego
                QMessageBox.warning(self, tr("No se pudo conectar"), str(exc))
                return False
            self._message_with_settings(QMessageBox.Warning, tr("No se pudo conectar"),
                                        str(exc),
                                        tr("Revisa la ruta del juego en Ajustes."))
            return False
        self.statusBar().showMessage(
            tr("Lanzando {name} → {address}", name=game.name, address=info.connect_address))
        recent.record_recent(self.cfg, game.id, info.address)
        self._save_config()
        self._update_sidebar()
        return True
