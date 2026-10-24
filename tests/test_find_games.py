import os
import threading

from PySide6.QtWidgets import QPushButton

from jase.config import Config
from jase.detect import Finding
from jase.ui import dialogs, find_games
from jase.ui.find_games import FindGamesDialog


def touch(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"")
    return path


def fake(result):
    def find(**kw):
        return result
    return find


def open_dialog(qtbot, cfg, result):
    d = FindGamesDialog(cfg, find_fn=fake(result))
    qtbot.addWidget(d)
    qtbot.waitUntil(lambda: bool(d.rows) or d.status_label.text().startswith("No se"))
    return d


def test_dialog_rows_and_statuses(qtbot, tmp_path):
    touch(tmp_path / "ok" / "CoD2MP_s.exe")
    cfg = Config()
    cfg.game_paths = {"cod2": str(tmp_path / "ok"), "cod1": str(tmp_path / "gone"),
                      "cod4": ""}
    result = {
        "cod2": [Finding("cod2", str(tmp_path / "ok"), "registro")],
        "cod1": [Finding("cod1", "X:\\CoD", "steam")],
        "cod4": [Finding("cod4", "X:\\CoD4", "registro")],
    }
    d = open_dialog(qtbot, cfg, result)
    assert {g: r[2].text() for g, r in d.rows.items()} == {
        "cod2": "ya configurado", "cod1": "ruta guardada no encontrada", "cod4": "nuevo"}
    assert {g: r[0].isChecked() for g, r in d.rows.items()} == {
        "cod2": False, "cod1": True, "cod4": True}
    assert d.selected_paths() == {"cod1": "X:\\CoD", "cod4": "X:\\CoD4"}


def test_dialog_keeps_valid_saved_path_unchecked(qtbot, tmp_path):
    touch(tmp_path / "ok" / "CoD2MP_s.exe")
    cfg = Config()
    cfg.game_paths = {"cod2": str(tmp_path / "ok")}
    d = open_dialog(qtbot, cfg, {"cod2": [Finding("cod2", "Z:\\other", "steam")]})
    d.accept()
    assert "cod2" not in d.selected_paths()
    assert cfg.game_paths == {"cod2": str(tmp_path / "ok")}


def test_dialog_multiple_paths_combo_ordered_by_source(qtbot):
    result = {"cod2": [Finding("cod2", "A", "registro"), Finding("cod2", "B", "steam")]}
    d = open_dialog(qtbot, Config(), result)
    combo = d.rows["cod2"][1]
    assert [combo.itemText(i) for i in range(combo.count())] == [
        "A (registro)", "B (steam)"]
    combo.setCurrentIndex(1)
    assert d.selected_paths()["cod2"] == "B"


def test_dialog_discards_stale_search(qtbot, monkeypatch):
    gate = threading.Event()
    calls = []

    def find(**kw):
        calls.append(kw["extra_root"])
        if kw["extra_root"] is None:
            gate.wait(5)
            return {"cod2": [Finding("cod2", "OLD", "registro")]}
        return {"cod1": [Finding("cod1", "NEW", "carpeta elegida")]}

    monkeypatch.setattr(find_games.QFileDialog, "getExistingDirectory",
                        staticmethod(lambda *a, **k: "D:\\Juegos"))
    d = FindGamesDialog(Config(), find_fn=find)
    qtbot.addWidget(d)
    # Cada búsqueda corre en su propio hilo: sin esperar, el orden de `calls` no está garantizado.
    qtbot.waitUntil(lambda: calls == [None])
    d.folder_button.click()
    qtbot.waitUntil(lambda: "cod1" in d.rows)
    gate.set()
    qtbot.wait(100)
    assert list(d.rows) == ["cod1"] and calls == [None, "D:\\Juegos"]


def test_dialog_cancel_search_discards_result(qtbot):
    gate = threading.Event()

    def find(**kw):
        gate.wait(5)
        return {"cod2": [Finding("cod2", "P", "registro")]}

    d = FindGamesDialog(Config(), find_fn=find)
    qtbot.addWidget(d)
    assert d.status_label.text() == "Buscando…"
    d.cancel_search_button.click()
    gate.set()
    qtbot.wait(100)
    assert not d.rows and d.status_label.text() == "Búsqueda cancelada"


def test_settings_find_games_fills_edits_without_saving(qtbot):
    cfg = Config()
    d = dialogs.SettingsDialog(cfg, on_find_games=lambda: {"cod1": "X:\\CoD"})
    qtbot.addWidget(d)
    d.findChild(QPushButton, "settings_find_games").click()
    assert d.path_edits["cod1"].text() == "X:\\CoD"
    assert cfg.game_paths.get("cod1") != "X:\\CoD"
    d.apply_to(cfg)
    assert cfg.game_paths["cod1"] == "X:\\CoD"


def test_folder_choice_is_normalized(qtbot, monkeypatch):
    calls = []

    def find(**kw):
        calls.append(kw["extra_root"])
        return {}

    monkeypatch.setattr(find_games.QFileDialog, "getExistingDirectory",
                        staticmethod(lambda *a, **k: "/a//b/../c/"))
    d = FindGamesDialog(Config(), find_fn=find)
    qtbot.addWidget(d)
    qtbot.waitUntil(lambda: calls == [None])  # los hilos de búsqueda no tienen orden entre sí
    d.folder_button.click()
    qtbot.waitUntil(lambda: len(calls) == 2)
    assert calls[1] == os.path.normpath("/a//b/../c/")


def test_dialog_found_count_singular_and_plural(qtbot, tmp_path):
    one = {"cod2": [Finding("cod2", str(tmp_path), "registro")]}
    assert open_dialog(qtbot, Config(), one).status_label.text() == "1 juego encontrado"
    two = {**one, "cod1": [Finding("cod1", str(tmp_path), "steam")]}
    assert open_dialog(qtbot, Config(), two).status_label.text() == "2 juegos encontrados"
