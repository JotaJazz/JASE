"""Ventana principal, árbol y tabla en inglés (el español lo cubren los tests existentes)."""
import pytest
from PySide6.QtCore import Qt

from jase import i18n
from jase.browse import BASE_MOD, Breakdown
from jase.config import Config
from jase.games import GAMES
from jase.models import ServerAddress, ServerInfo
from jase.ui.game_tree import GameTree
from jase.ui.main_window import MainWindow
from jase.ui.server_model import (
    COL_COUNTRY,
    COL_MODE,
    COL_NAME,
    COL_PING,
    COL_PLAYERS,
    COL_VERSION,
    ServerTableModel,
)
from test_main_window import FakeLaunch, FakeMaps, FakeScan, mk  # noqa: F401


@pytest.fixture
def english():
    i18n.set_language("en")


@pytest.fixture
def window(qtbot, tmp_path, english):
    scan = FakeScan({"cod2": [mk("1.1.1.1"), mk("2.2.2.2")]})
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=scan,
                   launch_fn=FakeLaunch(), map_service=FakeMaps())
    qtbot.addWidget(w)
    yield w
    w.close()


def test_menus_and_actions_in_english(window):
    titles = [a.text() for a in window.menuBar().actions()]
    assert titles == ["File", "View", "Filter", "Server", "Help"]
    acts = window.ui_actions
    assert acts["refresh"].text() == "Refresh list"
    assert acts["connect"].text() == "Connect"
    assert acts["quit"].text() == "Quit"
    assert acts["find_games"].text() == "Find games…"
    assert acts["favorite"].text() == "Add to favorites"
    assert window.filter_edit.placeholderText() == "Filter servers…"
    assert window.max_ping_combo.itemText(0) == "No limit"
    assert window.favorites_list.item(0).text() == "★ Favorites"


def test_table_headers_in_english(window):
    m = window.model
    h = {c: m.headerData(c, Qt.Horizontal) for c in
         (COL_COUNTRY, COL_NAME, COL_PING, COL_PLAYERS, COL_MODE, COL_VERSION)}
    assert h == {COL_COUNTRY: "Country", COL_NAME: "Server", COL_PING: "Ping",
                 COL_PLAYERS: "Players", COL_MODE: "Mode", COL_VERSION: "Version"}
    assert "DB-IP" in m.headerData(COL_COUNTRY, Qt.Horizontal, Qt.ToolTipRole)
    assert m.headerData(COL_COUNTRY, Qt.Horizontal, Qt.ToolTipRole).startswith("Country by IP")


def test_offline_row_in_english(english, qapp):
    from jase.ui import favorites
    m = ServerTableModel()
    info = favorites.offline_server(ServerAddress("9.9.9.9", 28960))
    m.add_or_update(info)
    assert m.index(0, COL_NAME).data().endswith(" — no response")


def test_tree_folders_in_english(qapp, english):
    t = GameTree()
    t.set_games(["a"])
    t.set_breakdown("a", Breakdown((("rpg", 2), (BASE_MOD, 6)), (("dm1", 5),), 4))
    game = t.game_item("a")
    names = [game.child(i).text(0) for i in range(game.childCount())]
    assert names == ["Mods", "Maps"]
    mods, maps = game.child(0), game.child(1)
    assert mods.child(1).text(0) == "(base) · 6"
    assert mods.child(1).data(0, Qt.UserRole) == BASE_MOD
    assert maps.child(maps.childCount() - 1).text(0) == "others… · 4"


def test_status_messages_in_english(window, qtbot):
    window.select_game("cod2")
    qtbot.waitUntil(lambda: window.model.rowCount() == 2)
    qtbot.waitUntil(lambda: not window.is_scanning())
    assert window.statusBar().currentMessage() == "2 of 3 servers responded"
    window.refresh_selected()  # sin selección: no hace nada
    window.table.selectRow(0)
    window.copy_selected_address()
    assert window.statusBar().currentMessage().startswith("Address copied: ")


def test_count_label_plural_english(window, qtbot):
    assert window.count_label.text() == "0 servers"
    window.select_game("cod2")
    qtbot.waitUntil(lambda: window.model.rowCount() == 2)
    assert window.count_label.text() == "2 servers"
    window.hide_empty_check.setChecked(True)
    window.proxy.set_text("1.1.1.1")
    assert window.count_label.text() == "1 server"


def test_card_and_detail_headers_english(window):
    assert window.players_table.horizontalHeaderItem(0).text() == "Name"
    assert window.rules_table.horizontalHeaderItem(1).text() == "Value"
    assert window.ui_actions["theme"].toolTip() == "Theme: System (click to change)"


