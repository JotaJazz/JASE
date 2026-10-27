"""Actualizar el zip portátil desde la app (Windows). Núcleo sin Qt.

El exe en uso no se puede sobrescribir: se descarga y verifica el zip de la release, se
extrae en la caché y un script .cmd espera a que JASE se cierre, reemplaza la carpeta
(apartando el `_internal` viejo hasta que la copia sale bien) y vuelve a abrir JASE. La
configuración vive en %APPDATA%, no en la carpeta, así que no se toca.
"""
from __future__ import annotations

import zipfile
from pathlib import Path, PurePosixPath

EXE_NAME = "JASE.exe"
ZIP_TOP = "JASE"  # Compress-Archive -Path dist\JASE: todo va dentro de «JASE/»
MAX_UNPACKED_BYTES = 1024 * 1024 * 1024
UNINSTALLER = "unins000.exe"  # lo deja Inno Setup: si está, es una instalación
SCRIPT_NAME = "jase-update.cmd"


class PortableUpdateError(ValueError):
    """El zip no sirve o la carpeta no se puede actualizar así."""


def is_portable(exe: Path, frozen: bool, platform: str) -> bool:
    """JASE corre desde un zip descomprimido (no del instalador ni desde el código)."""
    return frozen and platform == "win32" and not (exe.parent / UNINSTALLER).exists()


def extract_release(zip_path: Path, dest: Path) -> Path:
    """Extrae el zip en `dest` y devuelve la carpeta con JASE.exe. Rechaza rutas que
    salgan de `dest`, zips demasiado grandes y los que no traen JASE/JASE.exe."""
    dest = Path(dest)
    with zipfile.ZipFile(zip_path) as archive:
        members = archive.infolist()
        names = {PurePosixPath(m.filename.replace("\\", "/")) for m in members}
        if PurePosixPath(ZIP_TOP, EXE_NAME) not in names:
            raise PortableUpdateError("el zip no trae JASE/JASE.exe")
        if sum(m.file_size for m in members) > MAX_UNPACKED_BYTES:
            raise PortableUpdateError("zip demasiado grande")
        for name in names:
            if name.is_absolute() or ".." in name.parts or ":" in name.as_posix():
                raise PortableUpdateError(f"ruta no válida en el zip: {name}")
        archive.extractall(dest)
    return dest / ZIP_TOP


def updater_script(pid: int, source: Path, install_dir: Path) -> str:
    """Script de cmd que reemplaza `install_dir` con `source` cuando el proceso `pid`
    termina y vuelve a abrir JASE. Los caracteres que cmd interpreta (% " ^ & | < >)
    no pueden aparecer en las rutas."""
    for path in (source, install_dir):
        if any(c in str(path) for c in '%"^&|<>'):
            raise PortableUpdateError(f"carpeta con caracteres no admitidos: {path}")
    src, dst = str(source), str(install_dir)
    return "\r\n".join((
        "@echo off",
        "chcp 65001 >nul",  # el script va en UTF-8 (rutas con tildes)
        "setlocal",
        ":wait",
        f'tasklist /FI "PID eq {pid}" | find "{pid}" >nul',
        "if not errorlevel 1 (",
        "  timeout /t 1 /nobreak >nul",
        "  goto wait",
        ")",
        # El _internal viejo se aparta (no se borra) hasta que la copia salga bien.
        f'if exist "{dst}\\_internal.old" rmdir /s /q "{dst}\\_internal.old"',
        f'if exist "{dst}\\_internal" ren "{dst}\\_internal" _internal.old',
        f'robocopy "{src}" "{dst}" /E /R:5 /W:1 /NFL /NDL /NJH /NJS /NP >nul',
        "if errorlevel 8 goto failed",
        f'if exist "{dst}\\_internal.old" rmdir /s /q "{dst}\\_internal.old"',
        f'start "" "{dst}\\{EXE_NAME}"',
        "goto end",
        ":failed",  # fuera de bloques ( ): una ruta con paréntesis no rompe el script
        f'if exist "{dst}\\_internal" rmdir /s /q "{dst}\\_internal"',
        f'if exist "{dst}\\_internal.old" ren "{dst}\\_internal.old" _internal',
        "echo No se pudo copiar la version nueva de JASE. / The new JASE could not be copied.",
        "pause",
        ":end",
        '(goto) 2>nul & del "%~f0"',
        "",
    ))


def write_updater(cache_dir: Path, pid: int, source: Path, install_dir: Path) -> Path:
    script = Path(cache_dir) / SCRIPT_NAME
    script.parent.mkdir(parents=True, exist_ok=True)
    script.write_text(updater_script(pid, source, install_dir), encoding="utf-8")
    return script
