from PySide6.QtWidgets import QApplication, QMessageBox

from jase.config import Config
from jase.models import ServerAddress, ServerInfo
from jase.ui.main_window import MainWindow

class FakeQuery:
    def __init__(self, answers=None):
        self.answers = answers or {}

    async def __call__(self, family, addresses, on_server, **kw):
        for a in addresses:
            if a in self.answers:
                on_server(self.answers[a])


ADDR = ServerAddress("9.9.9.9", 28960)
URL = "jase://connect/cod2/9.9.9.9:28960"


def info():
    return ServerInfo(ADDR, ADDR, "Mi servidor", "mp_x", "dm", 1, 8, False, "1.3", 20)


def make(qtbot, tmp_path, query, launch):
    async def scan(*a, **k):
        return None
    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=scan, query_fn=query)
    qtbot.addWidget(w)
    w.connect_to = lambda game, i: launch.append((game.id, i.address)) or True
    return w


def test_invalid_link_warns_and_does_nothing(qtbot, tmp_path, monkeypatch):
    launched, warned = [], []
    w = make(qtbot, tmp_path, FakeQuery(), launched)
    monkeypatch.setattr(QMessageBox, "warning", lambda *a: warned.append(a))
    w.open_link("jase://connect/cod2/;rm -rf:1")
    assert warned and launched == []


def test_link_connects_only_after_yes(qtbot, tmp_path, monkeypatch):
    launched = []
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}), launched)
    monkeypatch.setattr(QMessageBox, "question", lambda *a, **k: QMessageBox.StandardButton.Yes)
    w.open_link(URL)
    qtbot.waitUntil(lambda: bool(launched), timeout=3000)
    assert launched == [("cod2", ADDR)]


def test_link_no_does_not_connect(qtbot, tmp_path, monkeypatch):
    launched, asked = [], []

    def no(*a, **k):
        asked.append(a)
        return QMessageBox.StandardButton.No
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}), launched)
    monkeypatch.setattr(QMessageBox, "question", no)
    w.open_link(URL)
    qtbot.waitUntil(lambda: bool(asked), timeout=3000)
    assert launched == []


def test_link_no_answer_shows_message(qtbot, tmp_path, monkeypatch):
    launched = []
    w = make(qtbot, tmp_path, FakeQuery(), launched)
    monkeypatch.setattr(QMessageBox, "question", lambda *a, **k: QMessageBox.StandardButton.Yes)
    w.open_link(URL)
    qtbot.waitUntil(lambda: "no respondi" in w.statusBar().currentMessage(), timeout=3000)
    assert launched == []


def test_copy_link_action(qtbot, tmp_path):
    w = make(qtbot, tmp_path, FakeQuery(), [])
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning())
    assert not w.ui_actions["copy_link"].isEnabled()
    w.model.add_or_update(info())
    assert w.select_address(ADDR)
    assert w.selected_server() is not None
    assert w.ui_actions["copy_link"].isEnabled()
    w.ui_actions["copy_link"].trigger()
    assert QApplication.clipboard().text() == URL
