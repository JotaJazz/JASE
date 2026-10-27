"""Agregar servidores a mano (juego, IP y puerto, uno o varios); desde Favoritos entran
como favoritos. También «Detectar juego» y Ctrl+V en la tabla."""
from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import QPoint
from PySide6.QtWidgets import QApplication, QDialog

from jase import identify, jase_url
from jase.games import GAMES
from jase.i18n import tr, tr_n
from jase.models import ServerAddress, ServerInfo
from jase.ui import favorites
from jase.ui._mixin_base import MixinBase
from jase.ui.add_server import AddServerDialog, find_addresses, query_candidates
from jase.ui.scan_worker import DetectWorker, QueryWorker


def _has_address(text: str) -> bool:
    return bool(find_addresses(text)) or jase_url.parse(text.strip()) is not None


class AddServerMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, model, game_ids, current_game, favorites_list, _workers,
    _query_fn, select_game, show_favorites, in_favorites_view, _in_direct_view,
    select_address, _on_server_response, _save_config, _update_sidebar y _after_list_changed.
    """

    _manual_worker: QueryWorker | None = None
    _manual_favorite = False
    _manual_origin: dict[ServerAddress, ServerAddress] = {}  # candidata → escrita
    _manual_placed: dict[ServerAddress, ServerInfo] = {}  # escrita → respuesta
    _manual_existing: set[ServerAddress] = set()  # ya eran favoritos
    _detect_worker: DetectWorker | None = None
    _detect_responses: list[tuple[str, ServerInfo]] = []
    _detect_done: Callable[[list[tuple[str, ServerAddress]]], None] | None = None

    def add_server_manually(self, favorite: bool | None = None, text: str | None = None) -> None:
        """Pide juego y direcciones; las consulta y muestra seleccionada la primera.

        Abierto desde Favoritos (la vista o su fila del panel), la casilla «Agregar a
        favoritos» viene marcada. Sin `text`, se propone lo que haya en el portapapeles
        si trae alguna dirección."""
        if favorite is None:
            favorite = self.in_favorites_view()
        if text is None:
            clip = QApplication.clipboard().text()
            text = clip if _has_address(clip) else ""
        dialog = AddServerDialog(self.game_ids, self.current_game, favorite, text,
                                 detect=self._detect_game, parent=self)
        try:
            accepted = dialog.exec() == QDialog.DialogCode.Accepted
            game_id, addresses, favorite = (dialog.game_id(), dialog.addresses(),
                                            dialog.favorite())
        finally:
            self._cancel_detect()
            dialog.deleteLater()
        if accepted and addresses and game_id in GAMES:
            self._add_servers(game_id, addresses, favorite)

    def paste_servers(self) -> None:
        """Ctrl+V en la tabla: abre «Agregar servidor» con lo copiado."""
        text = QApplication.clipboard().text()
        if not _has_address(text):
            self.statusBar().showMessage(
                tr("El portapapeles no tiene ninguna dirección IP:puerto"), 4000)
            return
        self.add_server_manually(text=text)

    def _on_direct_context_menu(self, pos: QPoint) -> None:
        """Clic derecho en «★ Favoritos» del panel: agregar un favorito a mano."""
        from jase.ui import main_window  # QMenu parcheable en los tests (ver window_actions)

        item = self.favorites_list.itemAt(pos)
        if item is None or self.favorites_list.row(item) != 0:
            return
        menu = main_window.QMenu(self)
        menu.addAction(tr("Agregar favorito…")).triggered.connect(
            lambda _checked=False: self.add_server_manually(favorite=True))
        menu.addAction(self.ui_actions["copy_favorites"])
        try:
            menu.exec(self.favorites_list.viewport().mapToGlobal(pos))
        finally:
            menu.deleteLater()

    # ------------------------------------------------------------- consulta
    def _add_servers(self, game_id: str, addresses: list[ServerAddress],
                     favorite: bool) -> None:
        game = GAMES[game_id]
        if favorite:
            if game_id != self.current_game or not self.in_favorites_view():
                self.select_game(game_id)
                self.show_favorites()
        else:
            self.select_game(game_id)  # desde Favoritos/Recientes vuelve a la lista normal
        origin: dict[ServerAddress, ServerAddress] = {}
        for address in addresses:
            for candidate in query_candidates(game, address.port, address.host):
                origin.setdefault(candidate, address)
        worker = QueryWorker(game, list(origin), query_fn=self._query_fn)
        worker.server_found.connect(self._on_manual_found)
        worker.finished.connect(self._on_manual_finished)
        self._workers.add(worker)
        if self._manual_worker is not None:
            self._manual_worker.cancel()  # su `finished` ya no es el actual
        self._manual_worker = worker
        self._manual_favorite = favorite
        self._manual_origin = origin
        self._manual_placed = {}
        self._manual_existing = set()
        if len(addresses) == 1:
            self.statusBar().showMessage(tr("Consultando {address}…", address=addresses[0]))
        else:
            self.statusBar().showMessage(
                tr_n("Consultando 1 servidor…", "Consultando {n} servidores…", len(addresses)))
        worker.start()

    def _manual_current(self, worker) -> bool:
        return (isinstance(worker, QueryWorker) and worker is self._manual_worker
                and worker.game.id == self.current_game)

    def _on_manual_found(self, info: ServerInfo) -> None:
        worker = self.sender()
        if not self._manual_current(worker):
            return
        typed = self._manual_origin.get(info.address)
        if typed is None or typed in self._manual_placed:
            return  # ya respondió por el otro puerto
        self._manual_placed[typed] = info
        self._on_server_response(worker.game.id, info)
        self._place_manual(worker.game.id, info, select=len(self._manual_placed) == 1)

    def _on_manual_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, QueryWorker):
            return
        worker.wait()
        self._workers.discard(worker)
        if worker is not self._manual_worker:
            return
        self._manual_worker = None
        if worker.game.id != self.current_game:
            return  # se cambió de juego mientras tanto
        typed = list(dict.fromkeys(self._manual_origin.values()))
        missing = [a for a in typed if a not in self._manual_placed]
        if self._manual_favorite:  # un favorito queda aunque no responda
            for address in missing:
                self._place_manual(worker.game.id, favorites.offline_server(address),
                                   select=address == typed[0] and not self._manual_placed)
        self._report_manual(typed, missing)

    def _place_manual(self, game_id: str, info: ServerInfo, select: bool) -> None:
        """Agrega la fila (y el favorito si se pidió) y, si toca, la selecciona."""
        if self._manual_favorite:
            if favorites.add_favorite(self.cfg, game_id, info.address):
                self._save_config()
                self._update_sidebar()
            else:
                self._manual_existing.add(info.address)
            show = self.in_favorites_view()
        else:
            show = not self._in_direct_view()
        if show:
            self.model.add_or_update(info)
            if select:
                self.select_address(info.address)
        self._after_list_changed()

    def _report_manual(self, typed: list[ServerAddress], missing: list[ServerAddress]) -> None:
        if len(typed) == 1:
            address = typed[0]
            if missing and self._manual_favorite:
                message = tr("{address} no respondió; quedó en favoritos", address=address)
            elif missing:
                message = tr("{address} no respondió; para guardarlo agrégalo como favorito",
                             address=address)
            elif self._manual_favorite:
                placed = self._manual_placed[address].address
                message = (tr("{address} ya estaba en favoritos", address=placed)
                           if placed in self._manual_existing
                           else tr("Agregado a favoritos: {address}", address=placed))
            else:
                message = tr("Servidor agregado: {address}",
                             address=self._manual_placed[address].address)
        elif self._manual_favorite:
            message = tr_n("1 favorito agregado ({missing} sin respuesta)",
                           "{n} favoritos agregados ({missing} sin respuesta)",
                           len(typed), missing=len(missing))
        else:
            message = tr_n("1 servidor respondió de {total}",
                           "{n} servidores respondieron de {total}",
                           len(typed) - len(missing), total=len(typed))
        self.statusBar().showMessage(message, 8000)

    # ------------------------------------------------------------- detectar
    def _detect_game(self, address: ServerAddress,
                     done: Callable[[list[tuple[str, ServerAddress]]], None]) -> None:
        """Para el diálogo: consulta `address` con todas las familias y llama a `done`
        con los juegos reconocidos (de más a menos probable)."""
        self._cancel_detect()
        worker = DetectWorker(address, query_fn=self._query_fn)
        worker.server_found.connect(self._on_detect_found)
        worker.finished.connect(self._on_detect_finished)
        self._workers.add(worker)
        self._detect_worker = worker
        self._detect_responses = []
        self._detect_done = done
        worker.start()

    def _cancel_detect(self) -> None:
        if self._detect_worker is not None:
            self._detect_worker.cancel()  # sigue en _workers hasta `finished`
        self._detect_worker = None
        self._detect_done = None

    def _on_detect_found(self, family: str, info: ServerInfo) -> None:
        if self.sender() is self._detect_worker:
            self._detect_responses.append((family, info))

    def _on_detect_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, DetectWorker):
            return
        worker.wait()
        self._workers.discard(worker)
        if worker is not self._detect_worker:
            return  # cancelada: se cerró el diálogo o se pidió otra
        done = self._detect_done
        self._detect_worker = None
        self._detect_done = None
        if done is not None:
            done(identify.detected_games(self._detect_responses))
