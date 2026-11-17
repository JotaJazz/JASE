//! Traducción de `src/jase/favorites_io.py`: exportar e importar favoritos.
use crate::config::Config;
use crate::favorites;
use crate::games::game;
use crate::pyjson;
use serde_json::{Map, Value, json};
use std::io;
use std::path::Path;

pub const FORMAT: &str = "jase-favorites";
pub const VERSION: i64 = 1;
pub const MAX_IMPORT_BYTES: u64 = 1_048_576;

#[derive(Debug, PartialEq, Eq)]
pub struct ImportResult {
    pub added: usize,
    pub skipped: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("file too large")]
    TooLarge,
    #[error("not a JASE favorites file")]
    Invalid,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub fn export_favorites(cfg: &Config) -> Value {
    let mut games = Map::new();
    for game_id in cfg.favorites.keys() {
        let mut entries = Vec::new();
        for address in favorites::favorite_addresses(cfg, game_id) {
            let text = address.to_string();
            let mut entry = Map::new();
            entry.insert("address".into(), json!(text));
            let note = favorites::get_note(cfg, game_id, &text);
            if !note.is_empty() {
                entry.insert("note".into(), json!(note));
            }
            let group = favorites::get_group(cfg, game_id, &text);
            if !group.is_empty() {
                entry.insert("group".into(), json!(group));
            }
            entries.push(Value::Object(entry));
        }
        if !entries.is_empty() {
            games.insert(game_id.clone(), Value::Array(entries));
        }
    }
    json!({"format": FORMAT, "version": VERSION, "favorites": games})
}

/// Fusiona `data` en `cfg`: no duplica ni pisa notas existentes.
pub fn import_favorites(cfg: &mut Config, data: &Value) -> Result<ImportResult, ImportError> {
    // `version != 1` de Python: 1, 1.0 y true son iguales a 1.
    let version_ok = |v: Option<&Value>| match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() == Some(1.0),
        _ => false,
    };
    let games = match data {
        Value::Object(o)
            if o.get("format").and_then(Value::as_str) == Some(FORMAT)
                && version_ok(o.get("version")) =>
        {
            o.get("favorites").and_then(Value::as_object)
        }
        _ => None,
    }
    .ok_or(ImportError::Invalid)?;
    let (mut added, mut skipped) = (0, 0);
    for (game_id, entries) in games {
        let Value::Array(entries) = entries else {
            skipped += 1;
            continue;
        };
        if game(game_id).is_none() {
            skipped += entries.len();
            continue;
        }
        for entry in entries {
            let address = entry
                .get("address")
                .and_then(Value::as_str)
                .and_then(favorites::parse_address);
            let Some(address) = address else {
                skipped += 1;
                continue;
            };
            if favorites::add_favorite(cfg, game_id, &address) {
                added += 1;
            }
            let text = address.to_string();
            if let Some(note) = entry.get("note").and_then(Value::as_str)
                && favorites::get_note(cfg, game_id, &text).is_empty()
            {
                favorites::set_note(cfg, game_id, &text, note);
            }
            if let Some(group) = entry.get("group").and_then(Value::as_str)
                && favorites::get_group(cfg, game_id, &text).is_empty()
            {
                favorites::set_group(cfg, game_id, &text, group);
            }
        }
    }
    Ok(ImportResult { added, skipped })
}

/// Lee y decodifica el JSON; `TooLarge` si pesa más de `MAX_IMPORT_BYTES` (sin leerlo).
pub fn read_import_file(path: &Path) -> Result<Value, ImportError> {
    if std::fs::metadata(path)?.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge);
    }
    pyjson::loads(&std::fs::read(path)?).ok_or(ImportError::Invalid)
}
