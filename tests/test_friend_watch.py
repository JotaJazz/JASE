import asyncio

import pytest

from jase.config import Config, load_config, save_config
from jase.games import GAMES
from jase.models import Player, ServerAddress, ServerInfo
from jase.ui.friend_watch import FriendWatcher


def mkinfo(addr, names=()):
    return ServerInfo(addr, addr, "srv", "m", "dm", len(names), 20, False, "1.3", 10,
                      [Player(n, 0, 0) for n in names], {})


class FakeQuery:
    def __init__(self):
        self.calls = []
        self.running = 0
        self.max_running = 0

    async def __call__(self, family, addresses, on_server, **kw):
        self.running += 1
        self.max_running = max(self.max_running, self.running)
        self.calls.append(list(addresses))
        await asyncio.sleep(0.02)
        for a in addresses:
            on_server(mkinfo(a, ["jota"]))
        self.running -= 1


A1, A2, A3 = (ServerAddress("1.1.1.1", p) for p in (1, 2, 3))


def test_walks_games_without_overlap(qtbot, monkeypatch):
    monkeypatch.setattr(FriendWatcher, "INTERVAL_MS", 20)
    q = FakeQuery()
    targets = [(GAMES["cod2"], [A1, A2]), (GAMES["cod4"], [A3]), (GAMES["cod2"], [])]
    w = FriendWatcher(lambda: targets, q)
    seen = []
    w.server_seen.connect(lambda gid, info: seen.append((gid, info.address)))
    w.start()
    qtbot.waitUntil(lambda: len(seen) >= 3, timeout=3000)
    w.stop()
    qtbot.waitUntil(lambda: w.current_worker() is None, timeout=3000)
    assert q.calls[:2] == [[A1, A2], [A3]]
    assert seen[:3] == [("cod2", A1), ("cod2", A2), ("cod4", A3)]
    assert q.max_running == 1


def test_stop_halts_timer(qtbot):
    w = FriendWatcher(lambda: [], FakeQuery())
    w.start()
    assert w.active and w._timer.isActive()
    w.stop()
    assert not w.active and not w._timer.isActive()


def test_config_roundtrip(tmp_path):
    p = tmp_path / "c.json"
    save_config(Config(watch_friends=True), p)
    assert load_config(p).watch_friends is True
    p.write_text('{"watch_friends": "si"}')
    assert load_config(p).watch_friends is False


@pytest.fixture
def win(qtbot, tmp_path):
    from jase.ui.main_window import MainWindow

    async def scan(*a, **k):
        return None
    q = FakeQuery()
    cfg = Config(friends=["jota"], watch_friends=True, favorites={"cod2": ["1.1.1.1:1"]},
                 recent={"cod2": ["1.1.1.1:1", "1.1.1.1:2"]})
    w = MainWindow(cfg, tmp_path / "c.json", scan_fn=scan, query_fn=q)
    qtbot.addWidget(w)
    return w, q


def test_window_runs_watcher_and_feeds_check(win, qtbot):
    w, q = win
    assert w._friend_watch.active
    assert [(g.id, a) for g, a in w._friend_watch_targets()] == [("cod2", [A1, A2])]
    got = []
    w._check_friends = lambda gid, info: got.append(gid)
    w._friend_watch.server_seen.disconnect()
    w._friend_watch.server_seen.connect(w._check_friends)
    w._friend_watch._tick()
    qtbot.waitUntil(lambda: len(got) == 2, timeout=3000)
    assert got == ["cod2", "cod2"]
    qtbot.waitUntil(lambda: w._friend_watch.current_worker() is None, timeout=3000)


def test_window_toggle_stops_and_close_returns_worker(win):
    w, q = win
    w.cfg.watch_friends = False
    w._set_friends(["jota"])
    assert not w._friend_watch.active
    w.cfg.watch_friends = True
    w._set_friends([])
    assert not w._friend_watch.active  # sin amigos no corre
    w._set_friends(["jota"])
    assert w._friend_watch.active
    w._friend_watch._tick()
    assert w._friend_watch_shutdown() == [w._friend_watch.current_worker()]
    assert not w._friend_watch.active


def test_dialog_watch_checkbox(qtbot):
    from jase.ui.friends_dialog import FriendsDialog
    d = FriendsDialog(["a"], True)
    qtbot.addWidget(d)
    assert d.watch()
    d.watch_check.setChecked(False)
    assert not d.watch()
