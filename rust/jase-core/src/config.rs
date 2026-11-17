//! Configuración de la app. Traducción de `src/jase/config.py`, compatible en los dos
//! sentidos con Python 1.3.0: mismos bytes al guardar y las claves que Python no conoce
//! se conservan en `Config::extra` (Python las ignora al leer).
use crate::filter_presets::{self, FilterPreset};
use crate::fsutil::atomic_write;
use crate::games::{GAMES, GameDef};
use crate::i18n::LANGUAGES;
use crate::models::QuickFilters;
use crate::pystr::py_strip;
use crate::{friends, party, pyjson};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value, json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Ids de tema válidos; deben coincidir con `jase.ui.theme.THEME_IDS` más "system".
pub const THEMES: [&str; 12] = [
    "system",
    "dark",
    "light",
    "neon",
    "brown",
    "green",
    "classic",
    "xp",
    "terminal",
    "synthwave",
    "rust",
    "goldsrc",
];
/// Colores de fósforo del tema Terminal.
pub const TERMINAL_COLORS: [&str; 4] = ["green", "white", "sepia", "amber"];
/// Minutos entre refrescos automáticos de la lista (0 = desactivado).
pub const AUTO_REFRESH_CHOICES: [u32; 5] = [0, 2, 5, 10, 15];

/// Las claves de `save_config`, en su orden (no es el de los campos).
const KEYS: [&str; 24] = [
    "game_paths",
    "theme",
    "terminal_color",
    "language",
    "version_filter",
    "masters",
    "quick_filters",
    "filter_presets",
    "download_map_images",
    "find_games_offered",
    "only_installed_games",
    "ui_state",
    "favorites",
    "favorite_notes",
    "favorite_groups",
    "recent",
    "auto_refresh_minutes",
    "minimize_to_tray",
    "friends",
    "watch_friends",
    "party_alerts",
    "check_app_updates",
    "steam_include_empty",
    "wine_command",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub game_paths: IndexMap<String, String>,
    pub theme: String,
    pub terminal_color: String,
    /// "auto" sigue el idioma del sistema; si no, uno de `LANGUAGES`.
    pub language: String,
    pub version_filter: IndexMap<String, String>,
    /// Solo para migrar: ver `masters::migrate_config_masters`.
    pub masters: IndexMap<String, Vec<(String, i64)>>,
    pub quick_filters: QuickFilters,
    pub filter_presets: Vec<FilterPreset>,
    pub download_map_images: bool,
    /// Ya se ofreció «Buscar juegos» en el primer arranque.
    pub find_games_offered: bool,
    pub only_installed_games: bool,
    /// Estado de la ventana y último juego.
    pub ui_state: IndexMap<String, String>,
    /// Juego → direcciones "ip:puerto".
    pub favorites: IndexMap<String, Vec<String>>,
    /// Juego → dirección → nota.
    pub favorite_notes: IndexMap<String, IndexMap<String, String>>,
    /// Juego → dirección → grupo.
    pub favorite_groups: IndexMap<String, IndexMap<String, String>>,
    /// Como `favorites`, del más nuevo al más antiguo.
    pub recent: IndexMap<String, Vec<String>>,
    pub auto_refresh_minutes: u32,
    pub steam_include_empty: bool,
    /// Linux/macOS: con qué se abren los .exe ("" = directamente).
    pub wine_command: String,
    pub minimize_to_tray: bool,
    pub friends: Vec<String>,
    pub watch_friends: bool,
    /// Juego → "host:puerto" → jugadores.
    pub party_alerts: IndexMap<String, IndexMap<String, u32>>,
    pub check_app_updates: bool,
    /// Claves que esta versión no conoce: se vuelven a escribir tal cual, al final.
    pub extra: Map<String, Value>,
}

fn default_game_paths() -> IndexMap<String, String> {
    GAMES
        .iter()
        .filter(|g| !g.default_path.is_empty())
        .map(|g| (g.id.to_owned(), g.default_path.to_owned()))
        .collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game_paths: default_game_paths(),
            theme: "goldsrc".to_owned(),
            terminal_color: "green".to_owned(),
            language: "auto".to_owned(),
            version_filter: IndexMap::new(),
            masters: IndexMap::new(),
            quick_filters: QuickFilters::default(),
            filter_presets: Vec::new(),
            download_map_images: false,
            find_games_offered: false,
            only_installed_games: false,
            ui_state: IndexMap::new(),
            favorites: IndexMap::new(),
            favorite_notes: IndexMap::new(),
            favorite_groups: IndexMap::new(),
            recent: IndexMap::new(),
            auto_refresh_minutes: 0,
            steam_include_empty: false,
            wine_command: "wine".to_owned(),
            minimize_to_tray: false,
            friends: Vec::new(),
            watch_friends: false,
            party_alerts: IndexMap::new(),
            check_app_updates: true,
            extra: Map::new(),
        }
    }
}

