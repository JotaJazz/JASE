import pytest
from PySide6.QtWidgets import QApplication, QSystemTrayIcon

from jase.config import Config
from jase.ui.main_window import MainWindow


@pytest.fixture
def tray_available(monkeypatch):
    monkeypatch.setattr(QSystemTrayIcon, "isSystemTrayAvailable", staticmethod(lambda: True))
    monkeypatch.setattr(QSystemTrayIcon, "supportsMessages", staticmethod(lambda: True))
    monkeypatch.setattr(QApplication, "alert", lambda *a, **k: None)


def make(qtbot, tmp_path, tray):
    async def scan(*a, **k):
        return None
    w = MainWindow(Config(minimize_to_tray=tray), tmp_path / "c.json", scan_fn=scan)
    qtbot.addWidget(w)
    return w


def test_no_tray_without_option(qtbot, tmp_path, tray_available):
    assert make(qtbot, tmp_path, False)._tray is None


def test_minimize_hides_window_with_tray(qtbot, tmp_path, tray_available):
    w = make(qtbot, tmp_path, True)
    assert w._tray is not None
    w.show()
    w.showMinimized()
    qtbot.waitUntil(lambda: w.isHidden(), timeout=2000)
    w.restore_from_tray()
    assert w.isVisible()


def test_minimize_without_option_keeps_window(qtbot, tmp_path, tray_available):
    w = make(qtbot, tmp_path, False)
    w.show()
    w.showMinimized()
    qtbot.wait(100)
    assert not w.isHidden()


def test_notify_without_tray_uses_status_bar(qtbot, tmp_path, tray_available):
    w = make(qtbot, tmp_path, False)
    w.notify("Hola", "mundo", lambda: None)
    assert w.statusBar().currentMessage() == "Hola: mundo"


def test_message_click_calls_callback(qtbot, tmp_path, tray_available, monkeypatch):
    w = make(qtbot, tmp_path, True)
    shown = []
    monkeypatch.setattr(w._tray, "showMessage", lambda *a: shown.append(a))
    w.hide()
    calls = []
    w.notify("T", "x", lambda: calls.append(1))
    assert shown == [("T", "x")]
    w._tray.messageClicked.emit()
    assert calls == [1]
    w._tray.messageClicked.emit()
    assert calls == [1]  # un solo uso


def test_apply_tray_toggles_at_runtime(qtbot, tmp_path, tray_available):
    w = make(qtbot, tmp_path, False)
    w.cfg.minimize_to_tray = True
    w._apply_tray()
    assert w._tray is not None
    w.cfg.minimize_to_tray = False
    w._apply_tray()
    assert w._tray is None


def test_slot_free_in_tray_notifies_then_asks(qtbot, tmp_path, tray_available, monkeypatch):
    from jase.games import GAMES
    from jase.models import ServerAddress, ServerInfo
    from jase.ui import window_slot_watch
    w = make(qtbot, tmp_path, True)
    a = ServerAddress("1.1.1.1", 1)
    info = ServerInfo(a, a, "srv", "m", "dm", 19, 20, False, "1", 1, [], {})
    shown = []
    monkeypatch.setattr(w._tray, "showMessage", lambda *a: shown.append(a))
    asked = []
    monkeypatch.setattr(window_slot_watch.QMessageBox, "question",
                        lambda *a: asked.append(a) or window_slot_watch.QMessageBox.No)

    class FakeWatcher:
        game = GAMES["cod2"]
    fw = FakeWatcher()
    w._slot_watch = fw
    monkeypatch.setattr(w, "sender", lambda: fw)
    monkeypatch.setattr(w, "_end_slot_watch", lambda: None)
    w.hide()
    w._on_slot_free(info)
    assert len(shown) == 1 and not asked
    w._tray.messageClicked.emit()
    assert w.isVisible() and len(asked) == 1
    w._slot_watch = None


def test_notify_hidden_without_balloons_restores_and_calls(qtbot, tmp_path, tray_available,
                                                            monkeypatch):
    w = make(qtbot, tmp_path, True)
    w.hide()
    monkeypatch.setattr(QSystemTrayIcon, "supportsMessages", staticmethod(lambda: False))
    calls = []
    w.notify("T", "x", lambda: calls.append(1))
    assert w.isVisible() and calls == [1]


def test_option_off_restores_hidden_window(qtbot, tmp_path, tray_available):
    w = make(qtbot, tmp_path, True)
    w.show()
    w.hide()
    w.cfg.minimize_to_tray = False
    w._apply_tray()
    assert w.isVisible() and w._tray is None and w._tray_menu is None


def test_close_shuts_down_tray(qtbot, tmp_path, tray_available):
    w = make(qtbot, tmp_path, True)
    tray = w._tray
    w.close()
    assert w._tray is None and not tray.isVisible()


def test_quit_from_tray_closes_hidden_window(qtbot, tmp_path, tray_available):
    w = make(qtbot, tmp_path, True)
    w.show()
    w.hide()
    closed = []
    orig = w.closeEvent
    w.closeEvent = lambda e: (closed.append(1), orig(e))
    quit_action = w._tray.contextMenu().actions()[1]
    quit_action.trigger()
    assert closed
