"""Sondeo de «Detectar juego» contra servidores reales de los masters.

Uso: python scripts/probe_detect.py [--sample N] [id_juego ...]

Por cada juego toma N servidores (por defecto 10) entre los que la app mostraría para
ese juego, los consulta como lo hace «Detectar» (todas las familias, sin filtros del
juego, ver `jase.identify`) y cuenta: ok (el juego sale primero), ambiguo (sale, pero no
primero), mal (sale otro), sin respuesta. De los que no salen ok imprime gamename,
versión y protocolo para ajustar las pistas. Los juegos con `steam_webapi` necesitan
JASE_STEAM_KEY.
"""
from __future__ import annotations

import argparse
import asyncio
import collections
import os
import sys

from jase import identify
from jase.games import GAMES, GameDef
from jase.models import ServerAddress, ServerInfo
from jase.scanner import fetch_master, game_query_options, query_servers

STEAM_KEY = os.environ.get("JASE_STEAM_KEY", "")


Responses = dict[ServerAddress, list[tuple[str, ServerInfo]]]


async def detect_many(addresses: list[ServerAddress]
                      ) -> tuple[dict[ServerAddress, list[str]], Responses]:
    """(dirección del master → juegos detectados, → respuestas por familia)."""
    responses: Responses = {a: [] for a in addresses}
    plans = {a: dict(identify.probe_plan(a)) for a in addresses}

    async def family_round(family: str) -> None:
        origin = {p: a for a, plan in plans.items() for p in plan[family]}

        def found(info: ServerInfo) -> None:
            if info.address in origin:
                responses[origin[info.address]].append((family, info))
        await query_servers(family, list(origin), found, timeout=2.0)

    await asyncio.gather(*(family_round(f) for f in identify.FAMILIES))
    detected = {a: [gid for gid, _q in identify.detected_games(r)]
                for a, r in responses.items()}
    return detected, responses


async def probe_game(game: GameDef, sample: int) -> collections.Counter:
    print(f"=== {game.name} ({game.id}) ===")
    listed: dict[ServerAddress, None] = {}
    for host, port in game.masters:
        try:
            listed.update(dict.fromkeys(await fetch_master(game, host, port, 10.0, STEAM_KEY)))
        except Exception as exc:  # noqa: BLE001 - sondeo: mostrar cualquier fallo
            print(f"  {host}:{port} -> ERROR {type(exc).__name__}: {exc}")
    # Solo los que la app mostraría para este juego: los masters compartidos (D-Day en
    # el de Quake II, OpenArena con servidores baseq3) listan también otros juegos.
    accepted: list[ServerAddress] = []
    await query_servers(game.family, list(listed)[:sample * 4],
                        lambda info: accepted.append(info.address), timeout=2.0,
                        **game_query_options(game))
    chosen = accepted[:sample]
    if not chosen:
        print(f"  (ninguno de {len(listed)} listados respondió como {game.id})\n")
        return collections.Counter()
    detected, responses = await detect_many(chosen)
    tally: collections.Counter = collections.Counter()
    for address in chosen:
        games = detected[address]
        if not responses[address]:
            tally["sin respuesta"] += 1
            continue
        verdict = ("ok" if games[:1] == [game.id]
                   else "ambiguo" if game.id in games else "mal")
        tally[verdict] += 1
        if verdict != "ok":
            for family, info in responses[address]:
                rules = info.rules
                print(f"  {verdict:7} {str(address):22} {family:8} → {games} "
                      f"gamename={rules.get('gamename', '')!r} "
                      f"version={rules.get('version', info.version)!r:.60} "
                      f"protocol={rules.get('protocol', '')!r} "
                      f"gamedir={rules.get('gamedir', '')!r} appid={rules.get('appid', '')!r}")
    print(f"  {len(chosen)} servidores: {dict(tally)}\n")
    return tally


async def main(ids: list[str], sample: int) -> int:
    unknown = [gid for gid in ids if gid not in GAMES]
    if unknown:
        print(f"Juegos desconocidos: {', '.join(unknown)}. Disponibles: {', '.join(GAMES)}")
        return 2
    total: collections.Counter = collections.Counter()
    for game_id in ids or list(GAMES):
        total += await probe_game(GAMES[game_id], sample)
    print(f"Total: {dict(total)}")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Sondeo de «Detectar juego».")
    parser.add_argument("--sample", type=int, metavar="N", default=10,
                        help="servidores por juego (por defecto 10)")
    parser.add_argument("games", nargs="*", metavar="id_juego")
    args = parser.parse_args()
    sys.exit(asyncio.run(main(args.games, args.sample)))
