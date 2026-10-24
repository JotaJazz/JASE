"""Refresco automático opcional de la lista de MainWindow y selección pendiente."""
from __future__ import annotations

from PySide6.QtWidgets import QApplication

from jase.ui._mixin_base import MixinBase
from jase import masters
from jase.models import ServerAddress, ServerInfo


class AutoRefreshMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, model, proxy, table, _auto_refresh_timer, _direct_view,
    _favorites_worker, _pending_select, _official, _custom, _steam_key, _game, is_scanning,
    selected_server, refresh y _query_direct_list.
    """

    def _apply_auto_refresh(self) -> None:
        """Arranca o detiene el timer según `cfg.auto_refresh_minutes` (0 = desactivado)."""
        minutes = self.cfg.auto_refresh_minutes
        if minutes > 0:
            self._auto_refresh_timer.start(minutes * 60_000)
        else:
            self._auto_refresh_timer.stop()

    def can_scan(self, game) -> bool:
        """Hay masters efectivos y, si el juego usa la API de Steam, hay clave."""
        if not masters.effective_masters(game.id, self._official, self._custom):
            return False
        return not (game.steam_webapi and not self._steam_key)

    def _auto_refresh(self) -> None:
        """Tick del timer: refresca sin molestar (no pisa un escaneo, un diálogo ni una
        ventana minimizada) y conserva el servidor seleccionado."""
        if (self.current_game is None or self.is_scanning()
                or self._favorites_worker is not None
                or QApplication.activeModalWidget() is not None or self.isMinimized()):
            return
        direct = self._direct_view is not None
        if not direct and not self.can_scan(self._game()):
            return  # refrescar vaciaría la tabla sin poder repoblarla
        selected = self.selected_server()
        self._pending_select = selected.address if selected is not None else None
        if direct:
            self._query_direct_list()  # Favoritos/Recientes: sin master
        else:
            self.refresh()

    # ------------------------------------------------------- selección pendiente
    def select_address(self, address: ServerAddress) -> bool:
        """Selecciona y muestra la fila de `address`; False si no está o el proxy la filtra."""
        row = self.model.row_of(address)
        if row < 0:
            return False
        index = self.proxy.mapFromSource(self.model.index(row, 0))
        if not index.isValid():
            return False
        self.table.selectRow(index.row())
        self.table.scrollTo(index)
        self._pending_select = None
        return True

    def _apply_pending_select(self, info: ServerInfo) -> None:
        """Si `info` es la dirección pendiente de selección y ya se ve, la selecciona."""
        if self._pending_select == info.address:
            self.select_address(info.address)

    def _clear_stale_pending(self) -> None:
        """Si el usuario eligió otra fila, la pendiente ya no aplica: al llegar no debe
        quitarle su selección (Enter conectaría a otro servidor)."""
        selected = self.selected_server()
        if (self._pending_select is not None and selected is not None
                and selected.address != self._pending_select):
            self._pending_select = None
