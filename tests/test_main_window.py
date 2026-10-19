import asyncio
import time

import keyring
import pytest
from keyring.backend import KeyringBackend
from keyring.errors import KeyringError, PasswordDeleteError
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QDialog, QLabel, QMessageBox, QPushButton

from jase import passwords
from jase.config import Config, load_config
from jase.games import GAMES
from jase.launcher import LaunchError
from jase.models import ServerAddress, ServerInfo
from jase.scanner import ScanResult
from jase.ui import dialogs, main_window
from jase.ui.dialogs import PasswordDialog
from jase.ui.main_window import MainWindow
from jase.ui.server_model import COL_MAP, COL_MODE, COL_NAME, COL_PING, COL_PLAYERS, COLUMNS
from jase.ui.scan_worker import ScanWorker


class MemKeyring(KeyringBackend):
    priority = 1

    def __init__(self):
        self.d = {}

    def get_password(self, s, u):
        return self.d.get((s, u))

    def set_password(self, s, u, p):
        self.d[(s, u)] = p

    def delete_password(self, s, u):
        if (s, u) not in self.d:
            raise PasswordDeleteError()
        del self.d[(s, u)]


class BrokenSetKeyring(MemKeyring):
    def set_password(self, s, u, p):
        raise KeyringError("boom")


@pytest.fixture(autouse=True)
def mem_keyring():
    previous = keyring.get_keyring()
    kr = MemKeyring()
    keyring.set_keyring(kr)
    yield kr
    keyring.set_keyring(previous)


def mk(host, version="1.3", port=28960, pw=False, connect_port=None, name=None):
    a = ServerAddress(host, port)
    c = ServerAddress(host, connect_port or port)
    return ServerInfo(a, c, name or f"srv {host}", "mp_toujane", "tdm", 3, 20, pw,
                      version, 42, [], {})


class FakeScan:
    """scan_fn falso: emite servidores fijos por juego; opcionalmente con retardo."""

    def __init__(self, servers_by_game, delays=None, error=None, no_master=(),
                 progress_hold=0):
        self.progress_hold = progress_hold  # pausa a mitad de la fase de consultas
        self.servers_by_game = servers_by_game
        self.delays = delays or {}
        self.error = error
        self.no_master = set(no_master)  # juegos cuyo master "no responde"
        self.calls = []

    async def __call__(self, game, masters, on_server, on_status, **kw):
        self.calls.append(game.id)
        on_status(f"Escaneando {game.id}")
        delay = self.delays.get(game.id, 0)
        if delay:
            await asyncio.sleep(delay)
        if self.error:
            raise self.error
        if game.id in self.no_master:
            on_status("Ningún master respondió")
            return ScanResult(masters_ok=0, servers_listed=0, servers_responded=0)
        servers = self.servers_by_game.get(game.id, [])
        on_progress = kw.get("on_progress")
        total = len(servers) + 1  # +1: un servidor "muerto" que nunca responde
        if on_progress:
            on_progress(0, total)
        for i, info in enumerate(servers, 1):
            on_server(info)
            if on_progress:
                on_progress(i, total)
        if self.progress_hold:
            await asyncio.sleep(self.progress_hold)
        if on_progress:
            on_progress(total, total)
        return ScanResult(masters_ok=1, servers_listed=len(servers) + 1,
                          servers_responded=len(servers))


class FakeLaunch:
    def __init__(self, error=None):
        self.calls = []
        self.error = error

    def __call__(self, game, install_dir, address, password, **kw):
        self.calls.append((game, install_dir, address, password))
        if self.error:
            raise self.error


class FakeMaps:
    """MapImageService falso: `paths` = mapa -> Path; `delays` = mapa -> segundos."""

    def __init__(self, paths=None, delays=None):
        self.paths = paths or {}
        self.delays = delays or {}
        self.calls = []
        self.started = []
        self.done = []

    def get(self, game, install_dir, map_name, allow_download):
        self.calls.append((game.id, install_dir, map_name, allow_download))
        self.started.append(map_name)
        time.sleep(self.delays.get(map_name, 0))
        self.done.append(map_name)
        return self.paths.get(map_name)


@pytest.fixture
def make_window(qtbot, tmp_path):
    windows = []

    def factory(scan, launch=None, cfg=None, map_service=None):
        w = MainWindow(cfg or Config(), tmp_path / "config.json", scan_fn=scan,
                       launch_fn=launch or FakeLaunch(),
                       map_service=map_service or FakeMaps())
        qtbot.addWidget(w)
        windows.append(w)
        return w

    yield factory
    for w in windows:
        w.close()


@pytest.fixture
def boxes(monkeypatch):
    """Registra las llamadas a QMessageBox sin mostrar nada."""
    calls = []

    def fake_static(kind):
        def f(parent, title, text, *a, **kw):
            calls.append((kind, title, text))
            return QMessageBox.Ok
        return staticmethod(f)

    for kind in ("warning", "information", "critical", "question"):
        monkeypatch.setattr(QMessageBox, kind, fake_static(kind))

    def fake_exec(self):
        calls.append(("exec", self.windowTitle(), self.text(),
                      [b.text() for b in self.buttons()]))
        return 0

    monkeypatch.setattr(QMessageBox, "exec", fake_exec)
    return calls


def sidebar_texts(w):
    return {gid: w.sidebar.item(i).text() for i, gid in enumerate(w.game_ids)}


def select_first_row(w):
    w.table.selectRow(0)
    assert w.selected_server() is not None


