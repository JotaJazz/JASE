"""Filtros de la barra lateral (vacíos, llenos, contraseña, solo bots, ping, país) y el
de versión de la barra de la tabla."""
from __future__ import annotations

from collections import Counter
from collections.abc import Callable

from PySide6.QtGui import QIcon
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QGridLayout,
    QLabel,
    QVBoxLayout,
    QWidget,
)

from jase.config import version_for
from jase.games import ALL_VERSIONS
from jase.geo import country_name
from jase.i18n import tr
from jase.models import QuickFilters, ServerInfo
from jase.ui._mixin_base import MixinBase
from jase.ui.flags import flag_icon
from jase.ui.server_model import FLAG_SIZE


def max_ping_options() -> dict[str, int]:
    """Opciones del filtro "Ping máx." (texto mostrado → ms; 0 = sin límite)."""
    return {tr("Sin límite"): 0, "50": 50, "100": 100, "150": 150, "250": 250}


class CountryCombo(QComboBox):
    """«País»: se rellena al abrirse con los países de la lista cargada (`refresh`)."""

    def __init__(self, refresh: Callable[[], None], parent=None) -> None:
        super().__init__(parent)
        self._refresh = refresh

    def showPopup(self) -> None:  # noqa: N802 (Qt)
        self._refresh()
        super().showPopup()


class FiltersMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, proxy, model, game_tree, favorites_list, version_combo,
    _game(), _save_config(), _after_list_changed() y _build_presets_button().
    """

    def _build_quick_filters(self) -> QWidget:
        """Lista de juegos y, debajo, los filtros rápidos (reflejan la config)."""
        q = self.cfg.quick_filters
        self.hide_empty_check = QCheckBox(tr("Ocultar vacíos"))
        self.hide_full_check = QCheckBox(tr("Ocultar llenos"))
        self.hide_password_check = QCheckBox(tr("Ocultar con contraseña"))
        self.hide_bots_check = QCheckBox(tr("Ocultar solo bots"))
        self.hide_bots_check.setToolTip(
            tr("Oculta los servidores cuyos jugadores son todos bots"))
        for check, value in ((self.hide_empty_check, q.hide_empty),
                             (self.hide_full_check, q.hide_full),
                             (self.hide_password_check, q.hide_password),
                             (self.hide_bots_check, q.hide_bots)):
            check.setChecked(value)
            check.toggled.connect(self._on_quick_filters_changed)

        self.max_ping_combo = QComboBox()
        self.max_ping_combo.setToolTip(tr("Ocultar servidores con ping mayor"))
        for text, ms in max_ping_options().items():
            self.max_ping_combo.addItem(text, ms)
        self._select_ping(q.max_ping)
        self.max_ping_combo.currentIndexChanged.connect(self._on_quick_filters_changed)

        self.country_combo = CountryCombo(self._refresh_country_combo)
        self.country_combo.setToolTip(tr("Mostrar solo servidores de un país"))
        self.country_combo.setIconSize(FLAG_SIZE)
        self._set_country_items(q.country, Counter())
        self.country_combo.currentIndexChanged.connect(self._on_quick_filters_changed)

        panel = QWidget()
        layout = QVBoxLayout(panel)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(self.game_tree, 1)
        layout.addWidget(self.favorites_list)
        for check in (self.hide_empty_check, self.hide_full_check, self.hide_password_check,
                      self.hide_bots_check):
            layout.addWidget(check)
        combos = QGridLayout()  # etiquetas en una columna: los combos quedan alineados
        combos.setContentsMargins(0, 0, 0, 0)
        combos.setColumnStretch(1, 1)
        for row, (label, combo) in enumerate(((tr("Ping máx."), self.max_ping_combo),
                                              (tr("País"), self.country_combo))):
            combos.addWidget(QLabel(label), row, 0)
            combos.addWidget(combo, row, 1)
        layout.addLayout(combos)
        layout.addWidget(self._build_presets_button())
        return panel

    def _select_ping(self, max_ping: int) -> None:
        combo = self.max_ping_combo
        idx = combo.findData(max_ping)
        if idx < 0:  # valor guardado fuera de la lista: se conserva tal cual
            combo.addItem(str(max_ping), max_ping)
            idx = combo.count() - 1
        combo.setCurrentIndex(idx)

    def _set_country_items(self, selected: str, counts: Counter) -> None:
        """«Todos los países» y los de `counts` (por nombre); el elegido queda aunque
        ya no haya servidores suyos."""
        combo = self.country_combo
        codes = set(counts) | ({selected} if selected else set())
        combo.blockSignals(True)
        try:
            combo.clear()
            combo.addItem(tr("Todos los países"), "")
            for code in sorted(codes, key=lambda c: country_name(c).casefold()):
                text = country_name(code)
                if counts.get(code):
                    text += f" ({counts[code]})"
                combo.addItem(flag_icon(code, FLAG_SIZE) or QIcon(), text, code)
            combo.setCurrentIndex(max(0, combo.findData(selected)))
        finally:
            combo.blockSignals(False)

    def _refresh_country_combo(self) -> None:
        servers: list[ServerInfo] = self.model.servers()
        counts = Counter(code for info in servers
                         if (code := self.model.country_code(info)) is not None)
        self._set_country_items(self.country_combo.currentData() or "", counts)

    def _quick_filters_from_widgets(self) -> QuickFilters:
        return QuickFilters(
            hide_empty=self.hide_empty_check.isChecked(),
            hide_full=self.hide_full_check.isChecked(),
            hide_password=self.hide_password_check.isChecked(),
            max_ping=int(self.max_ping_combo.currentData() or 0),
            hide_bots=self.hide_bots_check.isChecked(),
            country=self.country_combo.currentData() or "",
        )

    def _set_quick_filter_widgets(self, q: QuickFilters) -> None:
        """Pone los widgets sin disparar sus señales (luego, _on_quick_filters_changed)."""
        widgets = (self.hide_empty_check, self.hide_full_check, self.hide_password_check,
                   self.hide_bots_check, self.max_ping_combo)
        for w in widgets:
            w.blockSignals(True)
        try:
            self.hide_empty_check.setChecked(q.hide_empty)
            self.hide_full_check.setChecked(q.hide_full)
            self.hide_password_check.setChecked(q.hide_password)
            self.hide_bots_check.setChecked(q.hide_bots)
            self._select_ping(q.max_ping)
        finally:
            for w in widgets:
                w.blockSignals(False)
        self._set_country_items(q.country, Counter())
        # Con las señales bloqueadas el menú Filtro no se enteró: se refleja aquí.
        for key, check in (("hide_empty", self.hide_empty_check),
                           ("hide_full", self.hide_full_check),
                           ("hide_password", self.hide_password_check),
                           ("hide_bots", self.hide_bots_check)):
            self.ui_actions[key].setChecked(check.isChecked())
        index = self.max_ping_combo.currentIndex()
        if 0 <= index < len(self.ping_actions):
            self.ping_actions[index].setChecked(True)

    def _quick_filter_widgets(self) -> list:
        """Lo que se deshabilita en Favoritos/Recientes (ahí no se filtra)."""
        return [self.hide_empty_check, self.hide_full_check, self.hide_password_check,
                self.hide_bots_check, self.max_ping_combo, self.country_combo,
                *(self.ui_actions[k] for k in ("hide_empty", "hide_full", "hide_password",
                                               "hide_bots")),
                *self.ping_actions]

    def _on_quick_filters_changed(self, *_args) -> None:
        self.cfg.quick_filters = self._quick_filters_from_widgets()
        self.proxy.set_quick_filters(self.cfg.quick_filters)
        self._save_config()
        self._after_list_changed()

    # ------------------------------------------------------------ versiones
    def _populate_version_combo(self) -> None:
        game = self._game()
        if game is None:
            return
        selected = version_for(game, self.cfg)
        items = [ALL_VERSIONS, *game.known_versions]
        if selected not in items:
            items.append(selected)
        self.version_combo.blockSignals(True)
        self.version_combo.clear()
        for key in items:  # texto visible = etiqueta; el dato (clave) es lo que filtra
            self.version_combo.addItem(game.version_label(key), key)
        self.version_combo.setCurrentIndex(self.version_combo.findData(selected))
        self.version_combo.blockSignals(False)
        self.proxy.set_version(selected)

    def _add_observed_versions(self, servers: list[ServerInfo]) -> None:
        game = self._game()
        for info in servers:
            if info.version and self.version_combo.findData(info.version) < 0:
                label = game.version_label(info.version) if game else info.version
                self.version_combo.addItem(label, info.version)

    def _on_version_changed(self, index: int) -> None:
        game = self._game()
        version = self.version_combo.itemData(index) if index >= 0 else None
        if game is None or not version:
            return
        self.proxy.set_version(version)
        self.cfg.version_filter[game.id] = version
        self._save_config()
        self._after_list_changed()
