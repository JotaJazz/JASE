from PySide6.QtCore import Qt
from PySide6.QtGui import QIcon

from jase.browse import BASE_MOD, Breakdown
from jase.ui.game_tree import GameTree


def _bd(mods=(), maps=(("dm1", 5), ("dm2", 3)), other=0):
    return Breakdown(tuple(mods), tuple(maps), other)


def _folder(tree, gid, name):
    item = tree.game_item(gid)
    for i in range(item.childCount()):
        if item.child(i).text(0) == name:
            return item.child(i)
    return None


def _tree():
    t = GameTree()
    t.set_games(["a", "b"])
    return t


def test_set_games_order_and_labels(qapp):
    t = _tree()
    t.set_game_label("a", "Juego A", "Juego A completo")
    assert t.game_ids() == ["a", "b"]
    assert t.topLevelItemCount() == 2
    assert t.game_item("a").text(0) == "Juego A"
    assert t.game_item("a").toolTip(0) == "Juego A completo"
    t.set_game_icon("a", QIcon())


def test_breakdown_creates_checkable_leaves(qapp):
    t = _tree()
    t.set_breakdown("a", _bd(mods=[("rpg", 2), (BASE_MOD, 6)]))
    mods, maps = _folder(t, "a", "Mods"), _folder(t, "a", "Mapas")
    assert mods.childCount() == 2 and maps.childCount() == 2
    leaf = maps.child(0)
    assert leaf.text(0) == "dm1 · 5"
    assert leaf.data(0, Qt.UserRole) == "dm1"
    assert leaf.flags() & Qt.ItemIsUserCheckable
    assert leaf.checkState(0) == Qt.Unchecked
    t.set_breakdown("a", None)
    assert t.game_item("a").childCount() == 0


def test_mods_folder_hidden_without_mods(qapp):
    t = _tree()
    t.set_breakdown("a", _bd())
    assert _folder(t, "a", "Mods") is None
    assert _folder(t, "a", "Mapas") is not None


def test_other_maps_leaf_not_checkable(qapp):
    t = _tree()
    t.set_breakdown("a", _bd(other=7))
    maps = _folder(t, "a", "Mapas")
    other = maps.child(maps.childCount() - 1)
    assert other.text(0) == "otros… · 7"
    assert not other.flags() & Qt.ItemIsUserCheckable
    t.set_breakdown("a", _bd(other=0))
    assert _folder(t, "a", "Mapas").childCount() == 2


