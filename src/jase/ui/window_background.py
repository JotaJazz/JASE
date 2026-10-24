"""Tareas de fondo de MainWindow: base de países, masters e iconos de exe."""
from __future__ import annotations

import html
import sys
import time
from pathlib import Path

from PySide6.QtCore import QProcess, Qt, QTimer, QUrl
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import QLabel, QMessageBox, QProgressDialog


from jase.ui._mixin_base import MixinBase
from jase import app_update, masters
from jase.games import GAMES
from jase.i18n import tr
from jase.ui import country_loader, icons, masters_loader, update_dialog, update_loader
from jase.ui.country_loader import CountryDBRequest
from jase.ui.masters_loader import MastersUpdateRequest
from jase.ui.update_loader import UpdateRequest



UPDATE_POLL_MS = 3_600_000  # cada hora; el intervalo real de 24 h lo decide should_check


class BackgroundMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, game_tree, _exe_icons, _exe_icon_loader, _masters_request y los
    cargadores.
    """

    _country_request: CountryDBRequest | None
    _masters_request: MastersUpdateRequest | None
    _update_request: UpdateRequest | None
    _update_release: app_update.Release | None = None
    _download_request: update_dialog.DownloadRequest | None = None
    _download_progress: QProgressDialog | None = None
    _update_fetch_stream = None  # inyectable en tests; por defecto, red real

    def _refresh_game_icons(self) -> None:
        """Badge de inmediato para los juegos sin icono de exe y nueva resolución de los exes."""
        for game_id in GAMES:
            if game_id not in self._exe_icons:
                self.game_tree.set_game_icon(game_id, icons.badge_icon(game_id, self._theme_id))
        self._icon_generation += 1
        paths = {gid: p for gid, p in self.cfg.game_paths.items() if p and gid in GAMES}
        # Un juego sin ruta ya no tiene exe: su icono se descarta ahora mismo.
        for game_id in [g for g in self._exe_icons if g not in paths]:
            self._exe_icons.discard(game_id)
            self.game_tree.set_game_icon(game_id, icons.badge_icon(game_id, self._theme_id))
        if paths:
            self._exe_icon_loader.request(self._icon_generation, paths)

    def _on_exe_icon_resolved(self, generation: int, game_id: str, exe: str) -> None:
        if generation != self._icon_generation or game_id not in GAMES:
            return  # resultado de una resolución anterior
        icon = self._exe_icon_fn(exe) if exe else None
        if icon is not None and not icon.isNull():
            self._exe_icons.add(game_id)
            self.game_tree.set_game_icon(game_id, icon)
        else:  # sin exe (o sin icono): vuelve el badge
            self._exe_icons.discard(game_id)
            self.game_tree.set_game_icon(game_id, icons.badge_icon(game_id, self._theme_id))

    # --------------------------------------------------------------- países
    def _start_country_db(self, refresh: bool) -> bool:
        """Carga (o con `refresh`, descarga de nuevo) la base en un hilo daemon."""
        if self._geo is None or self._country_request is not None:
            return False
        self._country_request = CountryDBRequest(self._geo, self.country_db_path, refresh)
        country_loader.submit(self._country_request)
        return True

    def update_country_db(self) -> None:
        """Botón de Ajustes: vuelve a descargar la base; la actual sigue en uso."""
        if self._start_country_db(refresh=True):
            self.statusBar().showMessage(tr("Descargando base de países…"))

    def country_db_busy(self) -> bool:
        return self._country_request is not None

    def _on_country_db_loaded(self, request: CountryDBRequest, db, error) -> None:
        if request is not self._country_request:
            return  # de otra ventana
        self._country_request = None
        if db is None:
            # Sin diálogos: columna vacía (o la base anterior) y aviso breve.
            self.statusBar().showMessage(error or tr("Base de países no disponible"), 8000)
            return
        self._country_db = db
        self.model.set_country_lookup(db.lookup)
        self._update_card(self.selected_server())
        if request.refresh:
            self.statusBar().showMessage(tr("Base de países actualizada"), 5000)

    # -------------------------------------------------------------- masters
    def check_masters_update(self, force: bool) -> bool:
        """Pide la lista oficial a GitHub (como mucho cada 24 h salvo `force`)."""
        if self._masters_fetch is None or self._masters_request is not None:
            return False
        cache_dir = self.cfg_path.parent / "cache"
        now = time.time()
        if not force and not masters.should_check(cache_dir / masters.META_FILENAME, now):
            return False
        self._masters_request = MastersUpdateRequest(cache_dir, now, self._masters_fetch,
                                                     force=force)
        masters_loader.submit(self._masters_request)
        return True

    def _on_masters_loaded(self, request: MastersUpdateRequest, official, error) -> None:
        if request is not self._masters_request:
            return  # de otra ventana o descartada
        self._masters_request = None
        if official is not None:
            self._official = official
            kind = "updated"
        else:
            kind = "failed" if error else "unchanged"
        listener = self._settings_masters.outcome_listener if self._settings_masters else None
        if listener is not None and (kind == "updated" or request.force):
            listener(kind, official)
        if kind == "updated":
            date = official.updated or time.strftime("%Y-%m-%d")
            self.statusBar().showMessage(
                tr("Lista de masters actualizada ({date})", date=date), 5000)
        elif request.force and listener is None:  # pedido a mano y Ajustes ya se cerró
            self.statusBar().showMessage(
                tr("La lista de masters está al día") if kind == "unchanged"
                else tr("No se pudo descargar la lista de masters"), 5000)

    def _force_masters_check(self) -> None:
        """«Buscar actualización» de Ajustes: el resultado vuelve por `outcome_listener`."""
        if self.check_masters_update(force=True) or self._masters_request is not None:
            return  # en curso: _on_masters_loaded informará
        ctx = self._settings_masters
        if ctx is not None and ctx.outcome_listener is not None:
            ctx.outcome_listener("failed", None)  # sin red disponible

    # ------------------------------------------------------- versión nueva de JASE
    def _build_update_label(self) -> QLabel:
        """Enlace de la barra de estado (oculto hasta que haya versión nueva)."""
        self.update_label = QLabel()
        self.update_label.setTextFormat(Qt.TextFormat.RichText)
        self.update_label.linkActivated.connect(self._on_update_link)
        self.update_label.hide()
        return self.update_label

    def _start_update_check(self) -> None:
        update_loader.shared_relay().loaded.connect(self._on_update_checked)
        self._update_timer = QTimer(self)
        self._update_timer.timeout.connect(self.check_app_update)
        self._update_timer.start(UPDATE_POLL_MS)
        self.check_app_update()

    def _cancel_update_check(self) -> None:
        timer = getattr(self, "_update_timer", None)
        if timer is not None:
            timer.stop()
        if self._update_request is not None:
            self._update_request.cancelled = True
            self._update_request = None
        if self._download_request is not None:
            self._download_request.cancelled = True

    def check_app_update(self) -> bool:
        """Busca una versión nueva en GitHub (como mucho cada 24 h) si está activado."""
        if (self._update_fetch is None or self._update_request is not None
                or not self.cfg.check_app_updates):
            return False
        from jase.ui.main_window import app_version  # import tardío: evita el ciclo
        cache_dir = self.cfg_path.parent / "cache"
        now = time.time()
        if not app_update.should_check(cache_dir / app_update.META_FILENAME, now):
            return False
        self._update_request = UpdateRequest(cache_dir, now, app_version(), self._update_fetch)
        update_loader.submit(self._update_request)
        return True

    def _on_update_checked(self, request, release) -> None:
        if request is not self._update_request:
            return  # de otra ventana o descartada
        self._update_request = None
        if release is None or not release.url.startswith(app_update.RELEASE_PAGE_PREFIX):
            return
        self._update_release = release
        self.update_label.setText(
            f'<a href="{html.escape(release.url, quote=True)}">'
            + html.escape(tr("Nueva versión {version} disponible", version=release.version))
            + "</a>")
        self.update_label.show()

    def _on_update_link(self, _link: str) -> None:
        release = self._update_release
        if release is None:
            return
        if sys.platform == "win32" and release.installer is not None:
            action = update_dialog.ask_action(self, release)
            if action == update_dialog.INSTALL:
                self._start_update_download(release.installer)
            elif action == update_dialog.PAGE:
                QDesktopServices.openUrl(QUrl(release.url))
        else:
            QDesktopServices.openUrl(QUrl(release.url))

    def _start_update_download(self, asset: app_update.Asset) -> None:
        if self._download_request is not None:
            return
        relay = update_dialog.shared_relay()
        relay.progress.connect(self._on_update_progress)
        relay.finished.connect(self._on_update_downloaded)
        request = update_dialog.DownloadRequest(
            asset, self.cfg_path.parent / "cache" / "updates", self._update_fetch_stream)
        self._download_request = request
        dlg = QProgressDialog(tr("Descargando {name}…", name=asset.name), tr("Cancelar"),
                              0, 100, self)
        dlg.setWindowTitle(tr("Actualizar JASE"))
        dlg.setWindowModality(Qt.WindowModality.WindowModal)
        dlg.setMinimumDuration(0)
        dlg.setAutoClose(False)
        dlg.setAutoReset(False)
        dlg.canceled.connect(lambda: setattr(request, "cancelled", True))
        dlg.show()
        self._download_progress = dlg
        update_dialog.submit(request)

    def _end_update_download(self) -> None:
        relay = update_dialog.shared_relay()
        for signal, slot in ((relay.progress, self._on_update_progress),
                             (relay.finished, self._on_update_downloaded)):
            try:
                signal.disconnect(slot)
            except (RuntimeError, TypeError):
                pass
        if self._download_progress is not None:
            self._download_progress.close()
            self._download_progress.deleteLater()
        self._download_progress = None
        self._download_request = None

    def _on_update_progress(self, request, done: int, total: int) -> None:
        if request is not self._download_request or self._download_progress is None:
            return
        self._download_progress.setValue(min(100, done * 100 // total) if total else 0)

    def _on_update_downloaded(self, request: update_dialog.DownloadRequest, path,
                              error: str) -> None:
        if request is not self._download_request:
            return  # de otra ventana o descartada
        cancelled = request.cancelled
        self._end_update_download()
        if cancelled:
            return
        if path is not None:
            if QProcess.startDetached(str(Path(path))):
                self.close()  # el instalador actualiza en el sitio
                return
            error = tr("No se pudo ejecutar el instalador")
        box = QMessageBox(QMessageBox.Icon.Warning, tr("Actualizar JASE"),
                          tr("No se pudo descargar la actualización: {error}", error=error),
                          QMessageBox.StandardButton.NoButton, self)
        open_page = box.addButton(tr("Ver la página"), QMessageBox.ButtonRole.AcceptRole)
        box.addButton(tr("Cerrar"), QMessageBox.ButtonRole.RejectRole)
        box.exec()
        release = self._update_release
        if box.clickedButton() is open_page and release is not None:
            QDesktopServices.openUrl(QUrl(release.url))
