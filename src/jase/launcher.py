from __future__ import annotations

import subprocess

from jase.games import GameDef
from jase.models import ServerAddress
from jase.paths import resolve_exe


class LaunchError(Exception):
    """Error al lanzar el juego; el mensaje está pensado para mostrarse al usuario."""


def build_launch_command(
    game: GameDef, exe_path: str, address: ServerAddress, password: str | None
) -> tuple[list[str], str]:
    if game.family == "gamespy":
        target = f"{address}?password={password}" if password else str(address)
        args = [target]
    else:
        args = []
        if password:
            args += ["+password", password]
        args += ["+connect", str(address)]
    # Separa por cualquier separador: en Windows son "\\", en tests Linux pueden ser "/".
    cwd = exe_path[: max(exe_path.rfind("\\"), exe_path.rfind("/"))]
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
    exe_path = resolve_exe(install_dir, game.exe_relpath)
    if exe_path is None:
        raise LaunchError(
            f"No se encontró {game.exe_relpath} en {install_dir}. "
            "Revisa la ruta en Ajustes."
        )
    argv, cwd = build_launch_command(game, exe_path, address, password)
    popen(argv, cwd=cwd)
