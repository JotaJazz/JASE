from __future__ import annotations

import ntpath
import subprocess

from jase.games import GameDef
from jase.i18n import tr
from jase.models import ServerAddress
from jase.paths import resolve_exe


class LaunchError(Exception):
    """Error al lanzar el juego; el mensaje está pensado para mostrarse al usuario."""


def build_launch_command(
    game: GameDef, exe_path: str, address: ServerAddress, password: str | None,
    mod: str = "",
) -> tuple[list[str], str]:
    if game.launch_style == "unreal":
        target = f"{address}?password={password}" if password else str(address)
        args = [target]
    elif game.launch_style == "valve":
        args = ["-game", game.valve_gamedir]
        if password:
            args += ["+password", password]
        args += ["+connect", str(address)]
    elif game.launch_style == "bf1942":
        args = ["+restart", "1"]
        if mod and mod.casefold() not in {b.casefold() for b in game.base_mods}:
            args += ["+game", mod]
        if password:
            args += ["+password", password]
        args += ["+joinServer", str(address)]
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
    mod: str = "",
) -> None:
    if not install_dir:
        raise LaunchError(
            tr("No hay ruta configurada para {name}. Configúrala en Ajustes.",
               name=game.name)
        )
    exe_path = resolve_exe(install_dir, game.exe_relpath, game.exe_candidates,
                           game.preferred_exes)
    if exe_path is None:
        raise LaunchError(
            tr("No se encontró {exe} en {folder}. Revisa la ruta en Ajustes.",
               exe=game.exe_relpath, folder=install_dir)
        )
    argv, cwd = build_launch_command(game, exe_path, address, password, mod)
    popen(argv, cwd=cwd)
