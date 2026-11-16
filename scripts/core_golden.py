"""Volcados JSON de la versión Python para los tests de paridad del núcleo en Rust.

Uso: python scripts/core_golden.py  (escribe rust/jase-core/tests/golden/games.json)

Python está congelado en 1.3.0: solo hace falta volver a correrlo si cambia lo volcado.
"""
from __future__ import annotations

import dataclasses
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase.games import GAMES, GameDef, _not_tacops, is_tacops  # noqa: E402

OUT = ROOT / "rust" / "jase-core" / "tests" / "golden"


def _plain(value: Any) -> Any:
    if isinstance(value, (list, tuple)):
        return [_plain(v) for v in value]
    return value


def game_json(game: GameDef) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for f in dataclasses.fields(game):
        if f.name.startswith("_"):
            continue
        value = getattr(game, f.name)
        if f.name == "list_filter":
            value = {None: None, is_tacops: "is_tacops", _not_tacops: "not_tacops"}[value]
        result[f.name] = _plain(value)
    return result


def games_json() -> list[dict[str, Any]]:
    return [game_json(g) for g in GAMES.values()]


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    text = json.dumps(games_json(), ensure_ascii=False, indent=1) + "\n"
    (OUT / "games.json").write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