/// `%APPDATA%\JimmysASE\config.json`; si no, `~/.config/jimmys-ase/config.json`.
pub fn default_config_path() -> PathBuf {
    match std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        Some(appdata) => PathBuf::from(appdata).join("JimmysASE").join("config.json"),
        None => std::env::home_dir()
            .unwrap_or_default()
            .join(".config/jimmys-ase/config.json"),
    }
}

/// Objeto de texto → texto; las entradas con otro valor se descartan (Python haría `str(v)`).
fn str_map(raw: &Map<String, Value>) -> IndexMap<String, String> {
    raw.iter()
        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
        .collect()
}

/// Todo o nada, como el `try` de Python: cada entrada debe ser `[texto, entero]`.
fn parse_masters(raw: &Map<String, Value>) -> Option<IndexMap<String, Vec<(String, i64)>>> {
    raw.iter()
        .map(|(k, v)| {
            let list = v
                .as_array()?
                .iter()
                .map(|m| match m.as_array()?.as_slice() {
                    [h, p] => Some((h.as_str()?.to_owned(), p.as_i64()?)),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some((k.clone(), list))
        })
        .collect()
}

/// Cada campo por separado: uno inválido vuelve a su default sin afectar al resto.
pub fn parse_quick_filters(raw: &Map<String, Value>) -> QuickFilters {
    let flag = |name: &str| raw.get(name).and_then(Value::as_bool).unwrap_or(false);
    QuickFilters {
        hide_empty: flag("hide_empty"),
        hide_full: flag("hide_full"),
        hide_password: flag("hide_password"),
        max_ping: raw
            .get("max_ping")
            .and_then(Value::as_i64)
            .filter(|&p| p >= 0)
            .unwrap_or(0),
        hide_bots: flag("hide_bots"),
        country: raw
            .get("country")
            .and_then(Value::as_str)
            .filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()))
            .map(str::to_ascii_lowercase)
            .unwrap_or_default(),
    }
}

pub(crate) fn quick_filters_json(q: &QuickFilters) -> Value {
    json!({
        "hide_empty": q.hide_empty,
        "hide_full": q.hide_full,
        "hide_password": q.hide_password,
        "max_ping": q.max_ping,
        "hide_bots": q.hide_bots,
        "country": q.country,
    })
}

/// Juego por juego y entrada por entrada: lo inválido se descarta, sin duplicados.
fn parse_favorites(raw: &Map<String, Value>) -> IndexMap<String, Vec<String>> {
    raw.iter()
        .filter_map(|(game_id, entries)| {
            let clean: IndexSet<String> = entries
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .filter(|e| !e.is_empty())
                .map(str::to_owned)
                .collect();
            (!clean.is_empty()).then(|| (game_id.clone(), clean.into_iter().collect()))
        })
        .collect()
}

/// Juego por juego y nota por nota: lo inválido o vacío se descarta.
fn parse_notes(raw: &Map<String, Value>) -> IndexMap<String, IndexMap<String, String>> {
    raw.iter()
        .filter_map(|(game_id, notes)| {
            let clean: IndexMap<String, String> = notes
                .as_object()?
                .iter()
                .filter_map(|(a, n)| Some((a.clone(), n.as_str()?.to_owned())))
                .filter(|(a, n)| !a.is_empty() && !n.is_empty())
                .collect();
            (!clean.is_empty()).then(|| (game_id.clone(), clean))
        })
        .collect()
}

