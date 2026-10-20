import threading

import pytest
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QInputDialog, QMessageBox

from jase import i18n, masters
from jase.config import Config
from jase.games import GAMES
from jase.masters import CustomMasters, OfficialList, master_key
from jase.ui import masters_tab
from jase.ui.dialogs import SettingsDialog
from jase.ui.masters_tab import (COL_ACTIVE, COL_MASTER, COL_ORIGIN, COL_PROBE, MastersContext,
                                 MastersTab, parse_master_text)
from jase.ui.main_window import MainWindow
from test_main_window import FakeLaunch, FakeMaps, FakeScan

OFFICIAL = OfficialList(1, "2026-11-01", {
    "cod2": (("a.example.com", 20710), ("b.example.com", 20710)),
    "ut99": (("u.example.com", 28900),),
})


def make_tab(qtbot, custom=None, probe=None, check_update=None):
    ctx = MastersContext(OFFICIAL, custom or CustomMasters(), probe=probe,
                         check_update=check_update)
    tab = MastersTab(ctx)
    qtbot.addWidget(tab)
    select(tab, "cod2")
    return tab, ctx


def select(tab, game_id):
    tab.game_combo.setCurrentIndex(tab.game_combo.findData(game_id))


def cell(tab, row, col):
    return tab.table.item(row, col)


def test_settings_has_three_tabs(qtbot):
    d = SettingsDialog(Config())
    qtbot.addWidget(d)
    assert [d.tabs.tabText(i) for i in range(3)] == ["Juegos", "Masters", "General"]
    assert d.language_combo.parent() is not d.find_games_button.parent()


def test_settings_has_three_tabs_english(qtbot):
    i18n.set_language("en")
    try:
        d = SettingsDialog(Config())
        qtbot.addWidget(d)
        assert [d.tabs.tabText(i) for i in range(3)] == ["Games", "Masters", "General"]
        assert d.masters_tab.table.horizontalHeaderItem(COL_ORIGIN).text() == "Source"
    finally:
        i18n.set_language("es")


def test_masters_tab_lists_official_and_custom(qtbot):
    custom = CustomMasters(added={"cod2": [("mine.example.com", 1234)]})
    tab, _ = make_tab(qtbot, custom)
    assert tab.table.rowCount() == 3
    assert [cell(tab, r, COL_MASTER).text() for r in range(3)] == [
        "a.example.com:20710", "b.example.com:20710", "mine.example.com:1234"]
    assert [cell(tab, r, COL_ORIGIN).text() for r in range(3)] == [
        "Oficial", "Oficial", "Personal"]
    assert cell(tab, 0, COL_ACTIVE).checkState() == Qt.Checked
    assert tab.official_label.text() == "Lista oficial: 2026-11-01"
    select(tab, "ut99")
    assert tab.table.rowCount() == 1


def test_disable_official_and_save(qtbot):
    tab, ctx = make_tab(qtbot)
    cell(tab, 1, COL_ACTIVE).setCheckState(Qt.Unchecked)
    result = tab.custom_masters()
    assert result.disabled == {"cod2": {master_key(("b.example.com", 20710))}}
    assert ctx.custom.disabled == {}  # edita una copia
    assert masters.effective_masters("cod2", OFFICIAL, result) == [("a.example.com", 20710)]
    cell(tab, 1, COL_ACTIVE).setCheckState(Qt.Checked)
    assert tab.custom_masters().disabled == {}


def test_settings_dialog_exposes_custom_masters(qtbot):
    ctx = MastersContext(OFFICIAL, CustomMasters(), probe=None)
    d = SettingsDialog(Config(), masters=ctx)
    qtbot.addWidget(d)
    select(d.masters_tab, "cod2")
    cell(d.masters_tab, 0, COL_ACTIVE).setCheckState(Qt.Unchecked)
    assert d.custom_masters().disabled["cod2"] == {master_key(("a.example.com", 20710))}


def test_add_custom_validates_and_rejects_duplicate(qtbot):
    tab, _ = make_tab(qtbot)
    for bad in ("", "nohost", "host:0", "host:70000", "ho st:80", "host:abc", ":80"):
        assert tab.add_master(bad) == "invalid", bad
        assert parse_master_text(bad) is None
    assert tab.add_master("A.Example.com:20710") == "duplicate"  # oficial, sin mayúsculas
    assert tab.add_master(" new.example.com:27950 ") == "ok"
    assert tab.add_master("new.example.com:27950") == "duplicate"
    assert tab.custom_masters().added == {"cod2": [("new.example.com", 27950)]}
    assert cell(tab, 2, COL_ORIGIN).text() == "Personal"


