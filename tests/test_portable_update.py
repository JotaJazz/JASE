"""Actualizar el zip portátil: detección, extracción segura, script y flujo en la ventana."""
import hashlib
import io
import json
import zipfile
from pathlib import Path

import pytest
from PySide6.QtCore import QProcess
from PySide6.QtWidgets import QApplication

from jase import portable_update as pu
from jase.app_update import Asset, newest_release
from jase.ui import update_dialog, window_background
from test_app_update import make_window, payload, rel
from test_app_update_install import Stream


def make_zip(files):
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        for name, data in files.items():
            z.writestr(name, data)
    return buf.getvalue()


GOOD = make_zip({"JASE/JASE.exe": b"exe", "JASE/_internal/x.dll": b"dll"})
ZIP_URL = "https://github.com/JotaJazz/JASE/releases/download/v0.16.0/JASE-0.16.0-win64.zip"


def test_is_portable(tmp_path):
    exe = tmp_path / "JASE.exe"
    assert pu.is_portable(exe, True, "win32")
    assert not pu.is_portable(exe, False, "win32")  # desde el código
    assert not pu.is_portable(exe, True, "linux")
    (tmp_path / pu.UNINSTALLER).write_bytes(b"")
    assert not pu.is_portable(exe, True, "win32")  # instalado con Inno Setup


def test_extract_release_ok_and_rejects_bad_zips(tmp_path):
    zip_path = tmp_path / "a.zip"
    zip_path.write_bytes(GOOD)
    out = pu.extract_release(zip_path, tmp_path / "out")
    assert (out / "JASE.exe").read_bytes() == b"exe"
    for files in ({"otra/JASE.exe": b"x"},
                  {"JASE/JASE.exe": b"x", "../evil.txt": b"x"},
                  {"JASE/JASE.exe": b"x", "/abs.txt": b"x"},
                  {"JASE/JASE.exe": b"x", "C:/win.txt": b"x"}):
        zip_path.write_bytes(make_zip(files))
        with pytest.raises(pu.PortableUpdateError):
            pu.extract_release(zip_path, tmp_path / "bad")
    assert not (tmp_path / "evil.txt").exists()


def test_updater_script_waits_replaces_and_restarts(tmp_path):
    script = pu.updater_script(4321, Path(r"C:\cache\JASE"), Path(r"D:\Juegos\José\JASE"))
    lines = script.split("\r\n")
    assert 'tasklist /FI "PID eq 4321" | find "4321" >nul' in lines
    assert r'if exist "D:\Juegos\José\JASE\_internal" rmdir /s /q "D:\Juegos\José\JASE\_internal"' in lines
    assert any(line.startswith(r'robocopy "C:\cache\JASE" "D:\Juegos\José\JASE" /E') for line in lines)
    assert r'start "" "D:\Juegos\José\JASE\JASE.exe"' in lines
    assert "chcp 65001 >nul" in lines
    with pytest.raises(pu.PortableUpdateError):
        pu.updater_script(1, Path("C:/a"), Path("C:/100% JASE"))
    written = pu.write_updater(tmp_path, 1, Path("C:/a"), Path("C:/b"))
    assert written.name == pu.SCRIPT_NAME and written.read_text(encoding="utf-8")


def _release_with_zip(body):
    r = rel("v0.16.0")
    r["assets"] = [{"name": "JASE-0.16.0-win64.zip", "size": len(body),
                    "digest": "sha256:" + hashlib.sha256(body).hexdigest(),
                    "browser_download_url": ZIP_URL}]
    return r


def test_release_lists_portable_asset():
    r = newest_release(payload(_release_with_zip(GOOD)))
    assert r.portable == Asset("JASE-0.16.0-win64.zip", ZIP_URL, len(GOOD),
                               hashlib.sha256(GOOD).hexdigest())
    assert r.installer is None


def test_portable_click_downloads_extracts_and_runs_script(qtbot, tmp_path, monkeypatch):
    monkeypatch.setattr(window_background.sys, "platform", "win32")
    monkeypatch.setattr(window_background.sys, "frozen", True, raising=False)
    exe_dir = tmp_path / "portable"
    exe_dir.mkdir()
    monkeypatch.setattr(window_background.sys, "executable", str(exe_dir / "JASE.exe"))
    monkeypatch.setattr(update_dialog, "ask_action", lambda p, r: update_dialog.INSTALL)
    monkeypatch.setattr(QApplication, "quit", staticmethod(lambda: None))
    started = []
    monkeypatch.setattr(QProcess, "startDetached",
                        lambda prog, args=None: started.append((prog, args)) or True)
    data = json.dumps([_release_with_zip(GOOD)]).encode()
    w = make_window(qtbot, tmp_path, lambda url: data)
    w.show()
    qtbot.waitUntil(w.update_label.isVisible)
    w._update_fetch_stream = lambda url: Stream(GOOD)
    w.update_label.linkActivated.emit("x")
    qtbot.waitUntil(lambda: bool(started), timeout=5000)
    prog, args = started[0]
    script = Path(args[1])
    assert prog == "cmd.exe" and args[0] == "/c" and script.name == pu.SCRIPT_NAME
    text = script.read_text(encoding="utf-8")
    new_dir = tmp_path / "cache" / "updates" / "portable-JASE-0.16.0-win64.zip" / "JASE"
    assert (new_dir / "JASE.exe").read_bytes() == b"exe"
    assert f'robocopy "{new_dir}" "{exe_dir}"' in text