# --- diálogos, ficha, núcleo y países (Task 3)
def test_settings_dialog_in_english(qtbot, english):
    from PySide6.QtWidgets import QDialogButtonBox, QLabel, QLineEdit, QPushButton
    from jase.ui.dialogs import SettingsDialog
    d = SettingsDialog(Config(), on_update_countries=lambda: None,
                       on_find_games=lambda: {})
    qtbot.addWidget(d)
    assert d.windowTitle() == "Settings"
    assert d.find_games_button.text() == "Find games…"
    assert d.update_countries_button.text() == "Update country database"
    assert d.download_maps_check.text() == \
        "Download map images from the internet (GameTracker)"
    texts = {b.text() for b in d.findChildren(QPushButton)}
    assert {"Browse…", "Save", "Cancel"} <= texts
    assert any(e.placeholderText() == "Install folder" for e in d.findChildren(QLineEdit))
    d.update_countries_button.click()
    assert d.countries_status.text() == "Downloading in the background…"
    status = d.findChild(QLabel, "path_status_cod2").text()
    assert status.startswith("\u26a0 ") and "not found" in status
    assert d.findChild(QDialogButtonBox) is not None


def test_find_games_dialog_in_english(qtbot, tmp_path, english):
    from PySide6.QtWidgets import QPushButton
    from jase.detect import Finding
    from jase.ui.find_games import FindGamesDialog
    (tmp_path / "ok").mkdir()
    (tmp_path / "ok" / "CoD2MP_s.exe").write_bytes(b"")
    cfg = Config()
    cfg.game_paths = {"cod2": str(tmp_path / "ok"), "cod1": str(tmp_path / "gone"),
                      "cod4": ""}
    result = {"cod2": [Finding("cod2", str(tmp_path / "ok"), "registro")],
              "cod1": [Finding("cod1", "X:\\CoD", "steam")],
              "cod4": [Finding("cod4", "X:\\CoD4", "registro")]}
    d = FindGamesDialog(cfg, find_fn=lambda **kw: result)
    qtbot.addWidget(d)
    assert d.windowTitle() == "Find games"
    qtbot.waitUntil(lambda: bool(d.rows))
    assert {g: r[2].text() for g, r in d.rows.items()} == {
        "cod2": "already set up", "cod1": "saved path not found", "cod4": "new"}
    assert d.status_label.text() == "3 games found"
    assert d.rows["cod2"][1].itemText(0).endswith(" (registry)")
    assert d.rows["cod1"][1].itemText(0) == "X:\\CoD (Steam)"
    assert d.rows["cod1"][1].itemData(0) == "X:\\CoD"
    assert d.cancel_search_button.text() == "Cancel search"
    assert d.folder_button.text() == "Search in folder…"
    assert {b.text() for b in d.findChildren(QPushButton)} >= {"Apply", "Cancel"}
    d2 = FindGamesDialog(Config(), find_fn=lambda **kw: {})
    qtbot.addWidget(d2)
    qtbot.waitUntil(lambda: d2.status_label.text() == "No games found")


def test_server_card_in_english(qtbot, english):
    from test_server_card import card as _card, info, label  # noqa: F401
    from jase.ui.server_card import ServerCard
    c = ServerCard()
    qtbot.addWidget(c)
    assert label(c, "card_empty").text() == "Select a server"
    c.set_server(info(password=True), "dark")
    assert c.lock_label.text() == "🔒 Password"
    assert c.lock_label.toolTip() == "The server requires a password"
    assert c.copy_button.text() == "Copy"
    assert c.copy_button.toolTip() == "Copy IP:port to the clipboard"
    from PySide6.QtWidgets import QLabel
    captions = {w.text() for w in c.findChildren(QLabel) if w.property("role") == "caption"}
    assert captions == {"Map", "Mode", "Players", "Ping", "Version"}
    assert c.favorite_button.toolTip() == "Add to favorites"
    c.set_favorite(True)
    assert c.favorite_button.toolTip() == "Remove from favorites"
    c.set_server(info(online=False), "dark")
    assert label(c, "card_ping").text() == "No response"


def test_password_dialog_in_english(qtbot, english):
    from PySide6.QtWidgets import QPushButton
    from jase.ui.dialogs import PasswordDialog
    d = PasswordDialog("My Server", "pw")
    qtbot.addWidget(d)
    assert d.windowTitle() == "Server password"
    assert d.edit.placeholderText() == "Password"
    assert {b.text() for b in d.findChildren(QPushButton)} == {
        "Connect", "Cancel", "Forget password"}


def test_scanner_status_in_english(english):
    import asyncio
    from jase import scanner
    from jase.games import GAMES
    from test_scanner import T, dead_udp_port
    status = []

    async def main():
        return await scanner.scan(GAMES["cod2"], [("127.0.0.1", dead_udp_port())],
                                  lambda s: None, status.append,
                                  master_timeout=T, query_timeout=T)

    asyncio.run(main())
    assert status[-1] == "No master responded"


