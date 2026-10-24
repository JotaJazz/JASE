"""Tareas de fondo de MainWindow: base de países, masters e iconos de exe."""
from __future__ import annotations

import time


from jase import masters
from jase.games import GAMES
from jase.i18n import tr
from jase.ui import country_loader, icons, masters_loader
from jase.ui.country_loader import CountryDBRequest
from jase.ui.masters_loader import MastersUpdateRequest



class BackgroundMixin:
    """Mixin de MainWindow (no es QObject).

    Espera de MainWindow: cfg, game_tree, _exe_icons, _exe_icon_loader, _masters_request y los
    cargadores.
    """

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

    def _on_country_db_loaded(self, request, db, error) -> None:
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

    def _on_masters_loaded(self, request, official, error) -> None:
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
