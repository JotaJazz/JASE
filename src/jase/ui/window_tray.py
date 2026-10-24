"""Bandeja del sistema: minimizar a la bandeja y avisos (notify) para el resto de la ventana."""
from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import QEvent, QTimer
from PySide6.QtGui import QAction
from PySide6.QtWidgets import QApplication, QMenu, QSystemTrayIcon

from jase.i18n import tr
from jase.ui import icons
from jase.ui._mixin_base import MixinBase


class TrayMixin(MixinBase):
    """Mixin de MainWindow (no es QObject).
    No usar @Slot en métodos de mixins: no son QObject y rompe la desconexión al destruir la
    ventana (segfault).

    Espera de MainWindow: cfg, statusBar(), close(), y que el mixin vaya antes de QMainWindow
    en las bases (changeEvent llama a super()).
    """

    def _init_tray(self) -> None:
        self._tray: QSystemTrayIcon | None = None
        self._tray_callback: Callable[[], None] | None = None
        self._tray_menu: QMenu | None = None
        self._apply_tray()

    def _apply_tray(self) -> None:
        """Crea o quita el icono de bandeja según la opción y la disponibilidad."""
        want = self.cfg.minimize_to_tray and QSystemTrayIcon.isSystemTrayAvailable()
        if want and self._tray is None:
            tray = QSystemTrayIcon(icons.app_icon(), self)
            tray.setToolTip("Jimmy's All Seeing Eye")
            menu = QMenu(self)
            show_action = QAction(tr("Mostrar JASE"), menu)
            show_action.triggered.connect(self.restore_from_tray)
            quit_action = QAction(tr("Salir"), menu)
            quit_action.triggered.connect(self.close)
            menu.addAction(show_action)
            menu.addAction(quit_action)
            tray.setContextMenu(menu)
            tray.activated.connect(self._on_tray_activated)
            tray.messageClicked.connect(self._on_tray_message_clicked)
            self._tray_menu = menu
            self._tray = tray
            tray.show()
        elif not want and self._tray is not None:
            self._remove_tray()

    def _remove_tray(self) -> None:
        tray, self._tray = self._tray, None
        self._tray_callback = None
        menu, self._tray_menu = self._tray_menu, None
        if tray is not None:
            tray.hide()
            tray.deleteLater()
        if menu is not None:
            menu.deleteLater()
        if self.isHidden():
            self.restore_from_tray()  # sin bandeja, una ventana oculta no tendría salida

    def _shutdown_tray(self) -> None:
        tray, self._tray = self._tray, None
        self._tray_callback = None
        if tray is not None:
            tray.hide()

    def in_tray(self) -> bool:
        return self._tray is not None and self.isHidden()

    def restore_from_tray(self) -> None:
        self.showNormal()
        self.activateWindow()
        self.raise_()

    def notify(self, title: str, text: str, on_click: Callable[[], None] | None = None) -> None:
        """Aviso al usuario: globo si la ventana está en la bandeja; si no, alerta y barra."""
        if self.in_tray() and self._tray is not None:
            if QSystemTrayIcon.supportsMessages():
                self._tray_callback = on_click  # un solo pendiente: el último reemplaza
                self._tray.showMessage(title, text)
            else:
                # Sin globos, una alerta en una ventana oculta se perdería: se restaura.
                self.restore_from_tray()
                if on_click is not None:
                    on_click()
                else:
                    self.statusBar().showMessage(f"{title}: {text}")
            return
        QApplication.alert(self)
        self.statusBar().showMessage(f"{title}: {text}")

    def _on_tray_message_clicked(self) -> None:
        callback, self._tray_callback = self._tray_callback, None
        if callback is not None:
            callback()

    def _on_tray_activated(self, reason: QSystemTrayIcon.ActivationReason) -> None:
        if reason == QSystemTrayIcon.ActivationReason.DoubleClick:
            self.restore_from_tray()

    def changeEvent(self, event: QEvent) -> None:
        super().changeEvent(event)  # type: ignore[misc]
        if (event.type() == QEvent.Type.WindowStateChange and self._tray is not None
                and self.isMinimized()):
            QTimer.singleShot(0, self._hide_if_minimized)  # no ocultar dentro del evento

    def _hide_if_minimized(self) -> None:
        if self._tray is not None and self.isMinimized():
            self.hide()
