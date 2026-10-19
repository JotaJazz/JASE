import asyncio

import keyring
import pytest
from keyring.backend import KeyringBackend
from keyring.errors import KeyringError, PasswordDeleteError
from PySide6.QtWidgets import QDialog, QMessageBox

from jase import passwords
from jase.config import Config, load_config
from jase.games import GAMES
from jase.launcher import LaunchError
from jase.models import ServerAddress, ServerInfo
from jase.scanner import ScanResult
from jase.ui import dialogs, main_window
from jase.ui.dialogs import PasswordDialog
from jase.ui.main_window import MainWindow
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

    def __init__(self, servers_by_game, delays=None, error=None, no_master=()):
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
        for info in servers:
            on_server(info)
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


@pytest.fixture
def make_window(qtbot, tmp_path):
    windows = []

    def factory(scan, launch=None, cfg=None):
        w = MainWindow(cfg or Config(), tmp_path / "config.json", scan_fn=scan,
                       launch_fn=launch or FakeLaunch())
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
    assert cfg.game_paths == {"cod2": r"D:\Juegos\CoD2", "cod1": r"C:\CoD"}


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
