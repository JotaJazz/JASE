"""«Avisar cuando haya sitio»: vigila un servidor lleno y ofrece conectar al liberarse un hueco."""
from __future__ import annotations

from PySide6.QtWidgets import QApplication, QLabel, QMessageBox

from jase.i18n import tr
from jase.models import ServerInfo
from jase.ui.slot_watch import SlotWatcher


class SlotWatchMixin:
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: statusBar(), selected_server(), _game(), _query_fn, connect_to().
    """

    def _init_slot_watch(self) -> None:
        self._slot_watch: SlotWatcher | None = None
        self._slot_watch_name = ""
        self._lingering_watch_workers: set = set()  # consultas canceladas aún vivas
        self.watch_label = QLabel()
        self.watch_label.hide()
        self.statusBar().addPermanentWidget(self.watch_label)

    def is_watching(self, info: ServerInfo | None) -> bool:
        return (info is not None and self._slot_watch is not None
                and self._slot_watch.address == info.address
                and self._slot_watch.game.id == self.current_game)

    def toggle_slot_watch(self) -> None:
        """Vigila el servidor lleno seleccionado; si ya se vigila, deja de hacerlo."""
        info = self.selected_server()
        game = self._game()
        if info is None or game is None:
            return
        if self.is_watching(info):
            self._end_slot_watch()
            return
        self._end_slot_watch()  # uno solo a la vez: el nuevo reemplaza al anterior
        watcher = SlotWatcher(game, info.address, self._query_fn, self)
        watcher.slot_free.connect(self._on_slot_free)
        watcher.expired.connect(self._on_slot_watch_expired)
        self._slot_watch = watcher
        self._slot_watch_name = info.name
        self.watch_label.setText(tr("Vigilando {name}…", name=info.name))
        self.watch_label.show()
        watcher.start()
        self._update_actions()

    def _end_slot_watch(self) -> None:
        watcher, self._slot_watch = self._slot_watch, None
        self.watch_label.hide()
        if watcher is not None:
            watcher.stop()
            # La consulta en curso termina sola (cancelada); no se espera en el hilo de UI.
            worker = watcher.current_worker()
            if worker is not None:
                worker.finished.connect(lambda w=worker: w.wait())
                self._lingering_watch_workers.add(worker)
                worker.finished.connect(lambda w=worker: self._lingering_watch_workers.discard(w))
            watcher.deleteLater()
        self._update_actions()

    def _on_slot_free(self, info: ServerInfo) -> None:
        watcher = self._slot_watch
        if watcher is None or watcher is not self.sender():
            return
        game = watcher.game  # el juego vigilado, aunque la tabla muestre otro
        self._end_slot_watch()
        QApplication.alert(self)
        answer = QMessageBox.question(
            self, tr("Hay sitio"),
            tr("Hay sitio en {name} ({players}/{max}). ¿Conectar ahora?",
               name=info.name, players=info.num_players, max=info.max_players))
        if answer == QMessageBox.Yes:
            self.connect_to(game, info)

    def _on_slot_watch_expired(self) -> None:
        if self._slot_watch is None or self._slot_watch is not self.sender():
            return
        name = self._slot_watch_name
        self._end_slot_watch()
        self.statusBar().showMessage(
            tr("Se dejó de vigilar {name}: {minutes} min sin sitio", name=name,
               minutes=SlotWatcher.MAX_MS // 60_000))

    def _slot_watch_shutdown(self) -> list:
        """Detiene el vigilante al cerrar y devuelve los workers que hay que esperar."""
        workers = list(self._lingering_watch_workers)
        if self._slot_watch is not None:
            self._slot_watch.stop()
            worker = self._slot_watch.current_worker()
            if worker is not None:
                workers.append(worker)
            self._slot_watch = None
        return workers
