from PySide6.QtWidgets import QApplication, QDialog, QDialogButtonBox

from jase.config import Config
from jase.games import GAMES
from jase.models import ServerAddress, ServerInfo
from jase.ui import add_server
from jase.ui.add_server import (MAX_ADDRESSES, AddServerDialog, find_addresses,
                                query_candidates, split_host_port)
from jase.ui.main_window import MainWindow

ADDR = ServerAddress("9.9.9.9", 28960)


class FakeQuery:
    """`answers`: dirección → info; `family`: si se da, solo responde a esa familia."""

    def __init__(self, answers=None, family=None):
        self.answers = answers or {}
        self.family = family
        self.asked = []

    async def __call__(self, family, addresses, on_server, **kw):
        self.asked.append((family, list(addresses)))
        if self.family is not None and family != self.family:
            return
        for a in addresses:
            if a in self.answers:
                on_server(self.answers[a])


def info(address=ADDR):
    return ServerInfo(address, address, "Mi servidor", "mp_x", "dm", 1, 8, False, "1.3", 20)


def make(qtbot, tmp_path, query):
    async def scan(*a, **k):
        return None
    QApplication.clipboard().clear()  # otros tests copian enlaces
    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=scan, query_fn=query)
    qtbot.addWidget(w)
    return w


def fake_dialog(monkeypatch, game_id, text, port="", favorite=None, seen=None):
    """Sustituye exec(): rellena el diálogo como lo haría el usuario y acepta."""
    def exec_(self):
        if seen is not None:
            seen.append(self.favorite())
        index = self.game_combo.findData(game_id)
        self.game_combo.setCurrentIndex(index)
        self.host_edit.setText(text)
        if port:
            self.port_edit.setText(port)
        if favorite is not None:
            self.favorite_check.setChecked(favorite)
        return QDialog.DialogCode.Accepted
    monkeypatch.setattr(add_server.AddServerDialog, "exec", exec_)


def test_split_host_port():
    assert split_host_port(" 1.2.3.4:28960 ") == ("1.2.3.4", 28960)
    assert split_host_port("host.example.com") == ("host.example.com", None)
    assert split_host_port("1.2.3.4:99999") == ("1.2.3.4:99999", None)
    assert split_host_port("1.2.3.4:") == ("1.2.3.4:", None)


def test_query_candidates_adds_gamespy_query_port():
    assert query_candidates(GAMES["cod2"], 28960, "h") == [ServerAddress("h", 28960)]
    assert query_candidates(GAMES["ut99"], 7777, "h") == [ServerAddress("h", 7777),
                                                          ServerAddress("h", 7778)]
    assert query_candidates(GAMES["bf1942"], 60000, "h") == [ServerAddress("h", 60000)]


def test_dialog_paste_moves_port(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", False)
    qtbot.addWidget(d)
    ok = d.buttons.button(QDialogButtonBox.StandardButton.Ok)
    assert not ok.isEnabled()
    d.host_edit.setText("9.9.9.9:28960")  # un pegado: entra todo de golpe
    assert d.host_edit.text() == "9.9.9.9"
    assert d.port_edit.text() == "28960"
    assert ok.isEnabled() and d.result_address() == ADDR


def test_dialog_typing_splits_on_leave(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", False)
    qtbot.addWidget(d)
    qtbot.keyClicks(d.host_edit, "9.9.9.9:28960")
    assert d.host_edit.text() == "9.9.9.9:28960"  # tecla a tecla no se corta
    assert d.result_address() == ADDR  # pero ya vale
    d.host_edit.editingFinished.emit()
    assert (d.host_edit.text(), d.port_edit.text()) == ("9.9.9.9", "28960")


def test_dialog_port_typed_separately(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", True)
    qtbot.addWidget(d)
    assert d.favorite()
    qtbot.keyClicks(d.host_edit, "9.9.9.9")
    qtbot.keyClicks(d.port_edit, "28960")
    assert d.result_address() == ADDR
    d.host_edit.setText("bad host;")
    assert d.result_address() is None


def test_dialog_paste_jase_link_selects_game(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", False)
    qtbot.addWidget(d)
    d.host_edit.setText("jase://connect/ut99/1.2.3.4:7777")
    assert d.game_id() == "ut99"
    assert d.result_address() == ServerAddress("1.2.3.4", 7777)


def test_add_server_shows_and_selects(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}))
    fake_dialog(monkeypatch, "cod2", "9.9.9.9:28960")
    w.add_server_manually()
    assert w.current_game == "cod2"
    qtbot.waitUntil(lambda: w.selected_server() is not None, timeout=3000)
    assert w.selected_server().address == ADDR
    assert not w.cfg.favorites  # fuera de Favoritos no es favorito


def test_add_server_from_favorites_is_favorite(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}))
    w.select_game("cod2")
    w.show_favorites()
    seen = []
    fake_dialog(monkeypatch, "cod2", "9.9.9.9", "28960", seen=seen)
    w.add_server_manually()
    assert seen == [True]
    qtbot.waitUntil(lambda: w.selected_server() is not None, timeout=3000)
    assert w.in_favorites_view()
    assert w.cfg.favorites == {"cod2": [str(ADDR)]}


