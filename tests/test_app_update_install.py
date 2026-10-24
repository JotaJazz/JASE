import hashlib
import json

import pytest
from PySide6.QtCore import QProcess
from PySide6.QtWidgets import QApplication

from jase import app_update, i18n
from jase.app_update import Asset, Release, download_installer, newest_release
from jase.ui import update_dialog, window_background
from test_app_update import PAGE, make_window, payload, rel

DL = "https://github.com/JotaJazz/JASE/releases/download/v0.13.0/JASE-0.13.0-setup.exe"
BODY = b"installer-bytes" * 5000
SHA = hashlib.sha256(BODY).hexdigest()


def asset_json(**over):
    a = {"name": "JASE-0.13.0-setup.exe", "size": len(BODY), "digest": "sha256:" + SHA,
         "browser_download_url": DL}
    a.update(over)
    return a


def release(**over):
    r = rel("v0.13.0")
    r["assets"] = [asset_json(**over)]
    return r


def test_valid_asset_parsed():
    r = newest_release(payload(release()))
    assert r.installer == Asset("JASE-0.13.0-setup.exe", DL, len(BODY), SHA)


@pytest.mark.parametrize("over", [
    {"digest": None}, {"digest": "md5:abc"}, {"digest": "sha256:zz"},
    {"browser_download_url": "https://evil.example/JASE-0.13.0-setup.exe"},
    {"size": app_update.MAX_INSTALLER_BYTES + 1}, {"size": 0}, {"size": "9"},
    {"name": "JASE-0.11.0-setup.exe"},
])
def test_invalid_asset_gives_no_installer(over):
    r = newest_release(payload(release(**over)))
    assert r == Release("0.13.0", PAGE + "v0.13.0")
    assert r.installer is None


def test_missing_or_malformed_assets():
    assert newest_release(payload(rel("v0.13.0"))).installer is None
    bad = rel("v0.13.0")
    bad["assets"] = [5, None, "x"]
    assert newest_release(payload(bad)).installer is None
    bad["assets"] = "x"
    assert newest_release(payload(bad)).installer is None


class Stream:
    def __init__(self, data):
        self.data, self.closed = data, False

    def read(self, n):
        out, self.data = self.data[:n], self.data[n:]
        return out

    def close(self):
        self.closed = True


ASSET = Asset("JASE-0.13.0-setup.exe", DL, len(BODY), SHA)


def test_download_ok(tmp_path):
    seen = []
    s = Stream(BODY)
    path = download_installer(ASSET, tmp_path / "up", lambda url: s,
                              lambda d, t: seen.append((d, t)))
    assert path.read_bytes() == BODY and path.name == ASSET.name
    assert seen[-1] == (len(BODY), len(BODY)) and s.closed
    assert [p.name for p in path.parent.iterdir()] == [ASSET.name]


def test_download_bad_hash_deletes(tmp_path):
    bad = b"x" * len(BODY)
    with pytest.raises(ValueError):
        download_installer(ASSET, tmp_path, lambda url: Stream(bad))
    assert list(tmp_path.iterdir()) == []


def test_download_wrong_size_deletes(tmp_path):
    with pytest.raises(ValueError):
        download_installer(ASSET, tmp_path, lambda url: Stream(BODY[:-1]))
    with pytest.raises(ValueError):
        download_installer(ASSET, tmp_path, lambda url: Stream(BODY + b"x"))
    assert list(tmp_path.iterdir()) == []


def test_download_cancel_and_error(tmp_path):
    with pytest.raises(app_update.DownloadCancelled):
        download_installer(ASSET, tmp_path, lambda url: Stream(BODY), cancelled=lambda: True)

    def boom(url):
        raise OSError("sin red")

    with pytest.raises(ValueError):
        download_installer(ASSET, tmp_path, boom)
    assert list(tmp_path.iterdir()) == []


