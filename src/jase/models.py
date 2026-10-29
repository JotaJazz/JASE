from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class ServerAddress:
    host: str
    port: int

    def __str__(self) -> str:
        return f"{self.host}:{self.port}"


@dataclass
class Player:
    name: str
    score: int
    ping: int
    # Nombre con códigos de color ^N (solo CoD); "" si no aplica.
    raw_name: str = field(default="", compare=False)


@dataclass
class ServerInfo:
    address: ServerAddress
    connect_address: ServerAddress
    name: str
    map: str
    gametype: str
    num_players: int
    max_players: int
    password: bool
    version: str
    ping_ms: int
    players: list[Player] = field(default_factory=list)
    rules: dict[str, str] = field(default_factory=dict)
    # Nombre con códigos de color ^N (solo CoD); "" si no aplica.
    raw_name: str = field(default="", compare=False)
    # False = favorito que no respondió (fila "sin respuesta"; ver ui.favorites).
    online: bool = field(default=True, compare=False)
    # Cuántos de num_players son bots (Valve lo informa; en Quake, ver protocols.is_bot).
    bots: int = field(default=0, compare=False)
    # GameSpy sin `hostport`: el puerto de juego se dedujo del de consulta (no informado).
    connect_port_deduced: bool = field(default=False, compare=False)

    @property
    def humans(self) -> int:
        return max(0, self.num_players - self.bots)


@dataclass(frozen=True)
class QuickFilters:
    """Filtros rápidos de la barra lateral; `max_ping=0` significa sin límite y
    `country=""`, todos los países (si no, código ISO en minúsculas)."""

    hide_empty: bool = False
    hide_full: bool = False
    hide_password: bool = False
    max_ping: int = 0
    hide_bots: bool = False  # servidores donde solo hay bots
    country: str = ""
