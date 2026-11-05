"""run_daemon: hilo daemon que entrega el resultado al hilo de la UI por una señal."""
from PySide6.QtCore import QObject, Signal

from jase.ui.daemon import run_daemon


class Relay(QObject):
    done = Signal(object, object)


class Req:
    cancelled = False


def test_emits_result_with_request(qtbot):
    relay, req, got = Relay(), Req(), []
    relay.done.connect(lambda r, v: got.append((r, v)))
    run_daemon("t", req, lambda: (42,), relay.done)
    qtbot.waitUntil(lambda: got == [(req, 42)])


def test_cancelled_request_does_not_emit(qtbot):
    relay, req, got = Relay(), Req(), []
    req.cancelled = True
    relay.done.connect(lambda r, v: got.append(v))
    run_daemon("t", req, lambda: (1,), relay.done)
    qtbot.wait(200)
    assert got == []


def test_emit_runtime_error_is_swallowed(qtbot):
    import threading
    errors = []
    old = threading.excepthook
    threading.excepthook = lambda args: errors.append(args.exc_type)

    class Boom:
        def emit(self, *a):
            raise RuntimeError("cerrando")
    try:
        run_daemon("t", Req(), lambda: (1,), Boom())
        qtbot.wait(150)
    finally:
        threading.excepthook = old
    assert errors == []
