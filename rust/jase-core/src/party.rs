//! «Se armó partida». Traducción de `src/jase/party.py` (por ahora, solo lo que lee la config).
use indexmap::IndexMap;
use serde_json::Value;

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
