"""Ventana principal, árbol y tabla en inglés (el español lo cubren los tests existentes)."""
import pytest
from PySide6.QtCore import Qt

from jase import i18n
from jase.browse import BASE_MOD, Breakdown
from jase.config import Config
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
