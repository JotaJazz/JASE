from __future__ import annotations

import keyring
from keyring.errors import KeyringError

from jase.models import ServerAddress

SERVICE = "JimmysAllSeeingEye"


def _user(game_id: str, address: ServerAddress) -> str:
    return f"{game_id}:{address}"


def get_password(game_id: str, address: ServerAddress) -> str | None:
    try:
        return keyring.get_password(SERVICE, _user(game_id, address))
    except KeyringError:
        return None


def set_password(game_id: str, address: ServerAddress, pw: str) -> None:
    keyring.set_password(SERVICE, _user(game_id, address), pw)


def forget_password(game_id: str, address: ServerAddress) -> None:
    try:
        keyring.delete_password(SERVICE, _user(game_id, address))
    except KeyringError:
        pass


def _rcon_user(game_id: str, address: ServerAddress) -> str:
    return f"rcon:{game_id}:{address}"


def get_rcon_password(game_id: str, address: ServerAddress) -> str | None:
    try:
        return keyring.get_password(SERVICE, _rcon_user(game_id, address))
    except KeyringError:
        return None


def set_rcon_password(game_id: str, address: ServerAddress, pw: str) -> None:
    """Guarda la clave RCON; vacía = borrarla."""
    if pw:
        keyring.set_password(SERVICE, _rcon_user(game_id, address), pw)
        return
    try:
        keyring.delete_password(SERVICE, _rcon_user(game_id, address))
    except KeyringError:
        pass


_STEAM_KEY_USER = "steam:webapi"


def get_steam_key() -> str:
    try:
        return keyring.get_password(SERVICE, _STEAM_KEY_USER) or ""
    except KeyringError:
        return ""


def set_steam_key(key: str) -> None:
    """Guarda la clave de la API web de Steam; vacía = borrarla."""
    if key:
        keyring.set_password(SERVICE, _STEAM_KEY_USER, key)
        return
    try:
        keyring.delete_password(SERVICE, _STEAM_KEY_USER)
    except KeyringError:
        pass
