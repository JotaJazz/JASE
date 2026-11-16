//! Lista oficial de masters (descargable + empaquetada) y masters propios del usuario.
//! Traducción de `src/jase/masters.py` sin la descarga (fase 2b) ni la migración desde
//! la config (fase 4).
use crate::fsutil::atomic_write;
use crate::games::{GAMES, game};
use crate::pystr::casefold;
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{self, Read};
use std::path::Path;

pub const SCHEMA: u32 = 1;
pub const MAX_OFFICIAL_BYTES: usize = 256 * 1024;
pub const OFFICIAL_URL: &str =
    "https://raw.githubusercontent.com/JotaJazz/JASE/master/masters.json";
pub const CHECK_INTERVAL_S: u64 = 86400;
pub const RETRY_AFTER_FAILURE_S: u64 = 3600;
pub const FETCH_TIMEOUT_S: u64 = 10;
pub const META_FILENAME: &str = "masters.meta.json";
pub const OFFICIAL_FILENAME: &str = "masters.json";
pub const CUSTOM_FILENAME: &str = "masters.custom.json";

const BUNDLED: &[u8] = include_bytes!("../../../src/jase/assets/masters.json");

/// (host, puerto).
pub type Master = (String, u16);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct MastersError(pub String);

fn err(msg: impl Into<String>) -> MastersError {
    MastersError(msg.into())
}

pub fn master_key(m: &Master) -> String {
    format!("{}:{}", casefold(&m.0), m.1)
}