def test_check_leaf_emits_marks_changed(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("a", _bd())
    got = []
    t.marks_changed.connect(got.append)
    _folder(t, "a", "Mapas").child(1).setCheckState(0, Qt.Checked)
    assert got == ["a"]
    assert t.marks("a") == (frozenset(), frozenset({"dm2"}))
    _folder(t, "a", "Mapas").child(1).setCheckState(0, Qt.Unchecked)
    assert t.marks("a") == (frozenset(), frozenset())
    before = len(got)
    t.set_breakdown("a", _bd(maps=(("dm1", 9), ("dm2", 3))))  # solo cuentas: sin señal
    assert len(got) == before == 2


def test_update_breakdown_keeps_marks_and_expansion(qapp):
    t = _tree()
    t.set_breakdown("a", _bd(mods=[("rpg", 2)]))
    t.game_item("a").setExpanded(True)
    maps = _folder(t, "a", "Mapas")
    maps.setExpanded(True)
    maps.child(0).setCheckState(0, Qt.Checked)
    t.set_breakdown("a", _bd(mods=[("rpg", 4)], maps=(("dm1", 8), ("dm2", 1))))
    maps2 = _folder(t, "a", "Mapas")
    assert maps2 is maps  # en sitio
    assert maps2.child(0).text(0) == "dm1 · 8"
    assert maps2.child(0).checkState(0) == Qt.Checked
    assert maps2.isExpanded() and t.game_item("a").isExpanded()
    assert not _folder(t, "a", "Mods").isExpanded()
    # cambian las claves: se reconstruye pero conserva expansión y marcas
    t.set_breakdown("a", _bd(mods=[("rpg", 4)], maps=(("dm1", 8), ("dm3", 1))))
    maps3 = _folder(t, "a", "Mapas")
    assert maps3.isExpanded()
    assert maps3.child(0).checkState(0) == Qt.Checked


def test_vanished_leaf_drops_mark(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("a", _bd())
    _folder(t, "a", "Mapas").child(1).setCheckState(0, Qt.Checked)
    got = []
    t.marks_changed.connect(got.append)
    t.set_breakdown("a", _bd(maps=(("dm1", 5),)))
    assert t.marks("a") == (frozenset(), frozenset())
    assert got == ["a"]


def test_check_leaf_of_other_game_activates_it_first(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("b", _bd())
    events = []
    t.game_activated.connect(lambda g: events.append(("act", g)))
    t.marks_changed.connect(lambda g: events.append(("marks", g)))
    _folder(t, "b", "Mapas").child(0).setCheckState(0, Qt.Checked)
    assert events == [("act", "b"), ("marks", "b")]


def test_set_games_reorder_keeps_marks(qapp):
    t = _tree()
    t.set_game_label("a", "A", "tip")
    t.set_current_game("a")
    t.set_breakdown("a", _bd())
    t.game_item("a").setExpanded(True)
    _folder(t, "a", "Mapas").child(0).setCheckState(0, Qt.Checked)
    t.set_games(["b", "a"])
    assert t.game_ids() == ["b", "a"]
    assert t.game_item("a").text(0) == "A"
    assert t.game_item("a").isExpanded()
    assert t.marks("a") == (frozenset(), frozenset({"dm1"}))
    assert _folder(t, "a", "Mapas").child(0).checkState(0) == Qt.Checked


def _mark(t, gid, name, idx):
    _folder(t, gid, name).child(idx).setCheckState(0, Qt.Checked)


def test_breakdown_none_keeps_marks(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("a", _bd())
    _mark(t, "a", "Mapas", 0)
    got = []
    t.marks_changed.connect(got.append)
    t.set_breakdown("a", None)
    assert got == []
    assert t.marks("a") == (frozenset(), frozenset({"dm1"}))
    assert t.game_item("a").childCount() == 0
    t.set_breakdown("a", _bd())
    assert _folder(t, "a", "Mapas").child(0).checkState(0) == Qt.Checked


def test_no_prune_keeps_missing_marked_leaf_with_zero(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("a", _bd(mods=[("rpg", 2)]))
    _mark(t, "a", "Mods", 0)
    _mark(t, "a", "Mapas", 1)
    got = []
    t.marks_changed.connect(got.append)
    t.set_breakdown("a", _bd(maps=(("dm1", 5),)), prune=False)
    assert got == []
    assert t.marks("a") == (frozenset({"rpg"}), frozenset({"dm2"}))
    mods, maps = _folder(t, "a", "Mods"), _folder(t, "a", "Mapas")
    assert mods is not None and mods.child(0).text(0) == "rpg · 0"
    assert mods.child(0).checkState(0) == Qt.Checked
    assert maps.child(1).text(0) == "dm2 · 0"
    assert maps.child(1).checkState(0) == Qt.Checked
    # con prune (por defecto) se descartan
    t.set_breakdown("a", _bd(maps=(("dm1", 5),)))
    assert t.marks("a") == (frozenset(), frozenset())
    assert got == ["a"]


def test_label_and_icon_keep_state_and_emit_nothing(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("a", _bd())
    t.game_item("a").setExpanded(True)
    _mark(t, "a", "Mapas", 0)
    got = []
    t.marks_changed.connect(got.append)
    t.set_game_label("a", "Nuevo", "tip")
    t.set_game_icon("a", QIcon())
    assert got == []
    assert t.game_item("a").isExpanded()
    assert t.marks("a") == (frozenset(), frozenset({"dm1"}))
    assert _folder(t, "a", "Mapas").child(0).checkState(0) == Qt.Checked


def test_unchanged_breakdown_emits_nothing(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("a", _bd())
    _mark(t, "a", "Mapas", 0)
    got = []
    t.marks_changed.connect(got.append)
    t.set_breakdown("a", _bd())
    assert got == []


def test_clear_marks_unchecks_and_emits_once(qapp):
    t = _tree()
    t.set_current_game("a")
    t.set_breakdown("a", _bd(mods=[("rpg", 2)]))
    _mark(t, "a", "Mods", 0)
    _mark(t, "a", "Mapas", 0)
    got = []
    t.marks_changed.connect(got.append)
    t.clear_marks("a")
    assert got == ["a"]
    assert t.marks("a") == (frozenset(), frozenset())
    for name in ("Mods", "Mapas"):
        f = _folder(t, "a", name)
        assert all(f.child(i).checkState(0) == Qt.Unchecked for i in range(f.childCount()))


def test_set_current_game_does_not_activate(qapp):
    t = _tree()
    got = []
    t.game_activated.connect(got.append)
    t.set_current_game("b")
    t.set_current_game(None)
    assert got == []


def test_click_and_activate_game_node_emit(qapp):
    t = _tree()
    got = []
    t.game_activated.connect(got.append)
    t.itemClicked.emit(t.game_item("a"), 0)
    t.itemActivated.emit(t.game_item("b"), 0)
    assert got == ["a", "b"]
