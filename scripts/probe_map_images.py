"""Sondeo de las imágenes de mapas de GameTracker.

Uso: python scripts/probe_map_images.py [id_juego ...]

Por cada juego con `gametracker_id` pide un mapa stock y muestra el código HTTP
(200 = la carpeta y la normalización del nombre son correctas; 404 = no existe).
Herramienta de mantenimiento: usa la red y no corre en los tests.
"""
from __future__ import annotations

import sys
import urllib.error
import urllib.request

from jase.games import GAMES
from jase.maps.service import gametracker_url

# id de juego -> mapa stock que GameTracker debería tener.
PROBE_MAPS: dict[str, str] = {
    "cod1": "mp_carentan",
    "cod2": "mp_toujane",
    "coduo": "mp_brecourt",
    "cod4": "mp_crash",
    "q3": "q3dm6",
    "rtcw": "mp_beach",
    "et": "oasis",
    "urt": "ut4_turnpike",
    "oa": "oa_dm1",
    "sof2": "mp_shop",
    "ut99": "DM-Deck16][",
    "mohaa": "obj/obj_team2",
    "mohaas": "obj/obj_team2",
    "mohaab": "obj/obj_team2",
    "cs16": "de_dust2",
    "l4d": "l4d_hospital01_apartment",
    "l4d2": "c1m1_hotel",
    "css": "de_dust2",
    "tf2": "ctf_2fort",
    "hldm": "crossfire",
    "dod": "dod_anzio",
    "tfc": "2fort",
    "bf1942": "kursk",
    "q2": "q2dm1",
    "dday": "dday1",
    "gmod": "gm_construct",
    "ut2004": "DM-Rankin",
}


def probe(game_id: str) -> int:
    game = GAMES[game_id]
    url = gametracker_url(game.gametracker_id, PROBE_MAPS[game_id])
    req = urllib.request.Request(url, headers={"User-Agent": "JASE"})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            code = resp.status
    except urllib.error.HTTPError as exc:
        code = exc.code
    except OSError as exc:
        print(f"{game_id:8} ERROR {exc}  {url}")
        return -1
    print(f"{game_id:8} {code}  {url}")
    return code


def main(argv: list[str]) -> int:
    ids = argv or [g for g, d in GAMES.items() if d.gametracker_id]
    unknown = [g for g in ids if g not in GAMES or g not in PROBE_MAPS]
    if unknown:
        print("Juegos sin mapa de sondeo:", ", ".join(unknown))
        return 2
    bad = [g for g in ids if probe(g) != 200]
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
