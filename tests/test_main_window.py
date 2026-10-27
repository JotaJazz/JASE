import asyncio
import os
import sys
import time

import keyring
import pytest
from keyring.errors import KeyringError
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QDialog, QLabel, QLineEdit, QMessageBox, QPushButton

from conftest import MemKeyring
from jase import passwords
from jase.config import Config, load_config
from jase.games import GAMES
from jase.launcher import LaunchError
from jase.detect import Finding
from jase.models import Player, ServerAddress, ServerInfo
from jase.scanner import ScanResult
from jase.ui import dialogs, main_window, theme
from jase.ui.dialogs import PasswordDialog, SettingsDialog
from jase.ui.find_games import FindGamesDialog
from jase.ui.main_window import MainWindow
from jase.ui.player_search import PlayerSearchDialog
from jase.ui.server_model import COL_MAP, COL_MODE, COL_NAME, COL_PING, COL_PLAYERS, COLUMNS
from jase.ui.scan_worker import ScanWorker
from jase.ui.slot_watch import SlotWatcher


class BrokenSetKeyring(MemKeyring):
    def set_password(self, s, u, p):
        raise KeyringError("boom")


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
        self.kwargs = kw
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

    def factory(scan, launch=None, cfg=None, map_service=None, exe_icon_fn=None,
                steam_key=""):
        w = MainWindow(cfg or Config(), tmp_path / "config.json", scan_fn=scan,
                       launch_fn=launch or FakeLaunch(),
                       map_service=map_service or FakeMaps(), exe_icon_fn=exe_icon_fn,
                       steam_key=steam_key)
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
    return {gid: w.game_tree.game_item(gid).text(0) for gid in w.game_ids}


def select_first_row(w):
    w.table.selectRow(0)
    assert w.selected_server() is not None


def select_row_of(w, host):
    for row in range(w.proxy.rowCount()):
        if host in w.proxy.index(row, COL_NAME).data():
            w.table.selectRow(row)
            return
    raise AssertionError(host)


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


def _cod2x_servers():
    return [mk("1.1.1.1", "1.3"), mk("2.2.2.2", "1.4"), mk("3.3.3.3", "1.4")]


def test_version_combo_shows_cod2x_label_and_filters(make_window, qtbot, tmp_path):
    w = make_window(FakeScan({"cod2": _cod2x_servers()}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 3)
    labels = [w.version_combo.itemText(i) for i in range(w.version_combo.count())]
    assert "1.4 (CoD2x)" in labels and "1.4" not in labels
    w.version_combo.setCurrentText("1.4 (CoD2x)")
    assert w.proxy.rowCount() == 2
    assert load_config(tmp_path / "config.json").version_filter["cod2"] == "1.4"


def test_version_combo_cod2x_label_english(make_window, qtbot):
    from jase import i18n
    i18n.set_language("en")
    try:
        w = make_window(FakeScan({"cod2": _cod2x_servers()}))
        w.select_game("cod2")
        qtbot.waitUntil(lambda: w.model.rowCount() == 3)
        labels = [w.version_combo.itemText(i) for i in range(w.version_combo.count())]
        assert "1.4 (CoD2x)" in labels
    finally:
        i18n.set_language("es")


def test_saved_cod2x_filter_selects_label(make_window, qtbot):
    cfg = Config()
    cfg.version_filter["cod2"] = "1.4"
    w = make_window(FakeScan({"cod2": _cod2x_servers()}), cfg=cfg)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 3)
    assert w.version_combo.currentText() == "1.4 (CoD2x)"
    assert w.proxy.rowCount() == 2


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
    assert w.ui_actions["connect"].isEnabled()
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
    assert not w.ui_actions["connect"].isEnabled()
    w.connect_selected()
    assert boxes and boxes[0][0] == "exec" and "Abrir ajustes" in boxes[0][3]
    assert launch.calls == []


