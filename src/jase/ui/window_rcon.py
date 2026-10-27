"""Consola RCON del servidor seleccionado (Servidor → Consola RCON…)."""
from __future__ import annotations

from keyring.errors import KeyringError

from jase import passwords, rcon
from jase.games import GAMES
from jase.ui._mixin_base import MixinBase
from jase.ui.rcon_dialog import RconDialog
from jase.ui.scan_worker import RconWorker


class RconMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: current_game, selected_server(), _workers.
    """

    _rcon_dialogs: dict = {}  # (juego, dirección) → (RconDialog, RconWorker | None)
    _rcon_send_fn = staticmethod(rcon.send)  # los tests lo reemplazan

    def can_rcon(self) -> bool:
        game = GAMES.get(self.current_game or "")
        info = self.selected_server()
        return game is not None and info is not None and info.online \
            and rcon.kind_of(game) is not None

    def open_rcon(self) -> None:
        if not self.can_rcon():
            return
        game, info = GAMES[self.current_game or ""], self.selected_server()
        assert info is not None
        key = (game.id, info.connect_address)
        if not self._rcon_dialogs:
            self._rcon_dialogs = {}  # propio de esta ventana (no el de la clase)
        if key in self._rcon_dialogs:
            dialog = self._rcon_dialogs[key][0]
        else:
            dialog = RconDialog(info.name, str(info.connect_address),
                                passwords.get_rcon_password(game.id, info.connect_address),
                                self)
            dialog.command_requested.connect(
                lambda pw, cmd, remember, k=key: self._rcon_send(k, pw, cmd, remember))
            dialog.finished.connect(lambda _r, k=key: self._rcon_closed(k))
            self._rcon_dialogs[key] = (dialog, None)
        dialog.show()
        dialog.raise_()
        dialog.activateWindow()

    def _rcon_send(self, key, password: str, command: str, remember: bool) -> None:
        game_id, address = key
        dialog, _old = self._rcon_dialogs[key]
        try:
            passwords.set_rcon_password(game_id, address, password if remember else "")
        except KeyringError:
            pass  # sin llavero: se usa solo en esta sesión
        worker = RconWorker(GAMES[game_id], address, password, command,
                            send_fn=self._rcon_send_fn)
        worker.answered.connect(
            lambda text, error, k=key, w=worker: self._rcon_answered(k, w, text, error))
        worker.finished.connect(lambda w=worker: self._rcon_finished(w))
        self._workers.add(worker)
        self._rcon_dialogs[key] = (dialog, worker)
        worker.start()

    def _rcon_answered(self, key, worker: RconWorker, text: str, error: str) -> None:
        entry = self._rcon_dialogs.get(key)
        if entry is None or entry[1] is not worker:
            return  # se cerró la consola
        dialog = entry[0]
        if error:
            dialog.show_error(error, text)
        else:
            dialog.show_answer(text)

    def _rcon_finished(self, worker: RconWorker) -> None:
        worker.wait()
        self._workers.discard(worker)

    def _rcon_closed(self, key) -> None:
        dialog, worker = self._rcon_dialogs.pop(key, (None, None))
        if worker is not None:
            worker.cancel()
        if dialog is not None:
            dialog.deleteLater()
