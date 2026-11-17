//! Traducción de `src/jase/recent.py`: servidores recientes por juego.
use crate::config::Config;
use crate::favorites::parse_address;
use crate::models::ServerAddress;

pub const RECENT_MAX: usize = 20;

/// Pone `address` primero (sin duplicados) y recorta a `RECENT_MAX`.
pub fn record_recent(cfg: &mut Config, game_id: &str, address: &ServerAddress) {
    let text = address.to_string();
    let entries = cfg.recent.entry(game_id.to_owned()).or_default();
    entries.retain(|e| *e != text);
    entries.insert(0, text);
    entries.truncate(RECENT_MAX);
}

/// Recientes válidos, del más nuevo al más antiguo y sin repetir.
pub fn recent_addresses(cfg: &Config, game_id: &str) -> Vec<ServerAddress> {
    let mut result: Vec<ServerAddress> = Vec::new();
    for text in cfg.recent.get(game_id).into_iter().flatten() {
        if let Some(address) = parse_address(text)
            && !result.contains(&address)
        {
            result.push(address);
        }
    }
    result
}

/// `true` si se quitó; una lista que queda vacía se elimina de la config.
pub fn remove_recent(cfg: &mut Config, game_id: &str, address: &ServerAddress) -> bool {
    let text = address.to_string();
    let Some(entries) = cfg.recent.get_mut(game_id) else {
        return false;
    };
    if !entries.contains(&text) {
        return false;
    }
    entries.retain(|e| *e != text);
    if entries.is_empty() {
        cfg.recent.shift_remove(game_id);
    }
    true
}
