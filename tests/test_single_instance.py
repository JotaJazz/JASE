import itertools
import subprocess
import sys

from PySide6.QtNetwork import QLocalSocket

from jase.ui import single_instance
from jase.ui.single_instance import LinkServer, send_to_running

_n = itertools.count()


def _name():
    return f"jase-test-{next(_n)}-{id(_n)}"


def test_server_name_has_user_and_safe_chars(monkeypatch):
    monkeypatch.setattr(single_instance.getpass, "getuser", lambda: "ana maría/x")
    name = single_instance.server_name()
    assert name.startswith("jase-link-") and "/" not in name and " " not in name
    assert name.isascii()


def test_send_without_server_returns_false(monkeypatch, qtbot):
    monkeypatch.setattr(single_instance, "server_name", _name)
    assert send_to_running("jase://x", timeout_ms=100) is False


def test_send_delivers_lines(monkeypatch, qtbot):
    name = _name()
    monkeypatch.setattr(single_instance, "server_name", lambda: name)
    server = LinkServer(name)
    assert server.listen()
    got = []
    server.url_received.connect(got.append)
    # Como en la app real, envía otro proceso (con el emisor en un hilo de este proceso el
    # test se colgaba en Windows); este sigue con su bucle de eventos.
    code = ("import sys; from PySide6.QtCore import QCoreApplication; "
            "from jase.ui import single_instance as s; app = QCoreApplication([]); "
            "s.server_name = lambda: sys.argv[1]; "
            "sys.exit(0 if s.send_to_running(sys.argv[2], 3000) else 1)")
    proc = subprocess.Popen([sys.executable, "-c", code, name, "jase://connect/cod2/1.1.1.1:1"])
    try:
        qtbot.waitUntil(lambda: bool(got), timeout=10000)
        qtbot.waitUntil(lambda: proc.poll() is not None, timeout=10000)
    finally:
        if proc.poll() is None:
            proc.kill()
        proc.wait()
    assert got == ["jase://connect/cod2/1.1.1.1:1"] and proc.returncode == 0
    server.close()


def test_oversized_input_is_dropped(qtbot):
    name = _name()
    server = LinkServer(name)
    assert server.listen()
    got = []
    server.url_received.connect(got.append)
    sock = QLocalSocket()
    sock.connectToServer(name)
    assert sock.waitForConnected(1000)
    sock.write(b"x" * 5000 + b"\n")
    sock.flush()
    qtbot.wait(300)
    assert got == []
    sock.abort()
    server.close()


def test_listen_recovers(qtbot):
    name = _name()
    a = LinkServer(name)
    assert a.listen()
    b = LinkServer(name)
    assert b.listen()  # removeServer + reintento
    a.close()
    b.close()


def test_many_lines_on_one_connection_emit_once(qtbot):
    name = _name()
    server = LinkServer(name)
    assert server.listen()
    got = []
    server.url_received.connect(got.append)
    sock = QLocalSocket()
    sock.connectToServer(name)
    assert sock.waitForConnected(1000)
    sock.write((b"a" * 100 + b"\n") * 50)
    sock.flush()
    # En Windows (named pipes) la entrega tarda más: esperar la primera y dar margen.
    qtbot.waitUntil(lambda: bool(got), timeout=3000)
    qtbot.wait(300)
    assert len(got) == 1
    sock.abort()
    server.close()


def test_idle_connection_is_closed(qtbot, monkeypatch):
    monkeypatch.setattr(single_instance, "READ_TIMEOUT_MS", 50)
    name = _name()
    server = LinkServer(name)
    assert server.listen()
    sock = QLocalSocket()
    sock.connectToServer(name)
    assert sock.waitForConnected(1000)
    qtbot.waitUntil(lambda: sock.state() == QLocalSocket.LocalSocketState.UnconnectedState,
                    timeout=2000)
    server.close()
