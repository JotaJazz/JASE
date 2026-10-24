import pytest

from jase.config import Config, load_config, save_config
from jase.friends import FriendTracker, friends_in, normalize, parse
from jase.models import Player, ServerAddress, ServerInfo


def srv(names, port=27960, name="S"):
    addr = ServerAddress("1.2.3.4", port)
    players = [Player(n, 0, 0, n) for n in names]
    return ServerInfo(addr, addr, name, "m", "dm", len(players), 16, False, "1", 10, players, {})


def test_normalize_strips_colors_case_and_space():
    assert normalize("  ^1Jo^7TA ") == "jota"


def test_friends_in_ignores_colors_and_case():
    info = srv(["^1JoTa", "other"])
    assert friends_in(info, ["jota", "nobody"]) == ["jota"]


def test_tracker_no_repeat_leave_and_return_and_move():
    t = FriendTracker(["jota"])
    a, b = srv(["jota"], 1), srv(["jota"], 2)
    assert len(t.update("q3", a)) == 1
    assert t.update("q3", a) == []
    assert t.update("q3", srv([], 1)) == []
    assert len(t.update("q3", a)) == 1  # salió y volvió
    assert t.update("q3", b)[0].server is b  # otro servidor


def test_parse_limits_and_dedupes():
    assert parse(["A", "a", " ", 3, "^1A", "B"]) == ["A", "B"]
    assert len(parse([f"n{i}" for i in range(300)])) == 100
    assert parse("x") == []


def test_config_roundtrip_and_tolerant(tmp_path):
    p = tmp_path / "c.json"
    save_config(Config(friends=["a", "b"]), p)
    assert load_config(p).friends == ["a", "b"]
    p.write_text('{"friends": 5}')
    assert load_config(p).friends == []


@pytest.fixture
def win(qtbot, tmp_path):
    from jase.ui.main_window import MainWindow

    async def scan(*a, **k):
        return None
    w = MainWindow(Config(friends=["jota"]), tmp_path / "c.json", scan_fn=scan)
    qtbot.addWidget(w)
    notes = []
    w.notify = lambda title, text, on_click=None: notes.append((title, text, on_click))
    return w, notes


def test_single_notice_for_one_friend(win, qtbot):
    w, notes = win
    w._check_friends("cod4", srv(["jota"], name="Srv"))
    qtbot.waitUntil(lambda: len(notes) == 1, timeout=3000)
    assert "jota" in notes[0][1] and "Srv" in notes[0][1]


def test_batched_into_one_notice(win, qtbot):
    w, notes = win
    w._check_friends("cod4", srv(["jota"], 1))
    w._check_friends("cod4", srv(["jota"], 2))
    qtbot.waitUntil(lambda: len(notes) == 1, timeout=3000)
    qtbot.wait(100)
    assert len(notes) == 1 and "2" in notes[0][1]


def test_add_friend_persists_and_bolds(win):
    w, _ = win
    w.add_friend("Pepe")
    assert w.cfg.friends == ["jota", "Pepe"]
    w._show_details(srv(["pepe", "x"]))
    assert w.players_table.item(0, 0).font().bold() or w.players_table.item(1, 0).font().bold()
    bold = {w.players_table.item(r, 0).text(): w.players_table.item(r, 0).font().bold()
            for r in range(2)}
    assert bold == {"pepe": True, "x": False}


def test_dialog_add_remove(qtbot):
    from jase.ui.friends_dialog import FriendsDialog
    d = FriendsDialog(["a"])
    qtbot.addWidget(d)
    d.edit.setText("B")
    d.add_button.click()
    d.edit.setText("b")
    d.add_button.click()  # duplicado
    assert d.names() == ["a", "B"]
    d.list.setCurrentRow(0)
    d.list.item(0).setSelected(True)
    d.remove_button.click()
    assert d.names() == ["B"]
