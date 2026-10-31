from __future__ import annotations

import keyring
from keyring.errors import KeyringError

from jase.models import ServerAddress

SERVICE = "JimmysAllSeeingEye"


def _user(game_id: str, address: ServerAddress) -> str:
    return f"{game_id}:{address}"


def _get(user: str) -> str | None:
    try:
        return keyring.get_password(SERVICE, user)
    except KeyringError:
        return None


def _set(user: str, value: str) -> None:
    """Guarda `value`; vacío = borrar. Guardar no traga el error (la UI lo muestra)."""
    if value:
        keyring.set_password(SERVICE, user, value)
        return
    try:
        keyring.delete_password(SERVICE, user)
    except KeyringError:
        pass


def get_password(game_id: str, address: ServerAddress) -> str | None:
    return _get(_user(game_id, address))


def set_password(game_id: str, address: ServerAddress, pw: str) -> None:
    _set(_user(game_id, address), pw)


def forget_password(game_id: str, address: ServerAddress) -> None:
    _set(_user(game_id, address), "")


def _rcon_user(game_id: str, address: ServerAddress) -> str:
    return f"rcon:{game_id}:{address}"


def get_rcon_password(game_id: str, address: ServerAddress) -> str | None:
    return _get(_rcon_user(game_id, address))


def set_rcon_password(game_id: str, address: ServerAddress, pw: str) -> None:
    """Guarda la clave RCON; vacía = borrarla."""
    _set(_rcon_user(game_id, address), pw)


_STEAM_KEY_USER = "steam:webapi"


def get_steam_key() -> str:
    return _get(_STEAM_KEY_USER) or ""


def set_steam_key(key: str) -> None:
    """Guarda la clave de la API web de Steam; vacía = borrarla."""
    _set(_STEAM_KEY_USER, key)
