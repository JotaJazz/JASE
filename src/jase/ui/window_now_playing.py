"""«Ahora se juega»: escanea en segundo plano los juegos listos y junta los servidores con
gente en un diálogo. Las listas de la sesión se reutilizan y las nuevas quedan en la caché."""
from __future__ import annotations

from jase import masters
from jase.games import GAMES, GameDef
from jase.i18n import tr, tr_n
from jase.models import ServerInfo
from jase.ui._mixin_base import MixinBase
from jase.ui.now_playing import NowPlayingDialog
from jase.ui.scan_worker import ScanWorker

PARALLEL_SCANS = 3


class NowPlayingMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, game_ids, current_game, _cache, _workers, _scan_fn,
    _steam_key, _official, _custom, can_scan(), searchable_lists(), reveal_server() y
    _update_sidebar().
    """

    _now_dialog: NowPlayingDialog | None = None
    _now_workers: list[ScanWorker] = []
    _now_queue: list[GameDef] = []
    _now_lists: dict[str, list[ServerInfo]] = {}

    def now_playing_games(self) -> list[GameDef]:
        """Juegos listos para jugar (con carpeta, o que conectan por Steam) que se
        pueden escanear."""
        ready = [GAMES[g] for g in self.game_ids
                 if self.cfg.game_paths.get(g) or GAMES[g].launch_style == "steam"]
        return [g for g in ready if self.can_scan(g)]

    def show_now_playing(self) -> None:
        if self._now_dialog is None:
            dialog = self._now_dialog = NowPlayingDialog(self)
            dialog.server_chosen.connect(self.reveal_server)
            dialog.refresh_requested.connect(lambda: self._now_refresh(force=True))
            dialog.finished.connect(lambda _r: self._now_cancel())
        self._now_dialog.show()
        self._now_dialog.raise_()
        self._now_dialog.activateWindow()
        if not self._now_workers:
            self._now_refresh(force=False)

    def _now_refresh(self, force: bool) -> None:
        """Junta las listas: sin `force`, las de la sesión valen; el resto se escanea."""
        self._now_cancel()
        games = self.now_playing_games()
        session = self.searchable_lists()
        self._now_lists = {g.id: list(session[g.id]) for g in games
                           if not force and g.id in session}
        self._now_queue = [g for g in games if force or g.id not in session]
        if not games:
            self._now_show(tr("No hay juegos listos: configura sus carpetas en Ajustes o "
                              "con Archivo → Buscar juegos…"))
            return
        for _ in range(PARALLEL_SCANS):
            self._now_next()
        self._now_show()

    def _now_next(self) -> None:
        if not self._now_queue:
            return
        game = self._now_queue.pop(0)
        worker = ScanWorker(game, masters.effective_masters(game.id, self._official, self._custom),
                            self._scan_fn, steam_key=self._steam_key,
                            steam_include_empty=self.cfg.steam_include_empty)
        worker.server_found.connect(self._on_now_found)
        worker.finished.connect(self._on_now_finished)
        self._workers.add(worker)
        self._now_workers = [*self._now_workers, worker]
        self._now_lists[game.id] = []
        worker.start()

    def _now_cancel(self) -> None:
        for worker in self._now_workers:
            worker.cancel()  # siguen en _workers hasta `finished`
        self._now_workers = []
        self._now_queue = []

    def _on_now_found(self, info: ServerInfo) -> None:
        worker = self.sender()
        if isinstance(worker, ScanWorker) and worker in self._now_workers:
            self._now_lists[worker.game.id].append(info)

    def _on_now_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, ScanWorker):
            return
        worker.wait()
        self._workers.discard(worker)
        if worker not in self._now_workers:
            return  # cancelado
        self._now_workers = [w for w in self._now_workers if w is not worker]
        game_id = worker.game.id
        # Una lista completa sirve también a la ventana (salvo el juego que muestra ahora).
        complete = not worker.failed and worker.result is not None
        if complete and game_id != self.current_game:
            self._cache[game_id] = list(self._now_lists[game_id])
            self._update_sidebar(only=game_id)
        self._now_next()
        self._now_show()

    def _now_show(self, message: str | None = None) -> None:
        dialog = self._now_dialog
        if dialog is None:
            return
        dialog.set_lists(self._now_lists)
        pending = len(self._now_workers) + len(self._now_queue)
        dialog.set_busy(pending > 0)
        if message is None:
            people = sum(i.humans for servers in self._now_lists.values() for i in servers
                         if i.online)
            message = tr_n("{n} jugador en tus juegos", "{n} jugadores en tus juegos", people)
            if pending:
                message += " · " + tr_n("escaneando 1 juego…", "escaneando {n} juegos…",
                                        pending)
        dialog.set_status(message)
