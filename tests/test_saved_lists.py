"""Lista del último escaneo guardada en disco: al abrir JASE aparece sin escanear."""
import json

from jase import server_cache
from jase.config import Config
from jase.ui.main_window import MainWindow
from test_main_window import FakeLaunch, FakeMaps, FakeScan, mk, sidebar_texts


def window(qtbot, tmp_path, scan, cfg=None):
    w = MainWindow(cfg or Config(), tmp_path / "config.json", scan_fn=scan,
                   launch_fn=FakeLaunch(), map_service=FakeMaps())
    qtbot.addWidget(w)
    return w


def saved_dir(tmp_path):
    return tmp_path / "cache" / "servers"


def scanned(qtbot, tmp_path, servers):
    """Primera sesión: escanea cod2 y espera a que la lista quede en disco."""
    w = window(qtbot, tmp_path, FakeScan({"cod2": servers}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning())
    qtbot.waitUntil(lambda: (saved_dir(tmp_path) / "cod2.json").is_file())
    w.close()


def test_next_session_shows_saved_list_without_scanning(qtbot, tmp_path):
    scanned(qtbot, tmp_path, [mk("1.1.1.1"), mk("2.2.2.2")])
    scan = FakeScan({"cod2": [mk("3.3.3.3")]})
    w = window(qtbot, tmp_path, scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    assert scan.calls == []  # no se escaneó al abrir
    assert "lista guardada del" in w.statusBar().currentMessage()
    w.refresh()  # F5: la nueva lista reemplaza a la guardada
    qtbot.waitUntil(lambda: not w.is_scanning())
    assert [s.address.host for s in w.model.servers()] == ["3.3.3.3"]


def test_corrupt_saved_list_falls_back_to_scan(qtbot, tmp_path):
    scanned(qtbot, tmp_path, [mk("1.1.1.1")])
    (saved_dir(tmp_path) / "cod2.json").write_bytes(b"{roto")
    scan = FakeScan({"cod2": [mk("3.3.3.3")]})
    w = window(qtbot, tmp_path, scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: scan.calls == ["cod2"] and not w.is_scanning())
    assert [s.address.host for s in w.model.servers()] == ["3.3.3.3"]


def test_failed_scan_is_not_saved(qtbot, tmp_path):
    w = window(qtbot, tmp_path, FakeScan({}, no_master={"cod2"}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning())
    qtbot.wait(200)
    assert not (saved_dir(tmp_path) / "cod2.json").exists()


def test_sidebar_counts_saved_lists_of_other_games(qtbot, tmp_path):
    scanned(qtbot, tmp_path, [mk("1.1.1.1"), mk("2.2.2.2")])
    w = window(qtbot, tmp_path, FakeScan({"ut99": []}))
    w.select_game("ut99")
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"


def test_player_search_ignores_saved_lists(qtbot, tmp_path):
    scanned(qtbot, tmp_path, [mk("1.1.1.1")])
    w = window(qtbot, tmp_path, FakeScan({"ut99": []}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w.is_scanning())
    assert "cod2" not in w.searchable_lists()


def test_steam_empty_toggle_forgets_saved_steam_lists(qtbot, tmp_path):
    server_cache.save(saved_dir(tmp_path), "tf2", [mk("1.1.1.1")], now=1.0)
    w = window(qtbot, tmp_path, FakeScan({}))
    w._forget_saved_lists(lambda g: g == "tf2")
    qtbot.waitUntil(lambda: not (saved_dir(tmp_path) / "tf2.json").exists())
    assert "tf2" not in json.loads((saved_dir(tmp_path) / "index.json").read_text())


def slow_load(monkeypatch, delay=0.4):
    import time as _time
    real = server_cache.load

    def slow(*a):
        _time.sleep(delay)
        return real(*a)
    monkeypatch.setattr(server_cache, "load", slow)


def test_reveal_server_with_saved_list_scans_and_selects(qtbot, tmp_path):
    a, b = mk("1.1.1.1"), mk("2.2.2.2")
    scanned(qtbot, tmp_path, [a])
    w = window(qtbot, tmp_path, FakeScan({"cod2": [a, b], "ut99": []}))
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w.is_scanning())
    w.reveal_server("cod2", b.address)  # p. ej. aviso de amigo en un servidor nuevo
    qtbot.waitUntil(lambda: w.selected_server() is not None
                    and w.selected_server().address == b.address)


def test_failed_f5_is_not_replaced_by_late_saved_list(qtbot, tmp_path, monkeypatch):
    scanned(qtbot, tmp_path, [mk("1.1.1.1")])
    slow_load(monkeypatch)
    w = window(qtbot, tmp_path, FakeScan({}, no_master={"cod2"}))
    w.select_game("cod2")  # la carga del disco queda en camino
    w.refresh()  # F5 antes de que llegue; el master falla
    qtbot.waitUntil(lambda: not w.is_scanning())
    qtbot.wait(700)
    assert w.model.rowCount() == 0
    assert "lista guardada del" not in w.statusBar().currentMessage()


def test_forgotten_list_does_not_come_back_from_inflight_load(qtbot, tmp_path, monkeypatch):
    scanned(qtbot, tmp_path, [mk("1.1.1.1")])
    slow_load(monkeypatch)
    w = window(qtbot, tmp_path, FakeScan({"ut99": []}))
    w.select_game("cod2")
    w._forget_saved_lists(lambda g: g == "cod2")
    qtbot.wait(700)
    assert "cod2" not in w._cache