def test_scan_populates_table_and_sidebar_count(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    texts = sidebar_texts(w)
    qtbot.waitUntil(lambda: "2" in sidebar_texts(w)["cod2"])
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"
    assert "—" in texts["cod1"] and "—" in texts["ut99"]


def test_version_filter_default_hides_other_versions(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1", "1.3"), mk("2.2.2.2", "1.0")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    assert w.proxy.rowCount() == 1


def test_version_combo_lists_observed_versions_and_persists(make_window, qtbot, tmp_path):
    scan = FakeScan({"cod2": [mk("1.1.1.1", "1.3"), mk("2.2.2.2", "1.01b")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    items = [w.version_combo.itemText(i) for i in range(w.version_combo.count())]
    assert items[:4] == ["Todas", "1.0", "1.2", "1.3"] and "1.01b" in items
    assert w.version_combo.currentText() == "1.3"
    w.version_combo.setCurrentText("Todas")
    assert w.proxy.rowCount() == 2
    assert load_config(tmp_path / "config.json").version_filter["cod2"] == "Todas"


def test_switching_game_discards_stale_results(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")],
                     "ut99": [mk("9.9.9.9", "451", port=7778)]},
                    delays={"cod2": 0.3})
    w = make_window(scan)
    w.select_game("cod2")
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w._workers and w.model.rowCount() == 1)
    rows = w.model.servers()
    assert rows and all(s.address.host == "9.9.9.9" for s in rows)
    n = len(scan.calls)
    w.select_game("ut99")
    assert len(scan.calls) == n


def test_switching_back_restores_saved_list_without_rescan(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1")], "ut99": [mk("9.9.9.9", "451")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning())
    w.select_game("ut99")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1
                    and w.model.server_at(0).address.host == "9.9.9.9")
    qtbot.waitUntil(lambda: not w.is_scanning())
    w.select_game("cod2")
    assert scan.calls == ["cod2", "ut99"]
    assert [s.address.host for s in w.model.servers()] == ["1.1.1.1"]
    assert sidebar_texts(w)["ut99"] == "Unreal Tournament · 1"


def test_refresh_rescans_current_game(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    w.refresh()
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    assert scan.calls == ["cod2", "cod2"]


def test_connect_passes_connect_address_and_password(make_window, qtbot, monkeypatch, boxes):
    info = mk("5.5.5.5", "451", port=7778, pw=True, connect_port=7777)
    scan = FakeScan({"ut99": [info]})
    launch = FakeLaunch()
    seen_initial = []

    def fake_exec(self):
        seen_initial.append(self.edit.text())
        self.edit.setText("pw")
        return QDialog.Accepted

    monkeypatch.setattr(PasswordDialog, "exec", fake_exec)
    w = make_window(scan, launch)
    w.select_game("ut99")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    assert w.connect_button.isEnabled()
    w.connect_selected()
    assert launch.calls == [(GAMES["ut99"], r"D:\Juegos\UnrealTournament",
                             ServerAddress("5.5.5.5", 7777), "pw")]
    assert passwords.get_password("ut99", info.address) == "pw"
    assert seen_initial == [""]
    # Segunda vez el diálogo trae la contraseña guardada.
    w.connect_selected()
    assert seen_initial == ["", "pw"]
    assert boxes == []


def test_forget_password(make_window, qtbot, monkeypatch, boxes):
    info = mk("5.5.5.5", "451", port=7778, pw=True, connect_port=7777)
    passwords.set_password("ut99", info.address, "old")
    launch = FakeLaunch()

    def fake_exec(self):
        assert self.forget_button.isVisibleTo(self)
        self.forget_button.click()
        return QDialog.Rejected

    monkeypatch.setattr(PasswordDialog, "exec", fake_exec)
    w = make_window(FakeScan({"ut99": [info]}), launch)
    w.select_game("ut99")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    w.connect_selected()
    assert passwords.get_password("ut99", info.address) is None
    assert launch.calls == []


def test_empty_password_submit_forgets_saved_and_launches_without(
        make_window, qtbot, monkeypatch, boxes):
    info = mk("5.5.5.5", "451", port=7778, pw=True, connect_port=7777)
    passwords.set_password("ut99", info.address, "old")
    launch = FakeLaunch()

    def fake_exec(self):
        assert self.edit.text() == "old"
        self.edit.setText("")
        return QDialog.Accepted

    monkeypatch.setattr(PasswordDialog, "exec", fake_exec)
    w = make_window(FakeScan({"ut99": [info]}), launch)
    w.select_game("ut99")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    w.connect_selected()
    assert passwords.get_password("ut99", info.address) is None
    assert launch.calls == [(GAMES["ut99"], r"D:\Juegos\UnrealTournament",
                             ServerAddress("5.5.5.5", 7777), None)]


def test_double_click_row_connects(make_window, qtbot, boxes):
    launch = FakeLaunch()
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), launch)
    w.show()
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.proxy.rowCount() == 1)
    rect = w.table.visualRect(w.proxy.index(0, COL_NAME))
    qtbot.mouseClick(w.table.viewport(), Qt.LeftButton, pos=rect.center())
    qtbot.mouseDClick(w.table.viewport(), Qt.LeftButton, pos=rect.center())
    assert launch.calls == [(GAMES["cod2"], r"D:\Juegos\CoD2",
                             ServerAddress("1.1.1.1", 28960), None)]


def test_password_dialog_forget_button_only_with_initial(qtbot):
    d1 = PasswordDialog("srv", None)
    d2 = PasswordDialog("srv", "abc")
    qtbot.addWidget(d1)
    qtbot.addWidget(d2)
    assert not d1.forget_button.isVisibleTo(d1)
    assert d2.forget_button.isVisibleTo(d2) and d2.password() == "abc"
    assert d1.forget_requested is False


def test_keyring_error_still_launches(make_window, qtbot, monkeypatch, boxes):
    keyring.set_keyring(BrokenSetKeyring())
    info = mk("5.5.5.5", "451", port=7778, pw=True, connect_port=7777)
    launch = FakeLaunch()

    def fake_exec(self):
        self.edit.setText("pw")
        return QDialog.Accepted

    monkeypatch.setattr(PasswordDialog, "exec", fake_exec)
    w = make_window(FakeScan({"ut99": [info]}), launch)
    w.select_game("ut99")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    w.connect_selected()
    assert [b[0] for b in boxes] == ["warning"]
    assert "contraseña" in boxes[0][2].lower()
    assert launch.calls and launch.calls[0][3] == "pw"


def test_connect_without_path_shows_settings_prompt(make_window, qtbot, boxes):
    launch = FakeLaunch()
    w = make_window(FakeScan({"cod1": [mk("3.3.3.3", "1.5")]}), launch)
    w.select_game("cod1")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    assert not w.connect_button.isEnabled()
    w.connect_selected()
    assert boxes and boxes[0][0] == "exec" and "Abrir ajustes" in boxes[0][3]
    assert launch.calls == []


def test_missing_path_prompt_opens_settings(make_window, qtbot, monkeypatch):
    opened = []

    def click_open_settings(self):
        [b for b in self.buttons() if b.text() == "Abrir ajustes"][0].click()
        return 0

    def fake_settings_exec(self):
        opened.append(self.windowTitle())
        return QDialog.Rejected

    monkeypatch.setattr(QMessageBox, "exec", click_open_settings)
    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_settings_exec)
    w = make_window(FakeScan({"cod1": [mk("3.3.3.3", "1.5")]}))
    w.select_game("cod1")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    w.connect_selected()
    assert opened == ["Ajustes"]


@pytest.mark.parametrize("error", [LaunchError("No se encontró X"), OSError("No se encontró X")])
def test_launch_error_shows_message(make_window, qtbot, boxes, error):
    launch = FakeLaunch(error=error)
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), launch)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    w.connect_selected()
    assert len(launch.calls) == 1
    assert len(boxes) == 1 and boxes[0][0] == "exec"
    assert "No se encontró X" in boxes[0][2] and "Abrir ajustes" in boxes[0][3]


def test_theme_toggle_persists(make_window, qtbot, tmp_path):
    w = make_window(FakeScan({}))
    seen = []
    for _ in range(3):
        w.theme_button.click()
        seen.append(load_config(tmp_path / "config.json").theme)
    assert seen == ["dark", "light", "system"]


def test_scan_errors_are_reported_in_status(make_window, qtbot):
    w = make_window(FakeScan({}, error=RuntimeError("explotó")))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning())
    qtbot.waitUntil(lambda: "explotó" in w.statusBar().currentMessage())


def test_worker_cancel_stops_scan(qtbot):
    started = []

    async def slow(game, masters, on_server, on_status, **kw):
        started.append(True)
        await asyncio.sleep(30)

    worker = ScanWorker(GAMES["cod2"], [], scan_fn=slow)
    worker.start()
    try:
        qtbot.waitUntil(lambda: bool(started))
        worker.cancel()
        assert worker.wait(2000)
    finally:
        worker.cancel()
        worker.wait()


def test_settings_dialog_apply_to(qtbot):
    cfg = Config()
    d = dialogs.SettingsDialog(cfg)
    qtbot.addWidget(d)
    assert d.path_edits["cod2"].text() == r"D:\Juegos\CoD2"
    d.path_edits["cod1"].setText(r"C:\CoD")
    d.path_edits["ut99"].setText("")
    d.apply_to(cfg)
    assert cfg.game_paths == {"cod2": r"D:\Juegos\CoD2", "cod1": r"C:\CoD",
                              "mohaa": r"D:\Juegos\MOHAA_STOCK"}


def test_settings_shows_path_status(qtbot, tmp_path):
    (tmp_path / "CoD2MP_s.exe").write_bytes(b"")
    d = dialogs.SettingsDialog(Config(game_paths={"cod2": str(tmp_path)}))
    qtbot.addWidget(d)
    label = d.findChild(dialogs.QLabel, "path_status_cod2")
    assert label.text() == "\u2713 CoD2MP_s.exe"
    d.path_edits["cod2"].setText(str(tmp_path / "otra"))
    qtbot.waitUntil(lambda: label.text().startswith("\u26a0"))
    assert "CoD2MP_s.exe" in label.text()
    assert d.findChild(dialogs.QLabel, "path_status_cod1").text().startswith("\u26a0")


def test_settings_status_names_openmohaa_launcher(qtbot, tmp_path):
    (tmp_path / "MOHAA.exe").write_bytes(b"")
    (tmp_path / "launch_openmohaa_base.exe").write_bytes(b"")
    d = dialogs.SettingsDialog(Config(game_paths={"mohaa": str(tmp_path)}))
    qtbot.addWidget(d)
    label = d.findChild(dialogs.QLabel, "path_status_mohaa")
    assert label.text() == "\u2713 launch_openmohaa_base.exe"


def test_settings_status_is_debounced(qtbot, monkeypatch):
    calls = []
    monkeypatch.setattr(dialogs, "resolve_exe", lambda d, r, *c: calls.append(d) or None)
    d = dialogs.SettingsDialog(Config(game_paths={}))
    qtbot.addWidget(d)
    initial = len(calls)
    for text in ("a", "ab", "abc", "abcd"):
        d.path_edits["cod2"].setText(text)
    assert len(calls) == initial
    qtbot.waitUntil(lambda: len(calls) == initial + 1)
    qtbot.wait(600)
    assert len(calls) == initial + 1
    assert calls[-1] == "abcd"


def test_module_exposes_main_window():
    assert main_window.MainWindow is MainWindow


