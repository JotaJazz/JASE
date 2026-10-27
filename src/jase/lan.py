"""Búsqueda de servidores en la red local, sin master (como «LAN» en el ASE clásico).

Se manda la consulta de la familia por difusión (255.255.255.255 y el .255 de cada
dirección IPv4 propia) a los puertos típicos del juego; quien contesta es un servidor y
después se consulta normalmente (`scanner.query_servers`), con los filtros del juego.
"""
from __future__ import annotations

import asyncio
import ipaddress
import socket
from collections.abc import Callable

from jase.games import GameDef
from jase.models import ServerAddress, ServerInfo
from jase.protocols import gamespy, quake2, quake3, ut2004, valve, zandronum

# Puertos de consulta por defecto (más los siguientes, para varios servidores en un equipo).
_RANGE = 4
GAME_PORTS: dict[str, int] = {
    "cod2": 28960, "cod1": 28960, "coduo": 28960, "cod4": 28960,
    "q3": 27960, "rtcw": 27960, "et": 27960, "urt": 27960, "oa": 27960, "smokinguns": 27960,
    "jk2": 28070, "jka": 29070, "sof2": 20100, "xonotic": 26000, "warsow": 44400,
    "warfork": 44400, "tremulous": 30720,
    "ut99": 7778, "mohaa": 12300, "mohaas": 12300, "mohaab": 12300, "bf1942": 23000,
    "q2": 27910, "dday": 27910, "ut2004": 7778, "zandronum": 10666,
}
FAMILY_PORTS: dict[str, int] = {"valve": 27015, "quake3": 27960, "quake2": 27910,
                                "gamespy": 7778, "ut2004": 7778, "zandronum": 10666}
PROBES: dict[str, bytes] = {"quake3": quake3.GETSTATUS, "quake2": quake2.STATUS_QUERY,
                            "gamespy": gamespy.STATUS_QUERY, "valve": valve.A2S_INFO,
                            "ut2004": ut2004.INFO_QUERY, "zandronum": zandronum.build_query()}


def lan_ports(game: GameDef) -> list[int]:
    base = GAME_PORTS.get(game.id, FAMILY_PORTS[game.family])
    count = 6 if game.family == "valve" else _RANGE  # Valve: 27015-27020
    return list(range(base, base + count))


def broadcast_addresses() -> list[str]:
    """255.255.255.255 y el .255 (como /24) de cada IPv4 privada del equipo."""
    result = ["255.255.255.255"]
    try:
        own = [info[4][0] for info in
               socket.getaddrinfo(socket.gethostname(), None, socket.AF_INET)]
    except OSError:
        own = []
    try:  # IP de la interfaz con ruta por defecto (connect en UDP no envía nada)
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as probe:
            probe.connect(("192.0.2.1", 9))
            own.append(probe.getsockname()[0])
    except OSError:
        pass
    for text in own:
        ip = ipaddress.IPv4Address(text)
        if ip.is_private and not ip.is_loopback:
            net = ipaddress.IPv4Network(f"{ip}/24", strict=False)
            if str(net.broadcast_address) not in result:
                result.append(str(net.broadcast_address))
    return result


class _Collector(asyncio.DatagramProtocol):
    def __init__(self) -> None:
        self.found: dict[ServerAddress, None] = {}

    def datagram_received(self, data: bytes, addr) -> None:
        if data:
            self.found.setdefault(ServerAddress(addr[0], addr[1]), None)

    def error_received(self, exc: Exception) -> None:
        pass


def _disable_udp_connreset(sock: socket.socket) -> None:
    from jase.scanner import _disable_udp_connreset as disable

    disable(sock)


async def discover(game: GameDef, timeout: float = 1.5,
                   targets: list[str] | None = None) -> list[ServerAddress]:
    """Direcciones (IP:puerto de consulta) que contestan a la difusión."""
    loop = asyncio.get_running_loop()
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.setblocking(False)
        _disable_udp_connreset(sock)  # Windows: un ICMP «puerto inalcanzable» no la rompe
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        sock.bind(("0.0.0.0", 0))
        transport, collector = await loop.create_datagram_endpoint(_Collector, sock=sock)
    except BaseException:
        sock.close()
        raise
    try:
        probe = PROBES[game.family]
        for target in targets or broadcast_addresses():
            for port in lan_ports(game):
                try:
                    transport.sendto(probe, (target, port))
                except OSError:
                    pass  # sin ruta a esa red: se sigue con las demás
        await asyncio.sleep(timeout)
        return list(collector.found)
    finally:
        transport.close()


async def scan_lan(game: GameDef, masters, on_server: Callable[[ServerInfo], None],
                   on_status: Callable[[str], None], on_progress=None, **_kw):
    """Igual que `scanner.scan`, pero buscando en la red local en vez de en los masters."""
    from jase import scanner
    from jase.i18n import tr_n

    found = await discover(game)
    on_status(tr_n("{n} servidor en la red local, consultando…",
                   "{n} servidores en la red local, consultando…", len(found)))
    responded = 0

    def count(info: ServerInfo) -> None:
        nonlocal responded
        responded += 1
        on_server(info)

    await scanner.query_servers(game.family, found, count, **scanner.game_query_options(game))
    return scanner.ScanResult(masters_ok=1, servers_listed=len(found),
                              servers_responded=responded)
