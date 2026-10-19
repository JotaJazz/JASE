from jase.games import GAMES
from jase.launcher import build_launch_command, launch, LaunchError
from jase.models import ServerAddress
import pytest

A = ServerAddress("1.2.3.4", 28960)


def test_cod2_command_with_password():
    argv, cwd = build_launch_command(GAMES["cod2"], r"D:\Juegos\CoD2", A, "secreto")
    assert argv == [r"D:\Juegos\CoD2\CoD2MP_s.exe", "+password", "secreto", "+connect", "1.2.3.4:28960"]
    assert cwd == r"D:\Juegos\CoD2"


def test_cod2_command_without_password():
    argv, _ = build_launch_command(GAMES["cod2"], r"D:\Juegos\CoD2", A, None)
    assert argv == [r"D:\Juegos\CoD2\CoD2MP_s.exe", "+connect", "1.2.3.4:28960"]


def test_ut99_command():
    argv, cwd = build_launch_command(GAMES["ut99"], r"D:\Juegos\UnrealTournament", ServerAddress("5.6.7.8", 7777), "pw")
    assert argv == [r"D:\Juegos\UnrealTournament\System\UnrealTournament.exe", "5.6.7.8:7777?password=pw"]
    assert cwd == r"D:\Juegos\UnrealTournament\System"


def test_launch_without_path_raises():
    with pytest.raises(LaunchError, match="ruta"):
        launch(GAMES["cod1"], None, A, None)


def test_launch_missing_exe_raises(tmp_path):
    with pytest.raises(LaunchError, match="No se encontró"):
        launch(GAMES["cod2"], str(tmp_path), A, None)


def test_launch_calls_popen(tmp_path, monkeypatch):
    calls = []
    monkeypatch.setattr("jase.launcher.build_launch_command",
                        lambda g, d, a, p: ([str(tmp_path / "CoD2MP_s.exe"), "+connect", str(a)], str(tmp_path)))
    (tmp_path / "CoD2MP_s.exe").write_bytes(b"")
    launch(GAMES["cod2"], str(tmp_path), A, None, popen=lambda argv, cwd: calls.append((argv, cwd)))
    assert calls and calls[0][1] == str(tmp_path)
