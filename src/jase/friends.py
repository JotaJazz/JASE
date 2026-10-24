"""Lista de amigos: nombres normalizados y detección de apariciones nuevas (sin Qt)."""
from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass
from typing import Any

from jase.models import ServerAddress, ServerInfo
from jase.protocols.quake3 import strip_colors

MAX_FRIENDS = 100


def normalize(name: str) -> str:
    """Nombre sin códigos de color ^N, sin espacios en los extremos y en minúsculas."""
    return strip_colors(name).strip().casefold()


def parse(raw: Any) -> list[str]:
    """Tolerante: descarta lo inválido y los vacíos, únicos (casefold), como mucho MAX_FRIENDS."""
    if not isinstance(raw, list):
        return []
    result: list[str] = []
    seen: set[str] = set()
    for item in raw:
        if not isinstance(item, str):
            continue
        name = item.strip()
        key = normalize(name)
        if not key or key in seen:
            continue
        seen.add(key)
        result.append(name)
        if len(result) >= MAX_FRIENDS:
            break
    return result


def friends_in(info: ServerInfo, friends: Iterable[str]) -> list[str]:
    """Amigos presentes en el servidor (coincidencia exacta normalizada), con el nombre guardado."""
    present = {normalize(p.raw_name or p.name) for p in info.players}
    present |= {normalize(p.name) for p in info.players}
    return [f for f in friends if normalize(f) in present]


@dataclass(frozen=True)
class FriendSighting:
    friend: str
    game_id: str
    server: ServerInfo


class FriendTracker:
    """Recuerda dónde se vio a cada amigo para avisar solo de apariciones nuevas."""

    def __init__(self, friends: Iterable[str] = ()) -> None:
        self.friends: list[str] = list(friends)
        # (juego, dirección) → claves normalizadas de los amigos vistos allí.
        self._seen: dict[tuple[str, ServerAddress], set[str]] = {}

    def update(self, game_id: str, info: ServerInfo) -> list[FriendSighting]:
        key = (game_id, info.address)
        now = {normalize(f): f for f in friends_in(info, self.friends)}
        before = self._seen.get(key, set())
        if now:
            self._seen[key] = set(now)
        else:
            self._seen.pop(key, None)  # nadie: el que vuelva avisa de nuevo
        return [FriendSighting(name, game_id, info)
                for norm, name in now.items() if norm not in before]