def test_add_favorite_from_other_view_switches_to_favorites(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}))
    w.select_game("cod2")
    fake_dialog(monkeypatch, "mohaa", "9.9.9.9:28960", favorite=True)
    w.add_server_manually()
    assert w.current_game == "mohaa" and w.in_favorites_view()
    qtbot.waitUntil(lambda: bool(w.cfg.favorites), timeout=3000)
    assert w.cfg.favorites == {"mohaa": ["9.9.9.9:28960"]}


def test_gamespy_game_port_resolves_to_query_port(qtbot, tmp_path, monkeypatch):
    query_addr = ServerAddress("9.9.9.9", 7778)
    w = make(qtbot, tmp_path, FakeQuery({query_addr: info(query_addr)}))
    fake_dialog(monkeypatch, "ut99", "9.9.9.9:7777", favorite=True)
    w.add_server_manually()
    qtbot.waitUntil(lambda: bool(w.cfg.favorites), timeout=3000)
    assert w.cfg.favorites == {"ut99": [str(query_addr)]}


def test_no_answer_only_warns_outside_favorites(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery())
    fake_dialog(monkeypatch, "cod2", "9.9.9.9:28960")
    w.add_server_manually()
    qtbot.waitUntil(lambda: "no respondi" in w.statusBar().currentMessage(), timeout=3000)
    assert w.model.row_of(ADDR) < 0 and not w.cfg.favorites


def test_no_answer_favorite_is_kept_offline(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery())
    fake_dialog(monkeypatch, "cod2", "9.9.9.9:28960", favorite=True)
    messages = []
    w.statusBar().messageChanged.connect(messages.append)  # Favoritos también escribe
    w.add_server_manually()
    qtbot.waitUntil(lambda: any("no respondi" in m for m in messages), timeout=3000)
    assert not w.selected_server().online
    assert w.cfg.favorites == {"cod2": [str(ADDR)]}


def test_favorites_panel_context_menu_opens_as_favorite(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery())
    calls = []
    monkeypatch.setattr(w, "add_server_manually", lambda favorite=None: calls.append(favorite))

    class FakeMenu:
        def __init__(self, parent):
            self.actions = []

        def addAction(self, text):
            from PySide6.QtGui import QAction
            action = QAction(text)
            self.actions.append(action)
            return action

        def exec(self, _pos):
            self.actions[0].trigger()

        def deleteLater(self):
            pass

    from jase.ui import main_window
    monkeypatch.setattr(main_window, "QMenu", FakeMenu)
    rect = w.favorites_list.visualItemRect(w.favorites_list.item(0))
    w._on_direct_context_menu(rect.center())
    assert calls == [True]


def test_find_addresses_in_free_text():
    text = ("connect 1.2.3.4:28960\n"
            "otro: 5.6.7.8:28961, repetido 1.2.3.4:28960\n"
            "jase://connect/cod2/srv.example.com:28962\n"
            "sin punto localhost:1, puerto malo 9.9.9.9:99999, versión 1.2.3:4.5")
    assert find_addresses(text) == [ServerAddress("1.2.3.4", 28960),
                                    ServerAddress("5.6.7.8", 28961),
                                    ServerAddress("srv.example.com", 28962)]
    many = "\n".join(f"10.0.{i // 256}.{i % 256}:1" for i in range(MAX_ADDRESSES + 5))
    assert len(find_addresses(many)) == MAX_ADDRESSES


