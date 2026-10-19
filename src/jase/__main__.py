"""Punto de entrada: `python -m jase`."""
from __future__ import annotations

import sys

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication

from jase.config import Config, default_config_path, load_config
from jase.games import GAMES
from jase.geo import CountrySource
from jase.ui.main_window import MainWindow, shutdown_lingering_workers

DEFAULT_GAME = "cod2"


def initial_game(cfg: Config) -> str:
    """Último juego abierto si sigue existiendo; si no, el predeterminado."""
    last = cfg.ui_state.get("last_game")
    return last if last in GAMES else DEFAULT_GAME


def main() -> None:
    cfg_path = default_config_path()
    cfg = load_config(cfg_path)
    app = QApplication(sys.argv)
    app.setApplicationName("Jimmy's All Seeing Eye")
    # Aplica el tema a la aplicación; la base de países se carga en segundo plano.
    window = MainWindow(cfg, cfg_path, geo=CountrySource())
    window.show()
    window.select_game(initial_game(cfg))
    QTimer.singleShot(0, window.offer_find_games)
    code = app.exec()
    shutdown_lingering_workers()
    sys.exit(code)


if __name__ == "__main__":
    main()
