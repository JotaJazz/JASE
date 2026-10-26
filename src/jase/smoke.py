"""Autocomprobación `--smoke`: verifica que el paquete (p. ej. el exe de PyInstaller) arranca.

No abre ventanas visibles, no usa la config real ni la red.
"""
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path


def _check() -> str:
    """Ejecuta las comprobaciones y devuelve la versión; lanza ante cualquier fallo."""
    # Imports pesados aquí dentro: si fallan, run() los reporta en vez de un traceback.
    from PySide6.QtCore import QSize

    from jase import i18n
    from jase.config import Config

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
    for mode in ("classic", "xp"):  # iconos a color de 98 y XP (assets/icons/win98, xp)
        if icons.tool_icon("refresh", mode).isNull():
            raise RuntimeError(f"falta el icono «refresh» del tema {mode}")
    import re

    from jase.ui import theme

    for theme_id in theme.VARIANT_IDS:  # gráficos de 98/Luna y flechas teñidas
        for url in re.findall(r'url\("([^"]+)"\)', theme.build_stylesheet(theme_id)):
            if not Path(url).is_file():
                raise RuntimeError(f"el tema {theme_id} usa un gráfico ausente: {url}")
    flag = flags.flag_icon("cl", QSize(16, 12))
    if flag is None or flag.isNull():
        raise RuntimeError("falta la bandera «cl»")
    if sys.platform == "win32":
        import keyring
        from keyring.backends import fail

        if isinstance(keyring.get_keyring(), fail.Keyring):
            raise RuntimeError("keyring sin backend utilizable")
    from jase import masters

    official = masters.bundled_official()  # lanza si falta el asset empaquetado
    if not official.updated:
        raise RuntimeError("masters.json empaquetado sin fecha (se usaría games.py)")
    with tempfile.TemporaryDirectory(prefix="jase-smoke-") as tmp:
        window = MainWindow(Config(), Path(tmp) / "config.json",
                            scan_fn=lambda *a, **k: None, geo=None)
        window.close()
        window.deleteLater()
        app.processEvents()
    return app_version()


def _report(line: str) -> None:
    print(line)
    out = os.environ.get("JASE_SMOKE_OUT")
    if out:
        try:
            Path(out).write_text(line + "\n", encoding="utf-8")
        except OSError:
            pass


def run() -> int:
    try:
        version = _check()
    except Exception as exc:  # noqa: BLE001 - cualquier fallo es un fallo del smoke
        _report(f"SMOKE FAIL: {type(exc).__name__}: {exc}")
        return 1
    _report(f"SMOKE OK {version}")
    return 0
