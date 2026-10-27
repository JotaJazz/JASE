import pytest

from jase import party
from jase.config import Config, load_config, save_config
from jase.models import ServerAddress, ServerInfo

ADDR = ServerAddress("1.2.3.4", 28960)


def srv(players, bots=0, online=True):
    info = ServerInfo(ADDR, ADDR, "Clan Nox", "m", "dm", players, 16, False, "1.3", 30,
                      online=online)
    info.bots = bots
    return info


def test_tracker_first_answer_is_baseline_then_notifies_once():
    t = party.PartyTracker()
    assert not t.update("cod2", srv(1), 4)  # punto de partida
    assert not t.update("cod2", srv(3), 4)
    assert t.update("cod2", srv(4), 4)
    assert not t.update("cod2", srv(5), 4)
    assert not t.update("cod2", srv(3), 4)  # 4→3→4 no vuelve a avisar
    assert not t.update("cod2", srv(4), 4)
    assert not t.update("cod2", srv(2), 4)  # baja a la mitad: se rearma
    assert t.update("cod2", srv(4), 4)


def test_tracker_counts_people_not_bots_and_ignores_offline():
    t = party.PartyTracker()
    t.update("cod2", srv(0), 4)
    assert not t.update("cod2", srv(8, bots=6), 4)
    assert not t.update("cod2", srv(0, online=False), 4)
    assert t.update("cod2", srv(9, bots=4), 4)


def test_already_busy_server_does_not_notify_on_start():
    t = party.PartyTracker()
    assert not t.update("cod2", srv(10), 4)
    assert not t.update("cod2", srv(10), 4)


def test_parse_is_tolerant():
    raw = {"cod2": {"1.2.3.4:28960": 4, "bad": 3, "5.6.7.8:1": 0, "9.9.9.9:2": True,
                    "8.8.8.8:3": 999},
           "q3": [], 5: {"1.1.1.1:1": 2}, "et": {"2.2.2.2:27960": 6}}
    assert party.parse(raw) == {"cod2": {"1.2.3.4:28960": 4}, "et": {"2.2.2.2:27960": 6}}
    assert party.parse("x") == {}


def test_config_roundtrip(tmp_path):
    p = tmp_path / "c.json"
    save_config(Config(party_alerts={"cod2": {"1.2.3.4:28960": 5}}), p)
    assert load_config(p).party_alerts == {"cod2": {"1.2.3.4:28960": 5}}


@pytest.fixture
def win(qtbot, tmp_path):
    from jase.ui.main_window import MainWindow

    async def scan(*a, **k):
        return None
    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=scan)
    qtbot.addWidget(w)
    notes = []
    w.notify = lambda title, text, on_click=None: notes.append((title, text, on_click))
    return w, notes


def test_alert_notifies_and_starts_background_watch(win, tmp_path):
    w, notes = win
    assert not w._friend_watch.active
    w.set_party_alert("cod2", srv(1), 4)
    assert w._friend_watch.active
    assert load_config(tmp_path / "c.json").party_alerts == {"cod2": {str(ADDR): 4}}
    assert w._friend_watch_targets()[0][1] == [ADDR]
    w._on_server_response("cod2", srv(3))
    assert notes == []
    w._on_server_response("cod2", srv(5, bots=1))
    assert [(t, x) for t, x, _ in notes] == [
        ("Se armó partida", "Clan Nox: 4 jugadores en Call of Duty 2")]
    w.set_party_alert("cod2", srv(5), 0)
    assert w.cfg.party_alerts == {} and not w._friend_watch.active
    assert w.statusBar().currentMessage() == "Ya no se avisa de partidas en Clan Nox"


def test_edit_party_alert_uses_selected_server(win, qtbot, monkeypatch):
    from PySide6.QtWidgets import QInputDialog
    w, _ = win
    w.current_game = "cod2"
    w.model.add_or_update(srv(1))
    w.table.selectRow(0)
    asked = []

    def get_int(parent, title, label, value, lo, hi):
        asked.append(value)
        return 6, True
    monkeypatch.setattr(QInputDialog, "getInt", staticmethod(get_int))
    w.edit_party_alert()
    assert asked == [party.DEFAULT_THRESHOLD]
    assert w.party_threshold("cod2", ADDR) == 6
    assert w.ui_actions["party_alert"].text() == "Avisar de partida: 6 jugadores…"
