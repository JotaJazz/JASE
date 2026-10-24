"""Vistas directas de MainWindow (Favoritos y Recientes): lista, consulta y alta/baja."""
from __future__ import annotations

import json
from pathlib import Path
from typing import Literal

from PySide6.QtWidgets import QFileDialog, QInputDialog, QLineEdit, QMessageBox

from jase.ui._mixin_base import MixinBase
from jase import favorites_io, recent
from jase.fetch_meta import atomic_write
from jase.games import ALL_VERSIONS
from jase.i18n import tr, tr_n
from jase.models import ServerAddress, ServerInfo
from jase.ui import favorites
from jase.ui.scan_worker import QueryWorker
from jase.ui.server_model import (
    QuickFilters,
)

DirectView = Literal["favorites", "recent"]



class FavoritesMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, table, model, proxy, game_tree, filtros rápidos y los métodos de
    escaneo.
    """

    _direct_view: Literal["favorites", "recent"] | None
    _pending_select: ServerAddress | None
    _favorites_worker: QueryWorker | None
    _favorites_answered: set[ServerAddress]

    def in_favorites_view(self) -> bool:
        return self._direct_view == "favorites"

    def in_recent_view(self) -> bool:
        return self._direct_view == "recent"

    def _in_direct_view(self) -> bool:
        """True en cualquiera de las vistas de consulta directa (sin master)."""
        return self._direct_view is not None

    def show_favorites(self) -> None:
        """Vista «★ Favoritos» del juego actual: consulta directa, sin master.

        Los filtros rápidos y el de versión no se aplican (un favorito offline no
        tiene jugadores, ping ni versión); el filtro de texto sí. Se restauran al
        volver a un juego desde la barra lateral."""
        self._show_direct("favorites")

    def show_recent(self) -> None:
        """Vista «🕘 Recientes»: igual que Favoritos, con los servidores a los que
        se conectó el usuario (del más reciente al más antiguo)."""
        self._show_direct("recent")

    def _show_direct(self, view: DirectView) -> None:
        if self._game() is None or self._direct_view == view:
            return
        self._stop_active_scan()
        self._cancel_query()
        self._direct_view = view
        self._set_filters_bypassed(True)
        self._set_favorites_item_selected(0 if view == "favorites" else 1)
        self._query_direct_list()

    def _leave_direct_view(self) -> None:
        """Sale de la vista; quien llama (select_game) repone la lista y la versión."""
        if self._direct_view is None:
            return
        self._direct_view = None
        self._cancel_favorites_query()
        self._set_filters_bypassed(False)
        self._set_favorites_item_selected(None)

    def _direct_addresses(self, game_id: str) -> list[ServerAddress]:
        """Direcciones de la vista directa activa, en su orden."""
        if self._direct_view == "recent":
            return recent.recent_addresses(self.cfg, game_id)
        return favorites.favorite_addresses(self.cfg, game_id)

    def _in_direct_list(self, game_id: str, address: ServerAddress) -> bool:
        return address in self._direct_addresses(game_id)

    def _set_filters_bypassed(self, bypassed: bool) -> None:
        self.proxy.set_quick_filters(QuickFilters() if bypassed else self.cfg.quick_filters)
        self._apply_tree_filter()  # en Favoritos las marcas no se aplican
        if bypassed:
            self.proxy.set_version(ALL_VERSIONS)  # al salir lo repone _populate_version_combo
        for widget in (self.hide_empty_check, self.hide_full_check,
                       self.hide_password_check, self.max_ping_combo, self.version_combo,
                       *(self.ui_actions[k] for k in ("hide_empty", "hide_full",
                                                      "hide_password"))):
            widget.setEnabled(not bypassed)
        for action in self.ping_actions:
            action.setEnabled(not bypassed)
        if not bypassed:
            tip = tr("Versión del juego")
        elif self._direct_view == "recent":
            tip = tr("No aplica en Recientes")
        else:
            tip = tr("No aplica en Favoritos")
        self.version_combo.setToolTip(tip)
        self._update_actions()
        self._update_count_label()

    def _fit_favorites_list(self) -> None:
        """Alto justo para sus filas (depende del padding de la hoja de estilos)."""
        lst = self.favorites_list
        lst.ensurePolished()
        lst.setFixedHeight(
            lst.sizeHintForRow(0) * lst.count() + 2 * lst.frameWidth() + 2)

    def _set_favorites_item_selected(self, row: int | None) -> None:
        self.favorites_list.blockSignals(True)
        try:
            self.favorites_list.setCurrentRow(-1 if row is None else row)
            if row is None:
                self.favorites_list.clearSelection()
        finally:
            self.favorites_list.blockSignals(False)

    def _query_direct_list(self) -> None:
        """(Re)consulta la lista de la vista directa; los que no respondan quedan offline."""
        game = self._game()
        if game is None:
            return
        self._cancel_favorites_query()
        self._cancel_query()
        self.model.clear()
        addresses = self._direct_addresses(game.id)
        if not addresses:
            if self._direct_view == "recent":
                self.statusBar().showMessage(
                    tr("No hay servidores recientes de {name}", name=game.name))
            else:
                self.statusBar().showMessage(
                    tr("No hay favoritos de {name}: agrégalos con clic derecho "
                       "o con la estrella ☆ de la ficha", name=game.name))
            self._after_list_changed()
            return
        worker = QueryWorker(game, addresses, query_fn=self._query_fn)
        worker.server_found.connect(self._on_favorite_found)
        worker.finished.connect(self._on_favorites_finished)
        self._workers.add(worker)
        self._favorites_worker = worker
        self._favorites_answered = set()
        if self._direct_view == "recent":
            self.scanning_label.setText(tr("Consultando recientes…"))
            self.statusBar().showMessage(
                tr_n("Consultando 1 servidor reciente de {name}…",
                     "Consultando {n} servidores recientes de {name}…",
                     len(addresses), name=game.name))
        else:
            self.scanning_label.setText(tr("Consultando favoritos…"))
            self.statusBar().showMessage(
                tr_n("Consultando 1 favorito de {name}…",
                     "Consultando {n} favoritos de {name}…",
                     len(addresses), name=game.name))
        worker.start()
        self._after_list_changed()  # también habilita «Detener»

    def _cancel_favorites_query(self) -> None:
        if self._favorites_worker is not None:
            self._favorites_worker.cancel()  # sigue en _workers hasta `finished`
            self._favorites_worker = None
            self.scanning_label.setText("")
            self._update_actions()

    def _on_favorite_found(self, info: ServerInfo) -> None:
        worker = self.sender()
        if worker is not self._favorites_worker:
            return
        self._favorites_answered.add(info.address)
        if not self._in_direct_list(worker.game.id, info.address):
            return  # se quitó de la lista mientras se consultaba
        self.model.add_or_update(info)
        self._apply_pending_select(info)
        selected = self.selected_server()
        if selected is not None and selected.address == info.address:
            self._show_details(info)

    def _on_favorites_finished(self) -> None:
        worker = self.sender()
        if not isinstance(worker, QueryWorker):
            return
        worker.wait()
        self._workers.discard(worker)
        if worker is not self._favorites_worker:
            return  # cancelada: se salió de la vista o se volvió a consultar
        self._favorites_worker = None
        self.scanning_label.setText("")
        self._update_actions()
        # Solo los que siguen en la lista: uno quitado durante la consulta
        # no vuelve como "sin respuesta".
        current = [a for a in worker.addresses if self._in_direct_list(worker.game.id, a)]
        for address in current:
            if address not in self._favorites_answered:
                self.model.add_or_update(favorites.offline_server(address))
        if self._pending_select is not None:  # p. ej. un favorito offline seleccionado
            self.select_address(self._pending_select)
        self._pending_select = None
        online = sum(1 for a in current if a in self._favorites_answered)
        if worker.failed:
            self.statusBar().showMessage(
                tr("Error al consultar favoritos: {error}", error=worker.error))
        elif self._direct_view == "recent":
            self.statusBar().showMessage(
                tr("Recientes: {online} de {total} respondieron",
                   online=online, total=len(current)))
        else:
            self.statusBar().showMessage(
                tr("Favoritos: {online} de {total} respondieron",
                   online=online, total=len(current)))
        self._after_list_changed()

    def _is_selected_favorite(self) -> bool:
        game, info = self._game(), self.selected_server()
        return bool(game and info and favorites.is_favorite(self.cfg, game.id, info.address))

    def toggle_favorite_selected(self) -> None:
        """Agrega o quita el servidor seleccionado de los favoritos del juego.

        En la vista normal no consulta nada; en Favoritos, quitar borra la fila."""
        game, info = self._game(), self.selected_server()
        if game is None or info is None:
            return
        if favorites.is_favorite(self.cfg, game.id, info.address):
            favorites.remove_favorite(self.cfg, game.id, info.address)
            self._sync_notes()
            message = tr("Quitado de favoritos: {address}", address=info.address)
            if self.in_favorites_view():
                self.model.remove(info.address)
        else:
            favorites.add_favorite(self.cfg, game.id, info.address)
            message = tr("Agregado a favoritos: {address}", address=info.address)
        self._save_config()
        self._update_sidebar()
        self._after_list_changed()
        self.statusBar().showMessage(message, 4000)

    def _sync_notes(self) -> None:
        """Pasa al modelo las notas del juego actual."""
        game = self._game()
        self.model.set_notes(favorites.game_notes(self.cfg, game.id) if game else {})

    def edit_note_selected(self) -> None:
        """Edita la nota del favorito seleccionado (vacía = borrarla)."""
        game, info = self._game(), self.selected_server()
        if game is None or info is None or not self._is_selected_favorite():
            return
        current = favorites.get_note(self.cfg, game.id, info.address)
        text, ok = QInputDialog.getText(
            self, tr("Editar nota…"), tr("Nota para {address}", address=info.address),
            QLineEdit.EchoMode.Normal, current)
        if not ok:
            return
        favorites.set_note(self.cfg, game.id, info.address, text)
        self._save_config()
        self._sync_notes()
        self._after_list_changed()

    def remove_recent_selected(self) -> None:
        """Quita el servidor seleccionado de los recientes (solo en esa vista)."""
        game, info = self._game(), self.selected_server()
        if game is None or info is None or not self.in_recent_view():
            return
        if recent.remove_recent(self.cfg, game.id, info.address):
            self.model.remove(info.address)
            self._save_config()
            self._update_sidebar()
            self._after_list_changed()
            self.statusBar().showMessage(
                tr("Quitado de recientes: {address}", address=info.address), 4000)

    def export_favorites_to_file(self) -> None:
        """Guarda los favoritos (y sus notas) en un archivo JSON elegido por el usuario."""
        name, _ = QFileDialog.getSaveFileName(
            self, tr("Exportar favoritos…"), "jase-favorites.json", tr("Archivos JSON (*.json)"))
        if not name:
            return
        data = json.dumps(favorites_io.export_favorites(self.cfg), ensure_ascii=False, indent=2)
        try:
            atomic_write(Path(name), data.encode("utf-8"))
        except OSError as exc:
            QMessageBox.warning(self, tr("Exportar favoritos…"), str(exc))
            return
        self.statusBar().showMessage(tr("Favoritos exportados a {path}", path=name), 4000)

    def import_favorites_from_file(self) -> None:
        """Fusiona en los favoritos los de un archivo JSON exportado por JASE."""
        name, _ = QFileDialog.getOpenFileName(
            self, tr("Importar favoritos…"), "", tr("Archivos JSON (*.json)"))
        if not name:
            return
        try:
            result = favorites_io.import_favorites(
                self.cfg, favorites_io.read_import_file(Path(name)))
        except OSError as exc:
            QMessageBox.warning(self, tr("Importar favoritos…"),
                                tr("No se pudo leer el archivo: {error}", error=exc))
            return
        except ValueError as exc:  # JSON inválido, formato incorrecto o demasiado grande
            reason = (tr("El archivo es demasiado grande para ser una exportación de favoritos")
                      if isinstance(exc, favorites_io.FileTooLarge)
                      else tr("El archivo no es una exportación válida de favoritos de JASE"))
            QMessageBox.warning(self, tr("Importar favoritos…"), reason)
            return
        self._save_config()
        self._sync_notes()
        self._update_sidebar()
        if self.in_favorites_view():
            self._query_direct_list()
        self.statusBar().showMessage(
            tr_n("{n} favorito importado, {skipped} descartados",
                 "{n} favoritos importados, {skipped} descartados",
                 result.added, skipped=result.skipped), 6000)
