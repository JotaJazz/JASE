from jase.config import Config
from jase.models import ServerAddress, ServerInfo
from jase.ui.main_window import MainWindow

ADDR = ServerAddress("9.9.9.9", 28960)
OTHER = ServerAddress("8.8.8.8", 28960)


def _info(addr, online=True, maxp=8):
    return ServerInfo(addr, addr, "s", "m", "dm", 3, maxp, False, "1.3", 20, online=online)


def make(qtbot, tmp_path):
    async def scan(*a, **k):
        return None
    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=scan)
    qtbot.addWidget(w)
    return w


def test_records_only_favorites_and_recents(qtbot, tmp_path):
    w = make(qtbot, tmp_path)
    w.cfg.favorites["cod2"] = [str(ADDR)]
    w._on_server_response("cod2", _info(ADDR))
    w._on_server_response("cod2", _info(OTHER))
    assert w._history.samples("cod2", str(ADDR)) and not w._history.samples("cod2", str(OTHER))
    w.cfg.recent["cod2"] = [str(OTHER)]
    w._on_server_response("cod2", _info(OTHER, online=False))
    w._on_server_response("cod2", _info(OTHER, maxp=0))
    assert not w._history.samples("cod2", str(OTHER))
    w._on_server_response("cod2", _info(OTHER))
    assert w._history.samples("cod2", str(OTHER))


def test_saved_on_close(qtbot, tmp_path):
    w = make(qtbot, tmp_path)
    w.cfg.favorites["cod2"] = [str(ADDR)]
    w._on_server_response("cod2", _info(ADDR))
    w.close()
    assert (tmp_path / "cache" / "history.json").exists()
