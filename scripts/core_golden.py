"""Volcados JSON de la versión Python para los tests de paridad del núcleo en Rust.

Uso: python scripts/core_golden.py  (escribe rust/jase-core/tests/golden/games.json y los
archivos de datos de rust/jase-core/tests/golden/data/)

Python está congelado en 1.3.0: solo hace falta volver a correrlo si cambia lo volcado.
"""
from __future__ import annotations

import dataclasses
import json
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase.config import (  # noqa: E402
    AUTO_REFRESH_CHOICES,
    TERMINAL_COLORS,
    THEMES,
    Config,
    save_config,
)
from jase.filter_presets import FilterPreset  # noqa: E402
from jase.games import GAMES, GameDef, _not_tacops, is_tacops  # noqa: E402
from jase.models import QuickFilters  # noqa: E402

OUT = ROOT / "rust" / "jase-core" / "tests" / "golden"
DATA = OUT / "data"


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


def config_sample() -> Config:
    """Todos los campos fuera de su default, con textos no ASCII, comillas y tabuladores."""
    return Config(
        # En el orden de los defaults (load_config los pone primero): se reguarda igual.
        game_paths={"cod2": "E:\\Juegos\\Ñandú \"CoD2\"", "ut99": "",
                    "mohaa": "C:\\MOHAA", "q3": "/opt/q3\ttab"},
        theme="terminal",
        terminal_color="amber",
        language="pt",
        version_filter={"cod2": "1.0", "mohaa": "Todas"},
        masters={"cod2": [("cod2master.activision.com", 20710), ("mine.lan", 9)]},
        quick_filters=QuickFilters(hide_empty=True, hide_full=True, hide_password=True,
                                   max_ping=150, hide_bots=True, country="cl"),
        filter_presets=[
            FilterPreset("Ñandú «rápido»", QuickFilters(hide_empty=True, max_ping=80), "dm"),
            FilterPreset("Sin bots", QuickFilters(hide_bots=True, country="ar"), 'map:"x"'),
        ],
        download_map_images=True,
        find_games_offered=True,
        only_installed_games=True,
        ui_state={"geometry": "AdnQywADAAAAAAJYAAABLAAABqUAAAQX", "last_game": "cod2"},
        favorites={"cod2": ["1.2.3.4:28960", "5.6.7.8:28961"], "et": ["9.9.9.9:27960"]},
        favorite_notes={"cod2": {"1.2.3.4:28960": "Clan «Ñandú»\tnoche"}},
        favorite_groups={"cod2": {"1.2.3.4:28960": "Clanes", "5.6.7.8:28961": "Públicos"}},
        recent={"cod2": ["5.6.7.8:28961", "1.2.3.4:28960"]},
        auto_refresh_minutes=5,
        steam_include_empty=True,
        wine_command="umu-run --flag",
        minimize_to_tray=True,
        friends=["^1JoTa", "Ñandú"],
        watch_friends=True,
        party_alerts={"cod2": {"1.2.3.4:28960": 6}, "et": {"9.9.9.9:27960": 4}},
        check_app_updates=False,
    )


def write_data() -> None:
    DATA.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for name, cfg in (("config_default.json", Config()),
                          ("config_sample.json", config_sample())):
            path = Path(tmp) / name
            save_config(cfg, path)
            (DATA / name).write_bytes(path.read_bytes())
    constants = {"themes": sorted(THEMES), "terminal_colors": list(TERMINAL_COLORS),
                 "auto_refresh": list(AUTO_REFRESH_CHOICES)}
    (DATA / "config_constants.json").write_text(json.dumps(constants, indent=2) + "\n",
                                                encoding="utf-8")


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    text = json.dumps(games_json(), ensure_ascii=False, indent=1) + "\n"
    (OUT / "games.json").write_text(text, encoding="utf-8")
    write_data()
    return 0


if __name__ == "__main__":
    sys.exit(main())
