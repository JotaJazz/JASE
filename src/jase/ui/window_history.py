"""Historial de ocupación de MainWindow: registro de respuestas, guardado y gráfico de la ficha."""
from __future__ import annotations

import contextlib
import threading
import time

from PySide6.QtCore import QTimer

from jase.fetch_meta import atomic_write
from jase.history import History
from jase.models import ServerInfo
from jase.ui import favorites
from jase.ui._mixin_base import MixinBase

SAVE_INTERVAL_S = 600  # cada 10 min


class HistoryMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, cfg_path, server_card, _theme_id, _check_friends().
    """

    def _init_history(self) -> None:
        self._history = History(self.cfg_path.parent / "cache" / "history.json")
        self._history.load()
        self._history_timer = QTimer(self)
        self._history_timer.setInterval(SAVE_INTERVAL_S * 1000)
        self._history_timer.timeout.connect(self._history_save_background)
        self._history_timer.start()

    def _history_save_background(self) -> None:
        """Serializa en el hilo de UI y escribe en un hilo daemon (sin estado compartido)."""
        self._history.prune(self._history_keep())
        data, path = self._history.to_bytes(), self._history.path

        def write() -> None:
            with contextlib.suppress(OSError):
                atomic_write(path, data)

        threading.Thread(target=write, daemon=True, name="jase-history-save").start()

    def _history_keep(self) -> set[tuple[str, str]]:
        keep: set[tuple[str, str]] = set()
        for game_id, entries in self.cfg.favorites.items():
            keep.update((game_id, e) for e in entries)
        for game_id, entries in self.cfg.recent.items():
            keep.update((game_id, e) for e in entries)
        return keep

    def _history_shutdown(self) -> None:
        self._history_timer.stop()
        self._history.prune(self._history_keep())
        with contextlib.suppress(OSError):
            self._history.save()

    def _is_history_server(self, game_id: str, address) -> bool:
        return (favorites.is_favorite(self.cfg, game_id, address)
                or str(address) in self.cfg.recent.get(game_id, []))

    def _on_server_response(self, game_id: str, info: ServerInfo) -> None:
        """Gancho único para toda respuesta de servidor aceptada."""
        self._check_friends(game_id, info)
        self._check_party(game_id, info)
        if (info.online and info.max_players > 0
                and self._is_history_server(game_id, info.address)):
            self._history.record(game_id, str(info.address), info.num_players,
                                 info.max_players, time.time())

    def _update_history_chart(self, game_id: str | None, info: ServerInfo | None) -> None:
        card = self.server_card
        if info is None or not game_id or not self._is_history_server(game_id, info.address):
            card.set_history(None)
            return
        now = time.time()
        local = time.localtime(now)
        values = self._history.hourly_average(game_id, str(info.address), now, local.tm_gmtoff)
        card.set_history(values, info.max_players, local.tm_hour, self._theme_id)
