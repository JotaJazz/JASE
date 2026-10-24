"""Filtros guardados: botón con menú para aplicar, guardar y borrar combinaciones de filtros."""
from __future__ import annotations

from PySide6.QtWidgets import QInputDialog, QMenu, QToolButton

from jase.filter_presets import FilterPreset, remove, upsert
from jase.i18n import tr
from jase.models import QuickFilters
from jase.ui._mixin_base import MixinBase


class PresetsMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, hide_empty_check, hide_full_check, hide_password_check,
    max_ping_combo, filter_edit, _on_quick_filters_changed(), _save_config().
    """

    def _build_presets_button(self) -> QToolButton:
        self.presets_menu = QMenu(self)
        self.presets_menu.aboutToShow.connect(self._rebuild_presets_menu)
        button = QToolButton()
        button.setText(tr("Filtros guardados"))
        button.setPopupMode(QToolButton.ToolButtonPopupMode.InstantPopup)
        button.setMenu(self.presets_menu)
        self.presets_button = button
        self._rebuild_presets_menu()
        return button

    def _rebuild_presets_menu(self) -> None:
        menu = self.presets_menu
        menu.clear()
        presets = self.cfg.filter_presets
        for preset in presets:
            action = menu.addAction(preset.name)
            action.triggered.connect(lambda _c=False, p=preset: self.apply_filter_preset(p))
        if presets:
            menu.addSeparator()
        save = menu.addAction(tr("Guardar filtros actuales…"))
        save.triggered.connect(lambda _c=False: self.save_current_filters())
        delete_menu = menu.addMenu(tr("Borrar"))
        delete_menu.setEnabled(bool(presets))
        for preset in presets:
            action = delete_menu.addAction(preset.name)
            action.triggered.connect(
                lambda _c=False, n=preset.name: self.delete_filter_preset(n))

    def current_filter_preset(self, name: str) -> FilterPreset:
        return FilterPreset(
            name,
            QuickFilters(
                hide_empty=self.hide_empty_check.isChecked(),
                hide_full=self.hide_full_check.isChecked(),
                hide_password=self.hide_password_check.isChecked(),
                max_ping=int(self.max_ping_combo.currentData() or 0)),
            self.filter_edit.text())

    def save_current_filters(self) -> None:
        name, ok = QInputDialog.getText(self, tr("Guardar filtros actuales…"),
                                        tr("Nombre de la combinación:"))
        name = name.strip()
        if not ok or not name:
            return
        presets = upsert(self.cfg.filter_presets, self.current_filter_preset(name))
        if not any(p.name.casefold() == name.casefold() for p in presets):
            return  # tope de combinaciones alcanzado
        self.cfg.filter_presets = presets
        self._save_config()

    def delete_filter_preset(self, name: str) -> None:
        self.cfg.filter_presets = remove(self.cfg.filter_presets, name)
        self._save_config()

    def apply_filter_preset(self, preset: FilterPreset) -> None:
        q = preset.quick
        combo = self.max_ping_combo
        widgets = (self.hide_empty_check, self.hide_full_check, self.hide_password_check, combo)
        for w in widgets:
            w.blockSignals(True)
        try:
            self.hide_empty_check.setChecked(q.hide_empty)
            self.hide_full_check.setChecked(q.hide_full)
            self.hide_password_check.setChecked(q.hide_password)
            idx = combo.findData(q.max_ping)
            if idx < 0:  # fuera de la lista: se añade tal cual, como al cargar la config
                combo.addItem(str(q.max_ping), q.max_ping)
                idx = combo.count() - 1
            combo.setCurrentIndex(idx)
        finally:
            for w in widgets:
                w.blockSignals(False)
        self._on_quick_filters_changed()
        self.filter_edit.setText(preset.text)  # su señal ya refresca la lista