def test_signals_from_cancelled_worker_are_ignored(make_window, qtbot):
    """Aunque un worker cancelado alcance a emitir, sus servidores no se mezclan."""
    calls = []

    async def scan(game, masters, on_server, on_status, **kw):
        calls.append(game.id)
        if game.id == "cod2":
            try:
                await asyncio.sleep(30)
            except asyncio.CancelledError:
                on_status("estado obsoleto")
                on_server(mk("6.6.6.6"))
                raise
        else:
            on_server(mk("9.9.9.9", "451"))

    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: calls == ["cod2"])
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    assert [s.address.host for s in w.model.servers()] == ["9.9.9.9"]
    assert w.statusBar().currentMessage() != "estado obsoleto"
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · —"


def test_last_complete_result_survives_refresh_and_switch(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")],
                     "ut99": [mk("9.9.9.9", "451")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"
    scan.delays["cod2"] = 30            # el refresco queda colgado
    w.refresh()
    qtbot.waitUntil(lambda: scan.calls == ["cod2", "cod2"])
    assert w.is_scanning()
    assert "—" not in sidebar_texts(w)["cod2"]
    w.select_game("ut99")               # cambio a mitad del refresco
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"
    w.select_game("cod2")               # se restaura la última lista completa
    assert sorted(s.address.host for s in w.model.servers()) == ["1.1.1.1", "2.2.2.2"]
    assert scan.calls == ["cod2", "cod2", "ut99"]
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"


def test_errored_scan_is_not_cached(make_window, qtbot):
    scan = FakeScan({"ut99": [mk("9.9.9.9", "451")]}, error=None)
    calls = []

    async def failing(game, masters, on_server, on_status, **kw):
        calls.append(game.id)
        if game.id == "cod2":
            on_server(mk("1.1.1.1"))
            raise RuntimeError("explotó")
        await scan(game, masters, on_server, on_status)

    w = make_window(failing)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · —"
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w.is_scanning())
    w.select_game("cod2")
    qtbot.waitUntil(lambda: calls == ["cod2", "ut99", "cod2"])
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)


def test_no_master_scan_is_not_cached(make_window, qtbot):
    scan = FakeScan({"ut99": [mk("9.9.9.9", "451")]}, no_master={"cod2"})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · —"
    assert w.statusBar().currentMessage() == "Ningún master respondió"
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    w.select_game("cod2")                # sin lista guardada: se vuelve a escanear
    qtbot.waitUntil(lambda: scan.calls == ["cod2", "ut99", "cod2"])


def test_no_master_refresh_keeps_previous_list(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")],
                     "ut99": [mk("9.9.9.9", "451")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    scan.no_master.add("cod2")           # corte de red durante el refresco
    w.refresh()
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    w.select_game("cod2")
    assert sorted(s.address.host for s in w.model.servers()) == ["1.1.1.1", "2.2.2.2"]
    assert scan.calls == ["cod2", "cod2", "ut99"]


def test_successful_scan_shows_final_status(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    assert w.statusBar().currentMessage() == "2 de 3 servidores respondieron"


def test_worker_marks_no_master_as_failed(qtbot):
    async def no_master(game, masters, on_server, on_status, **kw):
        return ScanResult(masters_ok=0, servers_listed=0, servers_responded=0)

    worker = ScanWorker(GAMES["cod2"], [], scan_fn=no_master)
    worker.start()
    assert worker.wait(2000)
    assert worker.failed
    assert worker.result == ScanResult(0, 0, 0)


def test_text_filter_updates_selection_state(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1", name="alpha")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    assert w.connect_button.isEnabled()
    w.filter_edit.setText("zzz")
    assert w.proxy.rowCount() == 0
    assert not w.connect_button.isEnabled()
    assert w.count_label.text() == "0 servidores"


def test_close_does_not_freeze_on_stuck_worker(qtbot, tmp_path, monkeypatch):
    import time

    started = []

    async def stuck(game, masters, on_server, on_status, **kw):
        started.append(True)
        time.sleep(0.8)                 # bloquea el bucle: cancel() no lo interrumpe

    monkeypatch.setattr(MainWindow, "CLOSE_WAIT_MS", 100)
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=stuck,
                   launch_fn=FakeLaunch())
    w.select_game("cod2")
    qtbot.waitUntil(lambda: bool(started))
    t0 = time.monotonic()
    w.close()
    assert time.monotonic() - t0 < 0.6
    lingering = list(main_window._LINGERING)
    assert lingering and lingering[0].isRunning()
    main_window.shutdown_lingering_workers(5000)
    assert not lingering[0].isRunning() and not main_window._LINGERING


# ------------------------------------------------------------ pulido de la UI
def test_rules_panel_strips_color_codes(make_window, qtbot):
    info = mk("1.1.1.1")
    info.rules.update({"_Admin": "^7Tit^5a^4N", "sv_hostname": "^1Jimmy^7 TDM"})
    w = make_window(FakeScan({"cod2": [info]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    w.table.selectRow(0)
    values = {w.rules_table.item(r, 0).text(): w.rules_table.item(r, 1).text()
              for r in range(w.rules_table.rowCount())}
    assert values["_Admin"] == "TitaN"
    assert values["sv_hostname"] == "Jimmy TDM"
    assert info.rules["_Admin"] == "^7Tit^5a^4N"  # el dato crudo no se toca


def test_tables_do_not_wrap_and_elide(make_window):
    w = make_window(FakeScan({}))
    for t in (w.table, w.players_table, w.rules_table):
        assert t.wordWrap() is False
        assert t.textElideMode() == Qt.ElideRight


def test_map_and_mode_columns_are_wide_enough(make_window):
    w = make_window(FakeScan({}))
    h = w.table.horizontalHeader()
    assert h.sectionSize(COL_MAP) >= 160  # Mapa: "CTF-Bleak-CE100", "DM-Deck16]["
    assert h.sectionSize(COL_MODE) >= 140  # Modo: "AssignedTeamGame"


def test_ping_column_uses_colored_delegate(make_window):
    from jase.ui.server_model import PingDelegate
    w = make_window(FakeScan({}))
    assert isinstance(w.table.itemDelegateForColumn(COL_PING), PingDelegate)


def test_server_and_player_names_are_colored(make_window, qtbot):
    from jase.models import Player
    from jase.ui.colored_text import ColoredTextDelegate
    from jase.ui.server_model import COLOR_RUNS_ROLE
    info = mk("1.1.1.1", name="Jimmy")
    info.raw_name = "^1Jimmy"
    info.players.append(Player("Sgt.Jimmy", 5, 40, raw_name="Sgt.^2Jimmy"))
    w = make_window(FakeScan({"cod2": [info]}))
    assert isinstance(w.table.itemDelegateForColumn(COL_NAME), ColoredTextDelegate)
    assert isinstance(w.players_table.itemDelegateForColumn(0), ColoredTextDelegate)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    w.table.selectRow(0)
    item = w.players_table.item(0, 0)
    assert item.text() == "Sgt.Jimmy"
    assert [t for t, _ in item.data(COLOR_RUNS_ROLE)] == ["Sgt.", "Jimmy"]


# ------------------------------------------------------------ filtros rápidos
def test_hide_empty_checkbox_filters_and_persists(make_window, qtbot, tmp_path):
    empty = mk("2.2.2.2")
    empty.num_players = 0
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1"), empty]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 2)
    w.hide_empty_check.setChecked(True)
    assert w.proxy.rowCount() == 1
    assert w.count_label.text() == "1 servidor"
    assert load_config(tmp_path / "config.json").quick_filters.hide_empty is True


def test_quick_filter_widgets_reflect_config(make_window, tmp_path):
    from jase.ui.server_model import QuickFilters
    cfg = Config(quick_filters=QuickFilters(hide_full=True, hide_password=True, max_ping=150))
    w = make_window(FakeScan({}), cfg=cfg)
    assert not w.hide_empty_check.isChecked()
    assert w.hide_full_check.isChecked() and w.hide_password_check.isChecked()
    assert w.hide_empty_check.text() == "Ocultar vacíos"
    assert w.hide_full_check.text() == "Ocultar llenos"
    assert w.hide_password_check.text() == "Ocultar con contraseña"
    items = [w.max_ping_combo.itemText(i) for i in range(w.max_ping_combo.count())]
    assert items == ["Sin límite", "50", "100", "150", "250"]
    assert w.max_ping_combo.currentText() == "150"


def test_max_ping_combo_filters_and_persists(make_window, qtbot, tmp_path):
    far = mk("2.2.2.2")
    far.ping_ms = 120
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1"), far]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 2)
    w.max_ping_combo.setCurrentText("100")
    assert w.proxy.rowCount() == 1
    assert load_config(tmp_path / "config.json").quick_filters.max_ping == 100
    w.max_ping_combo.setCurrentText("Sin límite")
    assert w.proxy.rowCount() == 2
    assert load_config(tmp_path / "config.json").quick_filters.max_ping == 0


# ------------------------------------------------------- progreso y ocupación
def test_worker_emits_progress(qtbot):
    async def fake(game, masters, on_server, on_status, on_progress=None, **kw):
        on_progress(0, 2)
        on_progress(1, 2)
        on_progress(2, 2)
        return ScanResult(masters_ok=1, servers_listed=2, servers_responded=1)

    seen = []
    worker = ScanWorker(GAMES["cod2"], [], scan_fn=fake)
    worker.progress.connect(lambda done, total: seen.append((done, total)))
    worker.start()
    assert worker.wait(2000)
    qtbot.waitUntil(lambda: len(seen) == 3)
    assert seen == [(0, 2), (1, 2), (2, 2)]


def test_scan_progress_bar_visible_during_scan(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")]}, progress_hold=0.4))
    assert w.scan_progress.isHidden()
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.scan_progress.isHidden() and w.scan_progress.value() == 2)
    assert w.scan_progress.maximum() == 3
    assert w.scan_progress.format() == "Consultando %v / %m"
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    assert w.scan_progress.isHidden()


def test_scan_progress_bar_hidden_when_scan_cancelled(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")], "ut99": []},
                             progress_hold=0.5))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.scan_progress.isHidden())
    w._stop_active_scan()
    assert w.scan_progress.isHidden()
    qtbot.waitUntil(lambda: not w._workers)
    assert w.scan_progress.isHidden()  # señales tardías del worker cancelado se ignoran


def test_players_column_uses_occupancy_delegate(make_window, qtbot):
    from jase.ui.server_model import OccupancyDelegate
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}))
    assert isinstance(w.table.itemDelegateForColumn(COL_PLAYERS), OccupancyDelegate)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    w.resize(1100, 700)
    assert not w.table.grab().isNull()  # pinta la barra sin errores


# ------------------------------------------------------------ ficha del servidor
def select_host(w, host):
    for r in range(w.proxy.rowCount()):
        src = w.proxy.mapToSource(w.proxy.index(r, 0))
        if w.model.server_at(src.row()).address.host == host:
            w.table.selectRow(r)
            return
    raise AssertionError(host)


def solid_png(path, color):
    from PySide6.QtGui import QColor, QPixmap
    pix = QPixmap(160, 120)
    pix.fill(QColor(color))
    assert pix.save(str(path), "PNG")
    return path


def card_color(w):
    pix = w.server_card.map_label.pixmap()
    if pix.isNull():
        return None
    return pix.toImage().pixelColor(pix.width() // 2, pix.height() // 2).name()


def test_card_shows_selected_server(make_window, qtbot):
    from PySide6.QtWidgets import QLabel
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1", name="Alpha")]}))
    empty = w.server_card.findChild(QLabel, "card_empty")
    assert empty.text() == "Selecciona un servidor"
    w.show()
    assert empty.isVisible()
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_first_row(w)
    assert not empty.isVisible()
    assert "Alpha" in w.server_card.findChild(QLabel, "card_name").text()
    w.table.clearSelection()
    assert empty.isVisible()


def test_card_loads_map_image_off_ui_thread(make_window, qtbot, tmp_path):
    import threading
    maps = FakeMaps({"mp_toujane": solid_png(tmp_path / "a.png", "#ff0000")})
    threads = []
    real_get = maps.get

    def spy(*a):
        threads.append(threading.current_thread())
        return real_get(*a)

    maps.get = spy
    cfg = Config(game_paths={"cod2": "D:/CoD2"}, download_map_images=True)
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), cfg=cfg, map_service=maps)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_first_row(w)
    qtbot.waitUntil(lambda: card_color(w) == "#ff0000")
    assert maps.calls == [("cod2", "D:/CoD2", "mp_toujane", True)]
    assert threads and threads[0] is not threading.main_thread()


