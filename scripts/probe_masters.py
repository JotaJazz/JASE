"""Sondeo de los masters reales de los juegos de GAMES.

Uso: python scripts/probe_masters.py [--sample N] [id_juego ...]   (p. ej. `probe_masters.py mohaa cod4`)

Sin argumentos sondea todos. Con `--sample N` solo consulta N servidores por juego
(en vez de todos los listados). Por cada master imprime `host:port -> N direcciones` (o el
error); para juegos Quake3 también cuántos servidores devuelve cada protocolo. Después
hace un `scan` completo (el mismo que usa la app) y muestra los SAMPLE servidores con
menor ping y el reparto de versiones.
"""
from __future__ import annotations

import asyncio
import argparse
import collections
import os
import sys

from jase.games import GAMES, GameDef
from jase.models import ServerInfo
from jase.models import ServerAddress
from jase.scanner import (fetch_master, fetch_master_quake3, game_query_options, query_servers,
                          scan)

SAMPLE = 20


STEAM_KEY = os.environ.get("JASE_STEAM_KEY", "")  # solo juegos con steam_webapi


async def probe_game(game: GameDef, sample: int | None = None) -> None:
    print(f"=== {game.name} ({game.id}) ===")
    proto_of: dict = {}
    listed: dict[ServerAddress, None] = {}
    for host, port in game.masters:
        try:
            found = await fetch_master(game, host, port, 10.0, STEAM_KEY)
        except Exception as exc:  # noqa: BLE001 - sondeo: mostrar cualquier fallo
            print(f"  {host}:{port} -> ERROR {type(exc).__name__}: {exc}")
            continue
        listed.update(dict.fromkeys(found))
        print(f"  {host}:{port} -> {len(found)} direcciones ({len(set(found))} únicas)")
        if game.family == "quake3":
            for proto in game.protocols:
                try:
                    per = await fetch_master_quake3(host, port, [proto], timeout=5.0)
                    proto_of.update(dict.fromkeys(per, proto))
                    print(f"    protocolo {proto}: {len(per)} servidores")
                except Exception as exc:  # noqa: BLE001
                    print(f"    protocolo {proto}: ERROR {type(exc).__name__}: {exc}")

    results: list[ServerInfo] = []
    if sample is not None:
        chosen = dict.fromkeys(list(listed)[:sample])
        await query_servers(game.family, chosen, results.append, timeout=2.0,
                            **game_query_options(game))
        print(f"  Muestra: {len(chosen)} de {len(listed)} listados, "
              f"respondieron {len(results)}; los {SAMPLE} de menor ping:")
    else:
        result = await scan(game, list(game.masters), results.append,
                            lambda msg: print(f"  [scan] {msg}"),
                            master_timeout=10.0, query_timeout=2.0, steam_key=STEAM_KEY)
        if result.ok and results:
            print(f"  Listados {result.servers_listed}, respondieron "
                  f"{result.servers_responded}; los {SAMPLE} de menor ping:")
    if not results:
        print("  (ningún servidor respondió)\n")
        return
    for info in sorted(results, key=lambda i: i.ping_ms)[:SAMPLE]:
        proto = f" p{proto_of[info.address]}" if info.address in proto_of else ""
        conn = "" if info.connect_address == info.address else f" (juego {info.connect_address})"
        print(f"    {str(info.address):22} v={info.version!r:10} ping={info.ping_ms:4}ms{proto} "
              f"{info.num_players}/{info.max_players} {info.name[:40]!r}{conn}")
    versions = collections.Counter(i.version for i in results)
    print(f"  Versiones: {dict(versions.most_common())}\n")


async def main(ids: list[str], sample: int | None = None) -> int:
    unknown = [gid for gid in ids if gid not in GAMES]
    if unknown:
        print(f"Juegos desconocidos: {', '.join(unknown)}. Disponibles: {', '.join(GAMES)}")
        return 2
    for game_id in ids or list(GAMES):
        await probe_game(GAMES[game_id], sample)
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Sondeo de los masters reales.")
    parser.add_argument("--sample", type=int, metavar="N", default=None,
                        help="consultar solo N servidores por juego (por defecto, todos)")
    parser.add_argument("games", nargs="*", metavar="id_juego")
    args = parser.parse_args()
    sys.exit(asyncio.run(main(args.games, args.sample)))
