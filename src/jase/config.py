from __future__ import annotations

import contextlib
import json
import logging
import os
import shutil
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any

from jase.fetch_meta import atomic_write
from jase.games import GAMES, GameDef
from jase.i18n import LANGUAGES
from jase.models import QuickFilters

if TYPE_CHECKING:
    from jase.filter_presets import FilterPreset

_log = logging.getLogger(__name__)

# Ids de tema válidos (sin Qt aquí); deben coincidir con jase.ui.theme.THEME_IDS.
THEMES = {"system", "dark", "light", "neon", "brown", "green", "classic", "xp",
          "terminal", "synthwave", "rust", "goldsrc"}
# Colores de fósforo del tema Terminal (jase.ui.theme.TERMINAL_COLORS).
TERMINAL_COLORS = ("green", "white", "sepia", "amber")


def _default_game_paths() -> dict[str, str]:
    return {game.id: game.default_path for game in GAMES.values() if game.default_path}

# Minutos entre refrescos automáticos de la lista (0 = desactivado).
AUTO_REFRESH_CHOICES = (0, 2, 5, 10, 15)


@dataclass
class Config:
    game_paths: dict[str, str] = field(default_factory=_default_game_paths)
    theme: str = "goldsrc"  # tema por defecto de una config nueva
    terminal_color: str = "green"  # fósforo del tema Terminal
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
    only_installed_games: bool = False  # el árbol lista solo los juegos listos para jugar
    # Estado de la ventana (bytes de Qt en base64) y último juego; ver MainWindow.
    ui_state: dict[str, str] = field(default_factory=dict)
    # Favoritos por juego: id → direcciones de consulta "ip:puerto" (str(info.address)).
    favorites: dict[str, list[str]] = field(default_factory=dict)
    # Notas de favoritos: juego → dirección de consulta → texto corto.
    favorite_notes: dict[str, dict[str, str]] = field(default_factory=dict)
    favorite_groups: dict[str, dict[str, str]] = field(default_factory=dict)  # dirección → grupo
    # Recientes por juego: misma forma que favoritos, del más nuevo al más antiguo.
    recent: dict[str, list[str]] = field(default_factory=dict)
    auto_refresh_minutes: int = 0
    steam_include_empty: bool = False  # listar también servidores vacíos de Steam
    # Linux/macOS: con qué se abren los .exe de los juegos ("wine", "umu-run"...;
    # vacío = directamente). En Windows no se usa.
    wine_command: str = "wine"
    minimize_to_tray: bool = False
    friends: list[str] = field(default_factory=list)  # nombres de jugador (máx. 100)
    watch_friends: bool = False  # revisar favoritos y recientes en segundo plano
    # «Se armó partida»: {juego: {"host:puerto": jugadores}} (ver jase.party).
    party_alerts: dict[str, dict[str, int]] = field(default_factory=dict)
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
    values: dict[str, Any] = {}
    for name in ("hide_empty", "hide_full", "hide_password", "hide_bots"):
        v = raw.get(name)
        values[name] = v if isinstance(v, bool) else getattr(default, name)
    ping = raw.get("max_ping")
    ok_ping = isinstance(ping, int) and not isinstance(ping, bool) and ping >= 0
    values["max_ping"] = ping if ok_ping else default.max_ping
    country = raw.get("country")
    if isinstance(country, str) and len(country) == 2 and country.isascii() \
            and country.isalpha():
        values["country"] = country.lower()
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


def _keep_damaged(path: Path) -> None:
    """Copia la config ilegible a `.bad` para que los valores por defecto no la pisen."""
    # shortcut: un archivo bloqueado no se puede copiar y save_config lo pisará al cerrar;
    # bloquear el guardado si aparece en la práctica
    with contextlib.suppress(OSError):
        shutil.copy2(path, path.with_name(path.name + ".bad"))
    _log.warning("config dañada, copia en %s.bad", path.name)


