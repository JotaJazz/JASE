from __future__ import annotations

import ntpath
import subprocess

from jase.games import GameDef
from jase.models import ServerAddress
from jase.paths import resolve_exe


class LaunchError(Exception):
    """Error al lanzar el juego; el mensaje está pensado para mostrarse al usuario."""


def build_launch_command(
    game: GameDef, exe_path: str, address: ServerAddress, password: str | None
) -> tuple[list[str], str]:
    if game.launch_style == "unreal":
        target = f"{address}?password={password}" if password else str(address)
        args = [target]
    elif game.launch_style == "valve":
        args = ["-game", game.valve_gamedir]
        if password:
            args += ["+password", password]
        args += ["+connect", str(address)]
    else:
        args = []
        if password:
            args += ["+password", password]
        args += ["+connect", str(address)]
    # ntpath maneja \\ y /, raíces de unidad y rutas UNC.
    cwd = ntpath.dirname(exe_path)
    return [exe_path, *args], cwd


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
    exe_path = resolve_exe(install_dir, game.exe_relpath, game.exe_candidates,
                           game.preferred_exes)
    if exe_path is None:
        raise LaunchError(
            f"No se encontró {game.exe_relpath} en {install_dir}. "
            "Revisa la ruta en Ajustes."
        )
    argv, cwd = build_launch_command(game, exe_path, address, password)
    popen(argv, cwd=cwd)
