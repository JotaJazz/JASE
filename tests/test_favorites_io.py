"""Exportar e importar favoritos: formato, fusión y menús."""
import json

import pytest
from PySide6.QtWidgets import QFileDialog, QMessageBox

from jase import favorites_io
from jase.config import Config
from jase.favorites_io import FORMAT, MAX_IMPORT_BYTES, export_favorites, import_favorites
from jase.ui import favorites
from jase.ui.main_window import MainWindow
from test_main_window import FakeLaunch, FakeMaps, FakeScan


def sample_cfg():
    cfg = Config(favorites={"cod2": ["1.2.3.4:28960", "5.6.7.8:28961"], "ut99": ["9.9.9.9:7777"]})
    favorites.set_note(cfg, "cod2", "1.2.3.4:28960", "mi nota")
    return cfg


def test_export_shape():
    data = export_favorites(sample_cfg())
    assert data["format"] == FORMAT == "jase-favorites"
    assert data["version"] == 1
    assert data["favorites"]["cod2"] == [{"address": "1.2.3.4:28960", "note": "mi nota"},
                                         {"address": "5.6.7.8:28961"}]


def test_roundtrip():
    src = sample_cfg()
    dst = Config()
    result = import_favorites(dst, json.loads(json.dumps(export_favorites(src))))
    assert (result.added, result.skipped) == (3, 0)
    assert dst.favorites == src.favorites
    assert dst.favorite_notes == src.favorite_notes


def test_import_merges_without_overwriting():
    cfg = Config(favorites={"cod2": ["1.2.3.4:28960"]})
    favorites.set_note(cfg, "cod2", "1.2.3.4:28960", "mía")
    cfg.favorites["cod2"].append("5.6.7.8:28961")
    data = {"format": FORMAT, "version": 1, "favorites": {"cod2": [
        {"address": "1.2.3.4:28960", "note": "ajena"},
        {"address": "5.6.7.8:28961", "note": "nueva"},
        {"address": "7.7.7.7:1"}]}}
    result = import_favorites(cfg, data)
    assert (result.added, result.skipped) == (1, 0)
    assert cfg.favorites["cod2"] == ["1.2.3.4:28960", "5.6.7.8:28961", "7.7.7.7:1"]
    assert favorites.get_note(cfg, "cod2", "1.2.3.4:28960") == "mía"
    assert favorites.get_note(cfg, "cod2", "5.6.7.8:28961") == "nueva"


def test_import_skips_invalid():
    cfg = Config()
    data = {"format": FORMAT, "version": 1, "favorites": {
        "nogame": [{"address": "1.1.1.1:1"}],
        "cod2": [{"address": "bad"}, "1.2.3.4:5", 7, {"address": "2.2.2.2:2"}],
        "ut99": "oops"}}
    result = import_favorites(cfg, data)
    assert result.added == 1
    assert result.skipped == 5
    assert cfg.favorites == {"cod2": ["2.2.2.2:2"]}


@pytest.mark.parametrize("data", [
    [], {"format": "x", "version": 1, "favorites": {}},
    {"format": FORMAT, "version": 2, "favorites": {}},
    {"format": FORMAT, "version": 1, "favorites": []},
])
def test_import_wrong_format(data):
    with pytest.raises(ValueError):
        import_favorites(Config(), data)


def test_read_import_file_too_big(tmp_path, monkeypatch):
    path = tmp_path / "big.json"
    path.write_bytes(b" " * (MAX_IMPORT_BYTES + 1))
    with pytest.raises(ValueError):
        favorites_io.read_import_file(path)
    ok = tmp_path / "ok.json"
    ok.write_text(json.dumps(export_favorites(sample_cfg())))
    assert favorites_io.read_import_file(ok)["format"] == FORMAT


def make_window(qtbot, tmp_path, cfg):
    w = MainWindow(cfg, tmp_path / "config.json", scan_fn=FakeScan({}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps())
    qtbot.addWidget(w)
    return w


def test_menu_has_actions(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, Config())
    assert {"export_favorites", "import_favorites"} <= set(w.ui_actions)


def test_export_import_via_dialogs(qtbot, tmp_path, monkeypatch):
    path = tmp_path / "fav.json"
    w = make_window(qtbot, tmp_path, sample_cfg())
    monkeypatch.setattr(QFileDialog, "getSaveFileName", lambda *a, **k: (str(path), ""))
    w.export_favorites_to_file()
    assert json.loads(path.read_text())["favorites"]["ut99"] == [{"address": "9.9.9.9:7777"}]

    w2 = make_window(qtbot, tmp_path, Config())
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *a, **k: (str(path), ""))
    w2.import_favorites_from_file()
    assert w2.cfg.favorites == sample_cfg().favorites
    assert "3 favoritos importados, 0 descartados" in w2.statusBar().currentMessage()


def test_import_bad_file_warns(qtbot, tmp_path, monkeypatch):
    bad = tmp_path / "bad.json"
    bad.write_text("{nope")
    w = make_window(qtbot, tmp_path, Config())
    warned = []
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *a, **k: (str(bad), ""))
    monkeypatch.setattr(QMessageBox, "warning", lambda *a, **k: warned.append(a))
    w.import_favorites_from_file()
    assert len(warned) == 1
    assert warned[0][2] == "El archivo no es una exportación válida de favoritos de JASE"
    assert w.cfg.favorites == {}


def test_deeply_nested_json_is_value_error(tmp_path):
    path = tmp_path / "deep.json"
    path.write_bytes(b"[" * 200_000)
    with pytest.raises(ValueError):
        favorites_io.read_import_file(path)


def test_import_too_big_warns_translated(qtbot, tmp_path, monkeypatch):
    big = tmp_path / "big.json"
    big.write_bytes(b" " * (MAX_IMPORT_BYTES + 1))
    w = make_window(qtbot, tmp_path, Config())
    warned = []
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *a, **k: (str(big), ""))
    monkeypatch.setattr(QMessageBox, "warning", lambda *a, **k: warned.append(a))
    w.import_favorites_from_file()
    assert warned[0][2].startswith("El archivo es demasiado grande")
