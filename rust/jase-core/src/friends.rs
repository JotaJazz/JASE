//! Lista de amigos. Traducción de `src/jase/friends.py` (por ahora, solo lo que lee la config).
use crate::protocols::quake3::strip_colors;
use crate::pystr::{casefold, py_strip};
use serde_json::Value;
use std::collections::HashSet;

pub const MAX_FRIENDS: usize = 100;

/// Nombre sin códigos de color ^N, sin espacios en los extremos y en minúsculas.
pub fn normalize(name: &str) -> String {
    casefold(py_strip(&strip_colors(name)))
}

/// Tolerante: descarta lo inválido y los vacíos, únicos (normalizados), como mucho MAX_FRIENDS.
pub fn parse(raw: &Value) -> Vec<String> {
    let Some(items) = raw.as_array() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for name in items.iter().filter_map(Value::as_str).map(py_strip) {
        let key = normalize(name);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        result.push(name.to_owned());
        if result.len() >= MAX_FRIENDS {
            break;
        }
    }
    result
}
