"""Listas guardadas entre sesiones: al abrir JASE, cada juego muestra su último escaneo
completo (como el ASE original) hasta que se refresca."""
from __future__ import annotations

import functools
import queue
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass, field

from PySide6.QtCore import QDateTime, QLocale, QObject, Signal

from jase import server_cache
from jase.games import GAMES
from jase.i18n import tr
from jase.models import ServerInfo
from jase.ui._mixin_base import MixinBase
from jase.ui.daemon import run_daemon


class SavedListRelay(QObject):
    """Lleva la lista leída al hilo de la UI (sin padre; ver MapImageRelay)."""

    loaded = Signal(object, object)  # (SavedListRequest, (guardada en, servidores) | None)


@dataclass(eq=False)
class SavedListRequest:
    game_id: str
    cancelled: bool = field(default=False)


_RELAY: SavedListRelay | None = None
_WRITES: queue.SimpleQueue[Callable[[], None]] = queue.SimpleQueue()
_WRITER: threading.Thread | None = None


def shared_relay() -> SavedListRelay:
    global _RELAY
    if _RELAY is None:
        _RELAY = SavedListRelay()
    return _RELAY


def _write_in_order(job: Callable[[], None]) -> None:
    """Escrituras en un solo hilo daemon y en orden: dos guardados seguidos del mismo
    juego nunca se pisan al revés."""
    global _WRITER
    _WRITES.put(job)
    if _WRITER is None:
        def run() -> None:
            while True:
                try:
                    _WRITES.get()()
                except Exception:  # disco lleno, permisos: la lista anterior sigue
                    pass
        _WRITER = threading.Thread(target=run, name="jase-saved-lists", daemon=True)
        _WRITER.start()


class SavedListsMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg_path, current_game, model, _cache, _direct_view, is_scanning,
    _start_scan(), _show_cached_list() y _update_sidebar().
    """

    def _init_saved_lists(self) -> None:
        self._saved_dir = self.cfg_path.parent / "cache" / "servers"
        self._saved_index = server_cache.read_index(self._saved_dir)  # archivo pequeño
        self._saved_at: dict[str, float] = {}  # juego → fecha de la lista leída del disco
        self._saved_request: SavedListRequest | None = None
        shared_relay().loaded.connect(self._on_saved_list_loaded)

    def _from_disk(self, game_id: str) -> bool:
        """La lista de la sesión de `game_id` es la del disco (no un escaneo de ahora)."""
        return game_id in self._saved_at

    def _save_list(self, game_id: str, servers: list[ServerInfo]) -> None:
        """Lista de un escaneo completo: pasa a ser la de la próxima sesión."""
        servers, now = list(servers), time.time()
        self._saved_at.pop(game_id, None)
        self._saved_index[game_id] = (len(servers), now)
        folder = self._saved_dir
        _write_in_order(lambda: server_cache.save(folder, game_id, servers, now))

    def _forget_saved_lists(self, which: Callable[[str], bool]) -> None:
        folder = self._saved_dir
        for game_id in [g for g in GAMES if which(g)]:
            self._saved_index.pop(game_id, None)
            self._saved_at.pop(game_id, None)
            _write_in_order(functools.partial(server_cache.forget, folder, game_id))

    def _load_saved_list(self, game_id: str) -> bool:
        """Pide la lista guardada en un hilo; False si no hay ninguna (escanear)."""
        if game_id not in self._saved_index:
            return False
        if self._saved_request is not None:
            self._saved_request.cancelled = True
        request = self._saved_request = SavedListRequest(game_id)
        folder = self._saved_dir
        run_daemon("jase-saved-list", request,
                   lambda: (server_cache.load(folder, game_id),), shared_relay().loaded)
        self.statusBar().showMessage(
            tr("Cargando la lista guardada de {name}…", name=GAMES[game_id].name))
        return True

    def _on_saved_list_loaded(self, request: SavedListRequest, result) -> None:
        if request is not self._saved_request:
            return  # de otra ventana o reemplazada
        self._saved_request = None
        game_id = request.game_id
        current = (game_id == self.current_game and self._direct_view is None
                   and not self.is_scanning() and self.model.rowCount() == 0)
        if result is None:  # dañada o borrada: como si no hubiera lista
            self._saved_index.pop(game_id, None)
            if current:
                self._start_scan()
            self._update_sidebar(only=game_id)
            return
        saved_at, servers = result
        if game_id not in self._cache:  # una lista de esta sesión siempre manda
            self._cache[game_id] = servers
            self._saved_at[game_id] = saved_at
        if current:
            self._show_cached_list(game_id)
        self._update_sidebar(only=game_id)

    def _saved_list_message(self, game_id: str) -> str:
        when = QLocale().toString(QDateTime.fromSecsSinceEpoch(int(self._saved_at[game_id])),
                                  QLocale.FormatType.ShortFormat)
        return tr("{name}: lista guardada del {when}; F5 para refrescar",
                  name=GAMES[game_id].name, when=when)
