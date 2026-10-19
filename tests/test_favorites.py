"""Favoritos: lógica pura, config, filas offline en el modelo y estrella de la ficha."""
import json

import pytest
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QPushButton

from jase.config import Config, load_config, save_config
from jase.games import ALL_VERSIONS
from jase.models import QuickFilters, ServerAddress, ServerInfo
from jase.ui import favorites, theme
from jase.ui.server_card import ServerCard
from jase.ui.server_model import (
    COL_COUNTRY, COL_MAP, COL_NAME, COL_PING, COL_PLAYERS, SORT_ROLE, ServerFilterProxy,
    ServerTableModel,
)


def mk(host, ping=42, players=3, version="1.3"):
    a = ServerAddress(host, 28960)
    return ServerInfo(a, a, f"srv {host}", "mp_toujane", "tdm", players, 20, False,
                      version, ping, [], {})


# ------------------------------------------------------------------ lógica pura
def test_parse_address():
    assert favorites.parse_address("1.2.3.4:28960") == ServerAddress("1.2.3.4", 28960)
    for bad in ("", "1.2.3.4", "1.2.3.4:", ":28960", "1.2.3.4:abc", "1.2.3.4:0",
                "1.2.3.4:70000", 5, None):
        assert favorites.parse_address(bad) is None


def test_add_remove_and_query():
    cfg = Config()
    a = ServerAddress("1.2.3.4", 28960)
    assert not favorites.is_favorite(cfg, "cod2", a)
    assert favorites.add_favorite(cfg, "cod2", a)
    assert not favorites.add_favorite(cfg, "cod2", a)  # sin duplicados
    assert cfg.favorites == {"cod2": ["1.2.3.4:28960"]}
    assert favorites.is_favorite(cfg, "cod2", a)
    assert not favorites.is_favorite(cfg, "ut99", a)  # por juego
    assert favorites.favorite_addresses(cfg, "cod2") == [a]
    assert favorites.remove_favorite(cfg, "cod2", a)
    assert not favorites.remove_favorite(cfg, "cod2", a)
    assert cfg.favorites == {}  # lista vacía no queda guardada


def test_favorite_addresses_skips_invalid_entries():
    cfg = Config(favorites={"cod2": ["bogus", "5.5.5.5:1", "5.5.5.5:1"]})
    assert favorites.favorite_addresses(cfg, "cod2") == [ServerAddress("5.5.5.5", 1)]
    assert favorites.favorite_addresses(cfg, "ut99") == []


def test_offline_server_row():
    a = ServerAddress("9.9.9.9", 7778)
    info = favorites.offline_server(a)
    assert info.online is False
    assert info.address == a and info.connect_address == a
    assert info.name == "9.9.9.9:7778"
    assert (info.map, info.gametype, info.version) == ("", "", "")
    assert (info.num_players, info.max_players, info.ping_ms) == (0, 0, 0)
    assert not info.password and info.players == [] and info.rules == {}


def test_online_does_not_affect_equality():
    a = mk("1.1.1.1")
    b = mk("1.1.1.1")
    b.online = False
    assert a == b and a.online is True


# ----------------------------------------------------------------------- config
def test_favorites_roundtrip(tmp_path):
    p = tmp_path / "c.json"
    cfg = Config(favorites={"cod2": ["1.2.3.4:28960", "5.6.7.8:28960"], "ut99": ["9.9.9.9:7778"]})
    save_config(cfg, p)
    assert load_config(p).favorites == cfg.favorites


def test_v1_config_without_favorites_loads_empty(tmp_path):
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"theme": "dark", "game_paths": {"cod2": "X"}}))
    cfg = load_config(p)
    assert cfg.favorites == {} and cfg.theme == "dark"


def test_invalid_favorites_are_dropped_per_entry(tmp_path):
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"theme": "light", "favorites": {
        "cod2": ["1.2.3.4:28960", 5, None, "1.2.3.4:28960"], "ut99": "no-es-lista",
        "mohaa": [], "cod1": ["7.7.7.7:28960"]}}))
    cfg = load_config(p)
    assert cfg.favorites == {"cod2": ["1.2.3.4:28960"], "cod1": ["7.7.7.7:28960"]}
    assert cfg.theme == "light"
    p.write_text(json.dumps({"theme": "light", "favorites": ["x"]}))
    assert load_config(p).favorites == {}


