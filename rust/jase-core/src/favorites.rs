//! Traducción de `ui/favorites.py`: favoritos, notas y grupos sobre `Config`.
use crate::config::Config;
use crate::masters::valid_host;
use crate::models::{ServerAddress, ServerInfo};
use crate::pystr::{casefold, py_split, py_strip};
use indexmap::IndexMap;

pub const NOTE_MAX: usize = 200;
pub const GROUP_MAX: usize = 40;

/// `"host:puerto"` → dirección; `None` si el texto no es válido.
pub fn parse_address(text: &str) -> Option<ServerAddress> {
    let (host, port) = text.rsplit_once(':')?;
    if !valid_host(host) || host.starts_with(['.', '-']) {
        return None;
    }
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Más de 5 dígitos significativos no cabe en u16; `parse` falla y da `None`.
    let number: u16 = port.parse().ok().filter(|&n| n != 0)?;
    Some(ServerAddress::new(host, number))
}

fn dedup(texts: Option<&Vec<String>>) -> Vec<ServerAddress> {
    let mut result: Vec<ServerAddress> = Vec::new();
    for text in texts.into_iter().flatten() {
        if let Some(address) = parse_address(text)
            && !result.contains(&address)
        {
            result.push(address);
        }
    }
    result
}

/// Favoritos válidos del juego, en el orden guardado y sin repetir.
pub fn favorite_addresses(cfg: &Config, game_id: &str) -> Vec<ServerAddress> {
    dedup(cfg.favorites.get(game_id))
}

pub fn is_favorite(cfg: &Config, game_id: &str, address: &ServerAddress) -> bool {
    let text = address.to_string();
    cfg.favorites
        .get(game_id)
        .is_some_and(|e| e.contains(&text))
}

/// `true` si se agregó (`false` si ya estaba).
pub fn add_favorite(cfg: &mut Config, game_id: &str, address: &ServerAddress) -> bool {
    if is_favorite(cfg, game_id, address) {
        return false;
    }
    cfg.favorites
        .entry(game_id.to_owned())
        .or_default()
        .push(address.to_string());
    true
}

/// `true` si se quitó; una lista que queda vacía se elimina de la config.
pub fn remove_favorite(cfg: &mut Config, game_id: &str, address: &ServerAddress) -> bool {
    if !is_favorite(cfg, game_id, address) {
        return false;
    }
    let text = address.to_string();
    if let Some(entries) = cfg.favorites.get_mut(game_id) {
        entries.retain(|e| *e != text);
        if entries.is_empty() {
            cfg.favorites.shift_remove(game_id);
        }
    }
    set_note(cfg, game_id, &text, "");
    set_group(cfg, game_id, &text, "");
    true
}

fn get_text<'a>(map: &'a IndexMap<String, IndexMap<String, String>>, g: &str, a: &str) -> &'a str {
    map.get(g).and_then(|m| m.get(a)).map_or("", String::as_str)
}

/// Una línea, espacios colapsados, como mucho `max` caracteres (no bytes).
fn one_line(text: &str, max: usize) -> String {
    let joined = py_split(text).join(" ");
    py_strip(&joined.chars().take(max).collect::<String>()).to_owned()
}

fn set_text(
    map: &mut IndexMap<String, IndexMap<String, String>>,
    game_id: &str,
    address: &str,
    clean: String,
) {
    if !clean.is_empty() {
        map.entry(game_id.to_owned())
            .or_default()
            .insert(address.to_owned(), clean);
        return;
    }
    if let Some(inner) = map.get_mut(game_id) {
        inner.shift_remove(address);
        if inner.is_empty() {
            map.shift_remove(game_id);
        }
    }
}

pub fn get_note<'a>(cfg: &'a Config, game_id: &str, address: &str) -> &'a str {
    get_text(&cfg.favorite_notes, game_id, address)
}

/// Guarda la nota (una línea, máx. `NOTE_MAX`); texto vacío la borra.
pub fn set_note(cfg: &mut Config, game_id: &str, address: &str, text: &str) {
    set_text(
        &mut cfg.favorite_notes,
        game_id,
        address,
        one_line(text, NOTE_MAX),
    );
}

/// Notas del juego con direcciones válidas.
pub fn game_notes(cfg: &Config, game_id: &str) -> IndexMap<ServerAddress, String> {
    let mut result = IndexMap::new();
    for (text, note) in cfg.favorite_notes.get(game_id).into_iter().flatten() {
        if let Some(address) = parse_address(text) {
            result.insert(address, note.clone());
        }
    }
    result
}

/// Nombre de grupo en una línea y como mucho `GROUP_MAX`.
pub fn clean_group(name: &str) -> String {
    one_line(name, GROUP_MAX)
}

pub fn get_group<'a>(cfg: &'a Config, game_id: &str, address: &str) -> &'a str {
    get_text(&cfg.favorite_groups, game_id, address)
}

/// Mueve el favorito al grupo `name`; vacío lo deja sin grupo.
pub fn set_group(cfg: &mut Config, game_id: &str, address: &str, name: &str) {
    set_text(
        &mut cfg.favorite_groups,
        game_id,
        address,
        clean_group(name),
    );
}

/// Grupos en uso entre los favoritos actuales del juego, por nombre.
pub fn group_names(cfg: &Config, game_id: &str) -> Vec<String> {
    let current: Vec<String> = favorite_addresses(cfg, game_id)
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut names: Vec<String> = Vec::new();
    for (address, group) in cfg.favorite_groups.get(game_id).into_iter().flatten() {
        if current.contains(address) && !names.contains(group) {
            names.push(group.clone());
        }
    }
    names.sort_by(|a, b| casefold(a).cmp(&casefold(b)).then_with(|| a.cmp(b)));
    names
}

/// Fila de un favorito que no respondió: nombre = dirección, resto vacío.
pub fn offline_server(address: &ServerAddress) -> ServerInfo {
    let mut info = ServerInfo::new(address.clone(), address.clone());
    info.name = address.to_string();
    info.online = false;
    info
}