def test_https_only_redirect():
    import urllib.error
    import urllib.request
    h = app_update._HttpsOnly()
    req = urllib.request.Request("https://github.com/x")
    with pytest.raises(urllib.error.URLError):
        h.redirect_request(req, None, 302, "Found", {}, "http://evil.example/x")
    with pytest.raises(ValueError):
        app_update.default_fetch_stream("http://github.com/x")


# ------------------------------------------------------------------ UI
def window_with_release(qtbot, tmp_path, rel_json):
    w = make_window(qtbot, tmp_path, lambda url: json.dumps([rel_json]).encode())
    w.show()
    qtbot.waitUntil(w.update_label.isVisible)
    return w


@pytest.fixture
def opened(monkeypatch):
    urls = []
    monkeypatch.setattr(window_background.QDesktopServices, "openUrl",
                        lambda u: urls.append(u.toString()) or True)
    return urls


def test_click_off_windows_opens_page(qtbot, tmp_path, monkeypatch, opened):
    monkeypatch.setattr(window_background.sys, "platform", "linux")
    asked = []
    monkeypatch.setattr(update_dialog, "ask_action", lambda *a: asked.append(a))
    w = window_with_release(qtbot, tmp_path, release())
    w.update_label.linkActivated.emit("x")
    assert opened == [PAGE + "v0.13.0"] and not asked
    w.close()


def test_click_windows_without_installer_opens_page(qtbot, tmp_path, monkeypatch, opened):
    monkeypatch.setattr(window_background.sys, "platform", "win32")
    w = window_with_release(qtbot, tmp_path, rel("v99.0.0"))
    w.update_label.linkActivated.emit("x")
    assert opened == [PAGE + "v99.0.0"]
    w.close()


def test_click_windows_page_choice(qtbot, tmp_path, monkeypatch, opened):
    monkeypatch.setattr(window_background.sys, "platform", "win32")
    monkeypatch.setattr(update_dialog, "ask_action", lambda p, r: update_dialog.PAGE)
    w = window_with_release(qtbot, tmp_path, release())
    w.update_label.linkActivated.emit("x")
    assert opened == [PAGE + "v0.13.0"]
    monkeypatch.setattr(update_dialog, "ask_action", lambda p, r: None)
    w.update_label.linkActivated.emit("x")
    assert len(opened) == 1
    w.close()


def test_click_windows_install_runs_installer(qtbot, tmp_path, monkeypatch, opened):
    monkeypatch.setattr(window_background.sys, "platform", "win32")
    quits = []
    monkeypatch.setattr(QApplication, "quit", staticmethod(lambda: quits.append(1)))
    monkeypatch.setattr(update_dialog, "ask_action", lambda p, r: update_dialog.INSTALL)
    started = []
    monkeypatch.setattr(QProcess, "startDetached", lambda p: started.append(p) or True)
    w = window_with_release(qtbot, tmp_path, release())
    w._update_fetch_stream = lambda url: Stream(BODY)
    w.update_label.linkActivated.emit("x")
    qtbot.waitUntil(lambda: bool(started), timeout=5000)
    expected = tmp_path / "cache" / "updates" / "JASE-0.13.0-setup.exe"
    assert started == [str(expected)] and expected.read_bytes() == BODY
    assert not w.isVisible() and quits == [1]  # sale aunque estuviera en la bandeja
    assert opened == []


def test_install_error_offers_page(qtbot, tmp_path, monkeypatch, opened):
    monkeypatch.setattr(window_background.sys, "platform", "win32")
    monkeypatch.setattr(update_dialog, "ask_action", lambda p, r: update_dialog.INSTALL)
    started = []
    monkeypatch.setattr(QProcess, "startDetached", lambda p: started.append(p) or True)
    shown = []

    def fake_exec(box):
        shown.append(box.text())
        box.done(0)
        return 0

    monkeypatch.setattr(window_background.QMessageBox, "exec", fake_exec)
    monkeypatch.setattr(window_background.QMessageBox, "clickedButton", lambda box: None)
    w = window_with_release(qtbot, tmp_path, release())
    w._update_fetch_stream = lambda url: Stream(b"x" * len(BODY))
    w.update_label.linkActivated.emit("x")
    qtbot.waitUntil(lambda: bool(shown), timeout=5000)
    assert started == [] and w.isVisible()
    assert not list((tmp_path / "cache" / "updates").iterdir())
    assert w._download_request is None
    w.close()


