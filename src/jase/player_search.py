"""Búsqueda de jugadores por nombre en las listas de servidores ya cargadas (sin Qt)."""
from __future__ import annotations

import heapq
from collections.abc import Iterable, Mapping
from dataclasses import dataclass

from jase.games import GAMES
from jase.models import Player, ServerInfo


@dataclass(frozen=True)
class PlayerHit:
    game_id: str
    server: ServerInfo
    player: Player


def find_players(lists: Mapping[str, Iterable[ServerInfo]], query: str,
                 limit: int = 500) -> list[PlayerHit]:
    """Jugadores cuyo nombre plano contiene `query` (sin distinguir mayúsculas).

    Orden: juego (según GAMES), luego servidor y luego jugador, todo con casefold.
    """
    needle = query.strip().casefold()
    if not needle:
        return []
    order = {game_id: i for i, game_id in enumerate(GAMES)}
    hits: list[PlayerHit] = []
    for game_id in GAMES:
        for server in lists.get(game_id, ()):
            if not server.online:
                continue
            hits.extend(PlayerHit(game_id, server, p) for p in server.players
                        if needle in p.name.casefold())
    return heapq.nsmallest(limit, hits, key=lambda h: (
        order[h.game_id], h.server.name.casefold(), h.player.name.casefold()))
