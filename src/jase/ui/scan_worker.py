"""Hilo que ejecuta el scanner asíncrono y reenvía resultados como señales Qt."""
from __future__ import annotations

import asyncio
import threading

from PySide6.QtCore import QThread, Signal

from jase import identify, rcon, scanner
from jase.i18n import tr
from jase.games import GameDef
from jase.models import ServerAddress, ServerInfo


class _AsyncWorker(QThread):
    """QThread que corre una corrutina en su propio bucle y se puede cancelar.

    Las subclases implementan `_work()`. `generation` lo usa la ventana para
    descartar señales de un worker obsoleto.
    """

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self.generation = 0
        self._lock = threading.Lock()
        self._loop: asyncio.AbstractEventLoop | None = None
        self._task: asyncio.Task | None = None
        self._cancel_requested = False

    def cancel(self) -> None:
        with self._lock:
            self._cancel_requested = True
            loop, task = self._loop, self._task
        if loop is not None and task is not None:
            try:
                loop.call_soon_threadsafe(task.cancel)
            except RuntimeError:
                pass  # el bucle ya terminó

    async def _main(self) -> None:
        with self._lock:
            self._loop = asyncio.get_running_loop()
            self._task = asyncio.current_task()
            if self._cancel_requested:
                return
        await self._work()

    async def _work(self) -> None:
        raise NotImplementedError

    def _on_error(self, exc: Exception) -> None:
        pass

    def _on_end(self) -> None:
        pass

    def run(self) -> None:
        try:
            asyncio.run(self._main())
        except asyncio.CancelledError:
            pass
        except Exception as exc:  # noqa: BLE001 - el hilo nunca debe morir en silencio
            self._on_error(exc)
        finally:
            with self._lock:
                self._loop = None
                self._task = None
            self._on_end()


class ScanWorker(_AsyncWorker):
    server_found = Signal(object)
    status = Signal(str)
    progress = Signal(int, int)  # (consultados, total) en la fase de servidores

    def __init__(self, game: GameDef, masters: list[tuple[str, int]],
                 scan_fn=scanner.scan, steam_key: str = "",
                 steam_include_empty: bool = False, parent=None) -> None:
        super().__init__(parent)
        self.game = game
        self._steam_key = steam_key
        self._steam_include_empty = steam_include_empty
        self.masters = list(masters)
        self._scan_fn = scan_fn
        # True si scan_fn lanzó una excepción inesperada o ningún master respondió:
        # en ambos casos el resultado no debe reemplazar la última lista completa.
        self.failed = False
        self.result: scanner.ScanResult | None = None
        self.lan = False  # búsqueda en la red local: no reemplaza la lista guardada

    async def _work(self) -> None:
        extra = ({"steam_key": self._steam_key,
                  "steam_include_empty": self._steam_include_empty}
                 if self.game.uses_steam_api else {})
        result = await self._scan_fn(self.game, self.masters,
                                     self.server_found.emit, self.status.emit,
                                     on_progress=self.progress.emit, **extra)
        if isinstance(result, scanner.ScanResult):
            self.result = result
            if not result.ok:
                self.failed = True

    def _on_error(self, exc: Exception) -> None:
        self.failed = True
        self.status.emit(tr("Error en el escaneo: {error}", error=exc))


class QueryWorker(_AsyncWorker):
    """Consulta una lista fija de servidores (sin masters): refrescar uno, favoritos.

    `server_found` por cada respuesta válida; `done` al terminar (también si se
    canceló o falló). `answered` cuenta las respuestas.
    """

    server_found = Signal(object)
    done = Signal()

    def __init__(self, game: GameDef, addresses: list[ServerAddress],
                 query_fn=scanner.query_servers, parent=None) -> None:
        super().__init__(parent)
        self.game = game
        self.addresses = list(addresses)
        self._query_fn = query_fn
        self.answered = 0
        self.failed = False
        self.error: str | None = None  # mensaje de la excepción si query_fn falló

    def _found(self, info: ServerInfo) -> None:
        self.answered += 1
        self.server_found.emit(info)

    async def _work(self) -> None:
        await self._query_fn(self.game.family, self.addresses, self._found,
                             **scanner.game_query_options(self.game))

    def _on_error(self, exc: Exception) -> None:
        self.failed = True
        self.error = str(exc)

    def _on_end(self) -> None:
        self.done.emit()


class DetectWorker(_AsyncWorker):
    """«Detectar juego»: consulta una dirección con cada familia de protocolo a la vez.

    `server_found(familia, info)` por cada respuesta; quien escucha decide el juego
    (ver `jase.identify`)."""

    server_found = Signal(str, object)

    def __init__(self, address: ServerAddress, query_fn=scanner.query_servers,
                 parent=None) -> None:
        super().__init__(parent)
        self.address = address
        self._query_fn = query_fn

    async def _work(self) -> None:
        async def probe(family: str, addresses: list[ServerAddress]) -> None:
            def found(info: ServerInfo) -> None:
                self.server_found.emit(family, info)
            await asyncio.sleep(identify.probe_delay(family))
            await self._query_fn(family, addresses, found)

        await asyncio.gather(*(probe(f, a) for f, a in identify.probe_plan(self.address)),
                             return_exceptions=True)  # una familia que falla no tapa al resto


class RconWorker(_AsyncWorker):
    """Un comando RCON; `answered(texto, código de error)` al terminar (código "" si salió
    bien; ver rcon.ERRORS)."""

    answered = Signal(str, str)

    def __init__(self, game: GameDef, address: ServerAddress, password: str, command: str,
                 send_fn=rcon.send, parent=None) -> None:
        super().__init__(parent)
        self.game, self.address = game, address
        self._password, self.command = password, command
        self._send_fn = send_fn

    async def _work(self) -> None:
        try:
            text = await self._send_fn(self.game, self.address, self._password, self.command)
        except rcon.RconError as exc:
            self.answered.emit(exc.detail, exc.code)
            return
        self.answered.emit(text, "")

    def _on_error(self, exc: Exception) -> None:
        self.answered.emit(str(exc), "network")
