//! Lista de amigos y detección de apariciones nuevas. Traducción de `src/jase/friends.py`.
use crate::models::{ServerAddress, ServerInfo};
use crate::protocols::quake3::strip_colors;
use crate::pystr::{casefold, py_strip};
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

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

/// Amigos presentes en el servidor (coincidencia exacta normalizada), con el nombre guardado.
pub fn friends_in(info: &ServerInfo, friends: &[String]) -> Vec<String> {
    let mut present = HashSet::new();
    for p in &info.players {
        let shown = if p.raw_name.is_empty() {
            &p.name
        } else {
            &p.raw_name
        };
        present.insert(normalize(shown));
        present.insert(normalize(&p.name));
    }
    friends
        .iter()
        .filter(|f| present.contains(&normalize(f)))
        .cloned()
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct FriendSighting {
    pub friend: String,
    pub game_id: String,
    pub server: ServerInfo,
}

/// Recuerda dónde se vio a cada amigo para avisar solo de apariciones nuevas.
#[derive(Default)]
pub struct FriendTracker {
    pub friends: Vec<String>,
    /// (juego, dirección) → claves normalizadas de los amigos vistos allí.
    seen: HashMap<(String, ServerAddress), HashSet<String>>,
}

impl FriendTracker {
    pub fn new(friends: Vec<String>) -> Self {
        Self {
            friends,
            seen: HashMap::new(),
        }
    }

    pub fn update(&mut self, game_id: &str, info: &ServerInfo) -> Vec<FriendSighting> {
        let key = (game_id.to_owned(), info.address.clone());
        let now: IndexMap<String, String> = friends_in(info, &self.friends)
            .into_iter()
            .map(|f| (normalize(&f), f))
            .collect();
        let before = self.seen.get(&key);
        let new = now
            .iter()
            .filter(|(norm, _)| !before.is_some_and(|b| b.contains(*norm)))
            .map(|(_, name)| FriendSighting {
                friend: name.clone(),
                game_id: game_id.to_owned(),
                server: info.clone(),
            })
            .collect();
        if now.is_empty() {
            self.seen.remove(&key); // nadie: el que vuelva avisa de nuevo
        } else {
            self.seen.insert(key, now.into_keys().collect());
        }
        new
    }
}
