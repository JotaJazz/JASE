from __future__ import annotations

import os
import subprocess
from pathlib import PureWindowsPath

from jase.games import GameDef
from jase.models import ServerAddress


class LaunchError(Exception):
    """Error al lanzar el juego; el mensaje está pensado para mostrarse al usuario."""


def build_launch_command(
    game: GameDef, install_dir: str, address: ServerAddress, password: str | None
) -> tuple[list[str], str]:
    exe = PureWindowsPath(install_dir) / PureWindowsPath(game.exe_relpath)
    if game.family == "gamespy":
        target = f"{address}?password={password}" if password else str(address)
        args = [target]
    else:
        args = []
        if password:
            args += ["+password", password]
        args += ["+connect", str(address)]
    return [str(exe), *args], str(exe.parent)


def launch(
    game: GameDef,
    install_dir: str | None,
    address: ServerAddress,
    password: str | None,
    popen=subprocess.Popen,
) -> None:
    if not install_dir:
        raise LaunchError(
            f"No hay ruta configurada para {game.name}. Configúrala en Ajustes."
        )
    exe = PureWindowsPath(install_dir) / PureWindowsPath(game.exe_relpath)
    # Ruta nativa del SO para la comprobación (en Windows equivale a str(exe)).
    native_exe = os.path.join(install_dir, *PureWindowsPath(game.exe_relpath).parts)
    if not os.path.isfile(native_exe):
        raise LaunchError(f"No se encontró {exe}. Revisa la ruta en Ajustes.")
    argv, cwd = build_launch_command(game, install_dir, address, password)
    popen(argv, cwd=cwd)
