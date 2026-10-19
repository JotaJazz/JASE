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
