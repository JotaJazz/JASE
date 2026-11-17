//! Filtros guardados: combinaciones con nombre de filtros rápidos + texto del filtro.
//! Traducción de `src/jase/filter_presets.py`.
use crate::config::{parse_quick_filters, quick_filters_json};
use crate::models::QuickFilters;
use crate::pystr::{casefold, py_strip};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub const MAX_PRESETS: usize = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterPreset {
    pub name: String,
    pub quick: QuickFilters,
    pub text: String,
}

/// Tolerante: lo inválido se descarta; nombres únicos (casefold); como mucho MAX_PRESETS.
pub fn parse(raw: &Value) -> Vec<FilterPreset> {
    let Some(items) = raw.as_array() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let Some(item) = item.as_object() else {
            continue;
        };
        let Some(name) = item
            .get("name")
            .and_then(Value::as_str)
            .map(py_strip)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        if !seen.insert(casefold(name)) {
            continue;
        }
        let empty = Map::new();
        let quick = item
            .get("quick")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        result.push(FilterPreset {
            name: name.to_owned(),
            quick: parse_quick_filters(quick),
            text: item
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        });
        if result.len() >= MAX_PRESETS {
            break;
        }
    }
    result
}

pub fn to_json(presets: &[FilterPreset]) -> Value {
    presets
        .iter()
        .map(|p| json!({"name": p.name, "quick": quick_filters_json(&p.quick), "text": p.text}))
        .collect()
}

/// Mismo nombre (casefold) reemplaza en su sitio; si no, se añade (tope MAX_PRESETS).
pub fn upsert(presets: &[FilterPreset], preset: FilterPreset) -> Vec<FilterPreset> {
    let key = casefold(&preset.name);
    let mut out = presets.to_vec();
    if let Some(p) = out.iter_mut().find(|p| casefold(&p.name) == key) {
        *p = preset;
        return out;
    }
    out.push(preset);
    out.truncate(MAX_PRESETS);
    out
}

pub fn remove(presets: &[FilterPreset], name: &str) -> Vec<FilterPreset> {
    let key = casefold(name);
    presets
        .iter()
        .filter(|p| casefold(&p.name) != key)
        .cloned()
        .collect()
}
