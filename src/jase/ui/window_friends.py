"""Lista de amigos de MainWindow: diálogo, aviso al aparecer, menú de la tabla de jugadores."""
from __future__ import annotations

from PySide6.QtCore import QPoint, Qt, QTimer
from PySide6.QtGui import QFont
from PySide6.QtWidgets import QMenu, QTableWidgetItem

from jase import friends
from jase.friends import FriendSighting, FriendTracker
from jase.recent import recent_addresses
from jase.ui.favorites import favorite_addresses
from jase.ui.friend_watch import FriendWatcher
from jase.games import GAMES
from jase.i18n import tr, tr_n
from jase.models import ServerInfo
from jase.ui._mixin_base import MixinBase
from jase.ui.friends_dialog import FriendsDialog

NOTIFY_BATCH_MS = 1500  # apariciones de una misma tanda de escaneo → un solo aviso


class FriendsMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, _save_config(), players_table, notify(), restore_from_tray(),
    reveal_server(), _query_fn y party_addresses().
    """

    def _init_friends(self) -> None:
        self._friend_tracker = FriendTracker(self.cfg.friends)
        self._friend_pending: list[FriendSighting] = []
        self._friend_timer = QTimer(self)
        self._friend_timer.setSingleShot(True)
        self._friend_timer.setInterval(NOTIFY_BATCH_MS)
        self._friend_timer.timeout.connect(self._flush_friend_sightings)
        self._friend_watch = FriendWatcher(self._friend_watch_targets, self._query_fn, self)
        self._friend_watch.server_seen.connect(self._on_server_response)
        self._sync_friend_watch()
        self.players_table.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.players_table.customContextMenuRequested.connect(self._on_players_context_menu)

    def _watching_friends(self) -> bool:
        return self.cfg.watch_friends and bool(self.cfg.friends)

    def _friend_watch_targets(self) -> list:
        """Favoritos y recientes (si se vigilan amigos) más los de «Se armó partida»."""
        targets = []
        friends_on = self._watching_friends()
        for game_id, game in GAMES.items():
            mine = (favorite_addresses(self.cfg, game_id) + recent_addresses(self.cfg, game_id)
                    if friends_on else [])
            addresses = list(dict.fromkeys(mine + self.party_addresses(game_id)))
            if addresses:
                targets.append((game, addresses))
        return targets

    def _sync_friend_watch(self) -> None:
        """Arranca o detiene la revisión según amigos y avisos de partida."""
        if self._watching_friends() or self.cfg.party_alerts:
            self._friend_watch.start()
        else:
            self._friend_watch.stop()

    def _friend_watch_shutdown(self) -> list:
        """Detiene la revisión al cerrar y devuelve los workers que hay que esperar."""
        self._friend_timer.stop()  # una tanda pendiente no avisa con la ventana cerrada
        self._friend_pending = []
        self._friend_watch.stop()
        worker = self._friend_watch.current_worker()
        return [worker] if worker is not None else []

    def _set_friends(self, names: list[str]) -> None:
        self.cfg.friends = friends.parse(names)
        self._friend_tracker.friends = list(self.cfg.friends)
        self._save_config()
        self._sync_friend_watch()

    def _check_friends(self, game_id: str, info: ServerInfo) -> None:
        """Gancho para toda respuesta de servidor aceptada (escaneo, consulta, favoritos...)."""
        if not self.cfg.friends:
            return
        found = self._friend_tracker.update(game_id, info)
        if found:
            self._friend_pending.extend(found)
            if not self._friend_timer.isActive():
                self._friend_timer.start()

    def _flush_friend_sightings(self) -> None:
        pending, self._friend_pending = self._friend_pending, []
        if not pending:
            return
        first = pending[0]
        if len(pending) == 1:
            game = GAMES.get(first.game_id)
            text = tr("{friend} está en {server} ({game})", friend=first.friend,
                      server=first.server.name, game=game.name if game else first.game_id)
        else:
            text = tr_n("{n} amigo está en línea: {friend}",
                        "{n} amigos están en línea: {friend}", len(pending),
                        friend=", ".join(dict.fromkeys(s.friend for s in pending)))
        game_id, address = first.game_id, first.server.address
        self.notify(tr("Amigo en línea"), text,
                    on_click=lambda: (self.restore_from_tray(),
                                      self.reveal_server(game_id, address)))

    def _friend_font(self, item: QTableWidgetItem, name: str) -> None:
        """Pone en negrita el nombre de un jugador si es amigo."""
        if friends.normalize(name) in {friends.normalize(f) for f in self.cfg.friends}:
            font = QFont(item.font())
            font.setBold(True)
            item.setFont(font)

    def edit_friends(self) -> None:
        dialog = FriendsDialog(self.cfg.friends, self.cfg.watch_friends, self)
        if dialog.exec():
            self.cfg.watch_friends = dialog.watch()
            self._set_friends(dialog.names())
            self._show_details(self.selected_server())

    def _on_players_context_menu(self, pos: QPoint) -> None:
        item = self.players_table.itemAt(pos)
        if item is None:
            return
        name = self.players_table.item(item.row(), 0).text().strip()
        if not name:
            return
        menu = QMenu(self)
        try:
            action = menu.addAction(tr("Agregar a amigos"))
            already = friends.normalize(name) in {friends.normalize(f) for f in self.cfg.friends}
            action.setEnabled(not already)
            action.triggered.connect(lambda _c=False, n=name: self.add_friend(n))
            menu.exec(self.players_table.viewport().mapToGlobal(pos))
        finally:
            menu.deleteLater()

    def add_friend(self, name: str) -> None:
        self._set_friends([*self.cfg.friends, name])
        self._show_details(self.selected_server())
