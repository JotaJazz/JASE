//! Filtro de la tabla (`ServerFilterProxy` de `server_model.py`): texto, versión, filtros
//! rápidos y marcas del árbol.

use std::collections::HashSet;

use jase_core::browse::matches;
use jase_core::games::{ALL_VERSIONS, GameDef};
use jase_core::models::{QuickFilters, ServerInfo};
use jase_core::pystr::py_strip;

#[derive(Debug, Clone)]
pub struct ServerFilter {
    /// Recortado y en minúsculas (`set_text`).
    pub text: String,
    pub version: String,
    pub quick: QuickFilters,
    /// Mods y mapas solo cuentan con juego; los países, siempre.
    pub tree_game: Option<&'static GameDef>,
    pub mods: HashSet<String>,
    pub maps: HashSet<String>,
    pub countries: HashSet<String>,
}

impl Default for ServerFilter {
    fn default() -> Self {
        Self {
            text: String::new(),
            version: ALL_VERSIONS.to_owned(),
            quick: QuickFilters::default(),
            tree_game: None,
            mods: HashSet::new(),
            maps: HashSet::new(),
            countries: HashSet::new(),
        }
    }
}

impl ServerFilter {
    pub fn set_text(&mut self, raw: &str) {
        self.text = py_strip(raw).to_lowercase();
    }

    /// `country`: código del servidor en minúsculas, o `None` si no se conoce.
    pub fn passes_quick(&self, info: &ServerInfo, country: Option<&str>) -> bool {
        let q = &self.quick;
        !(q.hide_empty && info.num_players == 0
            || q.hide_bots && info.num_players > 0 && info.humans() == 0
            || !q.country.is_empty() && country != Some(q.country.as_str())
            || q.hide_full && info.max_players > 0 && info.num_players >= info.max_players
            || q.hide_password && info.password
            || q.max_ping > 0 && i64::from(info.ping_ms) > q.max_ping)
    }

    pub fn accepts(&self, info: &ServerInfo, country: Option<&str>) -> bool {
        if !self.passes_quick(info, country) {
            return false;
        }
        if self.version != ALL_VERSIONS && info.version != self.version {
            return false;
        }
        if let Some(game) = self.tree_game
            && (!self.mods.is_empty() || !self.maps.is_empty())
            && !matches(game, info, &self.mods, &self.maps)
        {
            return false;
        }
        if !self.countries.is_empty() && !country.is_some_and(|c| self.countries.contains(c)) {
            return false;
        }
        if self.text.is_empty() {
            return true;
        }
        let haystack = format!(
            "{}\n{}\n{}\n{}",
            info.name,
            info.map,
            info.gametype,
            country.unwrap_or("")
        );
        haystack.to_lowercase().contains(&self.text)
    }
}