def test_dialog_initial_text_single(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", False, "connect 9.9.9.9:28960")
    qtbot.addWidget(d)
    assert not d.is_multi()
    assert (d.host_edit.text(), d.port_edit.text()) == ("9.9.9.9", "28960")


def test_dialog_initial_text_several_goes_to_list(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", True, "9.9.9.9:28960\n8.8.8.8:28961")
    qtbot.addWidget(d)
    assert d.is_multi()
    assert d.addresses() == [ADDR, ServerAddress("8.8.8.8", 28961)]
    ok = d.buttons.button(QDialogButtonBox.StandardButton.Ok)
    assert ok.text() == "Agregar 2 servidores" and ok.isEnabled()
    assert not d.detect_button.isEnabled()  # «Detectar» es para una sola dirección


def test_dialog_paste_list_into_ip_switches_mode(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", False)
    qtbot.addWidget(d)
    d.host_edit.setText("9.9.9.9:28960\n8.8.8.8:28961")
    assert d.is_multi() and len(d.addresses()) == 2
    assert d.host_edit.text() == ""


def test_dialog_multi_toggle_keeps_typed_address(qtbot):
    d = AddServerDialog(list(GAMES), "cod2", False, "9.9.9.9:28960")
    qtbot.addWidget(d)
    d.multi_button.click()
    assert d.is_multi() and d.addresses() == [ADDR]
    d.multi_button.click()
    assert not d.is_multi() and d.addresses() == [ADDR]


def _detect_with(result):
    calls = []

    def detect(address, done):
        calls.append(address)
        done(result)
    return detect, calls


def test_dialog_detect_picks_game_and_query_port(qtbot):
    detect, calls = _detect_with([("ut99", ServerAddress("9.9.9.9", 7778))])
    d = AddServerDialog(list(GAMES), "cod2", False, "9.9.9.9:7777", detect=detect)
    qtbot.addWidget(d)
    d.detect_button.click()
    assert calls == [ServerAddress("9.9.9.9", 7777)]
    assert d.game_id() == "ut99" and d.port_edit.text() == "7778"
    assert d.detect_label.text() == "Detectado: Unreal Tournament"
    assert d.detect_button.isEnabled()


def test_dialog_detect_ambiguous_and_none(qtbot):
    detect, _ = _detect_with([("cs16", ADDR), ("css", ADDR)])
    d = AddServerDialog(list(GAMES), "cod2", False, "9.9.9.9:28960", detect=detect)
    qtbot.addWidget(d)
    d.detect_button.click()
    assert d.game_id() == "cs16" and d.detect_label.text().startswith("Puede ser: ")
    d._detect_fn, _ = _detect_with([])
    d.detect_button.click()
    assert d.game_id() == "cs16" and "no respondió" in d.detect_label.text()


def test_add_several_favorites(qtbot, tmp_path, monkeypatch):
    other = ServerAddress("8.8.8.8", 28961)
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}))
    fake_dialog(monkeypatch, "cod2", f"{ADDR}\n{other}", favorite=True)
    messages = []
    w.statusBar().messageChanged.connect(messages.append)  # Favoritos también escribe
    w.add_server_manually()
    qtbot.waitUntil(lambda: "2 favoritos agregados (1 sin respuesta)" in messages,
                    timeout=3000)
    assert w.in_favorites_view()
    assert len(w.cfg.favorites["cod2"]) == 2
    assert w.selected_server().address == ADDR  # la que respondió


def test_add_several_outside_favorites_only_answering(qtbot, tmp_path, monkeypatch):
    other = ServerAddress("8.8.8.8", 28961)
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}))
    fake_dialog(monkeypatch, "cod2", f"{ADDR}\n{other}")
    w.add_server_manually()
    qtbot.waitUntil(lambda: "respondi" in w.statusBar().currentMessage(), timeout=3000)
    assert w.model.row_of(ADDR) >= 0 and w.model.row_of(other) < 0
    assert w.statusBar().currentMessage() == "1 servidor respondió de 2"


def test_dialog_prefilled_from_clipboard(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery())
    seen = []

    def exec_(self):
        seen.append((self.game_id(), self.addresses()))
        return QDialog.DialogCode.Rejected
    monkeypatch.setattr(add_server.AddServerDialog, "exec", exec_)
    QApplication.clipboard().setText("jase://connect/ut99/9.9.9.9:7777")
    w.add_server_manually()
    QApplication.clipboard().setText("nada útil aquí")
    w.add_server_manually()
    assert seen[0] == ("ut99", [ServerAddress("9.9.9.9", 7777)])
    assert seen[1][1] == []


def test_ctrl_v_in_table_opens_dialog(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery())
    calls = []
    monkeypatch.setattr(w, "add_server_manually", lambda **kw: calls.append(kw))
    w.paste_servers()
    assert calls == [] and "portapapeles" in w.statusBar().currentMessage()
    QApplication.clipboard().setText("9.9.9.9:28960")
    w.paste_servers()
    assert calls == [{"text": "9.9.9.9:28960"}]


def test_window_detect_game(qtbot, tmp_path):
    cod2 = ServerInfo(ADDR, ADDR, "x", "mp_x", "dm", 0, 8, False, "1.3", 20,
                      rules={"gamename": "Call of Duty 2", "protocol": "118"})
    query = FakeQuery({ADDR: cod2}, family="quake3")
    w = make(qtbot, tmp_path, query)
    results = []
    w._detect_game(ADDR, results.append)
    qtbot.waitUntil(lambda: bool(results), timeout=3000)
    assert results == [[("cod2", ADDR)]]
    assert {family for family, _a in query.asked} == {"quake3", "quake2", "gamespy",
                                                       "valve", "ut2004"}


def test_window_detect_cancelled_is_discarded(qtbot, tmp_path):
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}, family="quake3"))
    results = []
    w._detect_game(ADDR, results.append)
    worker = w._detect_worker
    w._cancel_detect()  # el diálogo se cerró
    qtbot.waitUntil(lambda: worker not in w._workers, timeout=3000)
    assert results == []


def test_existing_favorite_is_reported(qtbot, tmp_path, monkeypatch):
    w = make(qtbot, tmp_path, FakeQuery({ADDR: info()}))
    w.cfg.favorites["cod2"] = [str(ADDR)]
    fake_dialog(monkeypatch, "cod2", str(ADDR), favorite=True)
    # La consulta de la vista Favoritos corre a la vez y puede pisar el aviso.
    messages = []
    w.statusBar().messageChanged.connect(messages.append)
    w.add_server_manually()
    qtbot.waitUntil(lambda: any("ya estaba" in m for m in messages), timeout=3000)
    assert w.cfg.favorites == {"cod2": [str(ADDR)]}
