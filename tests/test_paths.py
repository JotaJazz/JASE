from jase.paths import resolve_exe
from pathlib import Path

UT = "System\\UnrealTournament.exe"


def touch(p: Path) -> Path:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(b"")
    return p


def test_resolve_direct(tmp_path):
    exe = touch(tmp_path / "CoD2MP_s.exe")
    assert resolve_exe(str(tmp_path), "CoD2MP_s.exe") == str(exe)


def test_resolve_nested_one_level(tmp_path):
    exe = touch(tmp_path / "UnrealTournament" / "System" / "UnrealTournament.exe")
    assert resolve_exe(str(tmp_path), UT) == str(exe)


def test_resolve_chose_system_folder(tmp_path):
    exe = touch(tmp_path / "System" / "UnrealTournament.exe")
    assert resolve_exe(str(tmp_path / "System"), UT) == str(exe)


def test_resolve_install_dir_ends_in_exe_folder_only(tmp_path):
    # La carpeta termina en "System" y no hay otra copia en el padre.
    d = tmp_path / "Foo" / "System"
    exe = touch(d / "UnrealTournament.exe")
    assert resolve_exe(str(d), UT) == str(exe)


def test_resolve_parent(tmp_path):
    exe = touch(tmp_path / "CoD2MP_s.exe")
    (tmp_path / "main").mkdir()
    assert resolve_exe(str(tmp_path / "main"), "CoD2MP_s.exe") == str(exe)


def test_resolve_case_insensitive(tmp_path):
    exe = touch(tmp_path / "cod2mp_s.exe")
    assert resolve_exe(str(tmp_path), "CoD2MP_s.exe") == str(exe)


def test_resolve_case_insensitive_folders(tmp_path):
    exe = touch(tmp_path / "system" / "unrealtournament.EXE")
    assert resolve_exe(str(tmp_path), UT) == str(exe)


def test_resolve_missing(tmp_path):
    assert resolve_exe(str(tmp_path), "CoD2MP_s.exe") is None
    assert resolve_exe(str(tmp_path / "nope"), "CoD2MP_s.exe") is None


def test_resolve_none_dir():
    assert resolve_exe(None, "CoD2MP_s.exe") is None
    assert resolve_exe("", "CoD2MP_s.exe") is None
