//! Listas de servidores del último escaneo completo, guardadas entre sesiones.
//! Traducción de `src/jase/server_cache.py`.
//!
//! Un archivo por juego (`<id>.json`) y un índice pequeño (`index.json`: servidores y fecha
//! de cada lista) para los contadores del árbol sin leer las listas. Carga tolerante: un
//! archivo dañado no es una lista y una entrada inválida se descarta sola.
use crate::fsutil::atomic_write;
use crate::games::game;
use crate::masters::valid_host;
use crate::models::{Player, ServerAddress, ServerInfo};
use crate::pyjson::loads;
use indexmap::IndexMap;
use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub const SCHEMA: u32 = 1;
/// Una lista de Steam (10 000 servidores) ocupa unos 10 MB.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
pub const INDEX: &str = "index.json";

/// `_to_json`, con las claves en su orden.
pub fn server_to_json(info: &ServerInfo) -> Value {
    let players: Vec<Value> = info
        .players
        .iter()
        .map(|p| json!([p.name, p.score, p.ping, p.raw_name]))
        .collect();
    json!({
        "address": info.address.to_string(),
        "connect": info.connect_address.to_string(),
        "name": info.name, "map": info.map, "gametype": info.gametype,
        "num_players": info.num_players, "max_players": info.max_players,
        "password": info.password, "version": info.version, "ping_ms": info.ping_ms,
        "players": players,
        "rules": info.rules, "raw_name": info.raw_name, "bots": info.bots,
        "deduced": info.connect_port_deduced,
    })
}

fn address(v: &Value) -> Option<ServerAddress> {
    let (host, port) = v.as_str()?.rsplit_once(':')?;
    if !valid_host(host) || port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port = port.parse::<u16>().ok().filter(|&p| p >= 1)?;
    Some(ServerAddress::new(host, port))
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

/// `bounded_int` de un entero JSON (no bool ni decimal).
fn int(v: &Value) -> Option<i32> {
    v.as_i64()
        .map(|n| n.clamp(i32::MIN.into(), i32::MAX.into()) as i32)
}

fn player(v: &Value) -> Option<Player> {
    let [n, s, p, r] = v.as_array()?.as_slice() else {
        return None;
    };
    let mut player = Player::new(text(n)?, int(s)?, int(p)?);
    player.raw_name = text(r)?;
    Some(player)
}

/// `_from_json`: `None` donde Python lanza ValueError, TypeError o KeyError.
pub fn server_from_json(raw: &Value) -> Option<ServerInfo> {
    let raw = raw.as_object()?;
    let get = |k: &str| raw.get(k);
    let players = match get("players")? {
        Value::Array(list) => list.iter().map(player).collect::<Option<Vec<_>>>()?,
        // Python itera la cadena o el dict: vacíos son «sin jugadores»; si no, falla.
        Value::String(s) if s.is_empty() => Vec::new(),
        Value::Object(m) if m.is_empty() => Vec::new(),
        _ => return None,
    };
    let rules = get("rules")?
        .as_object()?
        .iter()
        .map(|(k, v)| Some((k.clone(), text(v)?)))
        .collect::<Option<IndexMap<_, _>>>()?;
    let mut info = ServerInfo::new(address(get("address")?)?, address(get("connect")?)?);
    info.name = text(get("name")?)?;
    info.map = text(get("map")?)?;
    info.gametype = text(get("gametype")?)?;
    info.num_players = int(get("num_players")?)?;
    info.max_players = int(get("max_players")?)?;
    info.password = get("password")?.as_bool()?;
    info.version = text(get("version")?)?;
    // ServerInfo.ping_ms es u32: un negativo queda en 0 (Python lo guarda negativo).
    info.ping_ms = int(get("ping_ms")?)?.max(0) as u32;
    info.players = players;
    info.rules = rules;
    info.raw_name = text(get("raw_name")?)?;
    info.bots = int(get("bots")?)?;
    info.connect_port_deduced = get("deduced")?.as_bool()?;
    Some(info)
}

/// `_valid_time`: instante Unix usable (número no bool, finito, `0 <= t < 2**33`).
fn valid_time(v: Option<&Value>) -> Option<f64> {
    v?.as_f64()
        .filter(|t| t.is_finite() && (0.0..8_589_934_592.0).contains(t))
}

fn path(cache_dir: &Path, game_id: &str) -> Option<PathBuf> {
    game(game_id).map(|_| cache_dir.join(format!("{game_id}.json")))
}

fn read_json(path: &Path, max_bytes: usize) -> Option<Value> {
    let mut data = Vec::new();
    File::open(path)
        .ok()?
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut data)
        .ok()?;
    if data.len() > max_bytes {
        return None;
    }
    loads(&data)
}