/// `load_config` sobre el objeto ya leído: cada clave conocida con un valor inválido
/// queda en su default; las desconocidas van a `extra`.
pub fn config_from_json(data: &Map<String, Value>) -> Config {
    let mut cfg = Config::default();
    let obj = |key: &str| data.get(key).and_then(Value::as_object);
    let text = |key: &str| data.get(key).and_then(Value::as_str);
    let flag = |key: &str, field: &mut bool| {
        if let Some(b) = data.get(key).and_then(Value::as_bool) {
            *field = b;
        }
    };
    if let Some(saved) = obj("game_paths") {
        // Las rutas por defecto de juegos nuevos se añaden; lo guardado manda.
        cfg.game_paths.extend(str_map(saved));
    }
    match text("theme") {
        Some(t) if THEMES.contains(&t) => cfg.theme = t.to_owned(),
        // Ámbar pasó a ser un color de Terminal.
        Some("amber") => {
            cfg.theme = "terminal".to_owned();
            cfg.terminal_color = "amber".to_owned();
        }
        _ => {}
    }
    if let Some(c) = text("terminal_color").filter(|c| TERMINAL_COLORS.contains(c)) {
        cfg.terminal_color = c.to_owned();
    }
    if let Some(l) = text("language").filter(|l| *l == "auto" || LANGUAGES.contains(l)) {
        cfg.language = l.to_owned();
    }
    if let Some(v) = obj("version_filter") {
        cfg.version_filter = str_map(v);
    }
    if let Some(m) = obj("masters").and_then(parse_masters) {
        cfg.masters = m;
    }
    if let Some(q) = obj("quick_filters") {
        cfg.quick_filters = parse_quick_filters(q);
    }
    let null = Value::Null;
    cfg.filter_presets = filter_presets::parse(data.get("filter_presets").unwrap_or(&null));
    flag("download_map_images", &mut cfg.download_map_images);
    flag("find_games_offered", &mut cfg.find_games_offered);
    flag("only_installed_games", &mut cfg.only_installed_games);
    if let Some(u) = obj("ui_state") {
        cfg.ui_state = str_map(u);
    }
    if let Some(f) = obj("favorites") {
        cfg.favorites = parse_favorites(f);
    }
    if let Some(n) = obj("favorite_notes") {
        cfg.favorite_notes = parse_notes(n);
    }
    if let Some(g) = obj("favorite_groups") {
        cfg.favorite_groups = parse_notes(g);
    }
    if let Some(r) = obj("recent") {
        cfg.recent = parse_favorites(r);
    }
    if let Some(m) = data
        .get("auto_refresh_minutes")
        .and_then(Value::as_i64)
        .and_then(|m| u32::try_from(m).ok())
        .filter(|m| AUTO_REFRESH_CHOICES.contains(m))
    {
        cfg.auto_refresh_minutes = m;
    }
    flag("minimize_to_tray", &mut cfg.minimize_to_tray);
    cfg.friends = friends::parse(data.get("friends").unwrap_or(&null));
    flag("watch_friends", &mut cfg.watch_friends);
    cfg.party_alerts = party::parse(data.get("party_alerts").unwrap_or(&null));
    flag("check_app_updates", &mut cfg.check_app_updates);
    flag("steam_include_empty", &mut cfg.steam_include_empty);
    if let Some(w) = text("wine_command") {
        cfg.wine_command = py_strip(w).chars().take(300).collect();
    }
    cfg.extra = data
        .iter()
        .filter(|(k, _)| !KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    cfg
}

/// El objeto que escribe `save_config`: claves de Python en su orden y luego `extra`.
pub fn config_to_json(cfg: &Config) -> Value {
    let masters: Map<String, Value> = cfg
        .masters
        .iter()
        .map(|(k, v)| (k.clone(), v.iter().map(|(h, p)| json!([h, p])).collect()))
        .collect();
    let mut data = Map::new();
    let mut put = |key: &str, value: Value| {
        data.insert(key.to_owned(), value);
    };
    put("game_paths", json!(cfg.game_paths));
    put("theme", json!(cfg.theme));
    put("terminal_color", json!(cfg.terminal_color));
    put("language", json!(cfg.language));
    put("version_filter", json!(cfg.version_filter));
    put("masters", Value::Object(masters));
    put("quick_filters", quick_filters_json(&cfg.quick_filters));
    put(
        "filter_presets",
        filter_presets::to_json(&cfg.filter_presets),
    );
    put("download_map_images", json!(cfg.download_map_images));
    put("find_games_offered", json!(cfg.find_games_offered));
    put("only_installed_games", json!(cfg.only_installed_games));
    put("ui_state", json!(cfg.ui_state));
    put("favorites", json!(cfg.favorites));
    put("favorite_notes", json!(cfg.favorite_notes));
    put("favorite_groups", json!(cfg.favorite_groups));
    put("recent", json!(cfg.recent));
    put("auto_refresh_minutes", json!(cfg.auto_refresh_minutes));
    put("minimize_to_tray", json!(cfg.minimize_to_tray));
    put("friends", json!(cfg.friends));
    put("watch_friends", json!(cfg.watch_friends));
    put("party_alerts", json!(cfg.party_alerts));
    put("check_app_updates", json!(cfg.check_app_updates));
    put("steam_include_empty", json!(cfg.steam_include_empty));
    put("wine_command", json!(cfg.wine_command));
    for (k, v) in &cfg.extra {
        // Una clave conocida en `extra` (armada a mano) no pisa la de la config.
        data.entry(k.clone()).or_insert_with(|| v.clone());
    }
    Value::Object(data)
}

/// Copia la config ilegible a `.bad` para que los valores por defecto no la pisen.
fn keep_damaged(path: &Path) {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bad");
    let _ = fs::copy(path, path.with_file_name(name));
}

/// Falta → default; ilegible, no UTF-8, con BOM, JSON inválido o no objeto → default y
/// copia en `<nombre>.bad`.
pub fn load_config(path: &Path) -> Config {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Config::default(),
        Err(_) => {
            keep_damaged(path);
            return Config::default();
        }
    };
    match pyjson::loads_str(&data) {
        Some(Value::Object(map)) => config_from_json(&map),
        _ => {
            keep_damaged(path);
            Config::default()
        }
    }
}

/// `json.dumps(indent=2, ensure_ascii=False)` de Python: los mismos bytes.
pub fn save_config(cfg: &Config, path: &Path) -> io::Result<()> {
    let text = serde_json::to_vec_pretty(&config_to_json(cfg)).map_err(io::Error::other)?;
    atomic_write(path, &text)
}

pub fn version_for<'a>(game: &'a GameDef, cfg: &'a Config) -> &'a str {
    cfg.version_filter
        .get(game.id)
        .map_or(game.default_version, String::as_str)
}
