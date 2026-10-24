"""Búsqueda de jugadores de MainWindow: listas de la sesión y revelar el servidor elegido."""
from __future__ import annotations

from jase.models import ServerAddress, ServerInfo
from jase.ui.player_search import PlayerSearchDialog


class PlayerSearchMixin:
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: _cache, current_game, _direct_view, model, _player_search,
    select_game y select_address.
    """

    def searchable_lists(self) -> dict[str, list[ServerInfo]]:
        """Listas de la sesión donde buscar jugadores: la caché más la tabla visible."""
        lists = dict(self._cache)
        if self.current_game is not None and self._direct_view is None:
            lists[self.current_game] = self.model.servers()
        return lists

    def reveal_server(self, game_id: str, address: ServerAddress) -> None:
        """Muestra `game_id` en vista normal y selecciona `address` (o lo deja pendiente)."""
        if game_id != self.current_game or self._direct_view is not None:
            self.select_game(game_id)
        if not self.select_address(address):
            self._pending_select = address

    def find_player(self) -> None:
        """Abre (o reutiliza) el diálogo de búsqueda de jugadores."""
        if self._player_search is None:
            self._player_search = PlayerSearchDialog(self.searchable_lists, self)
            self._player_search.server_chosen.connect(self.reveal_server)
        self._player_search.show()
        self._player_search.raise_()
        self._player_search.activateWindow()
