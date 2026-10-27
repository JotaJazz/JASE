"""Punto de entrada: `python -m jase`."""
from __future__ import annotations

import sys

if __name__ == "__main__" and "--smoke" in sys.argv[1:]:
    # Antes de los imports pesados: un fallo de import lo reporta smoke.run().
    from jase import smoke

    sys.exit(smoke.run())

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication

from jase import app_update, i18n, jase_url, masters
from jase.config import Config, default_config_path, load_config
from jase.games import GAMES
from jase.geo import CountrySource
from jase.ui import icons
from jase.ui.gc_guard import MainThreadGC
from jase.ui.single_instance import send_to_running
from jase.ui.main_window import MainWindow, shutdown_lingering_workers, system_locale_name

DEFAULT_GAME = "cod2"


def initial_game(cfg: Config) -> str:
    """Último juego abierto si sigue existiendo; si no, el predeterminado."""
    last = cfg.ui_state.get("last_game")
    return last if last in GAMES else DEFAULT_GAME


def link_from_argv(argv: list[str]) -> str | None:
    """Primer argumento con forma de enlace jase:// (sin validarlo: lo hace jase_url.parse)."""
    return next((a for a in argv if jase_url.is_link(a)), None)


def main() -> None:
    if "--smoke" in sys.argv[1:]:
        from jase import smoke

        sys.exit(smoke.run())
    cfg_path = default_config_path()
    cfg = load_config(cfg_path)
    app = QApplication(sys.argv)
    gc_guard = MainThreadGC()  # noqa: F841 (vive hasta que main() termina)
    link = link_from_argv(sys.argv[1:])
    if (link is not None and jase_url.parse(link) is not None
            and send_to_running(link)):
        sys.exit(0)  # la JASE ya abierta se encarga; no se crea otra ventana
    i18n.set_language(i18n.resolve_language(cfg.language, system_locale_name()))
    app.setApplicationName("Jimmy's All Seeing Eye")
    app.setWindowIcon(icons.app_icon())
    # Aplica el tema a la aplicación; la base de países se carga en segundo plano.
    window = MainWindow(cfg, cfg_path, geo=CountrySource(), masters_fetch=masters.default_fetch,
                        update_fetch=app_update.default_fetch)
    window.show()
    window.select_game(initial_game(cfg))
    window.start_link_server()
    if link is not None:
        QTimer.singleShot(0, lambda: window.open_link(link))
    QTimer.singleShot(0, window.offer_find_games)
    code = app.exec()
    shutdown_lingering_workers()
    sys.exit(code)


if __name__ == "__main__":
    main()
