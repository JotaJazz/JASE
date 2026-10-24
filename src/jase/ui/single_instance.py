"""Una sola instancia: un clic en un enlace jase:// se entrega a la JASE ya abierta."""
from __future__ import annotations

import getpass
import re

from PySide6.QtCore import QObject, Signal
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
            self._buffers[socket] = bytearray()
            socket.readyRead.connect(lambda s=socket: self._on_ready(s))
            socket.disconnected.connect(lambda s=socket: self._finish(s))
            if socket.bytesAvailable():
                self._on_ready(socket)

    def _on_ready(self, socket: QLocalSocket) -> None:
        buf = self._buffers.get(socket)
        if buf is None:
            return
        room = MAX_BYTES - len(buf)
        data = bytes(socket.readAll().data())
        if room > 0:
            buf += data[:room]
        if len(data) > room:
            del self._buffers[socket]  # pasa del tope: se descarta la conexión entera
            socket.abort()
            return
        while (nl := buf.find(b"\n")) >= 0:
            line = bytes(buf[:nl])
            del buf[:nl + 1]
            self._emit(line)

    def _finish(self, socket: QLocalSocket) -> None:
        buf = self._buffers.pop(socket, None)
        if buf:
            self._emit(bytes(buf))
        socket.deleteLater()

    def _emit(self, line: bytes) -> None:
        text = line.decode("utf-8", "replace").strip()
        if text:
            self.url_received.emit(text)
