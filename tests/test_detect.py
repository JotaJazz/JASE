from __future__ import annotations

import os

from jase import detect
from jase.config import Config
from jase.detect import Finding
from jase.games import GAMES
from jase.paths import resolve_exe


def touch(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"")
    return path


def test_is_configured_requires_resolving_exe(tmp_path):
    cfg = Config()
    cfg.game_paths = {"cod2": str(tmp_path / "nope")}
    assert not detect.is_configured(GAMES["cod2"], cfg)
    touch(tmp_path / "cod2" / "CoD2MP_s.exe")
    cfg.game_paths = {"cod2": str(tmp_path / "cod2")}
    assert detect.is_configured(GAMES["cod2"], cfg)


def test_registry_findings_with_fake_reader():
    def reader(subkey, value):
        return "C:\\Games\\CoD2" if subkey == "Activision\\Call of Duty 2" else None

    assert detect.registry_findings(reader) == [
        Finding("cod2", "C:\\Games\\CoD2", "registro")]


def test_registry_findings_without_winreg_is_empty():
    assert detect.registry_findings() == []


def test_steam_libraries_parses_vdf(tmp_path):
    steam = tmp_path / "Steam"
    other = tmp_path / "Lib2"
    (steam / "steamapps").mkdir(parents=True)
    esc = str(other).replace("\\", "\\\\")
    (steam / "steamapps" / "libraryfolders.vdf").write_text(
        '"libraryfolders"\n{\n "0"\n {\n  "path"\t\t"%s"\n }\n'
        ' "1"\n {\n  "path"\t\t"%s"\n }\n}\n' % (str(steam), esc))
    libs = detect.steam_libraries(str(steam))
    assert libs == [str(steam), str(other)]


def test_steam_findings_cod2(tmp_path):
    steam = tmp_path / "Steam"
    touch(steam / "steamapps" / "common" / "Call of Duty 2" / "CoD2MP_s.exe")
    found = detect.steam_findings([str(steam)])
    assert found == [Finding(
        "cod2", str(steam / "steamapps" / "common" / "Call of Duty 2"), "steam")]


def test_steam_findings_jka_gamedata_layout(tmp_path):
    steam = tmp_path / "Steam"
    jka = steam / "steamapps" / "common" / "Jedi Academy"
    touch(jka / "GameData" / "jamp.exe")
    found = detect.steam_findings([str(steam)])
    assert found == [Finding("jka", str(jka), "steam")]
    assert detect.resolve_game_exe(GAMES["jka"], str(jka)) is not None


def test_typical_roots_include_lucasarts():
    roots = detect.typical_roots(["D:\\"])
    for pf in ("Program Files", "Program Files (x86)"):
        assert os.path.join("D:\\", pf, "LucasArts") in roots
    assert os.path.join("D:\\", "Games") in roots


def _ids(findings):
    return {f.game_id for f in findings}


def test_scan_folder_finds_mohaa_variants(tmp_path):
    touch(tmp_path / "MOH" / "MOHAA.exe")
    touch(tmp_path / "MOH" / "moh_spearhead.exe")
    found = detect.scan_folder(str(tmp_path), "carpeta elegida", 3, lambda: False)
    assert _ids(found) == {"mohaa", "mohaas"}
    assert all(f.path == str(tmp_path / "MOH") for f in found)


def test_scan_folder_finds_jka_in_gamedata(tmp_path):
    touch(tmp_path / "JKA" / "GameData" / "jamp.exe")
    found = detect.scan_folder(str(tmp_path), "carpeta elegida", 3, lambda: False)
    assert found == [Finding("jka", str(tmp_path / "JKA"), "carpeta elegida")]
    game = GAMES["jka"]
    assert resolve_exe(found[0].path, game.exe_relpath, game.exe_candidates,
                       game.preferred_exes) == str(tmp_path / "JKA" / "GameData" / "jamp.exe")


def test_scan_folder_ut99_uses_parent_of_system(tmp_path):
    touch(tmp_path / "UT" / "System" / "UnrealTournament.exe")
    found = detect.scan_folder(str(tmp_path), "carpeta elegida", 3, lambda: False)
    assert found == [Finding("ut99", str(tmp_path / "UT"), "carpeta elegida")]


def test_scan_folder_survives_permission_error_and_depth(tmp_path, monkeypatch):
    (tmp_path / "locked").mkdir()
    touch(tmp_path / "a" / "ET.exe")
    touch(tmp_path / "b" / "c" / "d" / "WolfMP.exe")  # profundidad 3
    real = os.scandir

    def fake(path):
        if os.path.basename(str(path)) == "locked":
            raise PermissionError(path)
        return real(path)

    monkeypatch.setattr(os, "scandir", fake)
    found = detect.scan_folder(str(tmp_path), "carpeta típica", 2, lambda: False)
    assert _ids(found) == {"et"}


def test_scan_folder_cancel(tmp_path):
    touch(tmp_path / "a" / "ET.exe")
    assert detect.scan_folder(str(tmp_path), "x", 3, lambda: True) == []


def test_find_games_dedupes_keeping_best_source(tmp_path):
    game = tmp_path / "CoD2"
    touch(game / "CoD2MP_s.exe")
    res = detect.find_games(
        read_value=lambda s, v: str(game) if "Call of Duty 2" in s else None,
        steam_roots=[], drives=[], extra_root=str(tmp_path))
    assert res == {"cod2": [Finding("cod2", str(game), "registro")]}


def test_find_games_drops_unresolvable(tmp_path):
    res = detect.find_games(
        read_value=lambda s, v: str(tmp_path / "ghost"),
        steam_roots=[], drives=[])
    assert res == {}


def test_detect_hl_without_cstrike_is_not_cs16(tmp_path):
    hl = tmp_path / "Half-Life"
    touch(hl / "hl.exe")
    res = detect.find_games(read_value=lambda s, v: str(hl) if s == "Valve\\Half-Life" else None,
                            steam_roots=[], drives=[])
    assert res == {}


def test_detect_hl_with_cstrike_is_cs16(tmp_path):
    hl = tmp_path / "Half-Life"
    touch(hl / "hl.exe")
    (hl / "cstrike").mkdir()
    res = detect.find_games(read_value=lambda s, v: str(hl) if s == "Valve\\Half-Life" else None,
                            steam_roots=[], drives=[])
    assert res == {"cs16": [Finding("cs16", str(hl), "registro")]}
