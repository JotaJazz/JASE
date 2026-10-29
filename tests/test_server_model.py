from PySide6.QtCore import Qt
from jase.ui.server_model import (
    COL_LOCK, COL_MAP, COL_NAME, COL_PING, COL_PLAYERS, SORT_ROLE, ServerFilterProxy,
    ServerTableModel,
)
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
    assert m.data(m.index(0, COL_NAME)) == "A2" and m.data(m.index(0, COL_PLAYERS)) == "4/20"
    assert m.data(m.index(0, COL_LOCK)) in ("", None)


def test_lock_icon_and_clear(qtbot):
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "A", 50, 3, "1.3", pw=True))
    assert m.data(m.index(0, COL_LOCK)) == "🔒"
    assert m.server_at(0).name == "A" and len(m.servers()) == 1
    m.clear()
    assert m.rowCount() == 0 and m.servers() == []


def test_numeric_sort_on_ping(qtbot):
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    for h, ping in [("1.1.1.1", 100), ("2.2.2.2", 9), ("3.3.3.3", 25)]:
        m.add_or_update(mk(h, h, ping, 1, "1.3"))
    p.sort(COL_PING, Qt.AscendingOrder)
    assert [p.data(p.index(r, COL_PING), SORT_ROLE) for r in range(3)] == [9, 25, 100]


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
    idx = m.index(0, COL_PING)
    assert m.data(idx, Qt.ForegroundRole) == theme.ping_color(50, "dark")
    m.set_mode("light")
    assert m.data(idx, Qt.ForegroundRole) == theme.ping_color(50, "light")
    assert theme.ping_color(50, "light").name() == theme.TOKENS["light"]["ping_good"]


def test_tooltip_shows_full_text(qtbot):
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "UPJ COD2 Zombies | Account System | UP Facil", 50, 1, "1.3",
                       map_="CTF-Bleak-CE100"))
    assert m.data(m.index(0, COL_NAME), Qt.ToolTipRole) == "UPJ COD2 Zombies | Account System | UP Facil"
    assert m.data(m.index(0, COL_MAP), Qt.ToolTipRole) == "CTF-Bleak-CE100"


def test_name_column_exposes_color_runs(qtbot):
    from jase.ui.server_model import COLOR_RUNS_ROLE
    m = ServerTableModel()
    info = mk("1.1.1.1", "Jimmy TDM", 50, 1, "1.3")
    info.raw_name = "^1Jimmy^7 TDM"
    m.add_or_update(info)
    idx = m.index(0, COL_NAME)
    assert m.data(idx) == "Jimmy TDM"
    assert [t for t, _ in m.data(idx, COLOR_RUNS_ROLE)] == ["Jimmy", " TDM"]


# ------------------------------------------------------------ filtros rápidos
def _quick_proxy():
    from jase.ui.server_model import QuickFilters  # noqa: F401
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    m.add_or_update(mk("1.1.1.1", "vacio", 40, 0, "1.3"))
    m.add_or_update(mk("2.2.2.2", "lleno", 40, 20, "1.3"))
    m.add_or_update(mk("3.3.3.3", "clave", 40, 5, "1.3", pw=True))
    m.add_or_update(mk("4.4.4.4", "lejos", 120, 5, "1.3"))
    return m, p


def _names(p):
    return sorted(p.data(p.index(r, COL_NAME)) for r in range(p.rowCount()))


def test_quick_filter_hide_empty(qtbot):
    from jase.ui.server_model import QuickFilters
    _, p = _quick_proxy()
    p.set_quick_filters(QuickFilters(hide_empty=True))
    assert _names(p) == ["clave", "lejos", "lleno"]


def test_quick_filter_hide_full(qtbot):
    from jase.ui.server_model import QuickFilters
    m, p = _quick_proxy()
    # max_players == 0 nunca cuenta como lleno
    m.add_or_update(ServerInfo(ServerAddress("5.5.5.5", 1), ServerAddress("5.5.5.5", 1),
                               "sinmax", "m", "dm", 0, 0, False, "1.3", 40, [], {}))
    p.set_quick_filters(QuickFilters(hide_full=True))
    assert _names(p) == ["clave", "lejos", "sinmax", "vacio"]


