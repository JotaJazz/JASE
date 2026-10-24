from __future__ import annotations

import contextlib
import json
import os
import tempfile
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any

from jase.games import GAMES, GameDef
from jase.i18n import LANGUAGES
from jase.models import QuickFilters

if TYPE_CHECKING:
    from jase.filter_presets import FilterPreset

# Ids de tema válidos (sin Qt aquí); deben coincidir con jase.ui.theme.THEME_IDS.
THEMES = {"system", "dark", "light", "neon", "brown", "green", "classic", "xp",
          "amber", "synthwave", "rust"}


def _default_game_paths() -> dict[str, str]:
    return {game.id: game.default_path for game in GAMES.values() if game.default_path}

# Minutos entre refrescos automáticos de la lista (0 = desactivado).
AUTO_REFRESH_CHOICES = (0, 2, 5, 10, 15)


@dataclass
class Config:
    game_paths: dict[str, str] = field(default_factory=_default_game_paths)
    theme: str = "system"
    # "auto" sigue el idioma del sistema; si no, "es" o "en".
    language: str = "auto"
    version_filter: dict[str, str] = field(default_factory=dict)
    # Solo para migrar: ver jase.masters (los masters viven en masters.custom.json).
    masters: dict[str, list[tuple[str, int]]] = field(default_factory=dict)
    quick_filters: QuickFilters = field(default_factory=QuickFilters)
    # Combinaciones de filtros con nombre (ver jase.filter_presets).
    filter_presets: list[FilterPreset] = field(default_factory=list)
    download_map_images: bool = False
    # Ya se ofreció «Buscar juegos» en el primer arranque (solo se pregunta una vez).
    find_games_offered: bool = False
    # Estado de la ventana (bytes de Qt en base64) y último juego; ver MainWindow.
    ui_state: dict[str, str] = field(default_factory=dict)
    # Favoritos por juego: id → direcciones de consulta "ip:puerto" (str(info.address)).
    favorites: dict[str, list[str]] = field(default_factory=dict)
    # Notas de favoritos: juego → dirección de consulta → texto corto.
    favorite_notes: dict[str, dict[str, str]] = field(default_factory=dict)
    # Recientes por juego: misma forma que favoritos, del más nuevo al más antiguo.
    recent: dict[str, list[str]] = field(default_factory=dict)
    auto_refresh_minutes: int = 0
    steam_include_empty: bool = False  # listar también servidores vacíos de Steam
    minimize_to_tray: bool = False
    friends: list[str] = field(default_factory=list)  # nombres de jugador (máx. 100)
    watch_friends: bool = False  # revisar favoritos y recientes en segundo plano
    check_app_updates: bool = True  # avisar de versiones nuevas (GitHub Releases)


def default_config_path() -> Path:
    appdata = os.environ.get("APPDATA")
    if appdata:
        return Path(appdata) / "JimmysASE" / "config.json"
    return Path.home() / ".config" / "jimmys-ase" / "config.json"


