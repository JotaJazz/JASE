"""Servidores recientes por juego: lógica pura (sin Qt) sobre `Config.recent`.

Misma forma que los favoritos (direcciones de consulta "ip:puerto"), del más
reciente al más antiguo y con un tope por juego.
"""
from __future__ import annotations

from jase.config import Config
from jase.models import ServerAddress
from jase.ui.favorites import parse_address

RECENT_MAX = 20


def record_recent(cfg: Config, game_id: str, address: ServerAddress) -> None:
    """Pone `address` primero (sin duplicados) y recorta a RECENT_MAX."""
    entries = [e for e in cfg.recent.get(game_id, []) if e != str(address)]
    entries.insert(0, str(address))
    cfg.recent[game_id] = entries[:RECENT_MAX]


def recent_addresses(cfg: Config, game_id: str) -> list[ServerAddress]:
    """Recientes válidos del juego, del más nuevo al más antiguo y sin repetir."""
    result: list[ServerAddress] = []
    for text in cfg.recent.get(game_id, []):
        address = parse_address(text)
        if address is not None and address not in result:
            result.append(address)
    return result


def remove_recent(cfg: Config, game_id: str, address: ServerAddress) -> bool:
    """True si se quitó; una lista que queda vacía se elimina de la config."""
    entries = cfg.recent.get(game_id, [])
    if str(address) not in entries:
        return False
    entries[:] = [e for e in entries if e != str(address)]
    if not entries:
        del cfg.recent[game_id]
    return True
