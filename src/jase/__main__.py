"""Punto de entrada: `python -m jase`."""
from __future__ import annotations

import sys

from PySide6.QtWidgets import QApplication

from jase.config import default_config_path, load_config
from jase.ui.main_window import MainWindow, shutdown_lingering_workers

DEFAULT_GAME = "cod2"


def main() -> None:
    cfg_path = default_config_path()
    cfg = load_config(cfg_path)
    app = QApplication(sys.argv)
    app.setApplicationName("Jimmy's All Seeing Eye")
    window = MainWindow(cfg, cfg_path)  # aplica el tema a la aplicación
    window.show()
    window.select_game(DEFAULT_GAME)
    code = app.exec()
    shutdown_lingering_workers()
    sys.exit(code)


if __name__ == "__main__":
    main()