def test_quick_filter_hide_password(qtbot):
    from jase.ui.server_model import QuickFilters
    _, p = _quick_proxy()
    p.set_quick_filters(QuickFilters(hide_password=True))
    assert _names(p) == ["lejos", "lleno", "vacio"]


def test_quick_filter_max_ping(qtbot):
    from jase.ui.server_model import QuickFilters
    _, p = _quick_proxy()
    p.set_quick_filters(QuickFilters(max_ping=100))
    assert _names(p) == ["clave", "lleno", "vacio"]
    p.set_quick_filters(QuickFilters())  # 0 = sin límite
    assert p.rowCount() == 4


# ---------------------------------------------------------- barra de ocupación
def test_occupancy_ratio():
    from jase.ui.server_model import occupancy_ratio
    assert occupancy_ratio(5, 20) == 0.25
    assert occupancy_ratio(0, 20) == 0.0
    assert occupancy_ratio(25, 20) == 1.0   # nunca más de la celda
    assert occupancy_ratio(3, 0) == 0.0     # sin máximo conocido: sin barra


def test_occupancy_delegate_paints_bar(qtbot):
    from PySide6.QtGui import QColor, QImage, QPainter
    from PySide6.QtWidgets import QStyleOptionViewItem, QTableView
    from jase.ui.server_model import OccupancyDelegate, occupancy_color
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "lleno", 50, 20, "1.3"))   # barra en toda la celda
    m.add_or_update(mk("2.2.2.2", "vacio", 50, 0, "1.3"))    # sin barra
    view = QTableView(); qtbot.addWidget(view); view.setModel(m)
    delegate = OccupancyDelegate(view)

    def render(row):
        img = QImage(120, 28, QImage.Format_ARGB32)
        img.fill(QColor("#000000"))
        painter = QPainter(img)
        opt = QStyleOptionViewItem()
        opt.rect = img.rect()
        opt.widget = view
        delegate.paint(painter, opt, m.index(row, COL_PLAYERS))
        painter.end()
        return img.pixelColor(img.width() - 4, img.height() // 2)  # zona sin texto

    assert render(0) != render(1)
    c = occupancy_color("dark")
    assert 0 < c.alpha() < 255


def test_occupancy_tokens_in_both_themes():
    for mode in ("dark", "light"):
        assert theme.TOKENS[mode]["occupancy"].startswith("#")


# ---------------------------------------------------------------- columna País
COUNTRIES = {"1.1.1.1": "cl", "2.2.2.2": "br"}


def test_country_column_between_lock_and_name():
    from jase.ui.server_model import COL_COUNTRY, COLUMNS
    assert COL_LOCK < COL_COUNTRY < COL_NAME
    assert COLUMNS[COL_COUNTRY] == "País" and COLUMNS[COL_NAME] == "Servidor"


def test_country_column_icon_and_tooltip(qtbot):
    from PySide6.QtGui import QIcon
    from jase.ui.server_model import COL_COUNTRY
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "chileno", 40, 3, "1.3"))
    m.add_or_update(mk("9.9.9.9", "sin pais", 40, 3, "1.3"))
    idx, idx2 = m.index(0, COL_COUNTRY), m.index(1, COL_COUNTRY)
    assert m.data(idx, Qt.DecorationRole) is None  # sin base: vacío
    assert m.data(idx, Qt.ToolTipRole) is None
    m.set_country_lookup(COUNTRIES.get)
    icon = m.data(idx, Qt.DecorationRole)
    assert isinstance(icon, QIcon) and not icon.isNull()
    assert m.data(idx, Qt.ToolTipRole) == "Chile"
    assert m.data(idx, Qt.DisplayRole) in ("", None)  # solo la bandera
    assert m.data(idx2, Qt.DecorationRole) is None and m.data(idx2, Qt.ToolTipRole) is None
    assert m.country_code(m.server_at(0)) == "cl"


def test_set_country_lookup_repaints_country_column(qtbot):
    from jase.ui.server_model import COL_COUNTRY
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "a", 40, 3, "1.3"))
    m.add_or_update(mk("2.2.2.2", "b", 40, 3, "1.3"))
    seen = []
    m.dataChanged.connect(lambda tl, br, roles: seen.append(
        (tl.row(), tl.column(), br.row(), br.column())))
    m.set_country_lookup(COUNTRIES.get)
    assert (0, COL_COUNTRY, 1, COL_COUNTRY) in seen


