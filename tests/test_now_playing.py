"""«Ahora se juega»: orden, escaneo de los juegos listos y reutilización de la caché."""
from jase.config import Config
from jase.scanner import ScanResult
from jase.models import ServerAddress, ServerInfo
from jase.ui.main_window import MainWindow
from jase.ui.now_playing import busy_servers


def srv(host, people, bots=0, ping=40, name=None):
    a = ServerAddress(host, 28960)
    info = ServerInfo(a, a, name or host, "mp_x", "dm", people + bots, 20, False, "1.3", ping)
    info.bots = bots
    return info


def test_busy_servers_counts_people_and_sorts():
    lists = {"cod2": [srv("1.1.1.1", 3), srv("2.2.2.2", 0, bots=8), srv("3.3.3.3", 0)],
             "q3": [srv("4.4.4.4", 5, ping=90), srv("5.5.5.5", 5, ping=20)]}
    rows = [(g, i.address.host) for g, i in busy_servers(lists)]
    assert rows == [("q3", "5.5.5.5"), ("q3", "4.4.4.4"), ("cod2", "1.1.1.1")]
    assert len(busy_servers({"cod2": [srv(f"1.1.1.{i}", 1) for i in range(9)]}, limit=4)) == 4


class FakeScan:
    def __init__(self, servers):
        self.servers = servers
        self.scanned = []

    async def __call__(self, game, masters, on_server, on_status, on_progress=None, **kw):
        self.scanned.append(game.id)
        servers = self.servers.get(game.id, [])
        for info in servers:
            on_server(info)
        return ScanResult(masters_ok=1, servers_listed=len(servers),
                          servers_responded=len(servers))


def test_scans_ready_games_reuses_session_and_caches(qtbot, tmp_path):
    scan = FakeScan({"cod2": [srv("1.1.1.1", 2, name="Uno")],
                     "q3": [srv("2.2.2.2", 6, name="Seis")]})
    cfg = Config(game_paths={"cod2": "/c", "q3": "/q", "ut99": "/u"})
    w = MainWindow(cfg, tmp_path / "c.json", scan_fn=scan)
    qtbot.addWidget(w)
    w._cache["ut99"] = [srv("3.3.3.3", 1, name="Ya cargado")]
    assert {g.id for g in w.now_playing_games()} == {"cod2", "q3", "ut99"} | {
        g.id for g in w.now_playing_games() if g.launch_style == "steam"}
    w.show_now_playing()
    dialog = w._now_dialog
    qtbot.waitUntil(lambda: not w._now_workers and not w._now_queue, timeout=5000)
    assert "ut99" not in scan.scanned and {"cod2", "q3"} <= set(scan.scanned)
    names = [dialog.table.item(r, 1).text() for r in range(dialog.table.rowCount())]
    assert names == ["Seis", "Uno", "Ya cargado"]
    assert dialog.status.text() == "9 jugadores en tus juegos"
    assert w._cache["q3"][0].name == "Seis"  # la lista nueva sirve a la ventana
    dialog.table.cellDoubleClicked.emit(0, 0)
    assert w.current_game == "q3" and w.selected_server().name == "Seis"
    dialog.refresh_requested.emit()  # «Actualizar» vuelve a escanear todo
    qtbot.waitUntil(lambda: not w._now_workers and not w._now_queue, timeout=5000)
    assert "ut99" in scan.scanned
    dialog.close()


def test_no_ready_games_explains(qtbot, tmp_path, monkeypatch):
    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=FakeScan({}), steam_key="")
    qtbot.addWidget(w)
    monkeypatch.setattr(w, "can_scan", lambda game: False)  # p. ej. sin masters activos
    w.show_now_playing()
    assert w._now_dialog.status.text().startswith("No hay juegos listos")
    w._now_dialog.close()



def test_incomplete_scan_is_not_cached_and_close_stops_queue(qtbot, tmp_path, monkeypatch):
    from jase.games import GAMES

    async def unfinished(game, masters, on_server, on_status, on_progress=None, **kw):
        on_server(srv("9.9.9.9", 3))
        return None  # como un escaneo cancelado: sin ScanResult

    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=unfinished)
    qtbot.addWidget(w)
    monkeypatch.setattr(w, "now_playing_games", lambda: [GAMES["cod2"], GAMES["q3"],
                                                         GAMES["et"], GAMES["urt"]])
    w.show_now_playing()
    assert len(w._now_workers) == 3 and len(w._now_queue) == 1
    w.close()  # cancela los workers y vacía la cola
    assert w._now_workers == [] and w._now_queue == []
    w._now_refresh(force=True)
    qtbot.waitUntil(lambda: not w._now_workers and not w._now_queue, timeout=5000)
    assert "cod2" not in w._cache  # lista sin ScanResult: no se da por completa
    w._now_dialog.close()