def test_stale_map_image_is_ignored(make_window, qtbot, tmp_path):
    a = mk("1.1.1.1")
    a.map = "mp_slow"
    b = mk("2.2.2.2")
    b.map = "mp_fast"
    maps = FakeMaps({"mp_slow": solid_png(tmp_path / "a.png", "#ff0000"),
                     "mp_fast": solid_png(tmp_path / "b.png", "#00ff00")},
                    delays={"mp_slow": 0.4})
    w = make_window(FakeScan({"cod2": [a, b]}), map_service=maps)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    select_host(w, "1.1.1.1")
    # El cargador omite peticiones canceladas antes de empezar: hay que esperar
    # a que get() de A arranque de verdad para que su resultado llegue obsoleto.
    qtbot.waitUntil(lambda: "mp_slow" in maps.started, timeout=10000)
    select_host(w, "2.2.2.2")
    qtbot.waitUntil(lambda: card_color(w) == "#00ff00")
    qtbot.waitUntil(lambda: sorted(maps.done) == ["mp_fast", "mp_slow"])
    qtbot.wait(100)  # entrega de la señal tardía de A
    assert card_color(w) == "#00ff00"
    assert w.selected_server().address.host == "2.2.2.2"


def test_card_placeholder_when_no_image(make_window, qtbot):
    maps = FakeMaps()
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), map_service=maps)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_first_row(w)
    qtbot.waitUntil(lambda: maps.done == ["mp_toujane"])
    qtbot.wait(50)
    assert card_color(w) is None
    assert w.server_card.map_label.text() == "mp_toujane"


def test_card_image_requested_once_per_server(make_window, qtbot):
    info = mk("1.1.1.1")
    maps = FakeMaps()
    w = make_window(FakeScan({"cod2": [info]}), map_service=maps)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_first_row(w)
    w.cycle_theme()  # vuelve a pintar los detalles
    w._show_details(w.selected_server())  # refresco del mismo servidor: no repite la consulta
    qtbot.waitUntil(lambda: len(maps.done) == 1)
    qtbot.wait(50)
    assert len(maps.calls) == 1


def test_card_copy_reports_in_status_bar(make_window, qtbot):
    from PySide6.QtWidgets import QApplication, QPushButton
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1", connect_port=28961)]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_first_row(w)
    w.server_card.findChild(QPushButton, "copy_address").click()
    assert QApplication.clipboard().text() == "1.1.1.1:28961"
    assert "1.1.1.1:28961" in w.statusBar().currentMessage()


def test_default_map_service_uses_cache_next_to_config(qtbot, tmp_path):
    from jase.maps.service import MapImageService
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=FakeScan({}),
                   launch_fn=FakeLaunch())
    qtbot.addWidget(w)
    assert isinstance(w._map_service, MapImageService)
    assert w._map_service.cache_dir == tmp_path / "cache" / "maps"
    w.close()


def test_close_mid_map_task_is_prompt_and_harmless(qtbot, tmp_path):
    from PySide6.QtCore import QCoreApplication, QEvent
    maps = FakeMaps({"mp_toujane": solid_png(tmp_path / "a.png", "#ff0000")},
                    delays={"mp_toujane": 0.5})
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=FakeScan({"cod2": [mk("1.1.1.1")]}),
                   launch_fn=FakeLaunch(), map_service=maps)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning() and not w._workers)
    select_first_row(w)
    qtbot.waitUntil(lambda: len(maps.calls) == 1)
    start = time.monotonic()
    w.close()
    assert time.monotonic() - start < 0.2  # no espera a la tarea de imagen
    w.deleteLater()
    QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
    qtbot.waitUntil(lambda: maps.done == ["mp_toujane"])
    qtbot.wait(100)  # la señal tardía se entrega sin receptor: sin excepción ni aviso


def test_map_task_threads_are_daemon(make_window, qtbot):
    import threading
    seen = []
    maps = FakeMaps()
    real_get = maps.get
    maps.get = lambda *a: (seen.append(threading.current_thread()), real_get(*a))[1]
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), map_service=maps)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_first_row(w)
    qtbot.waitUntil(lambda: bool(seen))
    # Hilo de Python propio y daemon (no un hilo de QThreadPool, cuyo destructor
    # espera a las tareas): una descarga lenta nunca bloquea la salida del proceso.
    assert seen[0].name.startswith("jase-map-image")
    assert seen[0].daemon


