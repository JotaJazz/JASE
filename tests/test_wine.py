"""Lanzar juegos de Windows fuera de Windows con Wine (o el comando que se configure)."""
import pytest

from jase import launcher
from jase.config import Config, load_config, save_config
from jase.games import GAMES
from jase.models import ServerAddress

ADDR = ServerAddress("1.2.3.4", 28960)


def test_wine_wrapper_per_platform():
    assert launcher.wine_wrapper("wine", "linux") == ["wine"]
    assert launcher.wine_wrapper('flatpak run "org.winehq.Wine"', "linux") == [
        "flatpak", "run", "org.winehq.Wine"]
    assert launcher.wine_wrapper("wine", "win32") is None
    assert launcher.wine_wrapper("  ", "linux") is None
    assert launcher.wine_wrapper('"sin cerrar', "linux") is None


def test_launch_prefixes_exe_with_wrapper(tmp_path):
    (tmp_path / "CoD2MP_s.exe").write_bytes(b"")
    calls = []
    launcher.launch(GAMES["cod2"], str(tmp_path), ADDR, None,
                    popen=lambda argv, cwd: calls.append((argv, cwd)), wrapper=["wine"])
    argv, cwd = calls[0]
    assert argv[0] == "wine" and argv[1].endswith("CoD2MP_s.exe")
    assert argv[-2:] == ["+connect", "1.2.3.4:28960"]


def test_missing_wine_gives_a_clear_error(tmp_path):
    (tmp_path / "CoD2MP_s.exe").write_bytes(b"")

    def popen(argv, cwd):
        raise FileNotFoundError(argv[0])
    with pytest.raises(launcher.LaunchError, match="umu-run"):
        launcher.launch(GAMES["cod2"], str(tmp_path), ADDR, None, popen=popen,
                        wrapper=["umu-run"])


def test_wine_command_config_roundtrip(tmp_path):
    p = tmp_path / "c.json"
    assert load_config(p).wine_command == "wine"
    save_config(Config(wine_command="umu-run"), p)
    assert load_config(p).wine_command == "umu-run"
    p.write_text('{"wine_command": 5}')
    assert load_config(p).wine_command == "wine"