@pytest.mark.parametrize("data, code", [
    (b"x" * len(BODY), app_update.DOWNLOAD_HASH),
    (BODY[:-1], app_update.DOWNLOAD_SIZE),
    (BODY + b"x", app_update.DOWNLOAD_TOO_MUCH),
])
def test_download_error_codes(tmp_path, data, code):
    with pytest.raises(app_update.DownloadError) as exc:
        download_installer(ASSET, tmp_path, lambda url: Stream(data))
    assert exc.value.code == code


def test_download_error_codes_network_cancel_insecure(tmp_path):
    import urllib.error

    def boom(url):
        raise OSError("sin red")

    with pytest.raises(app_update.DownloadError) as exc:
        download_installer(ASSET, tmp_path, boom)
    assert exc.value.code == app_update.DOWNLOAD_NETWORK
    with pytest.raises(app_update.DownloadError) as exc:
        download_installer(ASSET, tmp_path, lambda url: Stream(BODY), cancelled=lambda: True)
    assert exc.value.code == app_update.DOWNLOAD_CANCELLED

    def redirect(url):
        app_update._HttpsOnly().redirect_request(
            urllib.request.Request(url), None, 302, "Found", {}, "http://evil.example/x")

    import urllib.request
    with pytest.raises(app_update.DownloadError) as exc:
        download_installer(ASSET, tmp_path, redirect)
    assert exc.value.code == app_update.DOWNLOAD_INSECURE
    with pytest.raises(app_update.DownloadError) as exc:
        download_installer(Asset(ASSET.name, "http://x/y", ASSET.size, SHA), tmp_path)
    assert exc.value.code == app_update.DOWNLOAD_INSECURE


def _capture_boxes(monkeypatch):
    shown = []

    def fake_exec(box):
        shown.append(box.text())
        box.done(0)
        return 0

    monkeypatch.setattr(window_background.QMessageBox, "exec", fake_exec)
    monkeypatch.setattr(window_background.QMessageBox, "clickedButton", lambda box: None)
    return shown


def test_install_hash_error_is_translated(qtbot, tmp_path, monkeypatch, opened):
    i18n.set_language("en")
    monkeypatch.setattr(window_background.sys, "platform", "win32")
    monkeypatch.setattr(update_dialog, "ask_action", lambda p, r: update_dialog.INSTALL)
    shown = _capture_boxes(monkeypatch)
    w = window_with_release(qtbot, tmp_path, release())
    w._update_fetch_stream = lambda url: Stream(b"x" * len(BODY))
    w.update_label.linkActivated.emit("x")
    qtbot.waitUntil(lambda: bool(shown), timeout=5000)
    assert shown == ["The update could not be downloaded: the SHA-256 checksum does not match"]
    w.close()


def test_installer_launch_failure_has_own_message(qtbot, tmp_path, monkeypatch, opened):
    monkeypatch.setattr(window_background.sys, "platform", "win32")
    monkeypatch.setattr(update_dialog, "ask_action", lambda p, r: update_dialog.INSTALL)
    monkeypatch.setattr(QProcess, "startDetached", lambda p: False)
    shown = _capture_boxes(monkeypatch)
    w = window_with_release(qtbot, tmp_path, release())
    w._update_fetch_stream = lambda url: Stream(BODY)
    w.update_label.linkActivated.emit("x")
    qtbot.waitUntil(lambda: bool(shown), timeout=5000)
    assert shown == ["No se pudo ejecutar el instalador."]
    assert w.isVisible()
    w.close()
