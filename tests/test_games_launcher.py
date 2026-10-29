from jase.games import GAMES
from jase.launcher import build_launch_command, launch, LaunchError
from jase.models import ServerAddress
import pytest

A = ServerAddress("1.2.3.4", 28960)


def test_cod2_command_with_password():
    argv, cwd = build_launch_command(GAMES["cod2"], r"D:\Juegos\CoD2\CoD2MP_s.exe", A, "secreto")
    assert argv == [r"D:\Juegos\CoD2\CoD2MP_s.exe", "+password", "secreto", "+connect", "1.2.3.4:28960"]
    assert cwd == r"D:\Juegos\CoD2"


def test_cod2_command_without_password():
    argv, _ = build_launch_command(GAMES["cod2"], r"D:\Juegos\CoD2\CoD2MP_s.exe", A, None)
    assert argv == [r"D:\Juegos\CoD2\CoD2MP_s.exe", "+connect", "1.2.3.4:28960"]


def test_ut99_command():
    argv, cwd = build_launch_command(GAMES["ut99"], r"D:\Juegos\UnrealTournament\System\UnrealTournament.exe", ServerAddress("5.6.7.8", 7777), "pw")
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
                        lambda g, d, a, p, m="": ([str(tmp_path / "CoD2MP_s.exe"), "+connect", str(a)], str(tmp_path)))
    (tmp_path / "CoD2MP_s.exe").write_bytes(b"")
    launch(GAMES["cod2"], str(tmp_path), A, None, popen=lambda argv, cwd: calls.append((argv, cwd)))
    assert calls and calls[0][1] == str(tmp_path)


def test_launch_forwards_mod_for_bf1942(tmp_path):
    (tmp_path / "BF1942.exe").write_bytes(b"")
    calls = []
    launch(GAMES["bf1942"], str(tmp_path), A, None, mod="xpack1",
           popen=lambda argv, cwd: calls.append(argv))
    assert calls[0][1:] == ["+restart", "1", "+game", "xpack1", "+joinServer", str(A)]


def test_launch_uses_resolved_nested_exe(tmp_path):
    exe = tmp_path / "UnrealTournament" / "System" / "UnrealTournament.exe"
    exe.parent.mkdir(parents=True)
    exe.write_bytes(b"")
    calls = []
    launch(GAMES["ut99"], str(tmp_path), ServerAddress("5.6.7.8", 7777), None,
           popen=lambda argv, cwd: calls.append((argv, cwd)))
    assert calls[0][0][0] == str(exe)
    assert calls[0][1] == str(exe.parent)


def test_cwd_drive_root():
    _, cwd = build_launch_command(GAMES["cod2"], "C:\\CoD2MP_s.exe", A, None)
    assert cwd == "C:\\"


def test_cwd_unc():
    _, cwd = build_launch_command(GAMES["cod2"], "\\\\srv\\share\\CoD2MP_s.exe", A, None)
    assert cwd == "\\\\srv\\share\\"


def test_cwd_posix_style(tmp_path):
    p = str(tmp_path / "CoD2MP_s.exe")
    _, cwd = build_launch_command(GAMES["cod2"], p, A, None)
    assert cwd == str(tmp_path)


def _steam_jka(tmp_path):
    install = tmp_path / "steamapps" / "common" / "Jedi Academy"
    (install / "GameData").mkdir(parents=True)
    (install / "GameData" / "jamp.exe").write_bytes(b"")
    return install


def test_steam_copy_of_jka_gets_steam_app_id(tmp_path, monkeypatch):
    # El exe de Steam abierto a mano da «Application load error 5:0000065434».
    monkeypatch.setenv("JASE_TEST_KEEP", "1")
    calls = []
    launch(GAMES["jka"], str(_steam_jka(tmp_path)), A, None,
           popen=lambda argv, cwd, env=None: calls.append(env))
    env = calls[0]
    assert env["SteamAppId"] == env["SteamGameId"] == "6020"
    assert env["JASE_TEST_KEEP"] == "1"  # hereda el entorno


def test_non_steam_copy_keeps_plain_popen(tmp_path):
    (tmp_path / "GameData").mkdir()
    (tmp_path / "GameData" / "jk2mp.exe").write_bytes(b"")
    calls = []
    launch(GAMES["jk2"], str(tmp_path), A, None, popen=lambda argv, cwd: calls.append(argv))
    assert calls and GAMES["jk2"].steam_appid == 6030


def test_jka_prefers_openjk_client(tmp_path):
    install = _steam_jka(tmp_path)
    openjk = install / "GameData" / "openjk.x86.exe"
    openjk.write_bytes(b"")
    calls = []
    launch(GAMES["jka"], str(install), A, None,
           popen=lambda argv, cwd, env=None: calls.append(argv))
    assert calls[0][0] == str(openjk)