def test_add_button_prompts_and_warns(qtbot, monkeypatch):
    tab, _ = make_tab(qtbot)
    warnings = []
    monkeypatch.setattr(QMessageBox, "warning", lambda *a: warnings.append(a[2]))
    answers = iter([("bad", True), ("a.example.com:20710", True), ("x.example.com:99", False),
                    ("ok.example.com:99", True)])
    monkeypatch.setattr(QInputDialog, "getText", lambda *a, **k: next(answers))
    for _ in range(4):
        tab.add_button.click()
    assert len(warnings) == 2
    assert tab.custom_masters().added == {"cod2": [("ok.example.com", 99)]}


def test_remove_only_custom(qtbot):
    custom = CustomMasters(added={"cod2": [("mine.example.com", 1234)]})
    tab, _ = make_tab(qtbot, custom)
    tab.table.selectRow(0)
    assert not tab.remove_button.isEnabled()
    tab.remove_selected()
    assert tab.table.rowCount() == 3
    tab.table.selectRow(2)
    assert tab.remove_button.isEnabled()
    tab.remove_button.click()
    assert tab.table.rowCount() == 2
    assert tab.custom_masters().added == {}


def test_restore_officials(qtbot):
    custom = CustomMasters(disabled={"cod2": {master_key(("a.example.com", 20710))},
                                     "ut99": {master_key(("u.example.com", 28900))}})
    tab, _ = make_tab(qtbot, custom)
    assert cell(tab, 0, COL_ACTIVE).checkState() == Qt.Unchecked
    tab.restore_button.click()
    assert cell(tab, 0, COL_ACTIVE).checkState() == Qt.Checked
    assert "cod2" not in tab.custom_masters().disabled
    assert "ut99" in tab.custom_masters().disabled  # solo el juego elegido


def test_probe_shows_count(qtbot):
    calls = []

    async def probe(game, host, port):
        calls.append((game.id, host, port))
        return 3

    tab, _ = make_tab(qtbot, probe=probe)
    tab.table.selectRow(1)
    with qtbot.waitSignal(tab.probe_finished) as sig:
        tab.probe_button.click()
    assert sig.args == [True]
    assert calls == [("cod2", "b.example.com", 20710)]
    assert cell(tab, 1, COL_PROBE).text() == "✓ 3 servidores"


def test_probe_singular_and_failure(qtbot):
    results = iter([1, RuntimeError("x"), 0])

    async def probe(game, host, port):
        r = next(results)
        if isinstance(r, Exception):
            raise r
        return r

    tab, _ = make_tab(qtbot, probe=probe)
    expected = ["✓ 1 servidor", "✗ no responde", "✗ no responde"]
    for text in expected:
        tab.table.selectRow(0)
        with qtbot.waitSignal(tab.probe_finished):
            tab.probe_button.click()
        assert cell(tab, 0, COL_PROBE).text() == text


@pytest.mark.parametrize("change", ["row", "game"])
def test_probe_result_discarded_after_selection_change(qtbot, change):
    gate = threading.Event()

    async def probe(game, host, port):
        while not gate.is_set():
            import asyncio
            await asyncio.sleep(0.01)
        return 5

    tab, _ = make_tab(qtbot, probe=probe)
    tab.table.selectRow(0)
    tab.probe_button.click()
    assert cell(tab, 0, COL_PROBE).text() == "Probando…"
    if change == "row":
        tab.table.selectRow(1)
    else:
        select(tab, "ut99")
    with qtbot.waitSignal(tab.probe_finished) as sig:
        gate.set()
    assert sig.args == [False]
    select(tab, "cod2")
    assert all(cell(tab, r, COL_PROBE).text() == "" for r in range(tab.table.rowCount()))
    assert cell(tab, 0, COL_PROBE).text() == ""


def test_default_probe_counts_addresses(monkeypatch):
    import asyncio
    seen = {}

    async def fake_fetch(game, host, port, timeout):
        seen.update(game=game.id, host=host, port=port, timeout=timeout)
        return [object(), object()]

    monkeypatch.setattr(masters_tab.scanner, "fetch_master", fake_fetch)
    assert asyncio.run(masters_tab.default_probe(GAMES["cod2"], "h", 1)) == 2
    assert seen == {"game": "cod2", "host": "h", "port": 1, "timeout": 5}