def test_settings_accept_rerequests_card_image(make_window, qtbot, monkeypatch):
    def fake_settings_exec(self):
        self.download_maps_check.setChecked(True)
        return QDialog.Accepted

    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_settings_exec)
    maps = FakeMaps()
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), map_service=maps)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_first_row(w)
    qtbot.waitUntil(lambda: len(maps.done) == 1)
    w.open_settings()
    qtbot.waitUntil(lambda: len(maps.done) == 2)
    assert [c[3] for c in maps.calls] == [False, True]


# ------------------------------------------- menú contextual, atajos, refrescar uno
class FakeQuery:
    """query_fn falso con la firma de scanner.query_servers."""

    def __init__(self, answers=None, delay=0.0):
        self.answers = answers or {}  # ServerAddress -> ServerInfo
        self.delay = delay
        self.calls = []

    async def __call__(self, family, addresses, on_server, **kw):
        addresses = list(addresses)
        self.calls.append((family, addresses))
        if self.delay:
            await asyncio.sleep(self.delay)
        for a in addresses:
            if a in self.answers:
                on_server(self.answers[a])


def test_query_worker_emits_found_and_done(qtbot):
    from jase.ui.scan_worker import QueryWorker
    a = mk("1.1.1.1")
    query = FakeQuery({a.address: a})
    worker = QueryWorker(GAMES["cod2"], [a.address, ServerAddress("2.2.2.2", 1)],
                         query_fn=query)
    found, done = [], []
    worker.server_found.connect(found.append)
    worker.done.connect(lambda: done.append(True))
    worker.start()
    assert worker.wait(2000)
    qtbot.waitUntil(lambda: bool(done))
    assert found == [a]
    assert query.calls == [("quake3", [a.address, ServerAddress("2.2.2.2", 1)])]


def test_query_worker_cancel(qtbot):
    from jase.ui.scan_worker import QueryWorker
    started = []

    async def slow(family, addresses, on_server, **kw):
        started.append(True)
        await asyncio.sleep(30)

    worker = QueryWorker(GAMES["cod2"], [ServerAddress("1.1.1.1", 1)], query_fn=slow)
    worker.start()
    try:
        qtbot.waitUntil(lambda: bool(started))
        worker.cancel()
        assert worker.wait(2000)
    finally:
        worker.cancel()
        worker.wait()


def make_query_window(make_window, qtbot, servers, query):
    w = make_window(FakeScan({"cod2": servers}))
    w._query_fn = query
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == len(servers) and not w.is_scanning())
    return w


def test_context_menu_actions(make_window, qtbot):
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1")], FakeQuery())
    select_first_row(w)
    menu = w._build_context_menu()
    texts = [a.text() for a in menu.actions() if not a.isSeparator()]
    assert texts == ["Conectar", "Copiar IP:puerto", "Refrescar este servidor",
                     "Agregar a favoritos"]
    assert all(a.isEnabled() for a in menu.actions())
    w.table.clearSelection()
    menu = w._build_context_menu()
    assert not any(a.isEnabled() for a in menu.actions() if not a.isSeparator())


def test_context_menu_copy_action_copies_connect_address(make_window, qtbot):
    from PySide6.QtWidgets import QApplication
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1", connect_port=28961)], FakeQuery())
    select_first_row(w)
    QApplication.clipboard().setText("")
    action = next(a for a in w._build_context_menu().actions() if a.text() == "Copiar IP:puerto")
    action.trigger()
    assert QApplication.clipboard().text() == "1.1.1.1:28961"
    assert "1.1.1.1:28961" in w.statusBar().currentMessage()


def show_active(w, qtbot):
    with qtbot.waitActive(w):
        w.show()
        w.activateWindow()
    assert w.isActiveWindow()


def test_ctrl_c_on_table_copies_address(make_window, qtbot):
    from PySide6.QtWidgets import QApplication
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1", connect_port=28961)], FakeQuery())
    show_active(w, qtbot)
    select_first_row(w)
    w.table.setFocus()
    QApplication.clipboard().setText("")
    qtbot.keyClick(w.table, Qt.Key_C, Qt.ControlModifier)
    assert QApplication.clipboard().text() == "1.1.1.1:28961"


def test_f5_refreshes(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    show_active(w, qtbot)
    qtbot.keyClick(w, Qt.Key_F5)
    qtbot.waitUntil(lambda: not w.is_scanning() and len(scan.calls) == 2)
    assert scan.calls == ["cod2", "cod2"]


def test_ctrl_f_focuses_filter_and_selects_text(make_window, qtbot):
    w = make_window(FakeScan({}))
    show_active(w, qtbot)
    w.filter_edit.setText("abc")
    w.table.setFocus()
    qtbot.keyClick(w.table, Qt.Key_F, Qt.ControlModifier)
    assert w.filter_edit.hasFocus()
    assert w.filter_edit.selectedText() == "abc"


@pytest.mark.parametrize("key", [Qt.Key_Return, Qt.Key_Enter])
def test_enter_on_table_connects(make_window, qtbot, boxes, key):
    launch = FakeLaunch()
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), launch=launch,
                    cfg=Config(game_paths={"cod2": "D:/CoD2"}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    show_active(w, qtbot)
    select_first_row(w)
    w.table.setFocus()
    qtbot.keyClick(w.table, key)
    assert len(launch.calls) == 1


def test_refresh_selected_updates_row_and_card(make_window, qtbot):
    from PySide6.QtWidgets import QLabel
    old = mk("1.1.1.1", name="Viejo")
    new = mk("1.1.1.1", name="Nuevo")
    new.ping_ms = 7
    query = FakeQuery({old.address: new})
    w = make_query_window(make_window, qtbot, [old, mk("2.2.2.2")], query)
    select_host(w, "1.1.1.1")
    w.refresh_selected()
    qtbot.waitUntil(lambda: not w._workers)
    row = next(s for s in w.model.servers() if s.address == old.address)
    assert row.ping_ms == 7 and row.name == "Nuevo"
    assert w.model.rowCount() == 2
    assert query.calls == [("quake3", [old.address])]
    assert "Nuevo" in w.server_card.findChild(QLabel, "card_name").text()
    assert w.selected_server().ping_ms == 7


def test_refresh_selected_no_answer_keeps_row(make_window, qtbot):
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1")], FakeQuery())
    select_first_row(w)
    w.refresh_selected()
    qtbot.waitUntil(lambda: not w._workers)
    assert w.statusBar().currentMessage() == "El servidor no respondió"
    assert w.model.rowCount() == 1 and w.model.server_at(0).ping_ms == 42


def test_refresh_selected_ignored_after_game_switch(make_window, qtbot):
    a = mk("1.1.1.1")
    fresh = mk("1.1.1.1")
    fresh.ping_ms = 7
    w = make_window(FakeScan({"cod2": [a], "ut99": []}))
    w._query_fn = FakeQuery({a.address: fresh}, delay=0.3)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning())
    select_first_row(w)
    w.refresh_selected()
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w._workers)
    qtbot.wait(50)
    assert w.model.rowCount() == 0
    assert w.statusBar().currentMessage() != "El servidor no respondió"


def test_close_waits_or_lingers_query_worker(qtbot, tmp_path, monkeypatch):
    started = []

    async def stuck(family, addresses, on_server, **kw):
        started.append(True)
        time.sleep(0.8)  # bloquea el bucle: cancel() no lo interrumpe

    monkeypatch.setattr(MainWindow, "CLOSE_WAIT_MS", 100)
    w = MainWindow(Config(), tmp_path / "config.json",
                   scan_fn=FakeScan({"cod2": [mk("1.1.1.1")]}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps())
    w._query_fn = stuck
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning() and not w._workers)
    select_first_row(w)
    w.refresh_selected()
    qtbot.waitUntil(lambda: bool(started))
    t0 = time.monotonic()
    w.close()
    assert time.monotonic() - t0 < 0.6
    lingering = list(main_window._LINGERING)
    assert lingering and lingering[0].isRunning()
    main_window.shutdown_lingering_workers(5000)
    assert not main_window._LINGERING


def test_enter_without_selection_does_nothing(make_window, qtbot, boxes):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    show_active(w, qtbot)
    w.table.clearSelection()
    w.table.setFocus()
    qtbot.keyClick(w.table, Qt.Key_Return)
    assert boxes == []


# ------------------------------------------------------ recordar la ventana
UI_KEYS = {"geometry", "body_splitter", "right_splitter", "details_splitter",
           "table_header", "table_header_cols", "last_game"}