def test_country_lookup_errors_are_silent(qtbot):
    from jase.ui.server_model import COL_COUNTRY

    def boom(ip):
        raise ValueError(ip)

    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "a", 40, 3, "1.3"))
    m.set_country_lookup(boom)
    assert m.data(m.index(0, COL_COUNTRY), Qt.DecorationRole) is None


def test_set_mode_repaints_name_through_ping(qtbot):
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "a", 40, 3, "1.3"))
    seen = []
    m.dataChanged.connect(lambda tl, br, roles: seen.append((tl.column(), br.column())))
    m.set_mode("light")
    lo, hi = seen[-1]
    assert lo <= min(COL_NAME, COL_PING) and hi >= max(COL_NAME, COL_PING)


def test_text_filter_matches_country_code(qtbot):
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "chileno", 40, 3, "1.3"))
    m.add_or_update(mk("2.2.2.2", "brasileño", 40, 3, "1.3"))
    m.add_or_update(mk("3.3.3.3", "otro", 40, 3, "1.3"))
    p = ServerFilterProxy()
    p.setSourceModel(m)
    p.set_text("cl")
    assert _names(p) == []  # sin base todavía
    m.set_country_lookup(COUNTRIES.get)
    assert _names(p) == ["chileno"]  # el filtro se reevalúa al llegar la base
    p.set_text("BR")
    assert _names(p) == ["brasileño"]


def test_sort_by_country(qtbot):
    from jase.ui.server_model import COL_COUNTRY
    m = ServerTableModel()
    m.set_country_lookup(COUNTRIES.get)
    m.add_or_update(mk("1.1.1.1", "chileno", 40, 3, "1.3"))
    m.add_or_update(mk("2.2.2.2", "brasileño", 40, 3, "1.3"))
    p = ServerFilterProxy()
    p.setSourceModel(m)
    p.sort(COL_COUNTRY, Qt.AscendingOrder)
    assert [p.data(p.index(r, COL_NAME)) for r in range(2)] == ["brasileño", "chileno"]


def _mk_rules(host, name, players, map_, mod):
    a = ServerAddress(host, 28960)
    return ServerInfo(a, a, name, map_, "tdm", players, 20, False, "1.3", 50, [],
                      {"fs_game": mod} if mod else {})


def test_tree_filter_combines_with_text_and_quick(qtbot):
    from jase.games import GAMES
    from jase.ui.server_model import QuickFilters
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    m.add_or_update(_mk_rules("1.1.1.1", "Alpha", 3, "mp_toujane", "mods/promod"))
    m.add_or_update(_mk_rules("2.2.2.2", "Alpha vacio", 0, "mp_toujane", "mods/promod"))
    m.add_or_update(_mk_rules("3.3.3.3", "Beta", 3, "mp_toujane", "mods/promod"))
    m.add_or_update(_mk_rules("4.4.4.4", "Alpha otro", 3, "mp_toujane", ""))
    m.add_or_update(_mk_rules("5.5.5.5", "Alpha mapa", 3, "mp_carentan", "mods/promod"))
    game = GAMES["cod2"]
    p.set_tree_filter(game, frozenset({"promod"}), frozenset({"mp_toujane"}))
    assert p.rowCount() == 3
    p.set_text("alpha")
    p.set_quick_filters(QuickFilters(hide_empty=True))
    assert p.rowCount() == 1
    assert p.data(p.index(0, COL_NAME)) == "Alpha"


def test_tree_filter_none_shows_all(qtbot):
    from jase.games import GAMES
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    m.add_or_update(_mk_rules("1.1.1.1", "A", 3, "mp_toujane", "mods/promod"))
    m.add_or_update(_mk_rules("2.2.2.2", "B", 3, "mp_carentan", ""))
    p.set_tree_filter(GAMES["cod2"], frozenset({"promod"}), frozenset())
    assert p.rowCount() == 1
    p.set_tree_filter(None, frozenset({"promod"}), frozenset())
    assert p.rowCount() == 2
    p.set_tree_filter(GAMES["cod2"], frozenset(), frozenset())
    assert p.rowCount() == 2