def _parse_masters(raw: Any) -> dict[str, list[tuple[str, int]]]:
    return {
        str(k): [(str(h), int(p)) for h, p in v]
        for k, v in raw.items()
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


def _parse_favorites(raw: dict) -> dict[str, list[str]]:
    """Juego por juego y entrada por entrada: lo inválido se descarta, sin duplicados."""
    result: dict[str, list[str]] = {}
    for game_id, entries in raw.items():
        if not isinstance(game_id, str) or not isinstance(entries, list):
            continue
        clean = list(dict.fromkeys(e for e in entries if isinstance(e, str) and e))
        if clean:
            result[game_id] = clean
    return result


def _parse_notes(raw: dict) -> dict[str, dict[str, str]]:
    """Juego por juego y nota por nota: lo inválido o vacío se descarta."""
    result: dict[str, dict[str, str]] = {}
    for game_id, notes in raw.items():
        if not isinstance(game_id, str) or not isinstance(notes, dict):
            continue
        clean = {a: n for a, n in notes.items()
                 if isinstance(a, str) and a and isinstance(n, str) and n}
        if clean:
            result[game_id] = clean
    return result


def load_config(path: Path) -> Config:
    cfg = Config()
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return cfg
    if not isinstance(data, dict):
        return cfg
    if isinstance(data.get("game_paths"), dict):
        saved = {str(k): str(v) for k, v in data["game_paths"].items()}
        # Las rutas por defecto de juegos nuevos se añaden; lo guardado manda
        # (una ruta vaciada en Ajustes se guarda como "" y no vuelve).
        cfg.game_paths = {**_default_game_paths(), **saved}
    if isinstance(data.get("theme"), str) and data["theme"] in THEMES:
        cfg.theme = data["theme"]
    if isinstance(data.get("language"), str) and data["language"] in ("auto", *LANGUAGES):
        cfg.language = data["language"]
    if isinstance(data.get("version_filter"), dict):
        cfg.version_filter = {str(k): str(v) for k, v in data["version_filter"].items()}
    if isinstance(data.get("masters"), dict):
        try:
            cfg.masters = _parse_masters(data["masters"])
        except (TypeError, ValueError):
            pass
    if isinstance(data.get("quick_filters"), dict):
        cfg.quick_filters = _parse_quick_filters(data["quick_filters"])
    from jase.filter_presets import parse as parse_presets  # evita el import circular
    cfg.filter_presets = parse_presets(data.get("filter_presets"))
    if isinstance(data.get("download_map_images"), bool):
        cfg.download_map_images = data["download_map_images"]
    if isinstance(data.get("find_games_offered"), bool):
        cfg.find_games_offered = data["find_games_offered"]
    if isinstance(data.get("ui_state"), dict):
        # Entrada por entrada: una inválida se descarta sin afectar al resto.
        cfg.ui_state = {k: v for k, v in data["ui_state"].items()
                        if isinstance(k, str) and isinstance(v, str)}
    if isinstance(data.get("favorites"), dict):
        cfg.favorites = _parse_favorites(data["favorites"])
    if isinstance(data.get("favorite_notes"), dict):
        cfg.favorite_notes = _parse_notes(data["favorite_notes"])
    if isinstance(data.get("recent"), dict):
        cfg.recent = _parse_favorites(data["recent"])
    minutes = data.get("auto_refresh_minutes")
    if isinstance(minutes, int) and not isinstance(minutes, bool) \
            and minutes in AUTO_REFRESH_CHOICES:
        cfg.auto_refresh_minutes = minutes
    if isinstance(data.get("minimize_to_tray"), bool):
        cfg.minimize_to_tray = data["minimize_to_tray"]
    from jase.friends import parse as parse_friends
    cfg.friends = parse_friends(data.get("friends"))
    if isinstance(data.get("watch_friends"), bool):
        cfg.watch_friends = data["watch_friends"]
    if isinstance(data.get("check_app_updates"), bool):
        cfg.check_app_updates = data["check_app_updates"]
    if isinstance(data.get("steam_include_empty"), bool):
        cfg.steam_include_empty = data["steam_include_empty"]
    return cfg


def save_config(cfg: Config, path: Path) -> None:
    from jase.filter_presets import to_json as presets_to_json
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    data = {
        "game_paths": cfg.game_paths,
        "theme": cfg.theme,
        "language": cfg.language,
        "version_filter": cfg.version_filter,
        "masters": {k: [[h, p] for h, p in v] for k, v in cfg.masters.items()},
        "quick_filters": asdict(cfg.quick_filters),
        "filter_presets": presets_to_json(cfg.filter_presets),
        "download_map_images": cfg.download_map_images,
        "find_games_offered": cfg.find_games_offered,
        "ui_state": cfg.ui_state,
        "favorites": cfg.favorites,
        "favorite_notes": cfg.favorite_notes,
        "recent": cfg.recent,
        "auto_refresh_minutes": cfg.auto_refresh_minutes,
        "minimize_to_tray": cfg.minimize_to_tray,
        "friends": cfg.friends,
        "watch_friends": cfg.watch_friends,
        "check_app_updates": cfg.check_app_updates,
        "steam_include_empty": cfg.steam_include_empty,
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


def version_for(game: GameDef, cfg: Config) -> str:
    return cfg.version_filter.get(game.id, game.default_version)