/// {juego: (servidores, guardada en)}; vacío si falta o está dañado.
pub fn read_index(cache_dir: &Path) -> IndexMap<String, (u64, f64)> {
    let Some(Value::Object(raw)) = read_json(&cache_dir.join(INDEX), MAX_BYTES) else {
        return IndexMap::new();
    };
    raw.iter()
        .filter(|(game_id, _)| game(game_id).is_some())
        .filter_map(|(game_id, entry)| {
            let [n, t] = entry.as_array()?.as_slice() else {
                return None;
            };
            Some((game_id.clone(), (n.as_u64()?, valid_time(Some(t))?)))
        })
        .collect()
}

/// `json.dumps(data)` con los separadores por defecto de Python: `{"cod2": [2, 5.0]}`.
/// shortcut: `saved_at` (aquí y en la lista) sale con los bytes de `repr(float)` para
/// `0.0001 <= t < 2**33` (probado con 20 000 al azar); debajo difieren (`1e-05` en Python,
/// `0.00001` en serde_json) pero se leen igual. Formatear a mano si alguna vez importa.
fn write_index(cache_dir: &Path, index: &IndexMap<String, (u64, f64)>) -> io::Result<()> {
    let entries: Vec<String> = index
        .iter()
        .map(|(g, (n, t))| format!("{}: [{n}, {}]", Value::from(g.as_str()), json!(t)))
        .collect();
    atomic_write(
        &cache_dir.join(INDEX),
        format!("{{{}}}", entries.join(", ")).as_bytes(),
    )
}

/// Guarda la lista y su entrada del índice. Bloqueante: llamarla desde un hilo.
pub fn save(cache_dir: &Path, game_id: &str, servers: &[ServerInfo], now: f64) -> io::Result<()> {
    let Some(path) = path(cache_dir, game_id) else {
        return Ok(());
    };
    let data = json!({
        "schema": SCHEMA,
        "saved_at": now,
        "servers": servers.iter().map(server_to_json).collect::<Vec<_>>(),
    });
    atomic_write(&path, &serde_json::to_vec(&data).map_err(io::Error::other)?)?;
    let mut index = read_index(cache_dir);
    index.insert(game_id.to_owned(), (servers.len() as u64, now));
    write_index(cache_dir, &index)
}

/// (guardada en, servidores) o `None` si no hay una lista válida. Bloqueante.
pub fn load(cache_dir: &Path, game_id: &str) -> Option<(f64, Vec<ServerInfo>)> {
    load_with_limit(cache_dir, game_id, MAX_BYTES)
}

#[doc(hidden)]
pub fn load_with_limit(
    cache_dir: &Path,
    game_id: &str,
    max_bytes: usize,
) -> Option<(f64, Vec<ServerInfo>)> {
    let raw = read_json(&path(cache_dir, game_id)?, max_bytes)?;
    let raw: &Map<String, Value> = raw.as_object()?;
    // `!= SCHEMA` de Python: 1, 1.0 y true valen lo mismo.
    let schema = raw.get("schema")?;
    if !(schema.as_bool() == Some(true) || schema.as_f64() == Some(f64::from(SCHEMA))) {
        return None;
    }
    let saved_at = valid_time(raw.get("saved_at"))?;
    let servers = raw.get("servers")?.as_array()?;
    Some((
        saved_at,
        servers.iter().filter_map(server_from_json).collect(),
    ))
}

/// Borra la lista guardada de un juego (p. ej. cambió el filtro de Steam).
pub fn forget(cache_dir: &Path, game_id: &str) {
    let Some(path) = path(cache_dir, game_id) else {
        return;
    };
    let _ = fs::remove_file(path);
    let mut index = read_index(cache_dir);
    if index.shift_remove(game_id).is_some() {
        let _ = write_index(cache_dir, &index);
    }
}