def load_config(path: Path) -> Config:
    cfg = Config()
    path = Path(path)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return cfg
    except (OSError, ValueError, RecursionError):  # RecursionError: anidado sin fin
        _keep_damaged(path)
        return cfg
    if not isinstance(data, dict):
        _keep_damaged(path)
        return cfg
    if isinstance(data.get("game_paths"), dict):
        saved = {str(k): str(v) for k, v in data["game_paths"].items()}
        # Las rutas por defecto de juegos nuevos se añaden; lo guardado manda
        # (una ruta vaciada en Ajustes se guarda como "" y no vuelve).
        cfg.game_paths = {**_default_game_paths(), **saved}
    if isinstance(data.get("theme"), str) and data["theme"] in THEMES:
        cfg.theme = data["theme"]
    elif data.get("theme") == "amber":  # Ámbar pasó a ser un color de Terminal
        cfg.theme, cfg.terminal_color = "terminal", "amber"
    if isinstance(data.get("terminal_color"), str) and data["terminal_color"] in TERMINAL_COLORS:
        cfg.terminal_color = data["terminal_color"]
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
    if isinstance(data.get("only_installed_games"), bool):
        cfg.only_installed_games = data["only_installed_games"]
    if isinstance(data.get("ui_state"), dict):
        # Entrada por entrada: una inválida se descarta sin afectar al resto.
        cfg.ui_state = {k: v for k, v in data["ui_state"].items()
                        if isinstance(k, str) and isinstance(v, str)}
    if isinstance(data.get("favorites"), dict):
        cfg.favorites = _parse_favorites(data["favorites"])
    if isinstance(data.get("favorite_notes"), dict):
        cfg.favorite_notes = _parse_notes(data["favorite_notes"])
    if isinstance(data.get("favorite_groups"), dict):
        cfg.favorite_groups = _parse_notes(data["favorite_groups"])
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
    from jase.party import parse as parse_party
    cfg.party_alerts = parse_party(data.get("party_alerts"))
    if isinstance(data.get("check_app_updates"), bool):
        cfg.check_app_updates = data["check_app_updates"]
    if isinstance(data.get("steam_include_empty"), bool):
        cfg.steam_include_empty = data["steam_include_empty"]
    if isinstance(data.get("wine_command"), str):
        cfg.wine_command = data["wine_command"].strip()[:300]
    return cfg


def save_config(cfg: Config, path: Path) -> None:
    from jase.filter_presets import to_json as presets_to_json
    path = Path(path)
    data = {
        "game_paths": cfg.game_paths,
        "theme": cfg.theme,
        "terminal_color": cfg.terminal_color,
        "language": cfg.language,
        "version_filter": cfg.version_filter,
        "masters": {k: [[h, p] for h, p in v] for k, v in cfg.masters.items()},
        "quick_filters": asdict(cfg.quick_filters),
        "filter_presets": presets_to_json(cfg.filter_presets),
        "download_map_images": cfg.download_map_images,
        "find_games_offered": cfg.find_games_offered,
        "only_installed_games": cfg.only_installed_games,
        "ui_state": cfg.ui_state,
        "favorites": cfg.favorites,
        "favorite_notes": cfg.favorite_notes,
        "favorite_groups": cfg.favorite_groups,
        "recent": cfg.recent,
        "auto_refresh_minutes": cfg.auto_refresh_minutes,
        "minimize_to_tray": cfg.minimize_to_tray,
        "friends": cfg.friends,
        "watch_friends": cfg.watch_friends,
        "party_alerts": cfg.party_alerts,
        "check_app_updates": cfg.check_app_updates,
        "steam_include_empty": cfg.steam_include_empty,
        "wine_command": cfg.wine_command,
    }
    atomic_write(path, json.dumps(data, indent=2, ensure_ascii=False).encode("utf-8"))


def version_for(game: GameDef, cfg: Config) -> str:
    return cfg.version_filter.get(game.id, game.default_version)
