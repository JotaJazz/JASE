//! Mods, mapas y países de los servidores de un juego. Traducción de `browse.py`.
use crate::games::GameDef;
use crate::models::ServerInfo;
use crate::protocols::quake3::strip_colors;
use crate::pystr::{casefold, py_strip};
use std::collections::{HashMap, HashSet};

pub const TOP_MAPS: usize = 10;
pub const BASE_MOD: &str = "(base)";
const MODS_PREFIX: &str = "mods/";

/// Carpeta del mod en minúsculas, sin `mods/`; `BASE_MOD` para el juego base.
pub fn mod_of(game: &GameDef, info: &ServerInfo) -> String {
    if game.mod_key.is_empty() {
        return BASE_MOD.into();
    }
    let raw = info.rules.get(game.mod_key).map_or("", String::as_str);
    let stripped = strip_colors(raw);
    let mut value = casefold(py_strip(&stripped));
    if let Some(rest) = value.strip_prefix(MODS_PREFIX) {
        value = rest.to_owned();
    }
    let value = py_strip(&value);
    if value.is_empty() || game.is_base_mod(value) {
        return BASE_MOD.into();
    }
    value.to_owned()
}

pub fn map_of(info: &ServerInfo) -> String {
    casefold(py_strip(&info.map))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Breakdown {
    pub mods: Vec<(String, usize)>,
    pub maps: Vec<(String, usize)>,
    pub other_maps: usize,
    /// Código ISO en minúsculas.
    pub countries: Vec<(String, usize)>,
}

/// Cantidad descendente y luego nombre.
fn ranked(counts: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Con vida propia: la búsqueda suele tomar prestado el modelo de la tabla.
pub type CountryOf<'a> = dyn Fn(&ServerInfo) -> Option<String> + 'a;

/// `country_of`: país de un servidor (`None` = desconocido, no se cuenta).
pub fn breakdown(
    game: &GameDef,
    servers: &[ServerInfo],
    country_of: Option<&CountryOf<'_>>,
) -> Breakdown {
    let (mut mods, mut maps, mut countries) = (HashMap::new(), HashMap::new(), HashMap::new());
    for info in servers {
        *mods.entry(mod_of(game, info)).or_insert(0) += 1;
        *maps.entry(map_of(info)).or_insert(0) += 1;
        if let Some(code) = country_of.and_then(|f| f(info)).filter(|c| !c.is_empty()) {
            *countries.entry(code).or_insert(0) += 1;
        }
    }
    let only_base = mods.keys().all(|m| m == BASE_MOD);
    let mut maps = ranked(maps);
    let other_maps = maps.iter().skip(TOP_MAPS).map(|m| m.1).sum();
    maps.truncate(TOP_MAPS);
    Breakdown {
        mods: if only_base { Vec::new() } else { ranked(mods) },
        maps,
        other_maps,
        countries: ranked(countries),
    }
}

/// Vacío = sin restricción; dentro de cada conjunto vale cualquiera (OR), entre ambos los dos (AND).
pub fn matches(
    game: &GameDef,
    info: &ServerInfo,
    mods: &HashSet<String>,
    maps: &HashSet<String>,
) -> bool {
    (mods.is_empty() || mods.contains(&mod_of(game, info)))
        && (maps.is_empty() || maps.contains(&map_of(info)))
}
