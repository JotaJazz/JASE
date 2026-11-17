//! Mod de un servidor. Traducción de `mod_of` de `src/jase/browse.py`.
use crate::games::GameDef;
use crate::models::ServerInfo;
use crate::protocols::quake3::strip_colors;
use crate::pystr::{casefold, py_strip};

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
