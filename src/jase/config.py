from __future__ import annotations

import contextlib
import json
import os
import tempfile
from dataclasses import asdict, dataclass, field
from pathlib import Path

from jase.games import GameDef
from jase.models import QuickFilters

THEMES = {"system", "dark", "light"}


def _default_game_paths() -> dict[str, str]:
    return {"cod2": r"D:\Juegos\CoD2", "ut99": r"D:\Juegos\UnrealTournament"}


@dataclass
class Config:
    game_paths: dict[str, str] = field(default_factory=_default_game_paths)
    theme: str = "system"
    version_filter: dict[str, str] = field(default_factory=dict)
    masters: dict[str, list[tuple[str, int]]] = field(default_factory=dict)
    quick_filters: QuickFilters = field(default_factory=QuickFilters)
    download_map_images: bool = False


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


def _parse_quick_filters(raw: dict) -> QuickFilters:
    """Cada campo por separado: uno inválido vuelve a su default sin afectar al resto."""
    default = QuickFilters()
    values = {}
    for name in ("hide_empty", "hide_full", "hide_password"):
        v = raw.get(name)
        values[name] = v if isinstance(v, bool) else getattr(default, name)
    ping = raw.get("max_ping")
    ok_ping = isinstance(ping, int) and not isinstance(ping, bool) and ping >= 0
    values["max_ping"] = ping if ok_ping else default.max_ping
    return QuickFilters(**values)


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
    if isinstance(data.get("theme"), str) and data["theme"] in THEMES:
        cfg.theme = data["theme"]
    if isinstance(data.get("version_filter"), dict):
        cfg.version_filter = {str(k): str(v) for k, v in data["version_filter"].items()}
    if isinstance(data.get("masters"), dict):
        try:
            cfg.masters = _parse_masters(data["masters"])
        except (TypeError, ValueError):
            pass
    if isinstance(data.get("quick_filters"), dict):
        cfg.quick_filters = _parse_quick_filters(data["quick_filters"])
    if isinstance(data.get("download_map_images"), bool):
        cfg.download_map_images = data["download_map_images"]
    return cfg


def save_config(cfg: Config, path: Path) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    data = {
        "game_paths": cfg.game_paths,
        "theme": cfg.theme,
        "version_filter": cfg.version_filter,
        "masters": {k: [[h, p] for h, p in v] for k, v in cfg.masters.items()},
        "quick_filters": asdict(cfg.quick_filters),
        "download_map_images": cfg.download_map_images,
    }
    # Escritura atómica: temporal en el mismo directorio y os.replace.
    fd, tmp = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(data, fh, indent=2, ensure_ascii=False)
        os.replace(tmp, path)
    except BaseException:
        with contextlib.suppress(OSError):
            os.unlink(tmp)
        raise


def masters_for(game: GameDef, cfg: Config) -> list[tuple[str, int]]:
    return list(cfg.masters.get(game.id) or game.masters)


def version_for(game: GameDef, cfg: Config) -> str:
    return cfg.version_filter.get(game.id, game.default_version)
