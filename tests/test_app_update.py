import json

from jase import app_update, i18n
from jase.app_update import (CHECK_INTERVAL_S, RETRY_AFTER_FAILURE_S, Release, check,
                             newest_release, parse_version, should_check)
from jase.config import Config
from jase.ui import update_loader
from jase.ui.main_window import MainWindow
from test_main_window import FakeLaunch, FakeMaps, FakeScan

PAGE = "https://github.com/JotaJazz/JASE/releases/tag/"


def rel(tag, draft=False, prerelease=False, url=None):
    return {"tag_name": tag, "draft": draft, "prerelease": prerelease,
            "html_url": url if url is not None else PAGE + tag}


def payload(*releases):
    return json.dumps(list(releases)).encode()


def test_parse_version():
    assert parse_version("v0.10.0") == (0, 10, 0)
    assert parse_version("0.10.0") == (0, 10, 0)
    assert parse_version("v1.0-beta") is None
    assert parse_version("") is None
    assert parse_version("v") is None
    assert parse_version("1..2") is None
    assert parse_version("dev") is None


def test_newest_release_skips_drafts_and_counts_prereleases():
    data = payload(rel("v0.12.0", draft=True), rel("v0.11.0", prerelease=True), rel("v0.10.0"))
    assert newest_release(data) == Release("0.11.0", PAGE + "v0.11.0")


def test_newest_release_compares_numerically():
    data = payload(rel("v0.9.0"), rel("v0.10.0"))
    assert newest_release(data).version == "0.10.0"


def test_newest_release_discards_foreign_url_and_bad_data():
    assert newest_release(payload(rel("v9.0.0", url="https://evil.example/x"))) is None
    assert newest_release(payload(rel("v9.0.0", url="javascript:alert(1)"))) is None
    assert newest_release(b"no es json") is None
    assert newest_release(b'{"a": 1}') is None
    assert newest_release(payload(rel("rc-1"), 5, "x")) is None


def test_check_only_returns_newer(tmp_path):
    data = payload(rel("v0.11.0"))
    assert check(tmp_path, 1000.0, "0.11.0", lambda url: data) is None
    assert check(tmp_path, 1000.0, "0.12.0", lambda url: data) is None
    assert check(tmp_path, 1000.0, "0.10.0", lambda url: data).version == "0.11.0"
    assert check(tmp_path, 1000.0, "dev", lambda url: data) is None


def test_check_failure_records_meta_and_never_raises(tmp_path):
    def boom(url):
        raise OSError("sin red")

    assert check(tmp_path, 1000.0, "0.10.0", boom) is None
    meta = json.loads((tmp_path / app_update.META_FILENAME).read_text())
    assert meta["last_attempt_ok"] is False
    assert check(tmp_path, 1000.0, "0.10.0", lambda url: b"x" * (app_update.MAX_BYTES + 1)) is None


def test_should_check_schedule(tmp_path):
    meta = tmp_path / app_update.META_FILENAME
    assert should_check(meta, 1000.0)
    check(tmp_path, 1000.0, "0.10.0", lambda url: payload(rel("v0.10.0")))
    assert not should_check(meta, 1000.0 + 7200)
    assert should_check(meta, 1000.0 + CHECK_INTERVAL_S)

    def boom(url):
        raise OSError

    check(tmp_path, 1000.0, "0.10.0", boom)
    assert RETRY_AFTER_FAILURE_S == 3600
    assert should_check(meta, 1000.0 + 7200)
    assert not should_check(meta, 1000.0 + 60)


def make_window(qtbot, tmp_path, fetch, cfg=None):
    w = MainWindow(cfg or Config(), tmp_path / "config.json", scan_fn=FakeScan({}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps(), update_fetch=fetch)
    qtbot.addWidget(w)
    return w


def test_window_shows_update_link(qtbot, tmp_path):
    data = payload(rel("v99.0.0"))
    w = make_window(qtbot, tmp_path, lambda url: data)
    w.show()
    qtbot.waitUntil(w.update_label.isVisible)
    assert "99.0.0" in w.update_label.text()
    assert PAGE + "v99.0.0" in w.update_label.text()
    assert not w.update_label.openExternalLinks()
    w.close()


def test_window_label_in_english(qtbot, tmp_path):
    i18n.set_language("en")
    try:
        w = make_window(qtbot, tmp_path, lambda url: payload(rel("v99.0.0")))
        w.show()
        qtbot.waitUntil(w.update_label.isVisible)
        assert "New version 99.0.0 available" in w.update_label.text()
        w.close()
    finally:
        i18n.set_language("es")


def test_window_hidden_by_default_and_no_network_without_fetch(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, None)
    assert w.update_label.isHidden()
    assert not w.check_app_update()
    w.close()


def test_window_respects_config_switch(qtbot, tmp_path):
    calls = []
    cfg = Config()
    cfg.check_app_updates = False
    w = make_window(qtbot, tmp_path, lambda url: calls.append(url) or b"[]", cfg)
    assert not w.check_app_update()
    assert calls == []
    assert w.update_label.isHidden()
    w.close()


def test_window_ignores_foreign_release_url(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, None)
    stale = update_loader.UpdateRequest(tmp_path, 0.0, "0.10.0", None)
    w._update_request = stale
    update_loader.shared_relay().loaded.emit(stale, Release("99.0.0", "https://evil.example/"))
    assert w.update_label.isHidden()
    w.close()


def test_stale_result_discarded(qtbot, tmp_path):
    w = make_window(qtbot, tmp_path, None)
    stale = update_loader.UpdateRequest(tmp_path, 0.0, "0.10.0", None)
    update_loader.shared_relay().loaded.emit(stale, Release("99.0.0", PAGE + "v99.0.0"))
    assert w.update_label.isHidden()
    w.close()


def test_periodic_timer_checks_without_duplicating(qtbot, tmp_path, monkeypatch):
    w = make_window(qtbot, tmp_path, lambda url: b"[]")
    submitted = []
    monkeypatch.setattr(update_loader, "submit", submitted.append)
    monkeypatch.setattr(app_update, "should_check", lambda path, now: True)
    w._cancel_update_check()
    assert not w._update_timer.isActive()
    w._update_timer.start(3_600_000)
    assert w._update_timer.interval() == 3_600_000
    w._update_timer.timeout.emit()
    assert len(submitted) == 1
    w._update_timer.timeout.emit()
    assert len(submitted) == 1
    w.close()
    assert not w._update_timer.isActive()
