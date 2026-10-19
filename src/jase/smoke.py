"""Autocomprobación `--smoke`: verifica que el paquete (p. ej. el exe de PyInstaller) arranca.

No abre ventanas visibles, no usa la config real ni la red.
"""
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

from PySide6.QtCore import QSize

from jase import i18n
from jase.config import Config


def _check() -> str:
    """Ejecuta las comprobaciones y devuelve la versión; lanza ante cualquier fallo."""
    if sys.platform != "win32":
        os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    from PySide6.QtWidgets import QApplication

    from jase.ui import flags, icons
    from jase.ui.main_window import MainWindow, app_version

    app = QApplication.instance() or QApplication(sys.argv[:1])
    i18n.set_language("en")
    if i18n.tr("Refrescar lista") != "Refresh list":
        raise RuntimeError("el catálogo inglés no traduce «Refrescar lista»")
    if icons.app_icon().isNull():
        raise RuntimeError("falta el icono de la aplicación")
    if icons.tool_icon("refresh", "dark").isNull():
        raise RuntimeError("falta el icono de herramienta «refresh»")
    flag = flags.flag_icon("cl", QSize(16, 12))
    if flag is None or flag.isNull():
        raise RuntimeError("falta la bandera «cl»")
    if sys.platform == "win32":
        import keyring
        from keyring.backends import fail

        if isinstance(keyring.get_keyring(), fail.Keyring):
            raise RuntimeError("keyring sin backend utilizable")
    with tempfile.TemporaryDirectory(prefix="jase-smoke-") as tmp:
        window = MainWindow(Config(), Path(tmp) / "config.json",
                            scan_fn=lambda *a, **k: None, geo=None)
        window.close()
        window.deleteLater()
        app.processEvents()
    return app_version()


def run() -> int:
    try:
        version = _check()
    except Exception as exc:  # noqa: BLE001 - cualquier fallo es un fallo del smoke
        print(f"SMOKE FAIL: {type(exc).__name__}: {exc}")
        return 1
    print(f"SMOKE OK {version}")
    return 0
