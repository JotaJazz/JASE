"""Refresco automático opcional de la lista de MainWindow."""
from __future__ import annotations

from PySide6.QtWidgets import QApplication


class AutoRefreshMixin:
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, _auto_refresh_timer, _direct_view, _favorites_worker,
    _pending_select, is_scanning, selected_server, refresh y _query_direct_list.
    """

    def _apply_auto_refresh(self) -> None:
        """Arranca o detiene el timer según `cfg.auto_refresh_minutes` (0 = desactivado)."""
        minutes = self.cfg.auto_refresh_minutes
        if minutes > 0:
            self._auto_refresh_timer.start(minutes * 60_000)
        else:
            self._auto_refresh_timer.stop()

    def _auto_refresh(self) -> None:
        """Tick del timer: refresca sin molestar (no pisa un escaneo, un diálogo ni una
        ventana minimizada) y conserva el servidor seleccionado."""
        if (self.current_game is None or self.is_scanning()
                or self._favorites_worker is not None
                or QApplication.activeModalWidget() is not None or self.isMinimized()):
            return
        selected = self.selected_server()
        self._pending_select = selected.address if selected is not None else None
        if self._direct_view is not None:
            self._query_direct_list()  # Favoritos/Recientes: sin master
        else:
            self.refresh()
