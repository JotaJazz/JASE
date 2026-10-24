import os
import re
import subprocess
import sys
from importlib import metadata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _env(home: Path, **extra) -> dict:
    env = dict(os.environ)
    env.update(QT_QPA_PLATFORM="offscreen", HOME=str(home), APPDATA=str(home),
               XDG_CONFIG_HOME=str(home / ".config"), XDG_CACHE_HOME=str(home / ".cache"))
    env.update(extra)
    return env


def _run_smoke(env: dict) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-m", "jase", "--smoke"], env=env,
                          capture_output=True, text=True, timeout=120, cwd=ROOT)


def test_version_is_0_12_0():
    assert metadata.version("jase") == "0.12.0"
    text = (ROOT / "pyproject.toml").read_text(encoding="utf-8")
    assert re.search(r'^version = "0\.12\.0"$', text, re.M)


def test_smoke_mode_ok(tmp_path):
    r = _run_smoke(_env(tmp_path))
    assert r.returncode == 0, r.stdout + r.stderr
    assert "SMOKE OK 0.12.0" in r.stdout


def test_smoke_mode_no_network_no_config(tmp_path):
    home = tmp_path / "home"
    home.mkdir()
    hook = tmp_path / "hook"
    hook.mkdir()
    (hook / "sitecustomize.py").write_text(
        "import socket\n"
        "def _deny(*a, **k):\n"
        "    raise OSError('network disabled in smoke test')\n"
        "socket.socket.connect = _deny\n"
        "socket.socket.connect_ex = _deny\n"
        "socket.socket.sendto = _deny\n"
        "socket.create_connection = _deny\n", encoding="utf-8")
    # La caché de fontconfig (ajena a JASE) va fuera del HOME vigilado.
    r = _run_smoke(_env(home, PYTHONPATH=str(hook), XDG_CACHE_HOME=str(tmp_path / "xdg")))
    assert r.returncode == 0, r.stdout + r.stderr
    assert "SMOKE OK" in r.stdout
    assert list(home.iterdir()) == []


def test_smoke_failure_returns_1(monkeypatch, capsys):
    from jase import smoke

    def boom():
        raise RuntimeError("fallo simulado")

    monkeypatch.setattr(smoke, "_check", boom)
    assert smoke.run() == 1
    assert "fallo simulado" in capsys.readouterr().out


def test_spec_includes_assets_and_icon():
    spec = (ROOT / "jase.spec").read_text(encoding="utf-8")
    assert "app_icon.ico" in spec
    assert "assets" in spec
    assert "console=False" in spec
    assert 'name="JASE"' in spec
    assert 'copy_metadata("jase")' in spec


def test_licenses_folder_complete():
    d = ROOT / "packaging" / "licenses"
    for name in ("JASE-LICENSE.txt", "PySide6-LGPL-3.0.txt", "flag-icons-MIT.txt", "DB-IP.txt"):
        assert (d / name).stat().st_size > 100, name
    assert (d / "JASE-LICENSE.txt").read_text() == (ROOT / "LICENSE").read_text()
    lgpl = (d / "PySide6-LGPL-3.0.txt").read_text()
    assert "GNU LESSER GENERAL PUBLIC LICENSE" in lgpl and "Version 3" in lgpl
    assert "GNU GENERAL PUBLIC LICENSE" in lgpl
    assert "CC BY 4.0" in (d / "DB-IP.txt").read_text()


def test_smoke_result_file_ok(tmp_path):
    out = tmp_path / "out.txt"
    r = _run_smoke(_env(tmp_path / "h", JASE_SMOKE_OUT=str(out)))
    assert r.returncode == 0, r.stdout + r.stderr
    assert out.read_text(encoding="utf-8").strip() == "SMOKE OK 0.12.0"


def test_smoke_result_file_on_failure(monkeypatch, tmp_path, capsys):
    from jase import smoke

    out = tmp_path / "out.txt"
    monkeypatch.setenv("JASE_SMOKE_OUT", str(out))
    monkeypatch.setattr(smoke, "_check", lambda: (_ for _ in ()).throw(RuntimeError("roto")))
    assert smoke.run() == 1
    assert out.read_text(encoding="utf-8").startswith("SMOKE FAIL: RuntimeError: roto")


def test_smoke_import_failure_is_reported_in_file(tmp_path):
    # Un fallo de import dentro del smoke queda en el archivo, sin traceback.
    hook = tmp_path / "hook"
    hook.mkdir()
    (hook / "sitecustomize.py").write_text(
        "import sys\nsys.modules['PySide6.QtWidgets'] = None\n", encoding="utf-8")
    out = tmp_path / "out.txt"
    r = _run_smoke(_env(tmp_path / "h", JASE_SMOKE_OUT=str(out), PYTHONPATH=str(hook)))
    assert r.returncode == 1, r.stdout + r.stderr
    assert out.read_text(encoding="utf-8").startswith("SMOKE FAIL")
    assert "Traceback" not in r.stderr


def test_pyinstaller_pin_is_exact():
    text = (ROOT / "pyproject.toml").read_text(encoding="utf-8")
    assert 'build = ["pyinstaller==6.22.3"]' in text


def test_smoke_check_requires_bundled_masters(monkeypatch):
    from jase import masters, smoke

    def broken():
        raise ValueError("sin asset")

    monkeypatch.setattr(masters, "bundled_official", broken)
    assert smoke.run() == 1