def test_close_saves_ui_state(make_window, qtbot, tmp_path):
    w = make_window(FakeScan({"ut99": []}))
    w.select_game("ut99")
    w.close()
    state = load_config(tmp_path / "config.json").ui_state
    assert set(state) == UI_KEYS
    assert state["last_game"] == "ut99"


def test_new_window_restores_column_width_and_sort(make_window, qtbot, tmp_path):
    w = make_window(FakeScan({}))
    header = w.table.horizontalHeader()
    header.resizeSection(COL_MAP, 333)
    w.table.sortByColumn(COL_PLAYERS, Qt.DescendingOrder)
    w.close()
    w2 = make_window(FakeScan({}), cfg=load_config(tmp_path / "config.json"))
    header2 = w2.table.horizontalHeader()
    assert header2.sectionSize(COL_MAP) == 333
    assert header2.sortIndicatorSection() == COL_PLAYERS
    assert header2.sortIndicatorOrder() == Qt.DescendingOrder
    assert w2.proxy.sortColumn() == COL_PLAYERS
    assert w2.proxy.sortOrder() == Qt.DescendingOrder


@pytest.mark.parametrize("value", ["%%% no es base64 %%%", "AAAA", ""])
def test_garbage_ui_state_is_ignored(make_window, value):
    keys = UI_KEYS - {"last_game"}
    w = make_window(FakeScan({}), cfg=Config(ui_state={k: value for k in keys}))
    assert w.table.horizontalHeader().sectionSize(COL_MAP) == main_window.COLUMN_WIDTHS[COL_MAP]
    assert w.proxy.sortColumn() == COL_PING


def test_initial_game_from_ui_state():
    from jase.__main__ import initial_game
    assert initial_game(Config()) == "cod2"
    assert initial_game(Config(ui_state={"last_game": "ut99"})) == "ut99"
    assert initial_game(Config(ui_state={"last_game": "nope"})) == "cod2"


def header_state_b64(n_cols, map_width=333):
    import base64
    from PySide6.QtGui import QStandardItemModel
    from PySide6.QtWidgets import QTableView
    view = QTableView()
    view.setModel(QStandardItemModel(0, n_cols, view))
    view.horizontalHeader().resizeSection(COL_MAP, map_width)
    view.setSortingEnabled(True)
    view.sortByColumn(COL_PLAYERS, Qt.DescendingOrder)
    data = base64.b64encode(bytes(view.horizontalHeader().saveState().data())).decode()
    view.deleteLater()
    return data


@pytest.mark.parametrize("n_cols,saved_cols", [
    (len(COLUMNS) + 1, str(len(COLUMNS) + 1)),  # estado de una versión con otra columna
    (len(COLUMNS) - 1, str(len(COLUMNS) - 1)),
    (len(COLUMNS), None),                       # sin conteo guardado
    (len(COLUMNS), "x"),
])
def test_header_state_with_other_column_count_is_ignored(make_window, n_cols, saved_cols):
    state = {"table_header": header_state_b64(n_cols)}
    if saved_cols is not None:
        state["table_header_cols"] = saved_cols
    w = make_window(FakeScan({}), cfg=Config(ui_state=state))
    header = w.table.horizontalHeader()
    assert header.count() == len(COLUMNS)
    assert header.sectionSize(COL_MAP) == main_window.COLUMN_WIDTHS[COL_MAP]
    assert w.proxy.sortColumn() == COL_PING and w.proxy.sortOrder() == Qt.AscendingOrder


def test_header_state_with_matching_column_count_is_restored(make_window):
    state = {"table_header": header_state_b64(len(COLUMNS)),
             "table_header_cols": str(len(COLUMNS))}
    w = make_window(FakeScan({}), cfg=Config(ui_state=state))
    assert w.table.horizontalHeader().sectionSize(COL_MAP) == 333
    assert w.proxy.sortColumn() == COL_PLAYERS


def test_refresh_selected_error_reports_exception(make_window, qtbot):
    async def boom(family, addresses, on_server, **kw):
        raise RuntimeError("explotó")

    w = make_query_window(make_window, qtbot, [mk("1.1.1.1")], boom)
    select_first_row(w)
    w.refresh_selected()
    qtbot.waitUntil(lambda: not w._workers)
    assert w.statusBar().currentMessage() == "Error al consultar: explotó"
    assert w.model.rowCount() == 1


def test_queued_refresh_result_dropped_after_game_switch(make_window, qtbot):
    import threading
    a = mk("1.1.1.1")
    fresh = mk("1.1.1.1", name="Tardío")
    emitted = threading.Event()

    async def answer_then_wait(family, addresses, on_server, **kw):
        on_server(fresh)          # queda en cola para el hilo de la UI
        emitted.set()
        await asyncio.sleep(0.2)

    w = make_window(FakeScan({"cod2": [a], "ut99": []}))
    w._query_fn = answer_then_wait
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning())
    select_first_row(w)
    w.refresh_selected()
    assert emitted.wait(2)        # sin procesar eventos: la señal sigue en cola
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w._workers and not w.is_scanning())
    qtbot.wait(50)
    assert w.model.rowCount() == 0
    assert all(s.name != "Tardío" for s in w.model.servers())


def test_right_click_on_blank_space_clears_selection(make_window, qtbot, monkeypatch):
    from PySide6.QtCore import QPoint
    from PySide6.QtWidgets import QMenu
    seen = []

    class RecordingMenu(QMenu):
        def exec(self, *_args):  # sin bucle modal: registra el estado de las acciones
            seen.append([act.isEnabled() for act in self.actions() if not act.isSeparator()])

    monkeypatch.setattr(main_window, "QMenu", RecordingMenu)
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1")], FakeQuery())
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    select_first_row(w)
    blank = QPoint(5, w.table.viewport().height() - 3)
    assert not w.table.indexAt(blank).isValid()
    w._on_table_context_menu(blank)
    assert w.selected_server() is None
    assert seen == [[False, False, False, False]]  # incluye «Agregar a favoritos»
    row = QPoint(5, w.table.visualRect(w.proxy.index(0, 0)).center().y())
    w._on_table_context_menu(row)
    assert w.selected_server() is not None
    assert seen[-1] == [True, True, True, True]


# ------------------------------------------- barra lateral: juegos con ruta primero
def _sidebar_names(w):
    return [w.sidebar.item(i).text().split(" · ")[0] for i in range(w.sidebar.count())]


def _then_rest(*first):
    """Los juegos dados primero y luego el resto en el orden de GAMES."""
    return [*first, *(gid for gid in GAMES if gid not in first)]


def test_sidebar_configured_games_first(make_window):
    w = make_window(FakeScan({}), cfg=Config(game_paths={"cod1": r"C:\CoD"}))
    assert w.game_ids == _then_rest("cod1")
    assert _sidebar_names(w) == [GAMES[gid].name for gid in _then_rest("cod1")]


def test_sidebar_without_paths_keeps_games_order(make_window):
    w = make_window(FakeScan({}), cfg=Config(game_paths={}))
    assert w.game_ids == list(GAMES)


def test_sidebar_default_config_order(make_window):
    w = make_window(FakeScan({}))
    assert w.game_ids == _then_rest("cod2", "ut99", "mohaa")


def test_sidebar_resorts_when_paths_change_and_keeps_selection(make_window, monkeypatch):
    w = make_window(FakeScan({}), cfg=Config(game_paths={}))
    w.select_game("ut99")
    assert w.sidebar.currentRow() == w.game_ids.index("ut99")

    class FakeDialog:
        def __init__(self, cfg, parent=None, **kwargs):
            pass

        def exec(self):
            return 1

        def apply_to(self, cfg):
            cfg.game_paths = {"ut99": r"D:\UT"}

        def deleteLater(self):
            pass

    monkeypatch.setattr("jase.ui.main_window.SettingsDialog", FakeDialog)
    w.open_settings()
    assert w.game_ids == _then_rest("ut99")
    assert w.current_game == "ut99"
    assert w.sidebar.currentRow() == 0
    assert _sidebar_names(w)[0] == "Unreal Tournament"
    w.sidebar.setCurrentRow(1)  # la fila 1 es cod2 en el nuevo orden
    assert w.current_game == "cod2"


# ------------------------------------------------------------ país por IP
class FakeCountryDB:
    def __init__(self, table):
        self.table = table

    def lookup(self, ip):
        return self.table.get(ip)


