"""Genera rust/jase-core/src/games_table.rs a partir de src/jase/games.py (una vez).

Uso: python scripts/gen_games_rs.py

Python está congelado en 1.3.0; desde que exista la tabla de Rust, se edita a mano allí
y este script solo sirve para regenerarla si cambiara games.py antes de retirarlo. Los
comentarios de games.py (fechas de sondeos) siguen en ese archivo.
"""
from __future__ import annotations

import dataclasses
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase.games import GAMES, GameDef, _not_tacops, is_tacops  # noqa: E402

OUT = ROOT / "rust" / "jase-core" / "src" / "games_table.rs"
ENUMS = {
    "family": {"quake3": "Quake3", "quake2": "Quake2", "gamespy": "Gamespy", "valve": "Valve",
               "ut2004": "Ut2004", "zandronum": "Zandronum"},
    "launch_style": {"q3": "Q3", "unreal": "Unreal", "valve": "Valve", "bf1942": "Bf1942",
                     "steam": "Steam", "zandronum": "Zandronum"},
}


def rust(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        escaped = value.replace("\\", "\\\\").replace('"', '\\"')
        return f'"{escaped}"'
    if isinstance(value, tuple):
        return "&[" + ", ".join(rust(v) for v in value) + "]"
    raise TypeError(value)


def field_value(game: GameDef, name: str, value: Any) -> str:
    if name in ENUMS:
        enum = "Family" if name == "family" else "LaunchStyle"
        return f"{enum}::{ENUMS[name][value]}"
    if name == "list_filter":
        return {None: "ListFilter::None", is_tacops: "ListFilter::Tacops",
                _not_tacops: "ListFilter::NotTacops"}[value]
    if name in ("masters", "version_aliases", "version_dirs"):
        return "&[" + ", ".join("(" + ", ".join(rust(x) for x in t) + ")" for t in value) + "]"
    return rust(value)


def main() -> int:
    defaults = {f.name: f.default for f in dataclasses.fields(GameDef)
                if f.default is not dataclasses.MISSING}
    out = ["// Generado por scripts/gen_games_rs.py desde src/jase/games.py (Python 1.3.0).",
           "// Desde la fase 2a se edita a mano; los comentarios de cada juego siguen en games.py.",
           "use super::{Family, GameDef, LaunchStyle, ListFilter, DEFAULT};", "",
           "pub static GAMES: &[GameDef] = &["]
    for game in GAMES.values():
        out.append("    GameDef {")
        for f in dataclasses.fields(game):
            if f.name.startswith("_"):
                continue
            value = getattr(game, f.name)
            if f.name == "short_name" and value == game.name:
                continue
            if f.name in defaults and value == defaults[f.name]:
                continue
            out.append(f"        {f.name}: {field_value(game, f.name, value)},")
        out.append("        ..DEFAULT")
        out.append("    },")
    out.append("];")
    OUT.write_text("\n".join(out) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
