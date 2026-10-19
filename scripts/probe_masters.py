"""Sondeo de los masters reales de cada juego de GAMES.

Uso: python scripts/probe_masters.py [id_juego ...]

Por cada master imprime `host:port -> N direcciones` (o el error), y luego consulta
hasta 20 servidores mostrando nombre, versión y ping. Para juegos Quake3 también
pregunta protocolo por protocolo cuántos servidores devuelve cada uno.
"""
from __future__ import annotations

import asyncio
import collections
import sys

from jase.games import GAMES, GameDef
from jase.models import ServerInfo
from jase.scanner import fetch_master_gamespy, fetch_master_quake3, query_servers

SAMPLE = 20


def _round_robin(lists: list[list]) -> list:
    out: list = []
    seen: set = set()
    for i in range(max(map(len, lists), default=0)):
        for lst in lists:
            if i < len(lst) and lst[i] not in seen:
                seen.add(lst[i])
                out.append(lst[i])
    return out


async def probe_game(game: GameDef) -> None:
    print(f"=== {game.name} ({game.id}) ===")
    addresses: dict = {}
    by_proto: dict[int, list] = {}
    for host, port in game.masters:
        try:
            if game.family == "quake3":
                found = await fetch_master_quake3(host, port, game.protocols, timeout=5.0)
            else:
                found = await fetch_master_gamespy(host, port, timeout=10.0)
        except Exception as exc:  # noqa: BLE001 - sondeo: mostrar cualquier fallo
            print(f"  {host}:{port} -> ERROR {type(exc).__name__}: {exc}")
            continue
        print(f"  {host}:{port} -> {len(found)} direcciones")
        addresses.update(dict.fromkeys(found))
        if game.family == "quake3":
            for proto in game.protocols:
                try:
                    per = await fetch_master_quake3(host, port, [proto], timeout=5.0)
                    by_proto.setdefault(proto, []).extend(per)
                    print(f"    protocolo {proto}: {len(per)} servidores")
                except Exception as exc:  # noqa: BLE001
                    print(f"    protocolo {proto}: ERROR {type(exc).__name__}: {exc}")

    if not addresses:
        print("  (ningún master devolvió servidores)\n")
        return
    # Muestra repartida entre protocolos para ver qué versión reporta cada uno.
    sample = _round_robin(list(by_proto.values()) or [list(addresses)])[:SAMPLE]
    proto_of = {a: p for p, addrs in by_proto.items() for a in addrs}
    results: list[ServerInfo] = []
    await query_servers(game.family, sample, results.append, timeout=2.0, retries=1)
    print(f"  Consultados {len(sample)}, respondieron {len(results)}:")
    for info in sorted(results, key=lambda i: i.ping_ms):
        proto = f" p{proto_of[info.address]}" if info.address in proto_of else ""
        conn = "" if info.connect_address == info.address else f" (juego {info.connect_address})"
        print(f"    {str(info.address):22} v={info.version!r:10} ping={info.ping_ms:4}ms{proto} "
              f"{info.num_players}/{info.max_players} {info.name[:40]!r}{conn}")
    versions = collections.Counter(i.version for i in results)
    print(f"  Versiones: {dict(versions)}\n")


async def main(ids: list[str]) -> None:
    for game_id in ids or list(GAMES):
        await probe_game(GAMES[game_id])


if __name__ == "__main__":
    asyncio.run(main(sys.argv[1:]))
