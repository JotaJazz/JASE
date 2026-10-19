"""Favoritos por juego: lógica pura (sin Qt) sobre `Config.favorites`.

Cada favorito es la dirección de **consulta** (`str(info.address)`); la de
conexión llega con la respuesta del servidor.
"""
from __future__ import annotations

from jase.config import Config
from jase.models import ServerAddress, ServerInfo


def parse_address(text: object) -> ServerAddress | None:
    """`"host:puerto"` → ServerAddress; None si el texto no es válido."""
    if not isinstance(text, str):
        return None
    host, sep, port = text.rpartition(":")
    if not sep or not host or not port.isdigit():
        return None
    number = int(port)
    if not 0 < number < 65536:
        return None
    return ServerAddress(host, number)


def favorite_addresses(cfg: Config, game_id: str) -> list[ServerAddress]:
    """Favoritos válidos del juego, en el orden guardado y sin repetir."""
    result: list[ServerAddress] = []
    for text in cfg.favorites.get(game_id, []):
        address = parse_address(text)
        if address is not None and address not in result:
            result.append(address)
    return result


def is_favorite(cfg: Config, game_id: str, address: ServerAddress) -> bool:
    return str(address) in cfg.favorites.get(game_id, [])


def add_favorite(cfg: Config, game_id: str, address: ServerAddress) -> bool:
    """True si se agregó (False si ya estaba)."""
    entries = cfg.favorites.setdefault(game_id, [])
    if str(address) in entries:
        return False
    entries.append(str(address))
    return True


def remove_favorite(cfg: Config, game_id: str, address: ServerAddress) -> bool:
    """True si se quitó; una lista que queda vacía se elimina de la config."""
    entries = cfg.favorites.get(game_id, [])
    if str(address) not in entries:
        return False
    entries[:] = [e for e in entries if e != str(address)]
    if not entries:
        del cfg.favorites[game_id]
    return True


def offline_server(address: ServerAddress) -> ServerInfo:
    """Fila de un favorito que no respondió: nombre = dirección, resto vacío."""
    return ServerInfo(address=address, connect_address=address, name=str(address), map="",
                      gametype="", num_players=0, max_players=0, password=False, version="",
                      ping_ms=0, online=False)
