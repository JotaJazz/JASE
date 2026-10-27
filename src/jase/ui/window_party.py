"""«Avisar cuando se arme partida»: umbral por servidor y aviso al llegar a él."""
from __future__ import annotations

from PySide6.QtWidgets import QInputDialog

from jase import party
from jase.games import GAMES
from jase.i18n import tr, tr_n
from jase.models import ServerAddress, ServerInfo
from jase.ui._mixin_base import MixinBase


class PartyMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, current_game, selected_server(), _save_config(), notify(),
    restore_from_tray(), reveal_server(), _sync_friend_watch() (la revisión en segundo
    plano de FriendsMixin consulta también estos servidores) y _update_actions().
    """

    def _init_party(self) -> None:
        self._party_tracker = party.PartyTracker()

    def party_threshold(self, game_id: str, address: ServerAddress) -> int:
        """Umbral guardado para el servidor; 0 si no se vigila."""
        return self.cfg.party_alerts.get(game_id, {}).get(str(address), 0)

    def party_addresses(self, game_id: str) -> list[ServerAddress]:
        from jase.ui.favorites import parse_address

        found = (parse_address(a) for a in self.cfg.party_alerts.get(game_id, {}))
        return [a for a in found if a is not None]

    def edit_party_alert(self) -> None:
        info = self.selected_server()
        game_id = self.current_game
        if info is None or game_id is None:
            return
        current = self.party_threshold(game_id, info.address)
        value, ok = QInputDialog.getInt(
            self, tr("Avisar cuando se arme partida"),
            tr("Avisar cuando {server} tenga al menos estos jugadores (0 = no avisar):",
               server=info.name),
            current or party.DEFAULT_THRESHOLD, 0, party.MAX_THRESHOLD)
        if ok:
            self.set_party_alert(game_id, info, value)

    def set_party_alert(self, game_id: str, info: ServerInfo, threshold: int) -> None:
        key = str(info.address)
        alerts = self.cfg.party_alerts.setdefault(game_id, {})
        if threshold > 0:
            alerts[key] = threshold
            message = tr_n("Avisaré cuando {server} tenga {n} jugador",
                           "Avisaré cuando {server} tenga {n} jugadores", threshold,
                           server=info.name)
        else:
            alerts.pop(key, None)
            message = tr("Ya no se avisa de partidas en {server}", server=info.name)
        if not alerts:
            del self.cfg.party_alerts[game_id]
        self._party_tracker.reset(game_id, key)
        self._party_tracker.update(game_id, info, threshold or 1)  # punto de partida
        self._save_config()
        self._sync_friend_watch()
        self._update_actions()
        self.statusBar().showMessage(message, 6000)

    def _check_party(self, game_id: str, info: ServerInfo) -> None:
        """Gancho para toda respuesta de servidor aceptada."""
        threshold = self.party_threshold(game_id, info.address)
        if not threshold or not self._party_tracker.update(game_id, info, threshold):
            return
        game = GAMES.get(game_id)
        address = info.address
        self.notify(tr("Se armó partida"),
                    tr_n("{server}: {n} jugador en {game}", "{server}: {n} jugadores en {game}",
                         info.humans, server=info.name,
                         game=game.name if game else game_id),
                    on_click=lambda: (self.restore_from_tray(),
                                      self.reveal_server(game_id, address)))
