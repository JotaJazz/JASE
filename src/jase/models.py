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
    raw_name: str = field(default="", compare=False)  # nombre con códigos de color ^N (solo CoD); "" si no aplica


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
    raw_name: str = field(default="", compare=False)  # nombre con códigos de color ^N (solo CoD); "" si no aplica
