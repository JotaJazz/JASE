//! Definición de cada juego. Traducción de `src/jase/games.py`; la tabla está en
//! `games_table.rs` (generada una vez desde Python, verificada contra un golden).
use crate::protocols::quake3::VersionAlias;
use crate::pystr::casefold;
use indexmap::IndexMap;

#[path = "games_table.rs"]
mod table;
pub use table::GAMES;

pub const ALL_VERSIONS: &str = "Todas";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Quake3,
    Quake2,
    Gamespy,
    Valve,
    Ut2004,
    Zandronum,
}

impl Family {
    /// En el orden de `FAMILIES` de Python.
    pub const ALL: [Family; 6] = [
        Self::Quake3,
        Self::Quake2,
        Self::Gamespy,
        Self::Valve,
        Self::Ut2004,
        Self::Zandronum,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quake3 => "quake3",
            Self::Quake2 => "quake2",
            Self::Gamespy => "gamespy",
            Self::Valve => "valve",
            Self::Ut2004 => "ut2004",
            Self::Zandronum => "zandronum",
        }
    }
}

/// Cómo se pasa la dirección al exe (ver `games.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchStyle {
    Q3,
    Unreal,
    Valve,
    Bf1942,
    Steam,
    Zandronum,
}

impl LaunchStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Q3 => "q3",
            Self::Unreal => "unreal",
            Self::Valve => "valve",
            Self::Bf1942 => "bf1942",
            Self::Steam => "steam",
            Self::Zandronum => "zandronum",
        }
    }
}

/// Master compartido con otro juego: qué servidores de la lista van en este.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListFilter {
    None,
    Tacops,
    NotTacops,
}

/// Un juego. Los campos y su significado son los de la dataclass `GameDef` de Python.
#[derive(Debug)]
pub struct GameDef {
    pub id: &'static str,
    pub name: &'static str,
    pub family: Family,
    pub masters: &'static [(&'static str, u16)],
    pub protocols: &'static [u32],
    pub known_versions: &'static [&'static str],
    pub default_version: &'static str,
    pub exe_relpath: &'static str,
    pub exe_candidates: &'static [&'static str],
    pub preferred_exes: &'static [&'static str],
    pub gametracker_id: &'static str,
    pub levelshot_dir: &'static str,
    pub cod_loadscreens: bool,
    pub gamespy_gamename: &'static str,
    pub gamespy_key: &'static str,
    pub status_gamename: &'static str,
    pub status_exclude_mods: &'static [&'static str],
    pub master_gamename: &'static str,
    pub status_gamedir: &'static str,
    pub launch_style: LaunchStyle,
    pub connect_port_fallback: i32,
    pub version_key: &'static str,
    pub version_aliases: &'static [(&'static str, &'static str, &'static str)],
    pub valve_appid: u32,
    pub steam_appid: u32,
    pub valve_gamedir: &'static str,
    pub valve_master_filter: &'static str,
    pub steam_webapi: bool,
    pub steam_webapi_optional: bool,
    pub default_path: &'static str,
    /// Vacío = `name` (ver `short_name()`).
    pub short_name: &'static str,
    pub mod_key: &'static str,
    pub base_mods: &'static [&'static str],
    pub badge: &'static str,
    pub list_filter: ListFilter,
    pub version_dirs: &'static [(&'static str, &'static str)],
}

/// Valores por defecto de la dataclass (los campos obligatorios quedan vacíos).
pub(crate) const DEFAULT: GameDef = GameDef {
    id: "",
    name: "",
    family: Family::Quake3,
    masters: &[],
    protocols: &[],
    known_versions: &[],
    default_version: "",
    exe_relpath: "",
    exe_candidates: &[],
    preferred_exes: &[],
    gametracker_id: "",
    levelshot_dir: "",
    cod_loadscreens: false,
    gamespy_gamename: "",
    gamespy_key: "",
    status_gamename: "",
    status_exclude_mods: &[],
    master_gamename: "",
    status_gamedir: "",
    launch_style: LaunchStyle::Q3,
    connect_port_fallback: 0,
    version_key: "shortversion",
    version_aliases: &[],
    valve_appid: 0,
    steam_appid: 0,
    valve_gamedir: "",
    valve_master_filter: "",
    steam_webapi: false,
    steam_webapi_optional: false,
    default_path: "",
    short_name: "",
    mod_key: "",
    base_mods: &[],
    badge: "",
    list_filter: ListFilter::None,
    version_dirs: &[],
};

impl GameDef {
    /// Nombre para la barra lateral (`name` si no tiene uno corto).
    pub fn short_name(&self) -> &'static str {
        if self.short_name.is_empty() {
            self.name
        } else {
            self.short_name
        }
    }

    /// Lista por la API web de Steam: siempre (`steam_webapi`) o con clave.
    pub fn uses_steam_api(&self) -> bool {
        self.steam_webapi || self.steam_webapi_optional
    }

    /// ¿Va este servidor de la lista del master en este juego?
    pub fn lists_server(&self, rules: &IndexMap<String, String>) -> bool {
        match self.list_filter {
            ListFilter::None => true,
            ListFilter::Tacops => is_tacops(rules),
            ListFilter::NotTacops => !is_tacops(rules),
        }
    }

    /// Texto del selector de versión para la clave `key`.
    pub fn version_label<'a>(&self, key: &'a str) -> &'a str
    where
        'static: 'a,
    {
        self.version_aliases
            .iter()
            .find(|(_, k, _)| *k == key)
            .map_or(key, |(_, _, label)| label)
    }

    /// ¿`name` es uno de `base_mods` (sin distinguir mayúsculas)?
    pub fn is_base_mod(&self, name: &str) -> bool {
        let folded = casefold(name);
        self.base_mods.iter().any(|m| casefold(m) == folded)
    }

    /// Alias de versión en la forma que piden los parsers.
    pub fn version_aliases(&self) -> Vec<VersionAlias> {
        self.version_aliases
            .iter()
            .map(|(p, k, l)| VersionAlias {
                prefix: (*p).into(),
                key: (*k).into(),
                label: (*l).into(),
            })
            .collect()
    }
}

pub fn game(id: &str) -> Option<&'static GameDef> {
    GAMES.iter().find(|g| g.id == id)
}

/// Tactical Ops se anuncia como `ut`: lo delatan el modo o la regla de TOST.
pub fn is_tacops(rules: &IndexMap<String, String>) -> bool {
    rules.contains_key("tostver") || rules.get("gametype").is_some_and(|g| is_tacops_gametype(g))
}

/// `fullmatch` de `TO\d+|TOST\w*|s_SWATGame` sin distinguir mayúsculas (texto latin-1).
fn is_tacops_gametype(g: &str) -> bool {
    let lower = g.to_lowercase();
    let word = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '_');
    if let Some(rest) = lower.strip_prefix("tost") {
        return word(rest);
    }
    if let Some(rest) = lower.strip_prefix("to") {
        return !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit());
    }
    lower == "s_swatgame"
}
