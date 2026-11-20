//! «Se armó partida»: avisar cuando un servidor vigilado llega a N jugadores. Traducción de
//! `party.py`.
use crate::models::ServerInfo;
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashMap;

pub const MAX_THRESHOLD: u32 = 128;
pub const DEFAULT_THRESHOLD: u32 = 4;

/// Tolerante: juego por juego y entrada por entrada; lo inválido se descarta.
pub fn parse(raw: &Value) -> IndexMap<String, IndexMap<String, u32>> {
    let Some(raw) = raw.as_object() else {
        return IndexMap::new();
    };
    raw.iter()
        .filter_map(|(game_id, alerts)| {
            let clean: IndexMap<String, u32> = alerts
                .as_object()?
                .iter()
                .filter(|(a, _)| a.contains(':'))
                .filter_map(|(a, n)| {
                    let n = u32::try_from(n.as_i64()?).ok()?;
                    (1..=MAX_THRESHOLD).contains(&n).then(|| (a.clone(), n))
                })
                .collect();
            (!clean.is_empty()).then(|| (game_id.clone(), clean))
        })
        .collect()
}

/// Avisa al cruzar el umbral hacia arriba, una vez; se rearma al bajar a la mitad del umbral.
/// La primera respuesta solo fija el punto de partida.
#[derive(Default)]
pub struct PartyTracker {
    armed: HashMap<(String, String), bool>,
}

impl PartyTracker {
    pub fn reset(&mut self, game_id: &str, address: &str) {
        self.armed.remove(&(game_id.to_owned(), address.to_owned()));
    }

    pub fn update(&mut self, game_id: &str, info: &ServerInfo, threshold: u32) -> bool {
        if !info.online {
            return false;
        }
        let humans = i64::from(info.humans());
        let limit = i64::from(threshold);
        let key = (game_id.to_owned(), info.address.to_string());
        match self.armed.get(&key).copied() {
            None => {
                self.armed.insert(key, humans < limit);
            }
            Some(true) if humans >= limit => {
                self.armed.insert(key, false);
                return true;
            }
            Some(false) if humans <= limit / 2 => {
                self.armed.insert(key, true);
            }
            _ => {}
        }
        false
    }
}