class FakeGeo:
    """CountrySource falso: nunca descarga; `results` = lista de (db, error) por llamada."""

    def __init__(self, *results, delay=0.0):
        self.results = list(results) or [(None, None)]
        self.delay = delay
        self.calls = []

    def load(self, path, refresh=False):
        self.calls.append((path, refresh))
        time.sleep(self.delay)
        return self.results[min(len(self.calls), len(self.results)) - 1]


def make_geo_window(qtbot, tmp_path, geo, scan=None, cfg=None):
    w = MainWindow(cfg or Config(), tmp_path / "config.json",
                   scan_fn=scan or FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")]}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps(), geo=geo)
    qtbot.addWidget(w)
    return w


def test_country_db_loads_in_background_from_cache_dir(qtbot, tmp_path):
    from jase.ui.server_model import COL_COUNTRY
    geo = FakeGeo((FakeCountryDB({"1.1.1.1": "cl"}), None), delay=0.2)
    w = make_geo_window(qtbot, tmp_path, geo)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    assert geo.calls == [(tmp_path / "cache" / "dbip-country-lite.csv", False)]
    # Aún cargando: columna vacía, la UI no esperó.
    assert w.model.data(w.model.index(0, COL_COUNTRY), Qt.ToolTipRole) is None
    qtbot.waitUntil(lambda: w.model.data(w.model.index(0, COL_COUNTRY), Qt.ToolTipRole) == "Chile")
    w.close()


def test_country_filter_and_card(qtbot, tmp_path):
    geo = FakeGeo((FakeCountryDB({"1.1.1.1": "cl"}), None))
    w = make_geo_window(qtbot, tmp_path, geo)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2 and w._country_db is not None)
    w.filter_edit.setText("cl")
    assert w.proxy.rowCount() == 1
    select_first_row(w)
    country = w.server_card.findChild(QLabel, "card_country")
    assert not country.isHidden() and country.text() == "Chile"
    assert not w.server_card.findChild(QLabel, "card_country_flag").isHidden()
    w.close()


def test_card_updates_when_db_arrives_after_selection(qtbot, tmp_path):
    geo = FakeGeo((FakeCountryDB({"1.1.1.1": "cl", "2.2.2.2": "cl"}), None), delay=0.3)
    w = make_geo_window(qtbot, tmp_path, geo)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    select_first_row(w)
    country = w.server_card.findChild(QLabel, "card_country")
    assert country.isHidden()
    qtbot.waitUntil(lambda: not country.isHidden() and country.text() == "Chile")
    w.close()


def test_country_db_failure_only_status_bar(qtbot, tmp_path, boxes):
    from jase.ui.server_model import COL_COUNTRY
    geo = FakeGeo((None, "No se pudo descargar la base de países"))
    w = make_geo_window(qtbot, tmp_path, geo)
    messages = []
    w.statusBar().messageChanged.connect(messages.append)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: "No se pudo descargar la base de países" in messages)
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    assert boxes == [] and not w.country_db_busy()
    assert w.model.data(w.model.index(0, COL_COUNTRY), Qt.DecorationRole) is None
    w.close()


def test_without_geo_nothing_is_loaded(make_window, qtbot):
    from jase.ui.server_model import COL_COUNTRY
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    assert w.model.data(w.model.index(0, COL_COUNTRY), Qt.DecorationRole) is None


def test_settings_button_refreshes_country_db(qtbot, tmp_path, monkeypatch):
    first = FakeCountryDB({})
    second = FakeCountryDB({"1.1.1.1": "br"})
    geo = FakeGeo((first, None), (second, None))
    w = make_geo_window(qtbot, tmp_path, geo)
    qtbot.waitUntil(lambda: w._country_db is first)

    def fake_exec(dialog):
        button = dialog.findChild(QPushButton, "update_country_db")
        assert button is not None and button.text() == "Actualizar base de países"
        button.click()
        return QDialog.Rejected

    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_exec)
    w.open_settings()
    qtbot.waitUntil(lambda: w._country_db is second)
    assert geo.calls[-1] == (tmp_path / "cache" / "dbip-country-lite.csv", True)
    assert "actualizada" in w.statusBar().currentMessage()
    w.close()


def test_failed_refresh_keeps_current_db(qtbot, tmp_path, boxes):
    first = FakeCountryDB({"1.1.1.1": "cl"})
    geo = FakeGeo((first, None), (None, "No se pudo descargar la base de países"))
    w = make_geo_window(qtbot, tmp_path, geo)
    qtbot.waitUntil(lambda: w._country_db is first)
    messages = []
    w.statusBar().messageChanged.connect(messages.append)
    w.update_country_db()
    qtbot.waitUntil(lambda: "No se pudo descargar la base de países" in messages)
    assert len(geo.calls) == 2 and not w.country_db_busy()
    assert w._country_db is first and boxes == []
    w.close()


def test_country_column_layout(make_window):
    from jase.ui.server_model import COL_COUNTRY, COL_LOCK
    w = make_window(FakeScan({}))
    h = w.table.horizontalHeader()
    assert COLUMNS[COL_COUNTRY] == "País"
    assert h.sectionSize(COL_COUNTRY) == main_window.COLUMN_WIDTHS[COL_COUNTRY] <= 50
    assert h.visualIndex(COL_LOCK) < h.visualIndex(COL_COUNTRY) < h.visualIndex(COL_NAME)


def test_v1_header_state_with_seven_columns_is_ignored(make_window):
    # Estado guardado por la versión anterior (7 columnas, sin País).
    state = {"table_header": header_state_b64(7), "table_header_cols": "7"}
    w = make_window(FakeScan({}), cfg=Config(ui_state=state))
    assert len(COLUMNS) == 8
    assert w.table.horizontalHeader().sectionSize(COL_MAP) == main_window.COLUMN_WIDTHS[COL_MAP]
    assert w.proxy.sortColumn() == COL_PING


def test_settings_country_button_disabled_without_geo(make_window, qtbot, monkeypatch):
    w = make_window(FakeScan({}))
    seen = []

    def fake_exec(dialog):
        seen.append(dialog.findChild(QPushButton, "update_country_db").isEnabled())
        return QDialog.Rejected

    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert seen == [False]


# ------------------------------------------------------------------- favoritos
def context_action(w, text):
    return next(a for a in w._build_context_menu().actions() if a.text() == text)


def offline_mk(host, port=28960):
    a = ServerAddress(host, port)
    return ServerInfo(a, a, str(a), "", "", 0, 0, False, "", 0, [], {}, online=False)


def test_context_menu_adds_and_removes_favorite(make_window, qtbot, tmp_path):
    query = FakeQuery()
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1", connect_port=28961)], query)
    select_first_row(w)
    star = w.server_card.findChild(QPushButton, "favorite_toggle")
    assert star.text() == "☆"
    context_action(w, "Agregar a favoritos").trigger()
    # Se guarda la dirección de consulta, no la de conexión.
    assert w.cfg.favorites == {"cod2": ["1.1.1.1:28960"]}
    assert load_config(tmp_path / "config.json").favorites == {"cod2": ["1.1.1.1:28960"]}
    assert star.text() == "★"
    assert query.calls == []  # agregar en la vista normal no consulta nada
    assert not w.is_scanning() and not w.in_favorites_view()
    context_action(w, "Quitar de favoritos").trigger()
    assert w.cfg.favorites == {}
    assert load_config(tmp_path / "config.json").favorites == {}
    assert star.text() == "☆"


def test_card_star_toggles_favorite(make_window, qtbot):
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1")], FakeQuery())
    select_first_row(w)
    star = w.server_card.findChild(QPushButton, "favorite_toggle")
    star.click()
    assert w.cfg.favorites == {"cod2": ["1.1.1.1:28960"]} and star.text() == "★"
    star.click()
    assert w.cfg.favorites == {} and star.text() == "☆"


def favorites_window(make_window, qtbot, query, favs, servers=None, quick=None):
    from jase.models import QuickFilters
    cfg = Config(favorites={"cod2": favs}, quick_filters=quick or QuickFilters())
    scan = FakeScan({"cod2": servers if servers is not None else [mk("1.1.1.1")]})
    w = make_window(scan, cfg=cfg)
    w._query_fn = query
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    return w, scan


