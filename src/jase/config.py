from __future__ import annotations

import json
import os
from dataclasses import dataclass, field
from pathlib import Path

from jase.games import GameDef

THEMES = {"system", "dark", "light"}


def _default_game_paths() -> dict[str, str]:
    return {"cod2": r"D:\Juegos\CoD2", "ut99": r"D:\Juegos\UnrealTournament"}


@dataclass
class Config:
    game_paths: dict[str, str] = field(default_factory=_default_game_paths)
    theme: str = "system"
    version_filter: dict[str, str] = field(default_factory=dict)
    masters: dict[str, list[tuple[str, int]]] = field(default_factory=dict)


def default_config_path() -> Path:
    appdata = os.environ.get("APPDATA")
    if appdata:
        return Path(appdata) / "JimmysASE" / "config.json"
    return Path.home() / ".config" / "jimmys-ase" / "config.json"


def _parse_masters(raw: object) -> dict[str, list[tuple[str, int]]]:
    return {
        str(k): [(str(h), int(p)) for h, p in v]
        for k, v in raw.items()  # type: ignore[union-attr]
    }


def load_config(path: Path) -> Config:
    cfg = Config()
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return cfg
    if not isinstance(data, dict):
        return cfg
    if isinstance(data.get("game_paths"), dict):
        cfg.game_paths = {str(k): str(v) for k, v in data["game_paths"].items()}
    if data.get("theme") in THEMES:
        cfg.theme = data["theme"]
    if isinstance(data.get("version_filter"), dict):
        cfg.version_filter = {str(k): str(v) for k, v in data["version_filter"].items()}
    if isinstance(data.get("masters"), dict):
        try:
            cfg.masters = _parse_masters(data["masters"])
        except (TypeError, ValueError):
            pass
    return cfg


def save_config(cfg: Config, path: Path) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    data = {
        "game_paths": cfg.game_paths,
        "theme": cfg.theme,
        "version_filter": cfg.version_filter,
        "masters": {k: [[h, p] for h, p in v] for k, v in cfg.masters.items()},
    }
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")


def masters_for(game: GameDef, cfg: Config) -> list[tuple[str, int]]:
    return list(cfg.masters.get(game.id) or game.masters)


def version_for(game: GameDef, cfg: Config) -> str:
    return cfg.version_filter.get(game.id, game.default_version)