/// `[A-Za-z0-9-]{1,63}(\.[A-Za-z0-9-]{1,63})*\.?`, hasta 253 caracteres.
pub fn valid_host(host: &str) -> bool {
    if host.chars().count() > 253 {
        return false;
    }
    let body = host.strip_suffix('.').unwrap_or(host);
    !body.is_empty()
        && body.split('.').all(|l| {
            (1..=63).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// Entero JSON (no bool ni decimal) entre 1 y 65535.
pub fn valid_port(port: &Value) -> Option<u16> {
    port.as_u64()
        .and_then(|p| u16::try_from(p).ok())
        .filter(|&p| p >= 1)
}

fn entry_master(e: &Value) -> Option<Master> {
    let host = e.get("host")?.as_str().filter(|h| valid_host(h))?;
    Some((host.to_owned(), valid_port(e.get("port")?)?))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialList {
    pub schema: u32,
    pub updated: String,
    pub games: IndexMap<String, Vec<Master>>,
}

fn parse_entries(entries: &Value, game_id: &str) -> Result<Vec<Master>, MastersError> {
    let list = entries
        .as_array()
        .ok_or_else(|| err(format!("{game_id}: se esperaba una lista")))?;
    let mut result: IndexMap<String, Master> = IndexMap::new();
    for e in list {
        if !e.is_object() {
            return Err(err(format!("{game_id}: entrada inválida")));
        }
        if !e
            .get("host")
            .and_then(Value::as_str)
            .is_some_and(valid_host)
        {
            return Err(err(format!("{game_id}: host inválido")));
        }
        let m = entry_master(e).ok_or_else(|| err(format!("{game_id}: puerto inválido")))?;
        result.entry(master_key(&m)).or_insert(m);
    }
    Ok(result.into_values().collect())
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

pub fn parse_official(data: &[u8]) -> Result<OfficialList, MastersError> {
    if data.len() > MAX_OFFICIAL_BYTES {
        return Err(err("masters.json demasiado grande"));
    }
    // Anidamiento > 128: error (en Python, RecursionError → inválido).
    let raw: Value = std::str::from_utf8(data)
        .ok()
        .and_then(|s| serde_json::from_str(s).ok())
        .ok_or_else(|| err("masters.json no es JSON válido"))?;
    let raw = raw
        .as_object()
        .ok_or_else(|| err("masters.json: se esperaba un objeto"))?;
    // En Python `schema != 1` deja pasar 1.0 (pero no True).
    if raw.get("schema").and_then(Value::as_f64) != Some(f64::from(SCHEMA)) {
        return Err(err("masters.json: schema desconocido"));
    }
    let updated = raw
        .get("updated")
        .and_then(Value::as_str)
        .filter(|u| is_iso_date(u))
        .ok_or_else(|| err("masters.json: 'updated' inválido"))?;
    let games_raw = raw
        .get("games")
        .and_then(Value::as_object)
        .ok_or_else(|| err("masters.json: falta 'games'"))?;
    let mut games = IndexMap::new();
    for (game_id, entries) in games_raw {
        if game(game_id).is_none() {
            continue; // ids desconocidos: se ignoran sin validar
        }
        games.insert(game_id.clone(), parse_entries(entries, game_id)?);
    }
    // Salvaguarda: si ningún juego conocido queda con masters, se rechaza.
    if !GAMES.iter().any(|g| {
        games
            .get(g.id)
            .map_or(!g.masters.is_empty(), |m| !m.is_empty())
    }) {
        return Err(err("masters.json: todos los juegos quedan sin masters"));
    }
    Ok(OfficialList {
        schema: SCHEMA,
        updated: updated.to_owned(),
        games,
    })
}

/// Lee a lo sumo MAX_OFFICIAL_BYTES + 1 bytes (el excedente lo rechaza el parser).
fn read_capped(path: &Path) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_OFFICIAL_BYTES as u64 + 1)
        .read_to_end(&mut data)?;
    Ok(data)
}

fn builtin_official() -> OfficialList {
    let games = GAMES
        .iter()
        .map(|g| {
            (
                g.id.to_owned(),
                g.masters
                    .iter()
                    .map(|(h, p)| ((*h).to_owned(), *p))
                    .collect(),
            )
        })
        .collect();
    OfficialList {
        schema: SCHEMA,
        updated: String::new(),
        games,
    }
}

pub fn bundled_official() -> Result<OfficialList, MastersError> {
    parse_official(BUNDLED)
}

/// La más nueva entre la caché válida y la copia empaquetada → masters de la tabla.
pub fn load_official(cache_dir: &Path) -> OfficialList {
    let cached = read_capped(&cache_dir.join(OFFICIAL_FILENAME))
        .ok()
        .and_then(|d| parse_official(&d).ok());
    let Ok(bundled) = bundled_official() else {
        return cached.unwrap_or_else(builtin_official);
    };
    // Fechas ISO (validadas por parse_official): se comparan como texto.
    match cached {
        Some(c) if c.updated >= bundled.updated => c,
        _ => bundled,
    }
}

pub fn official_for(game_id: &str, official: &OfficialList) -> Vec<Master> {
    if let Some(list) = official.games.get(game_id) {
        return list.clone();
    }
    game(game_id)
        .map(|g| {
            g.masters
                .iter()
                .map(|(h, p)| ((*h).to_owned(), *p))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomMasters {
    pub added: IndexMap<String, Vec<Master>>,
    pub disabled: IndexMap<String, BTreeSet<String>>,
}

pub fn load_custom(path: &Path) -> CustomMasters {
    let mut custom = CustomMasters::default();
    let Ok(raw) = read_capped(path) else {
        return custom;
    };
    if raw.len() > MAX_OFFICIAL_BYTES {
        return custom;
    }
    let Some(data) = std::str::from_utf8(&raw)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
    else {
        return custom;
    };
    if let Some(added) = data.get("added").and_then(Value::as_object) {
        for (game_id, entries) in added {
            let Some(entries) = entries.as_array() else {
                continue;
            };
            let mut seen: IndexMap<String, Master> = IndexMap::new();
            for m in entries.iter().filter_map(entry_master) {
                seen.entry(master_key(&m)).or_insert(m);
            }
            if !seen.is_empty() {
                custom
                    .added
                    .insert(game_id.clone(), seen.into_values().collect());
            }
        }
    }
    if let Some(disabled) = data.get("disabled").and_then(Value::as_object) {
        for (game_id, keys) in disabled {
            let Some(keys) = keys.as_array() else {
                continue;
            };
            let clean: BTreeSet<String> = keys
                .iter()
                .filter_map(Value::as_str)
                .filter(|k| !k.is_empty())
                .map(str::to_owned)
                .collect();
            if !clean.is_empty() {
                custom.disabled.insert(game_id.clone(), clean);
            }
        }
    }
    custom
}

/// Mismo JSON que Python (`indent=2`, sin escapar no-ASCII, `disabled` ordenado).
pub fn save_custom(custom: &CustomMasters, path: &Path) -> io::Result<()> {
    let added: serde_json::Map<String, Value> = custom
        .added
        .iter()
        .filter(|(_, ms)| !ms.is_empty())
        .map(|(gid, ms)| {
            (
                gid.clone(),
                ms.iter()
                    .map(|(h, p)| json!({"host": h, "port": p}))
                    .collect(),
            )
        })
        .collect();
    let disabled: serde_json::Map<String, Value> = custom
        .disabled
        .iter()
        .filter(|(_, keys)| !keys.is_empty())
        .map(|(gid, keys)| (gid.clone(), json!(keys)))
        .collect();
    let data = json!({"schema": SCHEMA, "added": added, "disabled": disabled});
    let text = serde_json::to_vec_pretty(&data).map_err(io::Error::other)?;
    atomic_write(path, &text)
}

pub fn effective_masters(
    game_id: &str,
    official: &OfficialList,
    custom: &CustomMasters,
) -> Vec<Master> {
    let disabled = custom.disabled.get(game_id);
    let mut result: IndexMap<String, Master> = IndexMap::new();
    for m in official_for(game_id, official) {
        let key = master_key(&m);
        if !disabled.is_some_and(|d| d.contains(&key)) {
            result.entry(key).or_insert(m);
        }
    }
    for m in custom.added.get(game_id).into_iter().flatten() {
        result.entry(master_key(m)).or_insert_with(|| m.clone());
    }
    result.into_values().collect()
}
