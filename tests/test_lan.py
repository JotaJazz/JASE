"""Búsqueda en la red local: puertos, difusión real en localhost y escaneo en la ventana."""
import asyncio
import socket
import threading

from jase import lan
from jase.config import Config
from jase.games import GAMES
from jase.models import ServerAddress, ServerInfo


def test_lan_ports_per_game_and_family():
    assert lan.lan_ports(GAMES["cod2"]) == [28960, 28961, 28962, 28963]
    assert lan.lan_ports(GAMES["cs16"]) == list(range(27015, 27021))
    assert lan.lan_ports(GAMES["mohaa"])[0] == 12300
    assert set(lan.GAME_PORTS) <= set(GAMES)
    assert all(g.family in lan.FAMILY_PORTS for g in GAMES.values())


def test_broadcast_addresses_always_include_limited_broadcast():
    targets = lan.broadcast_addresses()
    assert targets[0] == "255.255.255.255"
    assert all(t.endswith(".255") for t in targets)


def test_discover_finds_a_server_that_answers(monkeypatch):
    server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    server.bind(("127.0.0.1", 0))
    server.settimeout(2)
    port = server.getsockname()[1]
    monkeypatch.setattr(lan, "lan_ports", lambda game: [port])
    received = []

    def answer():
        try:
            data, addr = server.recvfrom(2048)
        except OSError:
            return
        received.append(data)
        server.sendto(b"\xff\xff\xff\xffstatusResponse\n", addr)

    thread = threading.Thread(target=answer)
    thread.start()
    try:
        found = asyncio.run(lan.discover(GAMES["cod2"], timeout=0.5, targets=["127.0.0.1"]))
    finally:
        thread.join()
        server.close()
    assert found == [ServerAddress("127.0.0.1", port)]
    assert received == [lan.PROBES["quake3"]]


def test_scan_lan_queries_the_responders(monkeypatch):
    from jase import scanner
    addr = ServerAddress("192.168.1.20", 28960)

    async def fake_discover(game, timeout=1.5, targets=None):
        return [addr]

    async def fake_query(family, addresses, on_server, **kw):
        assert family == "quake3" and list(addresses) == [addr]
        assert kw["version_key"] == GAMES["cod2"].version_key
        on_server(ServerInfo(addr, addr, "LAN", "m", "dm", 1, 8, False, "1.3", 1))

    monkeypatch.setattr(lan, "discover", fake_discover)
    monkeypatch.setattr(scanner, "query_servers", fake_query)
    seen, status = [], []
    result = asyncio.run(lan.scan_lan(GAMES["cod2"], [], seen.append, status.append))
    assert [i.name for i in seen] == ["LAN"]
    assert (result.servers_listed, result.servers_responded) == (1, 1)
    assert status == ["1 servidor en la red local, consultando…"]


def test_window_lan_scan_does_not_replace_saved_list(qtbot, tmp_path):
    from jase.ui.main_window import MainWindow
    internet = ServerAddress("8.8.8.8", 28960)
    local = ServerAddress("192.168.1.20", 28960)

    def fake(address, name):
        async def scan(game, masters, on_server, on_status, on_progress=None, **kw):
            on_server(ServerInfo(address, address, name, "m", "dm", 1, 8, False, "1.3", 5))
        return scan

    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=fake(internet, "Internet"),
                   lan_scan_fn=fake(local, "Casa"))
    qtbot.addWidget(w)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    w.ui_actions["scan_lan"].trigger()
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1
                    and w.model.servers()[0].name == "Casa")
    assert [i.name for i in w._cache["cod2"]] == ["Internet"]


def test_refreshing_a_lan_server_keeps_the_internet_list(qtbot, tmp_path):
    from jase.ui.main_window import MainWindow
    internet = ServerAddress("8.8.8.8", 28960)
    local = ServerAddress("192.168.1.20", 28960)

    def info(address, name):
        return ServerInfo(address, address, name, "m", "dm", 1, 8, False, "1.3", 5)

    async def scan(game, masters, on_server, on_status, on_progress=None, **kw):
        on_server(info(internet, "Internet"))

    async def lan_scan(game, masters, on_server, on_status, on_progress=None, **kw):
        on_server(info(local, "Casa"))

    async def query(family, addresses, on_server, **kw):
        for a in addresses:
            on_server(info(a, "Casa (refrescado)"))

    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=scan, lan_scan_fn=lan_scan,
                   query_fn=query)
    qtbot.addWidget(w)
    w.select_game("cod2")
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.rowCount() == 1)
    w.scan_lan()
    qtbot.waitUntil(lambda: not w.is_scanning() and w.model.servers()[0].name == "Casa")
    w.select_address(local)
    w.refresh_selected()
    qtbot.waitUntil(lambda: w.model.servers()[0].name == "Casa (refrescado)")
    assert [i.name for i in w._cache["cod2"]] == ["Internet"]
    w.select_game("q3")
    w.select_game("cod2")  # vuelve la lista de internet guardada
    assert [i.name for i in w.model.servers()] == ["Internet"] and not w._lan_view
