"""Exportar e importar favoritos (con sus notas) a un archivo JSON. Sin Qt."""
from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from jase.config import Config
from jase.games import GAMES
from jase.ui import favorites

FORMAT = "jase-favorites"
VERSION = 1
MAX_IMPORT_BYTES = 1_048_576


@dataclass(frozen=True)
class ImportResult:
    added: int
    skipped: int


def export_favorites(cfg: Config) -> dict:
    games: dict[str, list[dict[str, str]]] = {}
    for game_id in cfg.favorites:
        entries = []
        for address in favorites.favorite_addresses(cfg, game_id):
            entry = {"address": str(address)}
            note = favorites.get_note(cfg, game_id, address)
            if note:
                entry["note"] = note
            entries.append(entry)
        if entries:
            games[game_id] = entries
    return {"format": FORMAT, "version": VERSION, "favorites": games}


def import_favorites(cfg: Config, data: object) -> ImportResult:
    """Fusiona `data` en `cfg`: no duplica ni pisa notas existentes.

    ValueError si el formato o la versión no son los esperados."""
    if not isinstance(data, dict) or data.get("format") != FORMAT or data.get("version") != VERSION:
        raise ValueError("not a JASE favorites file")
    games = data.get("favorites")
    if not isinstance(games, dict):
        raise ValueError("not a JASE favorites file")
    added = skipped = 0
    for game_id, entries in games.items():
        if not isinstance(entries, list):
            skipped += 1
            continue
        if game_id not in GAMES:
            skipped += len(entries)
            continue
        for entry in entries:
            address = favorites.parse_address(entry.get("address")) if isinstance(entry, dict) \
                else None
            if address is None:
                skipped += 1
                continue
            if favorites.add_favorite(cfg, game_id, address):
                added += 1
            note = entry.get("note")
            if isinstance(note, str) and not favorites.get_note(cfg, game_id, address):
                favorites.set_note(cfg, game_id, address, note)
    return ImportResult(added, skipped)


def read_import_file(path: Path) -> object:
    """Lee y decodifica el JSON; ValueError si pesa más de MAX_IMPORT_BYTES (sin leerlo)."""
    if path.stat().st_size > MAX_IMPORT_BYTES:
        raise ValueError("file too large")
    return json.loads(path.read_bytes())