# ------------------------------------------------------------- modelo y proxy
def test_offline_row_display_is_muted_and_sorts_last_by_ping(qtbot):
    m = ServerTableModel()
    m.set_mode("dark")
    m.add_or_update(favorites.offline_server(ServerAddress("9.9.9.9", 1)))
    m.add_or_update(mk("1.1.1.1", ping=900))
    muted = theme.TOKENS["dark"]["muted"]
    for col in (COL_NAME, COL_PING, COL_PLAYERS, COL_MAP):
        assert m.data(m.index(0, col), Qt.ForegroundRole).name() == muted
    assert m.data(m.index(1, COL_NAME), Qt.ForegroundRole) is None
    assert "sin respuesta" in m.data(m.index(0, COL_NAME))
    assert m.data(m.index(0, COL_NAME)).startswith("9.9.9.9:1")
    assert m.data(m.index(0, COL_PING)) in ("", "—")
    assert m.data(m.index(0, COL_PLAYERS)) in ("", "—")
    assert m.data(m.index(0, COL_PING), SORT_ROLE) > m.data(m.index(1, COL_PING), SORT_ROLE)
    proxy = ServerFilterProxy()
    proxy.setSourceModel(m)
    proxy.sort(COL_PING, Qt.AscendingOrder)
    assert proxy.data(proxy.index(1, COL_NAME)).startswith("9.9.9.9")


def test_offline_row_country_lookup_does_not_crash(qtbot):
    m = ServerTableModel()
    m.set_country_lookup(lambda ip: "ar" if ip[0].isdigit() else None)  # como geo: no-IP → None
    m.add_or_update(favorites.offline_server(ServerAddress("9.9.9.9", 1)))
    m.add_or_update(favorites.offline_server(ServerAddress("host.example", 1)))
    ip, host = m.index(0, COL_COUNTRY), m.index(1, COL_COUNTRY)
    assert m.data(ip) == "" and m.data(ip, Qt.DecorationRole) is not None
    assert m.data(ip, Qt.ToolTipRole) == "Argentina"
    assert m.data(host) == "" and m.data(host, Qt.DecorationRole) is None
    assert m.data(host, Qt.ToolTipRole) is None and m.data(host, SORT_ROLE) == ""


def test_model_remove(qtbot):
    m = ServerTableModel()
    for host in ("1.1.1.1", "2.2.2.2", "3.3.3.3"):
        m.add_or_update(mk(host))
    assert m.remove(ServerAddress("2.2.2.2", 28960))
    assert not m.remove(ServerAddress("2.2.2.2", 28960))
    assert [s.address.host for s in m.servers()] == ["1.1.1.1", "3.3.3.3"]
    m.add_or_update(mk("3.3.3.3", ping=7))  # el índice sigue siendo coherente
    assert m.rowCount() == 2 and m.server_at(1).ping_ms == 7


def test_quick_filters_and_version_hide_offline_unless_bypassed(qtbot):
    m = ServerTableModel()
    m.add_or_update(favorites.offline_server(ServerAddress("9.9.9.9", 1)))
    proxy = ServerFilterProxy()
    proxy.setSourceModel(m)
    proxy.set_quick_filters(QuickFilters(hide_empty=True))
    proxy.set_version("1.3")
    assert proxy.rowCount() == 0
    proxy.set_quick_filters(QuickFilters())
    proxy.set_version(ALL_VERSIONS)
    assert proxy.rowCount() == 1


# ------------------------------------------------------------------------ ficha
@pytest.fixture
def card(qtbot):
    c = ServerCard()
    qtbot.addWidget(c)
    c.resize(700, 150)
    c.show()
    return c


def test_card_favorite_toggle(card, qtbot):
    button = card.findChild(QPushButton, "favorite_toggle")
    assert button is not None
    card.set_server(mk("1.1.1.1"), "dark")
    card.set_favorite(False)
    assert button.text() == "☆" and "Agregar" in button.toolTip()
    card.set_favorite(True)
    assert button.text() == "★" and "Quitar" in button.toolTip()
    with qtbot.waitSignal(card.favorite_toggle_requested):
        button.click()
    assert button.text() == "★"  # la ventana decide; el botón no cambia solo


def test_card_shows_offline_server(card):
    from PySide6.QtWidgets import QLabel
    info = favorites.offline_server(ServerAddress("9.9.9.9", 7778))
    card.set_server(info, "dark")
    assert card.findChild(QLabel, "card_ping").text() == "Sin respuesta"
    assert card.findChild(QLabel, "card_players").text() == "—"
    assert card.findChild(QLabel, "card_map").text() == "—"
    assert card.findChild(QLabel, "card_address").text() == "9.9.9.9:7778"
    assert not card.findChild(QLabel, "card_lock").isVisible()
