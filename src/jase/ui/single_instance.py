"""Una sola instancia: un clic en un enlace jase:// se entrega a la JASE ya abierta."""
from __future__ import annotations

import getpass
import re

from PySide6.QtCore import QObject, QTimer, Signal
from PySide6.QtNetwork import QLocalServer, QLocalSocket

MAX_BYTES = 1024  # por conexión; lo que sobre se descarta
READ_TIMEOUT_MS = 500


def server_name() -> str:
    try:
        user = getpass.getuser()
    except Exception:  # sin usuario resoluble (entornos raros)
        user = "user"
    return "jase-link-" + (re.sub(r"[^A-Za-z0-9_-]", "_", user)[:64] or "user")


def send_to_running(url: str, timeout_ms: int = 500) -> bool:
    """Entrega `url` a la instancia en marcha; False si no hay ninguna (nunca se queda colgado)."""
    socket = QLocalSocket()
    try:
        socket.connectToServer(server_name())
        if not socket.waitForConnected(timeout_ms):
            return False
        socket.write(url.encode("utf-8") + b"\n")
        ok = socket.waitForBytesWritten(timeout_ms)
        socket.disconnectFromServer()
        if ok and socket.state() != QLocalSocket.LocalSocketState.UnconnectedState:
            socket.waitForDisconnected(timeout_ms)
        return ok
    finally:
        socket.abort()
        socket.deleteLater()


class LinkServer(QObject):
    """Escucha en `server_name()` y emite `url_received` por cada línea recibida."""

    url_received = Signal(str)

    def __init__(self, name: str | None = None, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._name = name or server_name()
        self._server = QLocalServer(self)
        self._server.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)
        self._server.newConnection.connect(self._on_connection)
        self._buffers: dict[QLocalSocket, bytearray] = {}

    def listen(self) -> bool:
        if self._server.listen(self._name):
            return True
        QLocalServer.removeServer(self._name)  # socket viejo de una sesión caída
        return self._server.listen(self._name)

    def close(self) -> None:
        self._server.close()
        sockets = list(self._buffers)
        self._buffers.clear()
        for socket in sockets:
            socket.abort()

    def _on_connection(self) -> None:
        while (socket := self._server.nextPendingConnection()) is not None:
            socket.setParent(self)
            socket.setReadBufferSize(MAX_BYTES + 1)
            self._buffers[socket] = bytearray()
            timer = QTimer(socket)  # conexión ociosa: se cierra
            timer.setSingleShot(True)
            timer.timeout.connect(lambda s=socket: self._drop(s))
            timer.start(READ_TIMEOUT_MS)
            socket.readyRead.connect(lambda s=socket: self._on_ready(s))
            socket.disconnected.connect(lambda s=socket: self._finish(s))
            if socket.bytesAvailable():
                self._on_ready(socket)

    def _drop(self, socket: QLocalSocket) -> None:
        self._buffers.pop(socket, None)
        socket.abort()

    def _on_ready(self, socket: QLocalSocket) -> None:
        buf = self._buffers.get(socket)
        if buf is None:
            return
        buf += bytes(socket.readAll().data())
        nl = buf.find(b"\n")
        if nl < 0 or nl > MAX_BYTES:
            if len(buf) > MAX_BYTES:
                self._drop(socket)  # pasa del tope sin una línea válida
            return
        line = bytes(buf[:nl])
        del self._buffers[socket]  # una línea por conexión; el resto se descarta
        socket.abort()
        self._emit(line)

    def _finish(self, socket: QLocalSocket) -> None:
        buf = self._buffers.pop(socket, None)
        socket.deleteLater()
        if buf and len(buf) <= MAX_BYTES:
            self._emit(bytes(buf))

    def _emit(self, line: bytes) -> None:
        text = line.decode("utf-8", "replace").strip()
        if text:
            self.url_received.emit(text)
