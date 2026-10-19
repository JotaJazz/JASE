from PySide6.QtCore import Qt
from jase.ui.server_model import ServerTableModel, ServerFilterProxy, SORT_ROLE
from jase.games import ALL_VERSIONS
from jase.ui import theme
from jase.models import ServerAddress, ServerInfo


def mk(host, name, ping, players, version, map_="mp_toujane", pw=False):
    a = ServerAddress(host, 28960)
    return ServerInfo(a, a, name, map_, "tdm", players, 20, pw, version, ping, [], {})


def test_add_update_and_display(qtbot):
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "A", 50, 3, "1.3", pw=True))
    m.add_or_update(mk("1.1.1.1", "A2", 40, 4, "1.3"))   # misma dirección → actualiza
    assert m.rowCount() == 1
    assert m.data(m.index(0, 1)) == "A2" and m.data(m.index(0, 3)) == "4/20"
    assert m.data(m.index(0, 0)) in ("", None)


def test_lock_icon_and_clear(qtbot):
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "A", 50, 3, "1.3", pw=True))
    assert m.data(m.index(0, 0)) == "🔒"
    assert m.server_at(0).name == "A" and len(m.servers()) == 1
    m.clear()
    assert m.rowCount() == 0 and m.servers() == []


def test_numeric_sort_on_ping(qtbot):
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    for h, ping in [("1.1.1.1", 100), ("2.2.2.2", 9), ("3.3.3.3", 25)]:
        m.add_or_update(mk(h, h, ping, 1, "1.3"))
    p.sort(2, Qt.AscendingOrder)
    assert [p.data(p.index(r, 2), SORT_ROLE) for r in range(3)] == [9, 25, 100]


def test_text_and_version_filter(qtbot):
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    m.add_or_update(mk("1.1.1.1", "Clan Hispano", 50, 1, "1.3"))
    m.add_or_update(mk("2.2.2.2", "Euro Vets", 50, 1, "1.0", map_="mp_carentan"))
    p.set_text("hisp"); assert p.rowCount() == 1
    p.set_text("carentan"); assert p.rowCount() == 1
    p.set_text(""); p.set_version("1.3"); assert p.rowCount() == 1
    p.set_version(ALL_VERSIONS); assert p.rowCount() == 2


def test_ping_foreground_follows_mode(qtbot):
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "A", 50, 3, "1.3"))
    idx = m.index(0, 2)
    assert m.data(idx, Qt.ForegroundRole) == theme.ping_color(50, "dark")
    m.set_mode("light")
    assert m.data(idx, Qt.ForegroundRole) == theme.ping_color(50, "light")
    assert theme.ping_color(50, "light").name() == theme.TOKENS["light"]["ping_good"]
