import asyncio

import pytest

from jase.games import GAMES
from jase.models import ServerAddress, ServerInfo
from jase.ui.slot_watch import SlotWatcher, has_free_slot

ADDR = ServerAddress("1.1.1.1", 28960)


def mkinfo(players, max_players=20, online=True):
    return ServerInfo(ADDR, ADDR, "srv", "m", "dm", players, max_players, False, "1.3", 10,
                      [], {}, online=online)


class SeqQuery:
    """Responde con una secuencia de ServerInfo (None = sin respuesta); repite la última."""

    def __init__(self, seq, delay=0.0):
        self.seq = list(seq)
        self.delay = delay
        self.calls = 0

    async def __call__(self, family, addresses, on_server, **kw):
        i = min(self.calls, len(self.seq) - 1)
        self.calls += 1
        if self.delay:
            await asyncio.sleep(self.delay)
        if self.seq[i] is not None:
            on_server(self.seq[i])


@pytest.fixture(autouse=True)
def fast(monkeypatch):
    monkeypatch.setattr(SlotWatcher, "INTERVAL_MS", 20)


def make(qtbot, query):
    w = SlotWatcher(GAMES["cod2"], ADDR, query)
    return w


def test_has_free_slot():
    assert has_free_slot(mkinfo(19))
    assert not has_free_slot(mkinfo(20))
    assert not has_free_slot(mkinfo(0, max_players=0))
    assert not has_free_slot(mkinfo(1, online=False))


def test_emits_slot_free_once_on_third_query(qtbot):
    query = SeqQuery([mkinfo(20), mkinfo(20), mkinfo(19)])
    w = make(qtbot, query)
    got = []
    w.slot_free.connect(got.append)
    w.start()
    assert w.active
    qtbot.waitUntil(lambda: bool(got), timeout=3000)
    assert not w.active
    qtbot.wait(150)
    assert len(got) == 1 and got[0].num_players == 19
    assert query.calls == 3
    if w.current_worker():
        w.current_worker().wait(2000)


def test_no_answer_never_emits(qtbot):
    query = SeqQuery([None])
    w = make(qtbot, query)
    got = []
    w.slot_free.connect(got.append)
    w.start()
    qtbot.waitUntil(lambda: query.calls >= 5, timeout=3000)
    w.stop()
    assert got == []
    if w.current_worker():
        w.current_worker().wait(2000)


def test_zero_max_players_never_emits(qtbot):
    query = SeqQuery([mkinfo(0, max_players=0)])
    w = make(qtbot, query)
    got = []
    w.slot_free.connect(got.append)
    w.start()
    qtbot.waitUntil(lambda: query.calls >= 3, timeout=3000)
    w.stop()
    assert got == []
    if w.current_worker():
        w.current_worker().wait(2000)


def test_expires(qtbot, monkeypatch):
    monkeypatch.setattr(SlotWatcher, "MAX_MS", 100)
    w = make(qtbot, SeqQuery([mkinfo(20)]))
    expired = []
    w.expired.connect(lambda: expired.append(True))
    w.start()
    qtbot.waitUntil(lambda: bool(expired), timeout=3000)
    assert not w.active
    if w.current_worker():
        w.current_worker().wait(2000)


def test_stop_cancels_running_worker(qtbot):
    w = make(qtbot, SeqQuery([mkinfo(20)], delay=5))
    w.start()
    qtbot.waitUntil(lambda: w.current_worker() is not None, timeout=3000)
    worker = w.current_worker()
    qtbot.waitUntil(worker.isRunning, timeout=3000)
    w.stop()
    assert not w.active and worker.cancel_requested
    assert worker.wait(3000)