def test_favorites_view_shows_online_and_offline(make_window, qtbot):
    from jase.models import QuickFilters
    online = mk("2.2.2.2", version="1.0")  # otra versión: el filtro no la oculta aquí
    query = FakeQuery({online.address: online})
    quick = QuickFilters(hide_empty=True, hide_full=True, max_ping=50)
    w, scan = favorites_window(make_window, qtbot, query,
                               ["2.2.2.2:28960", "3.3.3.3:28960"], quick=quick)
    assert w.proxy.rowCount() == 1  # vista normal: 1.1.1.1 con 3/20 jugadores
    w.favorites_list.itemClicked.emit(w.favorites_list.item(0))
    assert w.in_favorites_view() and w.current_game == "cod2"
    qtbot.waitUntil(lambda: w.model.rowCount() == 2 and not w._workers)
    assert scan.calls == ["cod2"]  # sin master
    assert query.calls == [("quake3", [ServerAddress("2.2.2.2", 28960),
                                       ServerAddress("3.3.3.3", 28960)])]
    by_host = {s.address.host: s for s in w.model.servers()}
    assert by_host["2.2.2.2"].online and by_host["2.2.2.2"].name == "srv 2.2.2.2"
    assert by_host["3.3.3.3"].online is False and by_host["3.3.3.3"].name == "3.3.3.3:28960"
    assert w.proxy.rowCount() == 2  # filtros rápidos y versión no ocultan favoritos
    assert not w.hide_empty_check.isEnabled() and not w.version_combo.isEnabled()
    last = w.proxy.index(w.proxy.rowCount() - 1, COL_NAME)
    assert w.proxy.data(last).startswith("3.3.3.3")  # offline al final por ping
    assert "1 de 2" in w.statusBar().currentMessage()
    assert "2" in w.favorites_list.item(0).text()

    # Volver al juego: lista normal (de la sesión) con los filtros restaurados.
    w.sidebar.itemClicked.emit(w.sidebar.item(w.game_ids.index("cod2")))
    assert not w.in_favorites_view()
    assert [s.address.host for s in w.model.servers()] == ["1.1.1.1"]
    assert w.proxy.rowCount() == 1 and w.hide_empty_check.isEnabled()
    assert w.version_combo.isEnabled() and w.version_combo.currentText() == "1.3"
    w.model.add_or_update(offline_mk("4.4.4.4"))
    assert w.proxy.rowCount() == 1  # filtros activos otra vez
    assert scan.calls == ["cod2"]  # se usó la lista guardada
    assert w._cache["cod2"] == [mk("1.1.1.1")]  # la vista Favoritos no tocó la caché


def test_favorites_refresh_requeries_favorites(make_window, qtbot):
    online = mk("2.2.2.2")
    query = FakeQuery({online.address: online})
    w, scan = favorites_window(make_window, qtbot, query, ["2.2.2.2:28960"])
    w.show_favorites()
    qtbot.waitUntil(lambda: len(query.calls) == 1 and not w._workers)
    w.refresh()
    qtbot.waitUntil(lambda: len(query.calls) == 2 and not w._workers)
    assert scan.calls == ["cod2"] and w.in_favorites_view()
    assert w.model.rowCount() == 1


def test_favorites_view_without_favorites(make_window, qtbot):
    query = FakeQuery()
    w, _scan = favorites_window(make_window, qtbot, query, [])
    w.show_favorites()
    assert w.in_favorites_view() and w.model.rowCount() == 0
    assert "No hay favoritos" in w.statusBar().currentMessage()
    assert query.calls == [] and not w._workers


def test_removing_favorite_in_favorites_view_drops_row(make_window, qtbot):
    online = mk("2.2.2.2")
    w, _scan = favorites_window(make_window, qtbot, FakeQuery({online.address: online}),
                                ["2.2.2.2:28960", "3.3.3.3:28960"])
    w.show_favorites()
    qtbot.waitUntil(lambda: w.model.rowCount() == 2 and not w._workers)
    select_host(w, "3.3.3.3")
    context_action(w, "Quitar de favoritos").trigger()
    assert [s.address.host for s in w.model.servers()] == ["2.2.2.2"]
    assert w.cfg.favorites == {"cod2": ["2.2.2.2:28960"]}


def test_selecting_other_game_leaves_favorites_view(make_window, qtbot):
    w, scan = favorites_window(make_window, qtbot, FakeQuery(delay=0.3), ["2.2.2.2:28960"])
    w.show_favorites()
    w.select_game("ut99")
    assert not w.in_favorites_view() and w.current_game == "ut99"
    qtbot.waitUntil(lambda: not w._workers)
    assert w.model.rowCount() == 0  # el resultado tardío de Favoritos se descarta
    assert scan.calls == ["cod2", "ut99"]


def test_offline_favorite_card_and_connect(make_window, qtbot):
    maps = FakeMaps()
    cfg = Config(favorites={"cod2": ["3.3.3.3:28960"]})
    cfg.game_paths["cod2"] = "C:/cod2"
    launch = FakeLaunch()
    w = make_window(FakeScan({"cod2": []}), launch=launch, cfg=cfg, map_service=maps)
    w._query_fn = FakeQuery()
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    w.show_favorites()
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w._workers)
    select_first_row(w)
    assert w.selected_server().online is False
    assert w.server_card.findChild(QLabel, "card_ping").text() == "Sin respuesta"
    assert w.server_card.findChild(QPushButton, "favorite_toggle").text() == "★"
    qtbot.wait(50)
    assert maps.calls == []  # sin pedido de imagen de mapa
    assert w.connect_button.isEnabled()  # puede estar levantado ahora
    w.connect_selected()
    assert launch.calls[0][2] == ServerAddress("3.3.3.3", 28960)


def test_favorites_view_needs_a_game(make_window):
    w = make_window(FakeScan({}))
    w.show_favorites()
    assert not w.in_favorites_view()


class StagedQuery:
    """query_fn falso: responde `first`, espera `pause` y responde `later`."""

    def __init__(self, first=(), later=(), pause=0.4):
        self.first, self.later, self.pause = list(first), list(later), pause
        self.calls = []

    async def __call__(self, family, addresses, on_server, **kw):
        self.calls.append(list(addresses))
        for info in self.first:
            on_server(info)
        await asyncio.sleep(self.pause)
        for info in self.later:
            on_server(info)


def test_favorite_removed_mid_query_is_not_shown_offline(make_window, qtbot):
    fast = mk("2.2.2.2")  # responde enseguida; 3.3.3.3 no responde nunca
    w, _scan = favorites_window(make_window, qtbot, StagedQuery(first=[fast]),
                                ["2.2.2.2:28960", "3.3.3.3:28960"])
    w.show_favorites()
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_host(w, "2.2.2.2")
    context_action(w, "Quitar de favoritos").trigger()
    assert w.model.rowCount() == 0
    qtbot.waitUntil(lambda: not w._workers, timeout=3000)
    assert [(s.address.host, s.online) for s in w.model.servers()] == [("3.3.3.3", False)]
    assert "0 de 1" in w.statusBar().currentMessage()


def test_late_answer_of_removed_favorite_is_ignored(make_window, qtbot):
    from jase.ui import favorites
    late = mk("2.2.2.2")
    w, _scan = favorites_window(make_window, qtbot, StagedQuery(later=[late]),
                                ["2.2.2.2:28960", "3.3.3.3:28960"])
    w.show_favorites()
    qtbot.waitUntil(lambda: bool(w._favorites_worker and w._favorites_worker.isRunning()))
    favorites.remove_favorite(w.cfg, "cod2", ServerAddress("2.2.2.2", 28960))
    qtbot.waitUntil(lambda: not w._workers, timeout=3000)
    assert [(s.address.host, s.online) for s in w.model.servers()] == [("3.3.3.3", False)]


def test_entering_favorites_during_scan_discards_scan_rows(make_window, qtbot):
    from jase.models import QuickFilters
    cfg = Config(favorites={"cod2": ["9.9.9.9:28960"]}, quick_filters=QuickFilters())
    scan = FakeScan({"cod2": [mk("1.1.1.1")]}, delays={"cod2": 0.3})
    w = make_window(scan, cfg=cfg)
    w._query_fn = FakeQuery()
    w.select_game("cod2")
    assert w.is_scanning()
    w.show_favorites()
    assert not w.is_scanning()
    qtbot.waitUntil(lambda: not w._workers, timeout=3000)
    assert [s.address.host for s in w.model.servers()] == ["9.9.9.9"]
    assert "cod2" not in w._cache


def test_activating_current_game_leaves_favorites_view(make_window, qtbot):
    w, _scan = favorites_window(make_window, qtbot, FakeQuery(), ["2.2.2.2:28960"])
    w.show_favorites()
    w.sidebar.itemActivated.emit(w.sidebar.item(w.game_ids.index("cod2")))
    assert not w.in_favorites_view()
    assert [s.address.host for s in w.model.servers()] == ["1.1.1.1"]