def test_row_of(qtbot):
    m = ServerTableModel()
    a = mk("1.1.1.1", "A", 50, 3, "1.3")
    m.add_or_update(a)
    m.add_or_update(mk("2.2.2.2", "B", 50, 3, "1.3"))
    assert m.row_of(a.address) == 0
    assert m.row_of(ServerAddress("9.9.9.9", 28960)) == -1


def test_occupancy_classic_draws_progress_blocks(qtbot):
    from PySide6.QtGui import QColor, QImage, QPainter
    from PySide6.QtWidgets import QStyleOptionViewItem, QTableView
    from jase.ui.server_model import OccupancyDelegate
    m = ServerTableModel()
    m.add_or_update(mk("1.1.1.1", "lleno", 50, 20, "1.3"))
    m.add_or_update(mk("2.2.2.2", "vacio", 50, 0, "1.3"))
    view = QTableView(); qtbot.addWidget(view); view.setModel(m)
    delegate = OccupancyDelegate(view)
    delegate.set_mode("classic")
    navy = QColor(theme.TOKENS["classic"]["selection"])

    def render(row):
        img = QImage(160, 28, QImage.Format_ARGB32)
        img.fill(QColor("#ffffff"))
        painter = QPainter(img)
        opt = QStyleOptionViewItem()
        opt.rect = img.rect()
        opt.widget = view
        delegate.paint(painter, opt, m.index(row, COL_PLAYERS))
        painter.end()
        return {img.pixelColor(x, img.height() // 2).name() for x in range(80, 150)}

    assert navy.name() in render(0)       # bloques en la barra llena
    assert navy.name() not in render(1)   # barra vacía: solo el marco


# ------------------------------------------------------------------------ bots
def test_players_column_shows_humans_and_bots(qtbot):
    from jase.ui.server_model import QuickFilters
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    bots_only = mk("1.1.1.1", "bots", 50, 6, "1.3")
    bots_only.bots = 6
    mixed = mk("2.2.2.2", "mixto", 50, 8, "1.3")
    mixed.bots = 6
    m.add_or_update(bots_only)
    m.add_or_update(mixed)
    m.add_or_update(mk("3.3.3.3", "gente", 50, 3, "1.3"))
    assert m.data(m.index(1, COL_PLAYERS)) == "2+6/20"
    assert m.data(m.index(1, COL_PLAYERS), Qt.ToolTipRole) == "2 jugadores · 6 bots"
    assert m.data(m.index(2, COL_PLAYERS)) == "3/20"
    # se ordena por personas: 3 personas > 2 personas + 6 bots > solo bots
    p.sort(COL_PLAYERS, Qt.DescendingOrder)
    assert [p.data(p.index(r, COL_NAME)) for r in range(3)] == ["gente", "mixto", "bots"]
    p.set_quick_filters(QuickFilters(hide_bots=True))
    assert _names(p) == ["gente", "mixto"]


def test_country_quick_filter(qtbot):
    from jase.ui.server_model import QuickFilters
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    m.set_country_lookup({"1.1.1.1": "CL", "2.2.2.2": "AR"}.get)
    for host in ("1.1.1.1", "2.2.2.2", "3.3.3.3"):
        m.add_or_update(mk(host, host, 50, 1, "1.3"))
    p.set_quick_filters(QuickFilters(country="cl"))
    assert _names(p) == ["1.1.1.1"]
    p.set_quick_filters(QuickFilters())
    assert p.rowCount() == 3


def test_tree_filter_by_countries(qtbot):
    m = ServerTableModel(); p = ServerFilterProxy(); p.setSourceModel(m)
    m.set_country_lookup({"1.1.1.1": "CL", "2.2.2.2": "AR"}.get)
    m.add_or_update(_mk_rules("1.1.1.1", "A", 3, "mp_toujane", ""))
    m.add_or_update(_mk_rules("2.2.2.2", "B", 3, "mp_toujane", ""))
    m.add_or_update(_mk_rules("3.3.3.3", "C", 3, "mp_toujane", ""))
    p.set_tree_filter(None, frozenset(), frozenset(), frozenset({"cl"}))
    assert p.rowCount() == 1 and p.data(p.index(0, COL_NAME)) == "A"
    p.set_tree_filter(None, frozenset(), frozenset(), frozenset({"cl", "ar"}))
    assert p.rowCount() == 2
    p.set_tree_filter(None, frozenset(), frozenset())
    assert p.rowCount() == 3