def test_scanner_master_status_in_english(english):
    import asyncio
    import dataclasses
    from jase import scanner
    from jase.games import GAMES
    from jase.protocols import quake3 as q3
    from test_scanner import STATUS, T, _master_packet, fake_udp
    status = []
    game = dataclasses.replace(GAMES["cod2"], protocols=(118,))

    async def main():
        async with fake_udp(lambda d: [STATUS] if d == q3.GETSTATUS else []) as (srv, _):
            server = ServerAddress("127.0.0.1", srv)
            handler = (lambda d: [_master_packet([server], True)]
                       if d.startswith(q3.OOB + b"getservers ") else [])
            async with fake_udp(handler) as (mport, _):
                await scanner.scan(game, [("nonexistent.invalid", 20710),
                                          ("127.0.0.1", mport)],
                                   lambda s: None, status.append,
                                   master_timeout=T, query_timeout=T)

    asyncio.run(main())
    assert "Master nonexistent.invalid is not responding" in status
    assert "1 server on the master, querying…" in status


def test_launch_error_in_english(tmp_path, english):
    from jase.launcher import LaunchError, launch
    a = ServerAddress("1.2.3.4", 28960)
    with pytest.raises(LaunchError) as e:
        launch(GAMES["cod1"], None, a, None)
    assert str(e.value) == "No path is set for Call of Duty. Set it in Settings."
    with pytest.raises(LaunchError) as e:
        launch(GAMES["cod2"], str(tmp_path), a, None)
    assert str(e.value).startswith("Could not find ")
    assert str(e.value).endswith("Check the path in Settings.")


def test_country_name_follows_language(english):
    from jase.geo import country_name
    assert country_name("cl") == "Chile"
    assert country_name("de") == "Germany"
    assert country_name("zz-unknown") == "ZZ-UNKNOWN"
    i18n.set_language("es")
    assert country_name("cl") == "Chile"
    assert country_name("de") == "Alemania"


def test_country_names_en_same_keys():
    from jase.geo import COUNTRY_NAMES
    from jase.locales.countries_en import COUNTRY_NAMES_EN
    assert set(COUNTRY_NAMES_EN) == set(COUNTRY_NAMES)
    assert all(COUNTRY_NAMES_EN.values())
    assert COUNTRY_NAMES_EN["us"] == "United States"
    assert COUNTRY_NAMES_EN["kr"] == "South Korea"


# --- barrido de textos visibles (Task 4)
_SPANISH_CHARS = "áéíóúñ¿¡«»"
_SPANISH_WORDS = ("Servidor", "Jugadores", "Mapa", "Ajustes", "Buscar", "Conectar")


def _visible_texts(root):
    from PySide6.QtGui import QAction
    from PySide6.QtWidgets import (
        QAbstractButton, QComboBox, QLabel, QLineEdit, QMenu, QTableWidget, QWidget)
    out = []
    widgets = [root, *root.findChildren(QWidget)]
    for w in widgets:
        out += [w.windowTitle(), w.toolTip()]
        if isinstance(w, (QLabel, QAbstractButton)):
            out.append(w.text())
        if isinstance(w, QLineEdit):
            out.append(w.placeholderText())
        if isinstance(w, QComboBox):
            out += [w.itemText(i) for i in range(w.count())]
        if isinstance(w, QTableWidget):
            out += [w.horizontalHeaderItem(c).text() for c in range(w.columnCount())
                    if w.horizontalHeaderItem(c)]
    for a in root.findChildren(QAction):
        out += [a.text(), a.toolTip()]
    for m in root.findChildren(QMenu):
        out.append(m.title())
    return [t for t in out if t]


def _assert_no_spanish(texts, what):
    bad = [t for t in texts
           if any(c in t for c in _SPANISH_CHARS) or any(w in t for w in _SPANISH_WORDS)]
    assert not bad, f"{what}: {sorted(set(bad))}"


