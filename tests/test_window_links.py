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


def _exec(monkeypatch, answer, seen=None):
    def fake(self):
        if seen is not None:
            seen.append((self.text(), self.textFormat()))
        return answer
    monkeypatch.setattr(QMessageBox, "exec", fake)


def test_invalid_link_warns_and_does_nothing(qtbot, tmp_path, monkeypatch):
    launched, warned = [], []
    w = make(qtbot, tmp_path, FakeQuery(), launched)
    monkeypatch.setattr(QMessageBox, "warning", lambda *a: warned.append(a))
    w.open_link("jase://connect/cod2/;rm -rf:1")
    assert warned and launched == []


def test_link_connects_only_after_yes(qtbot, tmp_path, monkeypatch):
    launched = []
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}), launched)
    _exec(monkeypatch, QMessageBox.StandardButton.Yes)
    w.open_link(URL)
    qtbot.waitUntil(lambda: bool(launched), timeout=3000)
    assert launched == [("cod2", ADDR)]


def test_link_no_does_not_connect(qtbot, tmp_path, monkeypatch):
    launched, asked = [], []

    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}), launched)
    _exec(monkeypatch, QMessageBox.StandardButton.No, asked)
    w.open_link(URL)
    qtbot.waitUntil(lambda: bool(asked), timeout=3000)
    assert launched == []


def test_link_no_answer_shows_message(qtbot, tmp_path, monkeypatch):
    launched = []
    w = make(qtbot, tmp_path, FakeQuery(), launched)
    _exec(monkeypatch, QMessageBox.StandardButton.Yes)
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


def test_confirmation_is_plain_text_with_address(qtbot, tmp_path, monkeypatch):
    from PySide6.QtCore import Qt
    evil = ServerInfo(ADDR, ADDR, "^1<b>Oficial</b>", "m", "dm", 1, 8, False, "1.3", 20)
    seen = []
    w = make(qtbot, tmp_path, FakeQuery({ADDR: evil}), [])
    _exec(monkeypatch, QMessageBox.StandardButton.No, seen)
    w.open_link(URL)
    qtbot.waitUntil(lambda: bool(seen), timeout=3000)
    text, fmt = seen[0]
    assert fmt == Qt.TextFormat.PlainText
    assert "<b>Oficial</b>" in text and "^1" not in text and "9.9.9.9:28960" in text


def test_socket_invalid_link_is_status_message(qtbot, tmp_path, monkeypatch):
    warned = []
    w = make(qtbot, tmp_path, FakeQuery(), [])
    monkeypatch.setattr(QMessageBox, "warning", lambda *a: warned.append(a))
    w._on_link_received("jase://nope")
    assert not warned and w.statusBar().currentMessage()


def test_second_link_waits_for_first_dialog_then_latest_runs(qtbot, tmp_path, monkeypatch):
    other = ServerAddress("8.8.8.8", 1)
    third = ServerAddress("7.7.7.7", 1)
    infos = {a: ServerInfo(a, a, f"s{a.host}", "m", "dm", 1, 8, False, "1.3", 20)
             for a in (ADDR, other, third)}
    launched = []
    w = make(qtbot, tmp_path, FakeQuery(infos), launched)
    order, nested = [], []

    def fake(box):
        order.append(box.text())
        if len(order) == 1:  # con el diálogo abierto llegan dos enlaces más
            w.open_link("jase://connect/cod2/8.8.8.8:1")
            w.open_link("jase://connect/cod2/7.7.7.7:1")
            qtbot.wait(200)
            nested.append(len(order))
        return QMessageBox.StandardButton.Yes
    monkeypatch.setattr(QMessageBox, "exec", fake)
    w.open_link(URL)
    qtbot.waitUntil(lambda: len(launched) == 2, timeout=3000)
    assert nested == [1]  # ningún segundo diálogo mientras el primero seguía abierto
    assert [a for _, a in launched] == [ADDR, third]  # el intermedio se descartó


def test_link_waits_while_a_modal_dialog_is_open(qtbot, tmp_path, monkeypatch):
    from jase.ui import window_links
    other = ServerAddress("8.8.8.8", 1)
    infos = {a: ServerInfo(a, a, f"s{a.host}", "m", "dm", 1, 8, False, "1.3", 20)
             for a in (ADDR, other)}
    launched = []
    w = make(qtbot, tmp_path, FakeQuery(infos), launched)
    monkeypatch.setattr(window_links, "LINK_RETRY_MS", 20)
    selected = []
    orig_select = w.select_game
    w.select_game = lambda g: (selected.append(g), orig_select(g))
    _exec(monkeypatch, QMessageBox.StandardButton.Yes)
    modal = [object()]  # p. ej. Ajustes abierto
    monkeypatch.setattr(QApplication, "activeModalWidget", staticmethod(lambda: modal[0]))
    w.open_link(URL)
    w.open_link("jase://connect/cod2/8.8.8.8:1")  # solo el último espera
    qtbot.wait(150)
    assert selected == [] and launched == []
    modal[0] = None  # se cerró el diálogo
    qtbot.waitUntil(lambda: bool(launched), timeout=3000)
    qtbot.wait(100)
    assert launched == [("cod2", other)] and selected == ["cod2"]


def test_watch_label_is_plain_text(qtbot, tmp_path):
    from PySide6.QtCore import Qt
    w = make(qtbot, tmp_path, FakeQuery(), [])
    assert w.watch_label.textFormat() == Qt.TextFormat.PlainText