def test_check_update_button_calls_callback_and_shows_outcomes(qtbot):
    calls = []
    tab, ctx = make_tab(qtbot, check_update=lambda: calls.append(1))
    tab.check_button.click()
    assert calls == [1]
    assert tab.update_status.text() == "Buscando actualización…"
    ctx.outcome_listener("unchanged", None)
    assert tab.update_status.text() == "La lista de masters está al día"
    ctx.outcome_listener("failed", None)
    assert tab.update_status.text() == "No se pudo descargar la lista de masters"
    new = OfficialList(1, "2026-12-01", {"cod2": (("n.example.com", 1),)})
    ctx.outcome_listener("updated", new)
    assert "2026-12-01" in tab.update_status.text()
    assert tab.official_label.text() == "Lista oficial: 2026-12-01"
    assert cell(tab, 0, COL_MASTER).text() == "n.example.com:1"


def test_check_button_disabled_without_callback(qtbot):
    tab, _ = make_tab(qtbot)
    assert not tab.check_button.isEnabled()


# ------------------------------------------------------------- ventana principal
def make_window(qtbot, tmp_path, fetch=None):
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=FakeScan({}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps(), masters_fetch=fetch)
    qtbot.addWidget(w)
    return w


def test_refresh_without_active_masters_warns(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path)
    game = GAMES["cod2"]
    w._custom.disabled["cod2"] = {master_key(m)
                                  for m in masters.official_for("cod2", w._official)}
    w.select_game("cod2")
    assert not w.is_scanning()
    assert w.statusBar().currentMessage() == (
        f"{game.name}: no hay masters activos; revísalos en Ajustes → Masters")
    w.refresh()
    assert not w.is_scanning()
    w.close()


def test_open_settings_saves_custom_masters(qtbot, tmp_path, monkeypatch):
    w = make_window(qtbot, tmp_path)

    def fake_exec(self):
        select(self.masters_tab, "cod2")
        self.masters_tab.add_master("mine.example.com:1234")
        return 1

    monkeypatch.setattr(SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert w._custom.added == {"cod2": [("mine.example.com", 1234)]}
    assert masters.load_custom(w.custom_masters_path).added == w._custom.added
    w.close()


def test_open_settings_cancel_keeps_custom(qtbot, tmp_path, monkeypatch):
    w = make_window(qtbot, tmp_path)

    def fake_exec(self):
        self.masters_tab.add_master("mine.example.com:1234")
        return 0

    monkeypatch.setattr(SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert w._custom.added == {}
    assert not w.custom_masters_path.exists()
    w.close()


def _doc(updated, host="new.example.com"):
    import json
    return json.dumps({"schema": 1, "updated": updated, "games": {
        "cod2": [{"host": host, "port": 20510}]}}).encode()


@pytest.mark.parametrize("fetch_result,expected", [
    (_doc("2026-12-01", "other.example.com"), "2026-12-01"),
    (None, "La lista de masters está al día"),
    (b"{roto", "No se pudo descargar la lista de masters"),
])
def test_forced_check_reports_outcome_in_tab(qtbot, tmp_path, monkeypatch, fetch_result,
                                             expected):
    if fetch_result is None:  # la segunda consulta devuelve lo mismo que la primera
        first = _doc("2026-11-01")
        responses = iter([first, first])
    else:
        responses = iter([_doc("2026-11-01"), fetch_result])
    w = make_window(qtbot, tmp_path, lambda url: next(responses))
    qtbot.waitUntil(lambda: w._masters_request is None)  # revisión inicial
    seen = {}

    def fake_exec(self):
        self.masters_tab.check_button.click()
        qtbot.waitUntil(lambda: expected in self.masters_tab.update_status.text())
        seen["ok"] = True
        return 0

    monkeypatch.setattr(SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert seen == {"ok": True}
    w.close()


def test_forced_check_without_network_reports_failure(qtbot, tmp_path, monkeypatch):
    w = make_window(qtbot, tmp_path, None)

    def fake_exec(self):
        self.masters_tab.check_button.click()
        assert self.masters_tab.update_status.text() == (
            "No se pudo descargar la lista de masters")
        return 0

    monkeypatch.setattr(SettingsDialog, "exec", fake_exec)
    w.open_settings()
    w.close()
