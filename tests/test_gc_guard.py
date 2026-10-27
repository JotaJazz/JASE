import gc

from jase.ui import gc_guard


def test_guard_disables_automatic_gc_and_collects_on_timer(qtbot):
    guard = gc_guard.MainThreadGC()
    assert not gc.isenabled()
    assert guard._timer.isActive() and guard._timer.interval() == gc_guard.INTERVAL_MS
    guard.deleteLater()


def test_collect_if_due_frees_cycles(monkeypatch):
    calls = []
    monkeypatch.setattr(gc, "collect", lambda generation=2: calls.append(generation) or 0)
    monkeypatch.setattr(gc, "get_threshold", lambda: (10, 5, 5))
    monkeypatch.setattr(gc, "get_count", lambda: (3, 0, 0))
    gc_guard.collect_if_due()
    monkeypatch.setattr(gc, "get_count", lambda: (11, 0, 0))
    gc_guard.collect_if_due()
    monkeypatch.setattr(gc, "get_count", lambda: (11, 6, 0))
    gc_guard.collect_if_due()
    monkeypatch.setattr(gc, "get_count", lambda: (11, 6, 6))
    gc_guard.collect_if_due()
    assert calls == [0, 1, 2]