def test_steam_game_connects_without_path(make_window, qtbot, boxes):
    launch = FakeLaunch()
    w = make_window(FakeScan({"l4d2": [mk("3.3.3.3", "2.2.4.3", port=27015)]}), launch,
                    steam_key="K")
    w.select_game("l4d2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    assert w.ui_actions["connect"].isEnabled()
    w.connect_selected()
    assert not boxes
    assert launch.calls and launch.calls[0][0].id == "l4d2" and launch.calls[0][1] is None


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


def test_steam_launch_error_has_no_path_hint(make_window, qtbot, boxes):
    launch = FakeLaunch(error=LaunchError("No se pudo abrir Steam. ¿Está instalado?"))
    w = make_window(FakeScan({"l4d2": [mk("3.3.3.3", "2.2.4.3", port=27015)]}), launch,
                    steam_key="K")
    w.select_game("l4d2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    w.connect_selected()
    assert len(boxes) == 1 and boxes[0][0] == "warning"
    assert "Steam" in boxes[0][2] and "ruta" not in boxes[0][2]


def test_theme_choice_persists(make_window, qtbot, tmp_path):
    w = make_window(FakeScan({}))
    seen = []
    for tid in ("dark", "classic", "system"):
        w.theme_actions[tid].trigger()
        seen.append(load_config(tmp_path / "config.json").theme)
    assert seen == ["dark", "classic", "system"]


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
    d.path_edits["coduo"].setText("")
    d.apply_to(cfg)
    # ut99 tiene ruta por defecto: se guarda vacía para que no vuelva al cargar.
    assert cfg.game_paths == {"cod2": r"D:\Juegos\CoD2", "cod1": r"C:\CoD", "ut99": "",
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


def test_settings_status_steam_launch_game_needs_no_folder(qtbot, tmp_path):
    d = dialogs.SettingsDialog(Config(game_paths={}))
    qtbot.addWidget(d)
    label = d.findChild(dialogs.QLabel, "path_status_l4d2")
    assert label.text() == "Se conecta mediante Steam; no necesita carpeta"
    d.path_edits["l4d2"].setText(str(tmp_path))  # con ruta: la comprobación normal
    qtbot.waitUntil(lambda: label.text().startswith("\u26a0"))
    assert "left4dead2" in label.text()


def test_settings_status_names_openmohaa_launcher(qtbot, tmp_path):
    (tmp_path / "MOHAA.exe").write_bytes(b"")
    (tmp_path / "launch_openmohaa_base.exe").write_bytes(b"")
    d = dialogs.SettingsDialog(Config(game_paths={"mohaa": str(tmp_path)}))
    qtbot.addWidget(d)
    label = d.findChild(dialogs.QLabel, "path_status_mohaa")
    assert label.text() == "\u2713 launch_openmohaa_base.exe"


def test_settings_status_cs16_needs_cstrike_folder(qtbot, tmp_path):
    (tmp_path / "hl.exe").write_bytes(b"")
    d = dialogs.SettingsDialog(Config(game_paths={"cs16": str(tmp_path)}))
    qtbot.addWidget(d)
    label = d.findChild(dialogs.QLabel, "path_status_cs16")
    assert label.text().startswith("\u26a0") and "cstrike" in label.text()
    full = tmp_path / "full"
    (full / "cstrike").mkdir(parents=True)
    (full / "hl.exe").write_bytes(b"")
    d.path_edits["cs16"].setText(str(full))
    qtbot.waitUntil(lambda: label.text() == "\u2713 hl.exe")


def test_settings_game_paths_scroll_on_small_screens(qtbot):
    """Con 11 juegos el formulario va en un QScrollArea y el diálogo entra en 768 px."""
    from PySide6.QtWidgets import QScrollArea
    d = dialogs.SettingsDialog(Config())
    qtbot.addWidget(d)
    area = d.findChild(QScrollArea, "game_paths_scroll")
    assert area is not None and area.widgetResizable()
    for game_id in GAMES:
        assert area.widget().findChild(dialogs.QLabel, f"path_status_{game_id}") is not None
    assert d.sizeHint().height() <= 700


def test_settings_status_is_debounced(qtbot, monkeypatch):
    calls = []
    monkeypatch.setattr(dialogs, "resolve_game_exe", lambda g, d: calls.append(d) or None)
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
    assert w.ui_actions["connect"].isEnabled()
    w.filter_edit.setText("zzz")
    assert w.proxy.rowCount() == 0
    assert not w.ui_actions["connect"].isEnabled()
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
    w.set_theme("light")  # vuelve a pintar los detalles
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
    w.cfg.game_paths["cod2"] = "/x"  # «Conectar» exige ruta configurada
    select_first_row(w)
    menu = w._build_context_menu()
    texts = [a.text() for a in menu.actions() if not a.isSeparator()]
    assert texts == ["Conectar", "Copiar IP:puerto", "Copiar enlace jase://",
                     "Refrescar este servidor",
                     "Agregar a favoritos", "Editar nota…", "Grupo", "Avisar cuando haya sitio",
                     "Avisar cuando se arme partida…", "Agregar servidor…"]
    # «Avisar» solo se habilita con un servidor lleno (el de prueba no lo está).
    assert all(a.isEnabled() for a in menu.actions()
               if a.text() not in ("Avisar cuando haya sitio", "Editar nota…", "Grupo"))
    w.table.clearSelection()
    menu = w._build_context_menu()
    # Sin selección solo queda «Agregar servidor…», que no depende de la fila.
    assert [a.text() for a in menu.actions()
            if not a.isSeparator() and a.isEnabled()] == ["Agregar servidor…"]


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
    w.cfg.game_paths["cod2"] = "/x"
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    select_first_row(w)
    blank = QPoint(5, w.table.viewport().height() - 3)
    assert not w.table.indexAt(blank).isValid()
    w._on_table_context_menu(blank)
    assert w.selected_server() is None
    # Incluye «Agregar a favoritos», «Editar nota» y «Avisar»; «Agregar servidor…» siempre.
    assert seen == [[False] * 9 + [True]]
    row = QPoint(5, w.table.visualRect(w.proxy.index(0, 0)).center().y())
    w._on_table_context_menu(row)
    assert w.selected_server() is not None
    # «Editar nota»: solo favoritos; «Avisar cuando haya sitio»: lleno; «se arme partida»: siempre.
    assert seen[-1] == [True, True, True, True, True, False, False, False, True, True]


# ------------------------------------------- barra lateral: juegos con ruta primero
def _sidebar_names(w):
    return [w.game_tree.topLevelItem(i).text(0).split(" · ")[0]
            for i in range(w.game_tree.topLevelItemCount())]


STEAM_LAUNCH = [gid for gid in GAMES if GAMES[gid].launch_style == "steam"]


def _then_rest(*first):
    """Los juegos dados y los que conectan por Steam (listos sin ruta) primero, en el
    orden de GAMES; luego el resto."""
    ready = [gid for gid in GAMES if gid in first or gid in STEAM_LAUNCH]
    return [*ready, *(gid for gid in GAMES if gid not in ready)]


def test_sidebar_configured_games_first(make_window):
    w = make_window(FakeScan({}), cfg=Config(game_paths={"cod1": r"C:\CoD"}))
    assert w.game_ids == _then_rest("cod1")
    assert _sidebar_names(w) == [GAMES[gid].short_name for gid in _then_rest("cod1")]


def test_sidebar_without_paths_steam_launch_games_first(make_window):
    w = make_window(FakeScan({}), cfg=Config(game_paths={}))
    assert STEAM_LAUNCH and w.game_ids[:len(STEAM_LAUNCH)] == STEAM_LAUNCH
    assert w.game_ids == _then_rest()


def test_sidebar_default_config_order(make_window):
    w = make_window(FakeScan({}))
    assert w.game_ids == _then_rest("cod2", "ut99", "mohaa")


def test_sidebar_resorts_when_paths_change_and_keeps_selection(make_window, monkeypatch):
    w = make_window(FakeScan({}), cfg=Config(game_paths={}))
    w.select_game("ut99")
    assert w.game_tree.currentItem() is w.game_tree.game_item("ut99")

    class FakeDialog:
        def __init__(self, cfg, parent=None, **kwargs):
            pass

        def exec(self):
            return 1

        def apply_to(self, cfg):
            cfg.game_paths = {"ut99": r"D:\UT"}

        def steam_key(self):
            return ""

        def deleteLater(self):
            pass

    monkeypatch.setattr("jase.ui.main_window.SettingsDialog", FakeDialog)
    w.open_settings()
    assert w.game_ids == _then_rest("ut99")
    assert w.current_game == "ut99"
    assert w.game_tree.currentItem() is w.game_tree.topLevelItem(0)
    assert _sidebar_names(w)[0] == "Unreal Tournament"
    # Tras ut99 y los juegos de Steam viene cod2 en el nuevo orden.
    w.game_tree.game_activated.emit(w.game_ids[1 + len(STEAM_LAUNCH)])
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
    w.game_tree.game_activated.emit("cod2")
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
    # La dirección guardada es el puerto de consulta: no se lanza contra ella.
    assert not w.ui_actions["connect"].isEnabled()
    w.table.doubleClicked.emit(w.proxy.index(0, COL_NAME))
    assert launch.calls == []
    assert w.statusBar().currentMessage() == "El servidor no responde; refresca para conectar"
    w.statusBar().clearMessage()
    w._connect_if_selected()  # Enter
    assert launch.calls == []
    assert w.statusBar().currentMessage() == "El servidor no responde; refresca para conectar"
    # Si responde (p. ej. tras refrescar), se puede conectar.
    w.model.add_or_update(mk("3.3.3.3"))
    assert w.selected_server().online and w.ui_actions["connect"].isEnabled()
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
    w.game_tree.game_activated.emit("cod2")
    assert not w.in_favorites_view()
    assert [s.address.host for s in w.model.servers()] == ["1.1.1.1"]


# ------------------------------------------- barra lateral: nombres cortos sin scroll
def test_sidebar_has_no_horizontal_scrollbar_and_short_labels(make_window):
    w = make_window(FakeScan({}), cfg=Config(game_paths={}))
    assert w.game_tree.horizontalScrollBarPolicy() == Qt.ScrollBarAlwaysOff
    assert w.game_tree.textElideMode() == Qt.ElideMiddle
    for gid in w.game_ids:
        item = w.game_tree.game_item(gid)
        assert item.text(0) == f"{GAMES[gid].short_name} · —"
        assert item.toolTip(0) == GAMES[gid].name
    w._cache["mohaab"] = []
    w._update_sidebar()
    assert sidebar_texts(w)["mohaab"] == "MOHAA: Breakthrough · 0"


@pytest.mark.skipif(
    sys.platform == "win32" and os.environ.get("QT_QPA_PLATFORM") == "offscreen",
    reason="offscreen en Windows usa una fuente de respaldo con métricas no representativas")
def test_sidebar_default_width_fits_labels(make_window, qtbot):
    """Con el ancho por defecto, cada fila (nombre corto + conteo) entra entera."""
    w = make_window(FakeScan({}), cfg=Config(game_paths={}))
    for gid in w.game_ids:
        w._cache[gid] = [mk("1.1.1.1")] * 1234  # conteo de 4 cifras
    w._update_sidebar()
    w.resize(1280, 800)
    w.show()
    qtbot.waitExposed(w)
    assert w.body_splitter.sizes()[0] >= 220
    tree = w.game_tree
    metrics = tree.fontMetrics()
    # menos la sangría del nodo y el icono (sin icono si no hay)
    available = tree.viewport().width() - tree.indentation() - tree.iconSize().width()
    for i in range(tree.topLevelItemCount()):
        text = tree.topLevelItem(i).text(0)
        assert metrics.horizontalAdvance(text) + 12 <= available, text


def test_server_found_updates_only_current_sidebar_row(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")]}, progress_hold=0.5)
    w = make_window(scan)
    w.select_game("cod2")
    other = w.game_tree.game_item("cod1")
    other.setText(0, "centinela")
    qtbot.waitUntil(lambda: sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2")
    assert w.is_scanning()
    assert other.text(0) == "centinela"  # server_found no reescribe las demás filas
    qtbot.waitUntil(lambda: not w.is_scanning())
    assert other.text(0) == "Call of Duty · —"  # al terminar, refresco completo


def test_country_column_width_and_version_tooltip_in_favorites(make_window, qtbot):
    from jase.ui.server_model import COL_COUNTRY
    w, _scan = favorites_window(make_window, qtbot, FakeQuery(), ["2.2.2.2:28960"])
    assert w.table.horizontalHeader().sectionSize(COL_COUNTRY) == 48
    assert w.version_combo.toolTip() == "Versión del juego"
    w.show_favorites()
    assert not w.version_combo.isEnabled()
    assert w.version_combo.toolTip() == "No aplica en Favoritos"
    qtbot.waitUntil(lambda: not w._workers)
    w.select_game("cod2")
    assert w.version_combo.isEnabled() and w.version_combo.toolTip() == "Versión del juego"


# ------------------------------------------------ árbol de mods y mapas
def mkm(host, map_, mod=""):
    info = mk(host)
    info.map = map_
    info.rules = {"fs_game": mod} if mod else {}
    return info


def leaf(w, gid, folder, value):
    """Hoja marcable `value` dentro de la carpeta `folder` ("Mods" o "Mapas") del juego."""
    item = w.game_tree.game_item(gid)
    for i in range(item.childCount()):
        fo = item.child(i)
        if fo.text(0) == folder:
            for j in range(fo.childCount()):
                if fo.child(j).data(0, Qt.UserRole) == value:
                    return fo.child(j)
    return None


def tree_servers():
    return [mkm("1.1.1.1", "mp_a", "mods/pam"), mkm("2.2.2.2", "mp_a"),
            mkm("3.3.3.3", "mp_b", "mods/pam")]


def test_tree_shows_mods_and_maps_after_scan(make_window, qtbot):
    w = make_window(FakeScan({"cod2": tree_servers()}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    assert leaf(w, "cod2", "Mods", "pam").text(0) == "pam · 2"
    assert leaf(w, "cod2", "Mapas", "mp_a").text(0) == "mp_a · 2"


def test_marking_map_filters_table(make_window, qtbot):
    w = make_window(FakeScan({"cod2": tree_servers()}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    assert w.proxy.rowCount() == 1
    assert w.count_label.text() == "1 servidor · filtro de mod/mapa activo"
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Unchecked)
    assert w.proxy.rowCount() == 3
    assert w.count_label.text() == "3 servidores"


def test_marks_are_per_game_and_restored_on_switch(make_window, qtbot):
    w = make_window(FakeScan({"cod2": tree_servers(), "ut99": [mk("9.9.9.9")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    w.select_game("ut99")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    assert w.proxy.rowCount() == 1
    assert "filtro" not in w.count_label.text()
    w.select_game("cod2")
    assert w.proxy.rowCount() == 1 and w.model.rowCount() == 3
    assert w.game_tree.marks("cod2") == (frozenset(), frozenset({"mp_b"}))


def test_tree_marks_survive_live_scan(make_window, qtbot):
    scan = FakeScan({"cod2": tree_servers()})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    scan.delays["cod2"] = 0.5
    w.refresh()                       # el modelo queda vacío y el escaneo en curso
    assert w.is_scanning()
    assert w.game_tree.marks("cod2") == (frozenset(), frozenset({"mp_b"}))
    assert leaf(w, "cod2", "Mapas", "mp_b").text(0) == "mp_b · 0"
    qtbot.waitUntil(lambda: not w.is_scanning())
    assert w.game_tree.marks("cod2") == (frozenset(), frozenset({"mp_b"}))
    assert leaf(w, "cod2", "Mapas", "mp_b").text(0) == "mp_b · 1"
    assert w.proxy.rowCount() == 1


def test_marked_leaf_gone_after_refresh_drops_mark(make_window, qtbot):
    scan = FakeScan({"cod2": tree_servers()})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    scan.servers_by_game["cod2"] = [mkm("1.1.1.1", "mp_a"), mkm("2.2.2.2", "mp_a")]
    scan.delays["cod2"] = 0.3
    w.refresh()
    assert leaf(w, "cod2", "Mapas", "mp_b").text(0) == "mp_b · 0"
    qtbot.waitUntil(lambda: not w.is_scanning())
    assert w.game_tree.marks("cod2") == (frozenset(), frozenset())
    assert leaf(w, "cod2", "Mapas", "mp_b") is None
    assert w.proxy.rowCount() == 2
    assert "filtro" not in w.count_label.text()


def test_favorites_view_bypasses_tree_marks(make_window, qtbot):
    w, _scan = favorites_window(make_window, qtbot, FakeQuery(), ["2.2.2.2:28960"],
                                servers=[mkm("1.1.1.1", "mp_a"), mkm("4.4.4.4", "mp_b")])
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    assert w.proxy.rowCount() == 1
    w.show_favorites()
    qtbot.waitUntil(lambda: not w._workers)
    assert w.proxy.rowCount() == 1  # el favorito (sin mapa) se ve pese a la marca
    assert w.model.servers()[0].address.host == "2.2.2.2"
    assert w.count_label.text().endswith(" · filtro de mod/mapa (no aplica en Favoritos)")
    assert "activo" not in w.count_label.text()
    w.select_game("cod2")
    assert w.proxy.rowCount() == 1
    assert "filtro de mod/mapa activo" in w.count_label.text()


def test_count_label_mentions_tree_filter(make_window, qtbot):
    w = make_window(FakeScan({"cod2": tree_servers()}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    assert "filtro" not in w.count_label.text()
    leaf(w, "cod2", "Mods", "pam").setCheckState(0, Qt.Checked)
    assert w.count_label.text().endswith(" · filtro de mod/mapa activo")
    assert w.proxy.rowCount() == 2


def test_double_refresh_keeps_live_mark(make_window, qtbot):
    scan = FakeScan({"cod2": tree_servers()})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    scan.servers_by_game["cod2"] = [mkm("1.1.1.1", "mp_new")]
    scan.progress_hold = 5
    w.refresh()
    qtbot.waitUntil(lambda: leaf(w, "cod2", "Mapas", "mp_new") is not None)
    leaf(w, "cod2", "Mapas", "mp_new").setCheckState(0, Qt.Checked)
    w.refresh()                       # segundo F5 antes de terminar el primero
    assert w.game_tree.marks("cod2") == (frozenset(), frozenset({"mp_new"}))
    w._stop_active_scan()
    assert w.game_tree.marks("cod2") == (frozenset(), frozenset({"mp_new"}))


def test_failed_scan_keeps_mark_and_shows_model_breakdown(make_window, qtbot):
    scan = FakeScan({"cod2": tree_servers()})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 3)
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    scan.error = RuntimeError("explotó")
    w.refresh()
    qtbot.waitUntil(lambda: not w.is_scanning())
    assert w.game_tree.marks("cod2") == (frozenset(), frozenset({"mp_b"}))
    assert leaf(w, "cod2", "Mapas", "mp_b").text(0) == "mp_b · 0"
    assert leaf(w, "cod2", "Mapas", "mp_a") is None  # no el desglose de la caché vieja


# --------------------------------------------------- acciones, menú y barra

def test_toolbar_has_action_groups_in_order(make_window):
    w = make_window(FakeScan({}))
    items = []
    for a in w.toolbar.actions():
        if a.isSeparator():
            items.append("|")
        elif w.toolbar.widgetForAction(a) is w.filter_edit:
            items.append("filter")
        elif w.toolbar.widgetForAction(a) is w.version_combo:
            items.append("version")
        elif w.toolbar.widgetForAction(a) is w.group_combo:
            items.append("group")
        else:
            items.append(next(n for n, x in w.ui_actions.items() if x is a))
    assert items == ["refresh", "stop", "refresh_selected", "|", "connect", "favorite",
                     "copy", "|", "settings", "theme", "filter", "version", "group"]
    assert not w.toolbar.isMovable()
    assert w.toolbar.toolButtonStyle() == Qt.ToolButtonIconOnly
    assert w.toolbar.iconSize().width() == 20


def test_menus_titles(make_window):
    w = make_window(FakeScan({}))
    titles = [a.text().replace("&", "") for a in w.menuBar().actions()]
    assert titles == ["Archivo", "Ver", "Filtro", "Servidor", "Ayuda"]


def test_stop_action_cancels_scan(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}, delays={"cod2": 5}))
    assert not w.ui_actions["stop"].isEnabled()
    w.select_game("cod2")
    assert w.is_scanning() and w.ui_actions["stop"].isEnabled()
    w.ui_actions["stop"].trigger()
    assert not w.is_scanning()
    assert not w.ui_actions["stop"].isEnabled()


def test_actions_follow_selection(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning())
    keys = ("refresh_selected", "favorite", "copy")
    assert not any(w.ui_actions[k].isEnabled() for k in keys)
    select_first_row(w)
    assert all(w.ui_actions[k].isEnabled() for k in keys)
    assert w.ui_actions["favorite"].text() == "Agregar a favoritos"
    w.ui_actions["favorite"].trigger()
    assert w.ui_actions["favorite"].text() == "Quitar de favoritos"
    w.table.clearSelection()
    assert not any(w.ui_actions[k].isEnabled() for k in keys)


def test_context_menu_reuses_actions(make_window, qtbot):
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1")], FakeQuery())
    select_first_row(w)
    menu = w._build_context_menu()
    shared = [w.ui_actions[k] for k in ("connect", "copy", "copy_link", "refresh_selected", "favorite",
                                                  "edit_note", "watch_slot", "party_alert", "add_server")]
    shared.insert(6, w.group_menu.menuAction())
    assert [a for a in menu.actions() if not a.isSeparator()] == shared


def test_tool_icons_rerender_on_theme(make_window):
    w = make_window(FakeScan({}))
    w.cfg.theme = "dark"
    w.apply_theme()
    dark = w.ui_actions["refresh"].icon().pixmap(20, 20).toImage()
    w.cfg.theme = "light"
    w.apply_theme()
    light = w.ui_actions["refresh"].icon().pixmap(20, 20).toImage()
    assert dark != light
    assert w.ui_actions["theme"].toolTip() == "Tema: Claro"


def test_quick_filter_actions_mirror_checkboxes(make_window):
    w = make_window(FakeScan({}))
    act = w.ui_actions["hide_empty"]
    assert act.isCheckable() and act.isChecked() == w.hide_empty_check.isChecked()
    act.trigger()
    assert w.hide_empty_check.isChecked() == act.isChecked()
    w.hide_empty_check.setChecked(not act.isChecked())
    assert w.hide_empty_check.isChecked() == act.isChecked()


def test_clear_tree_marks_action_clears_current_game(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning())
    calls = []
    w.game_tree.clear_marks = calls.append
    # sin marcas la acción está desactivada
    assert not w.ui_actions["clear_tree_marks"].isEnabled()
    w._tree_marks = lambda: (frozenset({"m"}), frozenset())
    w._update_actions()
    w.ui_actions["clear_tree_marks"].trigger()
    assert calls == ["cod2"]


def test_find_games_action_enabled(make_window):
    assert make_window(FakeScan({})).ui_actions["find_games"].isEnabled()


def test_about_shows_package_version(make_window, monkeypatch):
    seen = []
    monkeypatch.setattr(main_window.QMessageBox, "about",
                        lambda parent, title, text: seen.append(text))
    w = make_window(FakeScan({}))
    w.ui_actions["about"].trigger()
    assert seen and "Jimmy" in seen[0] and main_window.app_version() in seen[0]


def test_window_fits_768(make_window):
    assert make_window(FakeScan({})).minimumSizeHint().height() <= 768


# ---------------------------------------------------------------- iconos de juego
def _icon_cfg(tmp_path, game_id="q3"):
    from jase.games import GAMES
    install = tmp_path / game_id
    install.mkdir()
    (install / GAMES[game_id].exe_relpath).write_bytes(b"")
    cfg = Config()
    cfg.game_paths = {game_id: str(install)}
    return cfg


def _tree_icon_key(w, game_id):
    return w.game_tree._items[game_id].icon(0).cacheKey()


def test_tree_uses_badge_without_path(make_window):
    w = make_window(FakeScan({}), cfg=Config())
    w.cfg.game_paths = {}
    w._refresh_game_icons()
    assert not w.game_tree._items["q3"].icon(0).isNull()
    assert w.game_tree._items["q3"].icon(0).cacheKey() == icons_badge("q3", w._theme_id).cacheKey()


def icons_badge(game_id, mode):
    from jase.ui import icons
    return icons.badge_icon(game_id, mode)


def test_tree_uses_exe_icon_when_resolved(make_window, qtbot, tmp_path):
    from PySide6.QtGui import QIcon, QPixmap
    marked = QIcon(QPixmap(16, 16))
    seen = []

    def exe_fn(path):
        seen.append(path)
        return marked

    cfg = _icon_cfg(tmp_path)
    w = make_window(FakeScan({}), cfg=cfg, exe_icon_fn=exe_fn)
    qtbot.waitUntil(lambda: _tree_icon_key(w, "q3") == marked.cacheKey(), timeout=3000)
    assert seen and seen[0].endswith(GAMES["q3"].exe_relpath)
    # Sin exe en una generación posterior, vuelve el badge.
    w.cfg.game_paths = {}
    w._refresh_game_icons()
    assert _tree_icon_key(w, "q3") == icons_badge("q3", w._theme_id).cacheKey()


def test_stale_exe_icon_result_discarded(make_window, qtbot, tmp_path):
    from PySide6.QtGui import QIcon, QPixmap
    marked = QIcon(QPixmap(16, 16))
    w = make_window(FakeScan({}), cfg=Config(), exe_icon_fn=lambda p: marked)
    w.cfg.game_paths = {}
    w._refresh_game_icons()
    w._refresh_game_icons()
    exe = str(tmp_path / "x.exe")
    w._on_exe_icon_resolved(w._icon_generation - 1, "q3", exe)  # generación vieja
    assert _tree_icon_key(w, "q3") == icons_badge("q3", w._theme_id).cacheKey()
    w._on_exe_icon_resolved(w._icon_generation, "q3", exe)
    assert _tree_icon_key(w, "q3") == marked.cacheKey()


def _touch(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"")
    return path


def _fake_find(result):
    return lambda **kw: result


def _accepting(monkeypatch, qtbot):
    def fake_exec(self):
        qtbot.waitUntil(lambda: bool(self.rows))
        return QDialog.DialogCode.Accepted
    monkeypatch.setattr(FindGamesDialog, "exec", fake_exec)


async def no_scan(*a, **k):
    raise AssertionError("no scan expected")


def test_main_find_games_applies_and_resorts(make_window, qtbot, monkeypatch, tmp_path):
    _touch(tmp_path / "g" / "CoD2MP_s.exe")
    cfg = Config()
    cfg.game_paths = {}
    w = make_window(no_scan, cfg=cfg)
    w._find_fn = _fake_find({"cod2": [Finding("cod2", str(tmp_path / "g"), "steam")]})
    _accepting(monkeypatch, qtbot)
    refreshed = []
    monkeypatch.setattr(w, "_refresh_game_icons", lambda: refreshed.append(1))
    w.find_games()
    assert cfg.game_paths["cod2"] == str(tmp_path / "g")
    assert w.game_ids[0] == "cod2" and refreshed
    assert load_config(w.cfg_path).game_paths["cod2"] == str(tmp_path / "g")


def test_first_run_offer_only_once(make_window, boxes, monkeypatch):
    cfg = Config()
    cfg.game_paths = {}
    w1 = make_window(no_scan, cfg=cfg)
    w2 = make_window(no_scan, cfg=cfg)
    w1.offer_find_games()
    w2.offer_find_games()
    questions = [c for c in boxes if c[0] == "exec"]
    assert len(questions) == 1
    assert "¿Buscar juegos instalados?" in questions[0][2]
    assert questions[0][3] == ["Buscar juegos", "Ahora no"]  # sin Yes/No en inglés
    assert cfg.find_games_offered
    assert load_config(w1.cfg_path).find_games_offered


def test_no_offer_when_a_game_is_configured(make_window, boxes, tmp_path):
    _touch(tmp_path / "g" / "CoD2MP_s.exe")
    cfg = Config()
    cfg.game_paths = {"cod2": str(tmp_path / "g")}
    w = make_window(no_scan, cfg=cfg)
    w.offer_find_games()
    assert not [c for c in boxes if c[0] == "exec"]
    assert cfg.find_games_offered  # se marca igual


def _click_button(monkeypatch, text):
    monkeypatch.setattr(QMessageBox, "exec", lambda self: 0)
    monkeypatch.setattr(
        QMessageBox, "clickedButton",
        lambda self: next(b for b in self.buttons() if b.text() == text))


def test_first_run_accept_finds_and_saves(make_window, qtbot, monkeypatch, tmp_path):
    _touch(tmp_path / "g" / "CoD2MP_s.exe")
    cfg = Config()
    cfg.game_paths = {}
    w = make_window(no_scan, cfg=cfg)
    w._find_fn = _fake_find({"cod2": [Finding("cod2", str(tmp_path / "g"), "steam")]})
    _accepting(monkeypatch, qtbot)
    _click_button(monkeypatch, "Buscar juegos")
    w.offer_find_games()
    assert cfg.game_paths["cod2"] == str(tmp_path / "g")
    assert load_config(w.cfg_path).game_paths["cod2"] == str(tmp_path / "g")


def test_first_run_decline_does_not_search(make_window, monkeypatch):
    cfg = Config()
    cfg.game_paths = {}
    w = make_window(no_scan, cfg=cfg)
    _click_button(monkeypatch, "Ahora no")
    monkeypatch.setattr(w, "find_games", lambda: pytest.fail("no debía buscar"))
    w.offer_find_games()


def test_paths_changed_helper_runs_whole_sequence(make_window, monkeypatch):
    w = make_window(no_scan, cfg=Config())
    calls = []
    for name in ("_save_config", "_resort_sidebar", "_refresh_game_icons", "_update_actions"):
        monkeypatch.setattr(w, name, lambda n=name: calls.append(n))
    w._card_key = ("x",)
    w._after_game_paths_changed()
    assert w._card_key is None
    assert calls == ["_save_config", "_resort_sidebar", "_refresh_game_icons",
                     "_update_actions"]


def test_stopped_scan_tree_count_uses_table_rows(make_window, qtbot):
    scan = FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")]})
    scan.progress_hold = 5
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    w.ui_actions["stop"].trigger()
    assert w.model.rowCount() == 2
    assert sidebar_texts(w)["cod2"] == "Call of Duty 2 · 2"


def test_marks_in_favorites_show_notice(make_window, qtbot):
    w, _scan = favorites_window(make_window, qtbot, FakeQuery(), ["2.2.2.2:28960"],
                                servers=[mkm("1.1.1.1", "mp_a"), mkm("4.4.4.4", "mp_b")])
    w.show_favorites()
    qtbot.waitUntil(lambda: not w._workers)
    assert "filtro" not in w.count_label.text()
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    assert w.count_label.text().endswith(" · filtro de mod/mapa (no aplica en Favoritos)")


def test_clear_marks_action_follows_marks_and_view(make_window, qtbot):
    w, _scan = favorites_window(make_window, qtbot, FakeQuery(), ["2.2.2.2:28960"],
                                servers=[mkm("1.1.1.1", "mp_a"), mkm("4.4.4.4", "mp_b")])
    act = w.ui_actions["clear_tree_marks"]
    assert not act.isEnabled()
    leaf(w, "cod2", "Mapas", "mp_b").setCheckState(0, Qt.Checked)
    assert act.isEnabled()
    w.show_favorites()
    assert not act.isEnabled()
    w.select_game("cod2")
    assert act.isEnabled()
    act.trigger()
    assert not act.isEnabled() and w.game_tree.marks("cod2") == (frozenset(), frozenset())


def test_ping_menu_mirrors_combo(make_window):
    w = make_window(FakeScan({}))
    assert [a.text() for a in w.ping_actions] == [
        w.max_ping_combo.itemText(i) for i in range(w.max_ping_combo.count())]
    assert w.ping_actions[w.max_ping_combo.currentIndex()].isChecked()
    w.ping_actions[2].trigger()                      # menú -> combo
    assert w.max_ping_combo.currentIndex() == 2
    assert w.cfg.quick_filters.max_ping == w.max_ping_combo.currentData()
    w.max_ping_combo.setCurrentIndex(1)              # combo -> menú
    assert [a.isChecked() for a in w.ping_actions].count(True) == 1
    assert w.ping_actions[1].isChecked()


def test_ping_actions_disabled_in_favorites(make_window, qtbot):
    w, _scan = favorites_window(make_window, qtbot, FakeQuery(), ["2.2.2.2:28960"])
    assert all(a.isEnabled() for a in w.ping_actions)
    w.show_favorites()
    assert not any(a.isEnabled() for a in w.ping_actions)
    w.select_game("cod2")
    assert all(a.isEnabled() for a in w.ping_actions)


def test_tree_shows_bf1942_mod_and_connect_passes_it(make_window, qtbot):
    info = mk("1.1.1.1")
    info.rules = {"gameId": "DC_Final"}
    base = mk("2.2.2.2")
    base.rules = {"gameId": "bf1942"}
    launch = FakeLaunch()
    w = make_window(FakeScan({"bf1942": [info, base]}), launch)
    w.cfg.wine_command = ""  # fuera de Windows se agregaría `wrapper` (ver test_wine)
    w.cfg.game_paths["bf1942"] = r"D:\Juegos\BF1942"
    w.select_game("bf1942")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    assert leaf(w, "bf1942", "Mods", "dc_final").text(0) == "dc_final · 1"
    select_row_of(w, "1.1.1.1")
    w.connect_selected()
    assert launch.kwargs == {"mod": "DC_Final"}  # valor crudo del servidor
    select_row_of(w, "2.2.2.2")
    w.connect_selected()
    assert launch.kwargs == {}


# --- selector de idioma (Task 4)
def test_settings_language_combo_saves(qtbot):
    d = SettingsDialog(Config(language="en"))
    qtbot.addWidget(d)
    assert [d.language_combo.itemText(i) for i in range(3)] == [
        "Automático", "Español", "English"]
    assert d.language_combo.currentData() == "en"
    d.language_combo.setCurrentIndex(d.language_combo.findData("es"))
    cfg = Config()
    d.apply_to(cfg)
    assert cfg.language == "es"


def test_language_change_shows_restart_notice(make_window, monkeypatch, boxes):
    monkeypatch.setattr("jase.ui.main_window.system_locale_name", lambda: "es-CL")
    w = make_window(FakeScan({}))

    def fake_exec(self):
        self.language_combo.setCurrentIndex(self.language_combo.findData("en"))
        return QDialog.Accepted

    monkeypatch.setattr(SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert w.cfg.language == "en"
    assert len(boxes) == 1 and boxes[0][0] == "exec"
    assert boxes[0][2] == "El idioma cambiará al reiniciar JASE."
    assert boxes[0][3] == ["Entendido"]
    boxes.clear()
    w.open_settings()  # mismo valor: sin aviso
    assert boxes == []


@pytest.mark.parametrize("old, new, system, notice", [
    ("auto", "en", "en-US", False),   # auto ya resolvía a inglés
    ("auto", "es", "en-US", True),
    ("en", "auto", "en-US", False),
    ("auto", "es", "es-CL", False),
    ("auto", "en", "es-CL", True),
])
def test_restart_notice_compares_resolved_language(
        make_window, monkeypatch, boxes, old, new, system, notice):
    from jase.ui import main_window
    monkeypatch.setattr(main_window, "system_locale_name", lambda: system)
    w = make_window(FakeScan({}), cfg=Config(language=old))

    def fake_exec(self):
        self.language_combo.setCurrentIndex(self.language_combo.findData(new))
        return QDialog.Accepted

    monkeypatch.setattr(SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert w.cfg.language == new
    assert len(boxes) == (1 if notice else 0)


def test_favorites_query_status_singular_spanish(make_window, qtbot):
    from jase.models import ServerAddress  # noqa: F401
    w = make_window(FakeScan({"cod2": []}), cfg=Config(favorites={"cod2": ["2.2.2.2:28960"]}))
    w._query_fn = FakeQuery(delay=0.1)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning())
    messages = []
    w.statusBar().messageChanged.connect(messages.append)
    w.show_favorites()
    qtbot.waitUntil(lambda: not w._workers)
    assert "Consultando 1 favorito de Call of Duty 2…" in messages


# ---------------------------------------------------------------- temas
ALL_THEMES = ("system", "dark", "light", "neon", "brown", "green", "classic", "xp", "terminal",
              "synthwave", "rust", "goldsrc")


def test_theme_menu_has_twelve_exclusive_actions(make_window):
    from PySide6.QtWidgets import QToolButton
    w = make_window(FakeScan({}))
    menu = w.ui_actions["theme"].menu()
    assert menu is not None
    acts = [a for a in menu.actions() if not a.isSeparator() and a.menu() is None]
    assert [a.data() for a in acts] == list(ALL_THEMES)
    assert [a.text() for a in acts] == ["Sistema", "Oscuro", "Claro", "Neón", "Marrón",
                                        "Verde", "Clásico", "Windows XP", "Terminal",
                                        "Synthwave", "Óxido", "GoldSrc"]
    assert all(a.isCheckable() for a in acts)
    group = acts[0].actionGroup()
    assert group is not None and group.isExclusive()
    assert all(a.actionGroup() is group for a in acts)
    assert [a.data() for a in acts if a.isChecked()] == ["system"]
    # el mismo menú en Ver → Tema y en el botón de la barra
    view = next(a.menu() for a in w.menuBar().actions() if a.text().replace("&", "") == "Ver")
    assert w.ui_actions["theme"] in view.actions()
    button = w.toolbar.widgetForAction(w.ui_actions["theme"])
    assert isinstance(button, QToolButton)
    assert button.popupMode() == QToolButton.InstantPopup
    w.theme_actions["xp"].trigger()
    assert [a.data() for a in acts if a.isChecked()] == ["xp"]
    assert w.cfg.theme == "xp" and w._theme_id == "xp"


@pytest.mark.parametrize("tid", ALL_THEMES)
def test_every_theme_renders_window(make_window, qtbot, tid):
    from PySide6.QtWidgets import QApplication
    from dataclasses import replace
    servers = [replace(mk("1.1.1.1", name="Rojo Blanco"), raw_name="^1Rojo ^7Blanco"),
               mk("2.2.2.2", pw=True)]
    w = make_window(FakeScan({"cod2": servers}), cfg=Config(theme=tid))
    assert w._theme_id in theme.TOKENS
    assert QApplication.instance().styleSheet() == theme.build_stylesheet(
        w._theme_id)
    w.resize(1100, 700)
    w.show()
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2 and not w.is_scanning())
    select_first_row(w)
    assert w.server_card.name_label.text()
    menu = w._build_context_menu()
    assert menu.actions()
    image = w.grab().toImage()
    assert not image.isNull()
    w.game_tree.game_item("cod2").setExpanded(True)
    w.grab()


def test_switch_theme_repaints_icons_and_rows(make_window, qtbot):
    from jase.ui import icons, theme
    from jase.ui.colored_text import COLOR_RUNS_ROLE
    from dataclasses import replace
    info = replace(mk("1.1.1.1", name="Verde"), raw_name="^2Verde")
    w = make_window(FakeScan({"cod2": [info]}), cfg=Config(theme="dark"))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning())
    select_first_row(w)
    w.set_theme("synthwave")
    assert w._theme_id == "synthwave" and w.model._mode == "synthwave"
    assert w.occupancy_delegate.mode == "synthwave"
    want = icons.tool_icon("refresh", "synthwave").pixmap(20, 20).toImage()
    assert w.ui_actions["refresh"].icon().pixmap(20, 20).toImage() == want
    idx = w.model.index(0, COL_PING)
    assert w.model.data(idx, Qt.ForegroundRole) == theme.ping_color(10, "synthwave")
    runs = w.model.data(w.model.index(0, COL_NAME), COLOR_RUNS_ROLE)
    assert runs[0][1].name() == theme.Q3_COLORS["synthwave"]["2"]
    badge = icons.badge_icon("cod2", "synthwave").pixmap(16, 16).toImage()
    assert w.game_tree.game_item("cod2").icon(0).pixmap(16, 16).toImage() == badge
    assert theme.TOKENS["synthwave"]["text"] in w.server_card.name_label.text() or \
        theme.Q3_COLORS["synthwave"]["2"] in w.server_card.name_label.text()


def test_settings_appearance_tab_applies_theme(make_window, qtbot, monkeypatch, tmp_path):
    from PySide6.QtWidgets import QApplication
    seen = {}

    def fake_exec(self):
        tabs = [self.tabs.tabText(i) for i in range(self.tabs.count())]
        seen["tabs"] = tabs
        lst = self.theme_list
        seen["ids"] = [lst.item(i).data(Qt.UserRole) for i in range(lst.count())]
        seen["icons"] = all(not lst.item(i).icon().isNull() for i in range(lst.count()))
        seen["current"] = lst.currentItem().data(Qt.UserRole)
        lst.setCurrentRow(seen["ids"].index("classic"))
        return QDialog.Accepted

    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_exec)
    w = make_window(FakeScan({}), cfg=Config(theme="dark"))
    w.open_settings()
    assert seen["tabs"] == ["Juegos", "Masters", "General", "Apariencia"]
    assert seen["ids"] == list(ALL_THEMES) and seen["icons"]
    assert seen["current"] == "dark"
    assert w.cfg.theme == "classic" and w._theme_id == "classic"
    assert load_config(tmp_path / "config.json").theme == "classic"
    assert QApplication.instance().styleSheet() == theme.build_stylesheet("classic")
    assert w.theme_actions["classic"].isChecked()


def test_settings_theme_swatches_use_theme_colors(qtbot):
    from PySide6.QtGui import QColor
    from jase.ui import theme
    d = dialogs.SettingsDialog(Config(theme="neon"))
    qtbot.addWidget(d)
    lst = d.theme_list
    item = next(lst.item(i) for i in range(lst.count()) if lst.item(i).data(Qt.UserRole) == "neon")
    assert item.text() == "Neón"
    image = item.icon().pixmap(lst.iconSize()).toImage()
    colors = {QColor(image.pixel(x, image.height() // 2)).name()
              for x in range(image.width())}
    t = theme.TOKENS["neon"]
    assert {t["bg"], t["surface"], t["accent"], t["selection"]} <= colors
    assert lst.currentItem().data(Qt.UserRole) == "neon"
    cfg = Config()
    d.apply_to(cfg)
    assert cfg.theme == "neon"


def test_window_fits_768_in_classic(make_window):
    w = make_window(FakeScan({}), cfg=Config(theme="classic"))
    assert w._theme_id == "classic"
    assert w.minimumSizeHint().height() <= 768


def test_settings_fits_768_in_classic(make_window, qtbot):
    make_window(FakeScan({}), cfg=Config(theme="classic"))  # aplica la hoja de Clásico
    d = dialogs.SettingsDialog(Config(theme="classic"))
    qtbot.addWidget(d)
    assert d.sizeHint().height() <= 768


def test_system_theme_follows_color_scheme(make_window, monkeypatch):
    from PySide6.QtWidgets import QApplication
    w = make_window(FakeScan({}), cfg=Config(theme="system"))
    hints = QApplication.instance().styleHints()
    for scheme, want in ((Qt.ColorScheme.Light, "light"), (Qt.ColorScheme.Dark, "dark")):
        monkeypatch.setattr(type(hints), "colorScheme", lambda self, s=scheme: s)
        w._on_color_scheme_changed(scheme)
        assert w._theme_id == want
        assert QApplication.instance().styleSheet() == theme.build_stylesheet(want)
        assert w.theme_actions["system"].isChecked()


def test_cod2x_label_in_version_column_and_card(make_window, qtbot):
    from jase.ui.server_model import COL_VERSION
    w = make_window(FakeScan({"cod2": [mk("2.2.2.2", "1.4")]}))
    w.select_game("cod2")
    w.version_combo.setCurrentText("Todas")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    assert w.model.index(0, COL_VERSION).data() == "1.4 (CoD2x)"
    w._update_card(w.model.server_at(0))
    assert w.server_card.version_value.text() == "1.4 (CoD2x)"


class KeyScan:
    """scan_fn que registra el kwarg steam_key (o su ausencia)."""

    def __init__(self):
        self.calls = []

    async def __call__(self, game, masters, on_server, on_status, **kw):
        self.calls.append((game.id, kw.get("steam_key", "<absent>")))
        return ScanResult(masters_ok=1, servers_listed=0, servers_responded=0)


def test_steam_game_without_key_shows_hint(make_window, qtbot):
    scan = KeyScan()
    w = make_window(scan, steam_key="")
    w.select_game("l4d2")
    assert "Ajustes → General" in w.statusBar().currentMessage()
    assert w._worker is None and scan.calls == []


def test_scan_receives_steam_key(make_window, qtbot):
    scan = KeyScan()
    w = make_window(scan, steam_key="K")
    w.select_game("l4d2")
    qtbot.waitUntil(lambda: scan.calls == [("l4d2", "K")])


def test_non_steam_scan_gets_no_key(make_window, qtbot):
    scan = KeyScan()
    w = make_window(scan, steam_key="K")
    w.select_game("cod2")
    qtbot.waitUntil(lambda: scan.calls == [("cod2", "<absent>")])


def test_window_loads_steam_key_from_keyring_by_default(qtbot, tmp_path):
    passwords.set_steam_key("FROMRING")
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=KeyScan(),
                   launch_fn=FakeLaunch(), map_service=FakeMaps())
    qtbot.addWidget(w)
    assert w._steam_key == "FROMRING"
    w.close()


def test_settings_saves_new_steam_key_and_refreshes(make_window, qtbot, monkeypatch):
    scan = KeyScan()
    w = make_window(scan, steam_key="OLD")
    w.select_game("l4d2")
    qtbot.waitUntil(lambda: scan.calls == [("l4d2", "OLD")])
    qtbot.waitUntil(lambda: not w.is_scanning())

    def fake_exec(self):
        self.steam_key_edit.setText(" NEW ")
        return QDialog.Accepted

    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert passwords.get_steam_key() == "NEW" and w._steam_key == "NEW"
    qtbot.waitUntil(lambda: scan.calls[-1] == ("l4d2", "NEW"))


def test_settings_steam_key_keyring_error_uses_key_in_session(make_window, qtbot,
                                                              monkeypatch, boxes):
    keyring.set_keyring(BrokenSetKeyring())
    w = make_window(KeyScan(), steam_key="")

    def fake_exec(self):
        self.steam_key_edit.setText("NEW")
        return QDialog.Accepted

    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert w._steam_key == "NEW"
    assert any(b[0] == "warning" for b in boxes)


def test_settings_key_field_is_password_mode(qtbot):
    d = SettingsDialog(Config(), steam_key="ABC")
    qtbot.addWidget(d)
    assert d.steam_key_edit.echoMode() == QLineEdit.Password and d.steam_key() == "ABC"


def test_settings_key_link_opens_externally(qtbot):
    d = SettingsDialog(Config())
    qtbot.addWidget(d)
    link = d.findChild(QLabel, "steam_key_link")
    assert link is not None and link.openExternalLinks()
    assert 'href="https://steamcommunity.com/dev/apikey"' in link.text()
    assert "Consíguela" in link.text()


def test_connect_to_returns_false_on_launch_error(make_window, qtbot, boxes):
    info = mk("5.5.5.5", "451", port=7778)
    game = GAMES["ut99"]
    w = make_window(FakeScan({}), FakeLaunch(error=LaunchError("x")))
    w.select_game("ut99")
    assert w.connect_to(game, info) is False
    ok = FakeLaunch()
    w2 = make_window(FakeScan({}), ok)
    w2.select_game("ut99")
    assert w2.connect_to(game, info) is True
    assert len(ok.calls) == 1


def test_select_address(make_window, qtbot):
    a, b = mk("1.1.1.1"), mk("2.2.2.2")
    w = make_window(FakeScan({"cod2": [a, b]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.proxy.rowCount() == 2)
    assert w.select_address(b.address) is True
    assert w.selected_server().address == b.address
    assert w.select_address(ServerAddress("9.9.9.9", 1)) is False


def test_pending_select_applies_when_row_arrives(make_window, qtbot):
    a, b = mk("1.1.1.1"), mk("2.2.2.2")
    w = make_window(FakeScan({"cod2": [a, b]}, delays={"cod2": 0.05}))
    w.select_game("cod2")  # select_game descarta la pendiente anterior: se fija después
    w._pending_select = b.address
    qtbot.waitUntil(lambda: not w.is_scanning() and w.proxy.rowCount() == 2)
    assert w.selected_server().address == b.address
    assert w._pending_select is None


def recent_window(make_window, qtbot, query, recent, launch=None):
    cfg = Config(recent={"cod2": recent} if recent else {})
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}), launch, cfg=cfg)
    w._query_fn = query
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and not w._workers)
    return w


def test_connect_records_recent(make_window, qtbot, tmp_path):
    from jase.recent import recent_addresses
    a = mk("1.1.1.1")
    w = recent_window(make_window, qtbot, FakeQuery(), [])
    assert w.connect_to(GAMES["cod2"], a) is True
    assert recent_addresses(w.cfg, "cod2") == [a.address]
    assert load_config(tmp_path / "config.json").recent == {"cod2": [str(a.address)]}


def test_failed_launch_not_recorded(make_window, qtbot, boxes):
    from jase.recent import recent_addresses
    w = recent_window(make_window, qtbot, FakeQuery(), [],
                      launch=FakeLaunch(error=LaunchError("x")))
    assert w.connect_to(GAMES["cod2"], mk("1.1.1.1")) is False
    assert recent_addresses(w.cfg, "cod2") == []


def test_recent_view_lists_and_marks_offline(make_window, qtbot):
    online = mk("2.2.2.2")
    query = FakeQuery({online.address: online})
    w = recent_window(make_window, qtbot, query, ["2.2.2.2:28960", "3.3.3.3:28960"])
    w.show_recent()
    assert w.in_recent_view() and not w.in_favorites_view()
    qtbot.waitUntil(lambda: w.model.rowCount() == 2 and not w._workers)
    by_host = {s.address.host: s for s in w.model.servers()}
    assert by_host["2.2.2.2"].online and by_host["3.3.3.3"].online is False
    assert "1 de 2" in w.statusBar().currentMessage()
    assert w.favorites_list.currentRow() == 1
    w.show_favorites()  # cambiar de vista directa re-consulta con la otra lista
    assert w.in_favorites_view() and not w.in_recent_view()


def test_remove_from_recent_context_action(make_window, qtbot):
    online = mk("2.2.2.2")
    query = FakeQuery({online.address: online})
    w = recent_window(make_window, qtbot, query, ["2.2.2.2:28960", "3.3.3.3:28960"])
    assert w.ui_actions["remove_recent"] not in w._build_context_menu().actions()
    w.show_recent()
    qtbot.waitUntil(lambda: w.model.rowCount() == 2 and not w._workers)
    select_host(w, "3.3.3.3")
    action = context_action(w, "Quitar de recientes")
    assert action.isVisible() and action.isEnabled()
    action.trigger()
    assert [s.address.host for s in w.model.servers()] == ["2.2.2.2"]
    assert w.cfg.recent == {"cod2": ["2.2.2.2:28960"]}


# ----------------------------------------------------------- buscar jugadores
def test_buscar_jugador_revela_el_servidor(qtbot, make_window):
    a, b = mk("1.1.1.1"), mk("2.2.2.2")
    b.players = [Player("Jimmy", 5, 40)]
    w = make_window(FakeScan({"cod2": [a, b], "ut99": [mk("3.3.3.3", version="")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2)
    w.select_game("ut99")
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and not w.is_scanning())
    w.find_player()
    dlg = w._player_search
    assert isinstance(dlg, PlayerSearchDialog)
    dlg.query_edit.setText("jim")
    qtbot.waitUntil(lambda: dlg.table.rowCount() == 1)
    dlg.table.cellDoubleClicked.emit(0, 0)
    assert w.current_game == "cod2"
    qtbot.waitUntil(lambda: w.selected_server() is not None)
    assert w.selected_server().address == b.address


def test_atajo_buscar_jugador_reutiliza_el_dialogo(qtbot, make_window):
    w = make_window(FakeScan({}))
    act = w.ui_actions["find_player"]
    assert act.shortcut().toString() == "Ctrl+Shift+F"
    act.trigger()
    first = w._player_search
    assert first.isVisible()
    act.trigger()
    assert w._player_search is first
    assert w.searchable_lists() == {}


# ------------------------------------------------------ avisar cuando haya sitio
def _full(host="1.1.1.1"):
    info = mk(host)
    info.num_players = info.max_players
    return info


def _watch_window(make_window, qtbot, launch, query=None):
    w = make_window(FakeScan({"cod2": [_full()]}), launch)
    if query is not None:
        w._query_fn = query
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 1)
    select_first_row(w)
    return w


def test_watch_slot_action_only_for_full_online(make_window, qtbot, boxes):
    w = make_window(FakeScan({"cod2": [mk("2.2.2.2"), _full()]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.proxy.rowCount() == 2)
    action = w.ui_actions["watch_slot"]
    w.table.clearSelection()
    w._update_actions()
    assert not action.isEnabled()
    for row in range(2):
        w.table.selectRow(row)
        info = w.selected_server()
        assert action.isEnabled() == (info.num_players == info.max_players)
    assert action.text() == "Avisar cuando haya sitio"


def test_watch_slot_shows_label_and_toggles(make_window, qtbot, boxes):
    w = _watch_window(make_window, qtbot, FakeLaunch(), FakeQuery(delay=5))
    w.ui_actions["watch_slot"].trigger()
    assert w._slot_watch is not None and w._slot_watch.active
    assert w.watch_label.text() == "Vigilando srv 1.1.1.1…"
    assert w.ui_actions["watch_slot"].text() == "Dejar de vigilar"
    w.ui_actions["watch_slot"].trigger()
    assert w._slot_watch is None and w.watch_label.isHidden()


@pytest.mark.parametrize("answer, calls", [(QMessageBox.Yes, 1), (QMessageBox.No, 0)])
def test_slot_free_offers_connect(make_window, qtbot, monkeypatch, boxes, answer, calls):
    monkeypatch.setattr(SlotWatcher, "INTERVAL_MS", 20)
    free = _full()
    free.num_players = free.max_players - 1
    asked = []
    monkeypatch.setattr(QMessageBox, "question",
                        lambda *a, **kw: asked.append(a) or answer)
    launch = FakeLaunch()
    w = _watch_window(make_window, qtbot, launch, FakeQuery({free.address: free}))
    w.ui_actions["watch_slot"].trigger()
    qtbot.waitUntil(lambda: bool(asked), timeout=3000)
    assert len(launch.calls) == calls
    assert w._slot_watch is None and w.watch_label.isHidden()
    assert "19/20" in asked[0][2]


def test_slot_watch_expired_shows_status(make_window, qtbot, monkeypatch, boxes):
    monkeypatch.setattr(SlotWatcher, "INTERVAL_MS", 20)
    monkeypatch.setattr(SlotWatcher, "MAX_MS", 80)
    w = _watch_window(make_window, qtbot, FakeLaunch(), FakeQuery())
    w.ui_actions["watch_slot"].trigger()
    qtbot.waitUntil(lambda: w._slot_watch is None, timeout=3000)
    assert "Se dejó de vigilar srv 1.1.1.1" in w.statusBar().currentMessage()


def test_close_with_active_slot_watch_leaves_no_thread(make_window, qtbot, monkeypatch, boxes):
    monkeypatch.setattr(SlotWatcher, "INTERVAL_MS", 20)
    w = _watch_window(make_window, qtbot, FakeLaunch(), FakeQuery(delay=5))
    w.ui_actions["watch_slot"].trigger()
    qtbot.waitUntil(lambda: w._slot_watch.current_worker() is not None, timeout=3000)
    worker = w._slot_watch.current_worker()
    qtbot.waitUntil(worker.isRunning, timeout=3000)
    w.close()
    assert not worker.isRunning()
    assert not main_window._LINGERING


# ---------------------------------------------------------------- refresco automático
def auto_window(make_window, qtbot, servers=None):
    scan = FakeScan({"cod2": servers or [mk("1.1.1.1"), mk("2.2.2.2")]})
    w = make_window(scan)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    return w, scan


def test_auto_refresh_timer_follows_config(make_window):
    w = make_window(FakeScan({}), cfg=Config(auto_refresh_minutes=2))
    assert w._auto_refresh_timer.isActive()
    assert w._auto_refresh_timer.interval() == 120_000
    w.cfg.auto_refresh_minutes = 0
    w._apply_auto_refresh()
    assert not w._auto_refresh_timer.isActive()
    assert not make_window(FakeScan({}))._auto_refresh_timer.isActive()


def test_auto_refresh_keeps_selection(make_window, qtbot):
    w, scan = auto_window(make_window, qtbot)
    b = ServerAddress("2.2.2.2", 28960)
    assert w.select_address(b)
    w._auto_refresh()
    qtbot.waitUntil(lambda: scan.calls == ["cod2", "cod2"])
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    qtbot.waitUntil(lambda: w.selected_server() is not None)
    assert w.selected_server().address == b


@pytest.mark.parametrize("block", ["scanning", "modal", "minimized"])
def test_auto_refresh_skips_when_busy(make_window, qtbot, monkeypatch, block):
    from PySide6.QtWidgets import QApplication
    w, scan = auto_window(make_window, qtbot)
    if block == "scanning":
        monkeypatch.setattr(w, "is_scanning", lambda: True)
    elif block == "modal":
        monkeypatch.setattr(QApplication, "activeModalWidget", staticmethod(lambda: object()))
    else:
        monkeypatch.setattr(w, "isMinimized", lambda: True)
    w._auto_refresh()
    assert scan.calls == ["cod2"]


def test_auto_refresh_in_favorites_requeries_favorites(make_window, qtbot):
    online = mk("2.2.2.2")
    query = FakeQuery({online.address: online})
    w, scan = favorites_window(make_window, qtbot, query, ["2.2.2.2:28960"])
    w.show_favorites()
    qtbot.waitUntil(lambda: len(query.calls) == 1 and not w._workers)
    w._auto_refresh()
    qtbot.waitUntil(lambda: len(query.calls) == 2 and not w._workers)
    assert scan.calls == ["cod2"] and w.in_favorites_view()


def test_settings_auto_refresh_combo_roundtrip(qtbot):
    cfg = Config(auto_refresh_minutes=10)
    d = SettingsDialog(cfg)
    qtbot.addWidget(d)
    assert d.auto_refresh_combo.currentData() == 10
    d.auto_refresh_combo.setCurrentIndex(d.auto_refresh_combo.findData(0))
    d.apply_to(cfg)
    assert cfg.auto_refresh_minutes == 0


def test_scan_worker_passes_include_empty_only_for_steam(qtbot):
    calls = []

    async def fake(game, masters, on_server, on_status, **kw):
        calls.append((game.id, dict(kw)))
        return ScanResult(masters_ok=1, servers_listed=0, servers_responded=0)

    for gid in ("tf2", "cod2"):
        worker = ScanWorker(GAMES[gid], [], scan_fn=fake, steam_key="K",
                            steam_include_empty=True)
        with qtbot.waitSignal(worker.finished, timeout=5000):
            worker.start()
    kw = dict(calls)
    assert kw["tf2"]["steam_include_empty"] is True
    assert "steam_include_empty" not in kw["cod2"]


def test_settings_include_empty_clears_only_steam_cache(make_window, qtbot, monkeypatch):
    scan = KeyScan()
    w = make_window(scan, steam_key="K")
    w._cache["tf2"] = []
    w._cache["cod2"] = []

    def fake_exec(self):
        self.steam_include_empty_check.setChecked(True)
        return QDialog.Accepted

    monkeypatch.setattr(dialogs.SettingsDialog, "exec", fake_exec)
    w.open_settings()
    assert w.cfg.steam_include_empty is True
    assert "tf2" not in w._cache and "cod2" in w._cache


def test_settings_include_empty_checkbox_follows_config():
    d = SettingsDialog(Config(steam_include_empty=True))
    assert d.steam_include_empty_check.isChecked()


# ------------------------------------------------- selección pendiente obsoleta
class StagedScan:
    """scan_fn falso: cada escaneo emite sus tandas de servidores separadas por una pausa
    (`rounds[i]` es el escaneo i; el último se repite)."""

    def __init__(self, rounds, pause=0.3):
        self.rounds = rounds
        self.pause = pause
        self.calls = []

    async def __call__(self, game, masters, on_server, on_status, **kw):
        stages = self.rounds[min(len(self.calls), len(self.rounds) - 1)]
        self.calls.append(game.id)
        for i, stage in enumerate(stages):
            if i:
                await asyncio.sleep(self.pause)
            for info in stage:
                on_server(info)
        n = sum(len(s) for s in stages)
        return ScanResult(masters_ok=1, servers_listed=n, servers_responded=n)


def test_user_selection_mid_scan_beats_stale_pending(make_window, qtbot):
    a, b = mk("1.1.1.1"), mk("2.2.2.2")
    w = make_window(StagedScan([[[a, b]], [[b], [a]]]))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    assert w.select_address(a.address)
    w._auto_refresh()  # deja A pendiente y reescanea: llega B, pausa, llega A
    assert w._pending_select == a.address
    qtbot.waitUntil(lambda: w.model.rowCount() == 1)
    select_host(w, "2.2.2.2")  # el usuario elige B a mitad del escaneo
    assert w._pending_select is None
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    assert w.selected_server().address == b.address


def test_pending_select_cleared_when_scan_ends(make_window, qtbot):
    a = mk("1.1.1.1")
    w = make_window(FakeScan({"cod2": [a]}))
    w.select_game("cod2")
    w._pending_select = ServerAddress("9.9.9.9", 1)  # nunca llega
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    assert w._pending_select is None


def test_pending_select_cleared_by_select_game(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")], "ut99": []}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning())
    w._pending_select = ServerAddress("9.9.9.9", 1)
    w.select_game("ut99")
    assert w._pending_select is None


def test_pending_offline_favorite_stays_selected(make_window, qtbot):
    off = ServerAddress("3.3.3.3", 28960)
    query = FakeQuery()
    w, _scan = favorites_window(make_window, qtbot, query, [str(off)])
    w.show_favorites()
    qtbot.waitUntil(lambda: not w._workers)
    select_host(w, "3.3.3.3")
    w._auto_refresh()  # reconsulta: la fila offline vuelve al terminar
    qtbot.waitUntil(lambda: len(query.calls) == 2 and not w._workers)
    assert w.selected_server().address == off
    assert w._pending_select is None


def test_reveal_server_hidden_by_filters_says_so(make_window, qtbot):
    from jase.models import QuickFilters
    full = _full()
    w = make_window(FakeScan({"cod2": [mk("2.2.2.2"), full]}),
                    cfg=Config(quick_filters=QuickFilters(hide_full=True)))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    w.reveal_server("cod2", full.address)
    assert w.statusBar().currentMessage() == "El servidor está oculto por los filtros activos"
    assert w._pending_select is None


def test_reveal_server_of_game_being_scanned_waits_for_it(make_window, qtbot):
    a, b = mk("1.1.1.1"), mk("2.2.2.2")
    w = make_window(StagedScan([[[b], [a]]], pause=0.2))
    w.reveal_server("cod2", a.address)
    assert w.is_scanning() and w._pending_select == a.address
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 2)
    assert w.selected_server().address == a.address


def test_reveal_server_absent_after_scan_leaves_nothing_pending(make_window, qtbot):
    w = make_window(FakeScan({"cod2": [mk("1.1.1.1")]}))
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning())
    w.reveal_server("cod2", ServerAddress("9.9.9.9", 1))
    assert w._pending_select is None


def test_enter_in_player_search_uses_fresh_results(qtbot):
    jim = mk("1.1.1.1")
    jim.players = [Player("Jimmy", 5, 40)]
    bob = mk("2.2.2.2")
    bob.players = [Player("Bob", 5, 40)]
    dlg = PlayerSearchDialog(lambda: {"cod2": [jim, bob]})
    qtbot.addWidget(dlg)
    dlg.query_edit.setText("jim")
    dlg.refresh()
    chosen = []
    dlg.server_chosen.connect(lambda game, address: chosen.append(address))
    dlg.query_edit.setText("bob")  # Enter antes de que venza el debounce
    assert dlg._timer.isActive()
    dlg._choose_current()
    assert chosen == [bob.address]
    assert not dlg._timer.isActive()


def test_slot_watch_expired_message_uses_constant(make_window, qtbot, monkeypatch, boxes):
    monkeypatch.setattr(SlotWatcher, "INTERVAL_MS", 20)
    monkeypatch.setattr(SlotWatcher, "MAX_MS", 5 * 60_000)
    w = _watch_window(make_window, qtbot, FakeLaunch(), FakeQuery())
    w.ui_actions["watch_slot"].trigger()
    w._slot_watch._expiry.timeout.emit()
    assert "5 min sin sitio" in w.statusBar().currentMessage()


def test_auto_refresh_keeps_rows_when_game_cannot_scan(make_window, qtbot):
    scan = KeyScan()
    w = make_window(scan, steam_key="")
    w.select_game("l4d2")
    for host in ("1.1.1.1", "2.2.2.2"):
        w.model.add_or_update(mk(host))
    assert w.model.rowCount() == 2 and not w.can_scan(w._game())
    w._auto_refresh()
    assert w.model.rowCount() == 2
    w._steam_key = "K"
    assert w.can_scan(w._game())


def test_edit_note_via_context_menu(make_window, qtbot, tmp_path, monkeypatch):
    from jase.ui import window_favorites
    w = make_query_window(make_window, qtbot, [mk("1.1.1.1")], FakeQuery())
    select_first_row(w)
    assert not context_action(w, "Editar nota…").isEnabled()  # no es favorito
    w.toggle_favorite_selected()
    assert context_action(w, "Editar nota…").isEnabled()
    monkeypatch.setattr(window_favorites.QInputDialog, "getText",
                        lambda *a, **k: ("mi nota", True))
    context_action(w, "Editar nota…").trigger()
    assert w.cfg.favorite_notes == {"cod2": {"1.1.1.1:28960": "mi nota"}}
    assert load_config(tmp_path / "config.json").favorite_notes == w.cfg.favorite_notes
    assert w.server_card.findChild(QLabel, "favorite_note").text() == "mi nota"
    assert "mi nota" in w.model.data(w.model.index(0, COL_NAME),
                                      Qt.ItemDataRole.ToolTipRole)
    monkeypatch.setattr(window_favorites.QInputDialog, "getText", lambda *a, **k: ("", True))
    w.edit_note_selected()
    assert w.cfg.favorite_notes == {}
    monkeypatch.setattr(window_favorites.QInputDialog, "getText", lambda *a, **k: ("z", False))
    w.edit_note_selected()
    assert w.cfg.favorite_notes == {}


def test_filter_presets_save_and_apply(make_window, monkeypatch):
    from PySide6.QtWidgets import QInputDialog
    w = make_window(lambda *a, **k: [])
    w.hide_empty_check.setChecked(True)
    w.max_ping_combo.addItem("77", 77)
    w.max_ping_combo.setCurrentIndex(w.max_ping_combo.count() - 1)
    w.filter_edit.setText("dm")
    monkeypatch.setattr(QInputDialog, "getText", staticmethod(lambda *a, **k: ("Mi mezcla", True)))
    w.save_current_filters()
    assert [p.name for p in w.cfg.filter_presets] == ["Mi mezcla"]
    w.hide_empty_check.setChecked(False)
    w.hide_full_check.setChecked(True)
    w.max_ping_combo.setCurrentIndex(0)
    w.filter_edit.setText("")
    w._rebuild_presets_menu()
    w.presets_menu.actions()[0].trigger()
    assert w.hide_empty_check.isChecked() and not w.hide_full_check.isChecked()
    assert w.max_ping_combo.currentData() == 77
    assert w.filter_edit.text() == "dm"
    assert w.cfg.quick_filters.max_ping == 77
    w.delete_filter_preset("mi mezcla")
    assert w.cfg.filter_presets == []


def test_terminal_color_menu_switches_to_terminal(make_window):
    w = make_window(FakeScan({}))
    menu = w.terminal_color_menu
    assert [a.data() for a in menu.actions()] == ["green", "white", "sepia", "amber"]
    w.terminal_color_actions["sepia"].trigger()
    assert (w.cfg.theme, w.cfg.terminal_color) == ("terminal", "sepia")
    assert w._theme_id == "terminal_sepia" and w.theme_actions["terminal"].isChecked()
    assert w.terminal_color_actions["sepia"].isChecked()
    w.set_theme("dark")
    assert w._theme_id == "dark" and w.cfg.terminal_color == "sepia"  # se recuerda


def test_settings_terminal_color_only_enabled_for_terminal(qtbot):
    from jase.config import Config
    from jase.ui.dialogs import SettingsDialog
    cfg = Config(theme="dark", terminal_color="white")
    d = SettingsDialog(cfg)
    qtbot.addWidget(d)
    combo = d.terminal_color_combo
    assert combo.currentData() == "white" and not combo.isEnabled()
    for i in range(d.theme_list.count()):
        if d.theme_list.item(i).data(Qt.UserRole) == "terminal":
            d.theme_list.setCurrentRow(i)
    assert combo.isEnabled()
    combo.setCurrentIndex(combo.findData("amber"))
    d.apply_to(cfg)
    assert (cfg.theme, cfg.terminal_color) == ("terminal", "amber")


def test_country_combo_lists_loaded_countries_and_filters(qtbot, tmp_path):
    geo = FakeGeo((FakeCountryDB({"1.1.1.1": "cl"}), None))
    w = make_geo_window(qtbot, tmp_path, geo)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: w.model.rowCount() == 2 and w._country_db is not None)
    combo = w.country_combo
    w._refresh_country_combo()
    assert [combo.itemText(i) for i in range(combo.count())] == ["Todos los países",
                                                                "Chile (1)"]
    combo.setCurrentIndex(1)
    assert w.proxy.rowCount() == 1 and w.cfg.quick_filters.country == "cl"
    assert load_config(tmp_path / "config.json").quick_filters.country == "cl"
    w.show_favorites()
    assert not combo.isEnabled() and not w.ui_actions["hide_bots"].isEnabled()
    w.close()


def test_hide_bots_check_mirrors_menu_and_presets(make_window, monkeypatch):
    from PySide6.QtWidgets import QInputDialog
    w = make_window(lambda *a, **k: [])
    assert w.hide_bots_check.text() == "Ocultar solo bots"
    w.ui_actions["hide_bots"].trigger()
    assert w.hide_bots_check.isChecked() and w.cfg.quick_filters.hide_bots
    monkeypatch.setattr(QInputDialog, "getText", staticmethod(lambda *a, **k: ("B", True)))
    w.save_current_filters()
    w.hide_bots_check.setChecked(False)
    w.apply_filter_preset(w.cfg.filter_presets[0])
    assert w.hide_bots_check.isChecked() and w.ui_actions["hide_bots"].isChecked()