def test_english_ui_has_no_spanish_text(window, qtbot, tmp_path, monkeypatch):
    from jase.detect import Finding
    from jase.ui.dialogs import SettingsDialog
    from jase.ui.find_games import FindGamesDialog
    window.select_game("cod2")
    qtbot.waitUntil(lambda: window.model.rowCount() == 2)
    qtbot.waitUntil(lambda: not window.is_scanning())
    window.table.selectRow(0)
    texts = _visible_texts(window)
    menu = window._build_context_menu()
    texts += [a.text() for a in menu.actions()]
    for m in window.menuBar().findChildren(type(menu)):
        texts += [a.text() for a in m.actions()]
    texts += [window.statusBar().currentMessage(), window.count_label.text()]
    model = window.model
    texts += [str(model.headerData(c, Qt.Horizontal) or "")
              for c in range(model.columnCount())]
    texts += [str(model.headerData(c, Qt.Horizontal, Qt.ToolTipRole) or "")
              for c in range(model.columnCount())]
    # árbol: solo las carpetas fijas (los juegos, mods y mapas son datos)
    game = window.game_tree.game_item("cod2")
    texts += [game.child(i).text(0) for i in range(game.childCount())
              if game.child(i).data(0, Qt.UserRole) is None][:2]
    _assert_no_spanish([t for t in texts if t], "main window")

    d = SettingsDialog(Config(), on_update_countries=lambda: None,
                       on_find_games=lambda: {})
    qtbot.addWidget(d)
    d.update_countries_button.click()
    st = _visible_texts(d)
    assert [d.language_combo.itemText(i) for i in range(3)] == [
        "Automatic", "Español", "English"]
    _assert_no_spanish([t for t in st if t != "Español"],
                       "settings")
    result = {"cod2": [Finding("cod2", str(tmp_path), "registro")]}
    fg = FindGamesDialog(Config(), find_fn=lambda **kw: result)
    qtbot.addWidget(fg)
    qtbot.waitUntil(lambda: bool(fg.rows))
    ft = _visible_texts(fg)
    _assert_no_spanish(ft, "find games")


def test_spanish_ui_has_no_english_translations(qtbot, tmp_path):
    from jase.locales.en import MESSAGES
    scan = FakeScan({"cod2": [mk("1.1.1.1")]})
    w = MainWindow(Config(), tmp_path / "config.json", scan_fn=scan,
                   launch_fn=FakeLaunch(), map_service=FakeMaps())
    qtbot.addWidget(w)
    english = {v for k, v in MESSAGES.items() if v != k and "{" not in v}
    # nombres propios que coinciden con inglés
    english -= {"Ping"}
    texts = _visible_texts(w)
    w.close()
    assert not [t for t in texts if t in english]


def test_country_db_failure_status_in_english(qtbot, tmp_path, english):
    from jase.geo import CountrySource

    def failing_fetch(url):
        raise OSError("sin red")

    w = MainWindow(Config(), tmp_path / "config.json",
                   scan_fn=FakeScan({"cod2": [mk("1.1.1.1")]}),
                   launch_fn=FakeLaunch(), map_service=FakeMaps(),
                   geo=CountrySource(fetch=failing_fetch))
    qtbot.addWidget(w)
    messages = []
    w.statusBar().messageChanged.connect(messages.append)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: "Could not download the country database" in messages)


def test_scan_error_in_english(qtbot, english):
    from jase.ui.scan_worker import ScanWorker
    seen = []

    async def boom(*a, **kw):
        raise RuntimeError("kaput")

    worker = ScanWorker(GAMES["cod2"], [], scan_fn=boom)
    worker.status.connect(seen.append)
    worker.start()
    qtbot.waitUntil(lambda: worker.isFinished())
    assert seen[-1] == "Scan error: kaput"


def _favorites_status_messages(window, qtbot, favorites):
    from test_main_window import FakeQuery
    window.cfg.favorites = {"cod2": favorites}
    window._query_fn = FakeQuery(delay=0.1)
    window.select_game("cod2")
    qtbot.waitUntil(lambda: not window.is_scanning())
    messages = []
    window.statusBar().messageChanged.connect(messages.append)
    window.show_favorites()
    qtbot.waitUntil(lambda: not window._workers)
    return messages


def test_favorites_query_status_singular_english(window, qtbot):
    messages = _favorites_status_messages(window, qtbot, ["2.2.2.2:28960"])
    assert "Querying 1 Call of Duty 2 favorite…" in messages


def test_favorites_query_status_plural_english(window, qtbot):
    messages = _favorites_status_messages(
        window, qtbot, ["2.2.2.2:28960", "3.3.3.3:28960"])
    assert "Querying 2 Call of Duty 2 favorites…" in messages


def test_language_restart_notice_in_english(window, monkeypatch):
    from PySide6.QtWidgets import QDialog, QMessageBox
    from jase.ui.dialogs import SettingsDialog
    seen = []

    def fake_box_exec(self):
        seen.append((self.text(), [b.text() for b in self.buttons()]))
        return 0

    def fake_exec(self):
        self.language_combo.setCurrentIndex(self.language_combo.findData("es"))
        return QDialog.Accepted

    monkeypatch.setattr(QMessageBox, "exec", fake_box_exec)
    monkeypatch.setattr(SettingsDialog, "exec", fake_exec)
    monkeypatch.setattr("jase.ui.main_window.system_locale_name", lambda: "en-US")
    window.open_settings()
    assert seen == [("The language will change when you restart JASE.", ["Got it"])]
