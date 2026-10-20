import json
import time

import pytest

from jase import i18n, masters
from jase.config import Config
from jase.masters import (CHECK_INTERVAL_S, MAX_BYTES, fetch_official, should_check,
                          update_official)
from jase.ui import masters_loader
from jase.ui.main_window import MainWindow
from test_main_window import FakeLaunch, FakeMaps, FakeScan


def doc(games, updated="2026-11-01"):
    return json.dumps({"schema": 1, "updated": updated, "games": games}).encode()


NEW = doc({"cod2": [{"host": "new.example.com", "port": 20510}]})


def test_should_check_interval(tmp_path):
    meta = tmp_path / "masters.meta.json"
    assert should_check(meta, 1000.0)  # sin meta
    meta.write_text("basura")
    assert should_check(meta, 1000.0)
    meta.write_text(json.dumps({"last_check": 1000.0}))
    assert not should_check(meta, 1000.0 + CHECK_INTERVAL_S - 1)
    assert should_check(meta, 1000.0 + CHECK_INTERVAL_S)
    assert should_check(meta, 500.0)  # reloj retrasado


def test_update_writes_cache_and_meta(tmp_path):
    result = update_official(tmp_path, 123.0, lambda url: NEW)
    assert result is not None and result.games["cod2"] == (("new.example.com", 20510),)
    assert (tmp_path / "masters.json").read_bytes() == NEW
    assert json.loads((tmp_path / "masters.meta.json").read_text()) == {"last_check": 123.0}
    assert masters.load_official(tmp_path) == result
    assert not [p for p in tmp_path.iterdir() if p.name.endswith(".tmp")]


def test_update_keeps_previous_on_invalid(tmp_path):
    update_official(tmp_path, 1.0, lambda url: NEW)
    update_official(tmp_path, 2.0, lambda url: b"{no json")
    assert (tmp_path / "masters.json").read_bytes() == NEW
    assert json.loads((tmp_path / "masters.meta.json").read_text())["last_check"] == 2.0


def test_update_network_error_writes_meta_only(tmp_path):
    def boom(url):
        raise OSError("sin red")

    assert update_official(tmp_path, 5.0, boom) is None
    assert not (tmp_path / "masters.json").exists()
    assert json.loads((tmp_path / "masters.meta.json").read_text()) == {"last_check": 5.0}


def test_update_returns_none_when_unchanged(tmp_path):
    assert update_official(tmp_path, 1.0, lambda url: NEW) is not None
    same_content = doc({"cod2": [{"host": "new.example.com", "port": 20510}]},
                       updated="2026-12-01")
    assert update_official(tmp_path, 2.0, lambda url: same_content) is None


def test_fetch_rejects_oversize():
    with pytest.raises(ValueError):
        fetch_official(lambda url: b"x" * (MAX_BYTES + 1))
    assert fetch_official(lambda url: b"x" * MAX_BYTES) == b"x" * MAX_BYTES


def make_window(qtbot, tmp_path, fetch):
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=FakeScan({}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps(), masters_fetch=fetch)
    qtbot.addWidget(w)
    return w


def test_window_applies_update_and_shows_status(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, lambda url: NEW)
    qtbot.waitUntil(lambda: w._official.games.get("cod2") == (("new.example.com", 20510),))
    assert "2026-11-01" in w.statusBar().currentMessage()
    assert not w.check_masters_update(force=False)  # revisado hace instantes
    w.close()


def test_stale_update_result_discarded(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, None)
    before = w._official
    stale = masters_loader.MastersUpdateRequest(tmp_path, 0.0, None)
    masters_loader.shared_relay().loaded.emit(stale, masters.parse_official(NEW), None)
    assert w._official is before
    w.close()


def test_masters_update_status_in_english(qtbot, tmp_path):
    i18n.set_language("en")
    try:
        w = make_window(qtbot, tmp_path, lambda url: NEW)
        qtbot.waitUntil(lambda: "Masters list updated (2026-11-01)"
                        in w.statusBar().currentMessage())
        w.close()
    finally:
        i18n.set_language("es")
