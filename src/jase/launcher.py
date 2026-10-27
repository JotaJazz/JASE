from __future__ import annotations

import ntpath
import shlex
import sys
import os
import subprocess
import webbrowser

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
    elif game.launch_style == "zandronum":
        # El IWAD (DOOM2.WAD...) lo informa el servidor; los PWAD deben estar ya en la
        # carpeta de Zandronum (no se descargan).
        args = ["-iwad", mod] if mod else []
        if password:
            args += ["+cl_password", password, "+cl_joinpassword", password]
        args += ["-connect", str(address)]
    elif game.launch_style == "bf1942":
        args = ["+restart", "1"]
        if mod and mod.casefold() not in {b.casefold() for b in game.base_mods}:
            args += ["+game", mod]
        if password:
            args += ["+password", password]
        args += ["+joinServer", str(address)]
    else:
        # Un mod de Quake II listado como juego aparte arranca ya en su carpeta.
        args = ["+set", "game", game.status_gamedir] if game.status_gamedir else []
        if password:
            args += ["+password", password]
        args += ["+connect", str(address)]
    # ntpath maneja \\ y /, raíces de unidad y rutas UNC.
    cwd = ntpath.dirname(exe_path)
    return [exe_path, *args], cwd


def wine_wrapper(command: str, platform: str = sys.platform) -> list[str] | None:
    """Fuera de Windows, el comando (partido como en la shell) que abre los .exe."""
    if platform == "win32" or not command.strip():
        return None
    try:
        return shlex.split(command)
    except ValueError:  # comillas sin cerrar
        return None


def steam_url(address: ServerAddress, password: str | None) -> str:
    return f"steam://connect/{address}" + (f"/{password}" if password else "")


def _default_open_url(url: str) -> None:
    opener = getattr(os, "startfile", None)  # solo existe en Windows
    if opener is not None:
        opener(url)
    else:
        webbrowser.open(url)


def launch(
    game: GameDef,
    install_dir: str | None,
    address: ServerAddress,
    password: str | None,
    popen=subprocess.Popen,
    mod: str = "",
    open_url=None,
    wrapper: list[str] | None = None,
) -> None:
    """`wrapper`: fuera de Windows, el programa que abre el .exe (p. ej. ["wine"])."""
    if game.launch_style == "steam":
        # Steam resuelve el juego instalado; no hace falta ruta ni exe.
        try:
            (open_url or _default_open_url)(steam_url(address, password))
        except OSError:
            # Sin la URL: puede llevar la contraseña.
            raise LaunchError(tr("No se pudo abrir Steam. ¿Está instalado?")) from None
        return
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
    if wrapper and exe_path.lower().endswith(".exe"):
        argv = [*wrapper, *argv]
    try:
        popen(argv, cwd=cwd)
    except FileNotFoundError:
        if wrapper:
            raise LaunchError(tr("No se encontró «{command}» para abrir el juego. Instala Wine "
                                 "o cambia el comando en Ajustes → General.",
                                 command=wrapper[0])) from None
        raise
