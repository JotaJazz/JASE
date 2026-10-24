"""Favoritos por juego: lógica pura (sin Qt) sobre `Config.favorites`.

Cada favorito es la dirección de **consulta** (`str(info.address)`); la de
conexión llega con la respuesta del servidor.
"""
from __future__ import annotations

from jase.config import Config
from jase.masters import valid_host
from jase.models import ServerAddress, ServerInfo


def parse_address(text: object) -> ServerAddress | None:
    """`"host:puerto"` → ServerAddress; None si el texto no es válido."""
    if not isinstance(text, str):
        return None
    host, sep, port = text.rpartition(":")
    # Mismo criterio de host que los enlaces jase://: nada de basura en config ni importación.
    if (not sep or not valid_host(host) or host[0] in ".-"
            or not (port.isascii() and port.isdigit())):
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
    set_note(cfg, game_id, address, "")
    return True


NOTE_MAX = 200


def get_note(cfg: Config, game_id: str, address: ServerAddress | str) -> str:
    return cfg.favorite_notes.get(game_id, {}).get(str(address), "")


def set_note(cfg: Config, game_id: str, address: ServerAddress | str, text: str) -> None:
    """Guarda la nota (una línea, máx. NOTE_MAX); texto vacío la borra."""
    clean = " ".join(text.split())[:NOTE_MAX].strip()
    if clean:
        cfg.favorite_notes.setdefault(game_id, {})[str(address)] = clean
        return
    notes = cfg.favorite_notes.get(game_id)
    if notes is not None:
        notes.pop(str(address), None)
        if not notes:
            del cfg.favorite_notes[game_id]


def game_notes(cfg: Config, game_id: str) -> dict[ServerAddress, str]:
    """Notas del juego con direcciones válidas."""
    result: dict[ServerAddress, str] = {}
    for text, note in cfg.favorite_notes.get(game_id, {}).items():
        address = parse_address(text)
        if address is not None:
            result[address] = note
    return result


def offline_server(address: ServerAddress) -> ServerInfo:
    """Fila de un favorito que no respondió: nombre = dirección, resto vacío."""
    return ServerInfo(address=address, connect_address=address, name=str(address), map="",
                      gametype="", num_players=0, max_players=0, password=False, version="",
                      ping_ms=0, online=False)
