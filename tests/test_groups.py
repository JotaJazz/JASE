"""Grupos de favoritos: lógica pura, config, import/export y vista Favoritos."""
from PySide6.QtWidgets import QApplication

from jase import favorites_io
from jase.config import Config, load_config, save_config
from jase.models import ServerAddress, ServerInfo
from jase.ui import favorites
from jase.ui import window_groups
from jase.ui.main_window import MainWindow

A = ServerAddress("1.1.1.1", 28960)
B = ServerAddress("2.2.2.2", 28960)
C = ServerAddress("3.3.3.3", 28960)


def srv(address, name):
    return ServerInfo(address, address, name, "m", "dm", 2, 16, False, "1.3", 20)


def test_set_group_cleans_and_group_names_only_count_current_favorites():
    cfg = Config(favorites={"cod2": [str(A), str(B)]})
    favorites.set_group(cfg, "cod2", A, "  Clan   Nox ")
    favorites.set_group(cfg, "cod2", B, "lan")
    favorites.set_group(cfg, "cod2", "9.9.9.9:1", "Viejo")  # ya no es favorito
    assert favorites.get_group(cfg, "cod2", A) == "Clan Nox"
    assert favorites.group_names(cfg, "cod2") == ["Clan Nox", "lan"]
    favorites.set_group(cfg, "cod2", B, "   ")
    assert favorites.get_group(cfg, "cod2", B) == ""
    favorites.remove_favorite(cfg, "cod2", A)
    assert favorites.get_group(cfg, "cod2", A) == ""
    assert len(favorites.clean_group("x" * 100)) == favorites.GROUP_MAX


def test_groups_roundtrip_config_and_export(tmp_path):
    cfg = Config(favorites={"cod2": [str(A)]}, favorite_groups={"cod2": {str(A): "Clan"}})
    p = tmp_path / "c.json"
    save_config(cfg, p)
    assert load_config(p).favorite_groups == {"cod2": {str(A): "Clan"}}
    data = favorites_io.export_favorites(cfg)
    assert data["favorites"]["cod2"] == [{"address": str(A), "group": "Clan"}]
    fresh = Config()
    favorites_io.import_favorites(fresh, data)
    assert favorites.get_group(fresh, "cod2", A) == "Clan"


class FakeQuery:
    def __init__(self, names):
        self.names = names
        self.asked = []

    async def __call__(self, family, addresses, on_server, **kw):
        self.asked.append(list(addresses))
        for a in addresses:
            on_server(srv(a, self.names[a]))


def make(qtbot, tmp_path, query):
    async def scan(*a, **k):
        return None
    cfg = Config(favorites={"cod2": [str(A), str(B), str(C)]},
                 favorite_groups={"cod2": {str(A): "Clan", str(B): "LAN"}})
    w = MainWindow(cfg, tmp_path / "c.json", scan_fn=scan, query_fn=query)
    qtbot.addWidget(w)
    return w


def test_group_combo_filters_favorites_view_and_copy(qtbot, tmp_path):
    query = FakeQuery({A: "Clan Nox", B: "Casa", C: "Suelto"})
    w = make(qtbot, tmp_path, query)
    w.select_game("cod2")
    assert not w.group_combo_action.isVisible()
    w.show_favorites()
    qtbot.waitUntil(lambda: w.model.rowCount() == 3 and w._favorites_worker is None)
    combo = w.group_combo
    assert [combo.itemText(i) for i in range(combo.count())] == [
        "Todos los grupos", "Clan", "LAN", "Sin grupo"]
    assert w.group_combo_action.isVisible()
    combo.setCurrentIndex(combo.findData("Clan"))
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and w._favorites_worker is None)
    assert query.asked[-1] == [A]  # solo se consulta el grupo
    w.copy_favorites_as_text()
    assert QApplication.clipboard().text().splitlines() == [
        "Call of Duty 2 · Clan · JASE", f"{A}  Clan Nox"]
    combo.setCurrentIndex(combo.findData(""))
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and w._favorites_worker is None)
    assert w.model.servers()[0].address == C
    w.select_game("ut99")
    assert not w.group_combo_action.isVisible()


def test_move_selected_to_group_from_menu(qtbot, tmp_path, monkeypatch):
    from PySide6.QtWidgets import QInputDialog
    w = make(qtbot, tmp_path, FakeQuery({A: "Clan Nox", B: "Casa", C: "Suelto"}))
    w.select_game("cod2")
    w.show_favorites()
    qtbot.waitUntil(lambda: w.model.rowCount() == 3 and w._favorites_worker is None)
    w.select_address(C)
    w._rebuild_group_menu()
    texts = [a.text() for a in w.group_menu.actions() if not a.isSeparator()]
    assert texts == ["Clan", "LAN", "Sin grupo", "Nuevo grupo…"]
    monkeypatch.setattr(QInputDialog, "getText",
                        staticmethod(lambda *a, **k: ("Probar", True)))
    w.group_menu.actions()[-1].trigger()
    assert favorites.get_group(w.cfg, "cod2", C) == "Probar"
    assert load_config(tmp_path / "c.json").favorite_groups["cod2"][str(C)] == "Probar"
    assert "Probar" in [w.group_combo.itemText(i) for i in range(w.group_combo.count())]
    # Con un grupo a la vista, mover fuera de él quita la fila.
    w.group_combo.setCurrentIndex(w.group_combo.findData("Probar"))
    qtbot.waitUntil(lambda: w.model.rowCount() == 1 and w._favorites_worker is None)
    w.select_address(C)
    w.move_selected_to_group("")
    assert w.statusBar().currentMessage() == "Suelto quedó sin grupo"
    # «Probar» quedó vacío: el combo vuelve a «Todos los grupos» y se ven los 3.
    assert w.group_combo.currentData() == window_groups.ALL_GROUPS
    qtbot.waitUntil(lambda: w.model.rowCount() == 3 and w._favorites_worker is None)
