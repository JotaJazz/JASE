"""Abrir servidores desde enlaces jase:// (siempre con confirmación del usuario)."""
from __future__ import annotations

from PySide6.QtWidgets import QApplication, QMessageBox

from jase import jase_url
from jase.games import GAMES
from jase.i18n import tr
from jase.models import ServerInfo
from jase.ui._mixin_base import MixinBase
from jase.ui.scan_worker import QueryWorker
from jase.ui.single_instance import LinkServer


class LinksMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: model, statusBar, _workers, _query_fn, restore_from_tray,
    select_game, select_address y connect_to.
    """

    def _init_links(self) -> None:
        self._link_worker: QueryWorker | None = None
        self._link_server: LinkServer | None = None

    def start_link_server(self) -> bool:
        """Escucha enlaces de otras instancias; False si no se pudo (no es grave)."""
        if self._link_server is not None:
            return True
        server = LinkServer(parent=self)
        if not server.listen():
            server.deleteLater()
            return False
        server.url_received.connect(self.open_link)
        self._link_server = server
        return True

    def _shutdown_links(self) -> None:
        self._link_worker = None  # la respuesta que llegue se descarta
        if self._link_server is not None:
            self._link_server.close()
            self._link_server = None

    def open_link(self, url: str) -> None:
        parsed = jase_url.parse(url)
        if parsed is None:
            QMessageBox.warning(self, tr("Enlace jase://"), tr("El enlace no es válido."))
            return
        game_id, address = parsed
        game = GAMES[game_id]
        self.restore_from_tray()
        self.select_game(game_id)
        worker = QueryWorker(game, [address], query_fn=self._query_fn)
        worker.server_found.connect(self._on_link_found)
        worker.finished.connect(self._on_link_finished)
        if self._link_worker is not None:
            self._link_worker.cancel()  # sigue en _workers hasta `finished`
        self._link_worker = worker
        self._workers.add(worker)
        self.statusBar().showMessage(tr("Consultando {address}…", address=address))
        worker.start()

    def _on_link_found(self, info: ServerInfo) -> None:
        worker = self.sender()
        if worker is not self._link_worker or not isinstance(worker, QueryWorker):
            return
        if worker.game.id != self.current_game:
            return  # se cambió de juego mientras tanto
        self._link_worker = None
        self.model.add_or_update(info)
        self.select_address(info.address)
        name = info.name or str(info.address)
        answer = QMessageBox.question(self, tr("Enlace jase://"),
                                      tr("¿Conectar a {name}?", name=name))
        if answer == QMessageBox.StandardButton.Yes:
            self.connect_to(worker.game, info)

    def _on_link_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, QueryWorker):
            return
        worker.wait()
        self._workers.discard(worker)
        if worker is self._link_worker:
            self._link_worker = None
            if worker.failed:
                self.statusBar().showMessage(tr("Error al consultar: {error}", error=worker.error))
            else:
                self.statusBar().showMessage(tr("El servidor no respondió"))

    def copy_selected_link(self) -> None:
        info = self.selected_server()
        if info is None or self.current_game is None:
            return
        QApplication.clipboard().setText(jase_url.build(self.current_game, info.address))
        self.statusBar().showMessage(tr("Enlace copiado"))
