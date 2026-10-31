"""Abrir servidores desde enlaces jase:// (siempre con confirmación del usuario)."""
from __future__ import annotations

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication, QMessageBox

from jase import jase_url
from jase.games import GAMES
from jase.i18n import tr
from jase.models import ServerInfo
from jase.protocols.quake3 import strip_colors
from jase.ui._mixin_base import MixinBase
from jase.ui.dialogs import confirm_plain
from jase.ui.scan_worker import QueryWorker
from jase.ui.single_instance import LinkServer

LINK_RETRY_MS = 500  # con un diálogo modal abierto, el enlace espera y se reintenta


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
        self._link_busy = False  # consulta o diálogo de un enlace en curso
        self._pending_link: tuple[str, bool] | None = None  # solo el último enlace espera
        self._link_retry = QTimer(self)
        self._link_retry.setSingleShot(True)
        self._link_retry.timeout.connect(self._retry_pending_link)

    def start_link_server(self) -> bool:
        """Escucha enlaces de otras instancias; False si no se pudo (no es grave)."""
        if self._link_server is not None:
            return True
        server = LinkServer(parent=self)
        if not server.listen():
            server.deleteLater()
            return False
        server.url_received.connect(self._on_link_received)
        self._link_server = server
        return True

    def _shutdown_links(self) -> None:
        self._link_worker = None  # la respuesta que llegue se descarta
        self._pending_link = None
        self._link_retry.stop()
        if self._link_server is not None:
            self._link_server.close()
            self._link_server = None

    def _on_link_received(self, url: str) -> None:
        self.open_link(url, interactive=False)

    def open_link(self, url: str, interactive: bool = True) -> None:
        """Abre un enlace. `interactive`: el aviso de enlace inválido es un diálogo (argv)."""
        if self._link_busy:
            self._pending_link = (url, interactive)  # solo se recuerda el último
            return
        if QApplication.activeModalWidget() is not None:
            # Ajustes, Amigos, Buscar juegos…: no cambiar de juego ni preguntar debajo.
            self._pending_link = (url, interactive)
            self._link_retry.start(LINK_RETRY_MS)
            return
        parsed = jase_url.parse(url)
        if parsed is None:
            message = tr("El enlace no es válido.")
            if interactive:
                QMessageBox.warning(self, tr("Enlace jase://"), message)
            else:
                self.statusBar().showMessage(message, 6000)
            return
        game_id, address = parsed
        game = GAMES[game_id]
        self._link_busy = True
        self.restore_from_tray()
        self.select_game(game_id)
        worker = QueryWorker(game, [address], query_fn=self._query_fn)
        worker.server_found.connect(self._on_link_found)
        worker.finished.connect(self._on_link_finished)
        self._link_worker = worker
        self._workers.add(worker)
        self.statusBar().showMessage(tr("Consultando {address}…", address=address))
        worker.start()

    def _retry_pending_link(self) -> None:
        if self._link_busy:
            return  # _link_done abrirá el pendiente
        pending, self._pending_link = self._pending_link, None
        if pending is not None:
            self.open_link(*pending)

    def _link_done(self) -> None:
        self._link_busy = False
        pending, self._pending_link = self._pending_link, None
        if pending is not None:
            self.open_link(*pending)

    def _on_link_found(self, info: ServerInfo) -> None:
        worker = self.sender()
        if worker is not self._link_worker or not isinstance(worker, QueryWorker):
            return
        if worker.game.id != self.current_game:
            self._link_worker = None
            self._link_done()
            return  # se cambió de juego mientras tanto
        self._link_worker = None
        self._on_server_response(worker.game.id, info)
        self.model.add_or_update(info)
        self.select_address(info.address)
        if self._confirm_link(worker.game.name, info):
            self.connect_to(worker.game, info)
        self._link_done()

    def _confirm_link(self, game_name: str, info: ServerInfo) -> bool:
        """Pregunta en texto plano (el nombre es remoto) con juego y dirección visibles."""
        name = strip_colors(info.name) or str(info.address)
        return confirm_plain(self, tr("Enlace jase://"),
                             tr("¿Conectar a {name}?\n\nJuego: {game}\nDirección: {address}",
                                name=name, game=game_name, address=info.address))

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
            self._link_done()

    def copy_selected_link(self) -> None:
        info = self.selected_server()
        if info is None or self.current_game is None:
            return
        QApplication.clipboard().setText(jase_url.build(self.current_game, info.address))
        self.statusBar().showMessage(tr("Enlace copiado"))
