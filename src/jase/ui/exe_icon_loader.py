"""Resuelve el exe de cada juego en un hilo daemon (disco, nunca en el hilo de UI)."""
from __future__ import annotations

import threading

from PySide6.QtCore import QObject, Signal

from jase import detect, paths
from jase.games import GAMES


class _Relay(QObject):
    """Lleva el resultado al hilo de la UI (sin padre; ver CountryDBRelay)."""

    resolved = Signal(int, str, str, str)


class ExeIconLoader(QObject):
    # (generación, game_id, exe para el icono, exe del juego según detect; "" = no hay).
    # El del juego exige además la carpeta del mod (hl.exe solo es CS 1.6 con cstrike\).
    resolved = Signal(int, str, str, str)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._relay = _Relay()  # afinidad: hilo de la UI; sin padre
        self._relay.resolved.connect(self.resolved)

    def request(self, generation: int, paths_by_game: dict[str, str]) -> None:
        """Resuelve el exe de cada juego en un hilo daemon y emite `resolved` por juego."""
        relay = self._relay
        items = dict(paths_by_game)

        def run() -> None:
            for game_id, install_dir in items.items():
                game = GAMES.get(game_id)
                if game is None:
                    continue
                try:
                    exe = paths.resolve_exe(install_dir, game.exe_relpath,
                                            game.exe_candidates, game.preferred_exes)
                    game_exe = detect.resolve_game_exe(game, install_dir)
                except Exception:  # ruta ilegible: sin exe, vuelve el badge
                    exe = game_exe = None
                try:
                    relay.resolved.emit(generation, game_id, exe or "", game_exe or "")
                except RuntimeError:  # aplicación cerrándose
                    return

        threading.Thread(target=run, name="jase-exe-icons", daemon=True).start()
