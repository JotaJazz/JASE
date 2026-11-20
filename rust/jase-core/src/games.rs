//! Definición de cada juego. Traducción de `games.py` (Python 1.3.0); la tabla está en
//! `games_table.rs`, que se edita a mano.
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

/// Cómo se pasa la dirección al exe (ver `GameDef::launch_style`).
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

/// Un juego (los campos de la dataclass `GameDef` de Python 1.3.0).
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
    /// Ejecutables alternativos, probados en orden si no está `exe_relpath`
    /// (p. ej. los lanzadores de OpenMoHAA).
    pub exe_candidates: &'static [&'static str],
    /// Ejecutables que, si están en la misma carpeta donde se resolvió el exe,
    /// se usan en su lugar (una carpeta de OpenMoHAA conserva MOHAA.exe).
    pub preferred_exes: &'static [&'static str],
    /// Carpeta de GameTracker para imágenes de mapas; "" = no hay imágenes.
    pub gametracker_id: &'static str,
    /// Quake 3: carpeta con los .pk3 que traen `levelshots/`; "" = juego de CoD (IWI).
    pub levelshot_dir: &'static str,
    /// Pantallas de carga IWI de CoD en lugar de levelshots.
    pub cod_loadscreens: bool,
    /// Credenciales del master GameSpy (gamename y clave de validación).
    pub gamespy_gamename: &'static str,
    pub gamespy_key: &'static str,
    /// GameSpy: si no está vacío, se descartan los servidores cuyo \status\ trae
    /// otro `gamename` (los masters de MOHAA mezclan AA, Spearhead y Breakthrough).
    pub status_gamename: &'static str,
    /// Quake3: se descartan los servidores cuyo `gamename` (carpeta del mod) esté aquí;
    /// para masters compartidos entre juegos ioq3 con el mismo protocolo.
    pub status_exclude_mods: &'static [&'static str],
    /// Quake3: gamename de la consulta al master ("getservers Xonotic 3 …"); necesario en
    /// dpmaster, que mezcla juegos. "" = consulta clásica sin gamename.
    pub master_gamename: &'static str,
    /// Quake II: solo se aceptan servidores con esta `gamedir` (un mod que se lista como
    /// juego aparte, p. ej. D-Day); se pasa al exe con "+set game".
    pub status_gamedir: &'static str,
    /// Cómo se pasa la dirección al exe: "+connect" (q3), "host:port?password=" (unreal)
    /// "-game <valve_gamedir> +connect" (valve, hl.exe) o "+restart 1 +joinServer" (bf1942).
    /// "-iwad <iwad> -connect" (zandronum).
    pub launch_style: LaunchStyle,
    /// GameSpy: desplazamiento sobre el puerto de consulta cuando falta `hostport`.
    pub connect_port_fallback: i32,
    /// Quake3: cvar del statusResponse que lleva la versión.
    pub version_key: &'static str,
    /// Quake3: (prefijo, clave, etiqueta) para agrupar versiones crudas en una sola entrada
    /// del selector (CoD2x anuncia "1.4.6.8", "1.4.4.6"… → clave "1.4", «1.4 (CoD2x)»).
    /// El valor crudo sigue en `ServerInfo.rules`.
    pub version_aliases: &'static [(&'static str, &'static str, &'static str)],
    /// Valve: appid del filtro del master (\\appid\\N) y carpeta del mod para -game.
    pub valve_appid: u32,
    /// Appid de la tienda de Steam de un juego no-Valve (JKA, JK2): su exe de Steam lleva el
    /// DRM de Steam y, abierto a mano, da «Application load error 5:0000065434».
    pub steam_appid: u32,
    pub valve_gamedir: &'static str,
    /// Valve: filtro crudo del master; vacío = \\appid\\{valve_appid}. Con `valve_gamedir`
    /// se descartan además los servidores que informan otra carpeta de juego.
    pub valve_master_filter: &'static str,
    /// Valve: la lista sale de la API web de Steam (necesita clave) en vez del master UDP.
    pub steam_webapi: bool,
    /// Valve: además de los masters UDP, api.steampowered.com en sus masters se consulta
    /// por la API web si hay clave (sin clave se sigue con los demás). CS 1.6.
    pub steam_webapi_optional: bool,
    /// Ruta de instalación propuesta en una config nueva ("" = ninguna).
    pub default_path: &'static str,
    /// Nombre para la barra lateral (vacío = `name`, ver `short_name()`); el completo va en
    /// el tooltip.
    pub short_name: &'static str,
    /// Regla del servidor que trae el mod ("" = juego sin mods) y los valores que
    /// cuentan como «(base)» (comparados con casefold).
    pub mod_key: &'static str,
    pub base_mods: &'static [&'static str],
    /// Iniciales (2 caracteres) del icono dibujado cuando no hay icono del exe.
    pub badge: &'static str,
    /// Master compartido con otro juego sin más distinción que las reglas del servidor:
    /// solo se listan los servidores que cumplan esto. No filtra consultas directas.
    pub list_filter: ListFilter,
    /// Una instalación completa por versión: (versión del servidor, subcarpeta).
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
