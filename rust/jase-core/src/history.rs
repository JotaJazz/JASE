//! Historial de ocupación por servidor. Traducción de `src/jase/history.py`.
//!
//! Guarda muestras (instante, jugadores, máximo) de favoritos y recientes en
//! `cache/history.json` y calcula el promedio de jugadores por hora local. El tiempo
//! siempre se inyecta (`now`).
use crate::fsutil::atomic_write;
use crate::pyjson::loads;
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::PathBuf;

pub const MIN_GAP_S: i64 = 300;
pub const KEEP_DAYS: i64 = 14;
pub const MAX_SAMPLES: usize = 4032;
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

pub struct History {
    pub path: PathBuf,
    /// {game_id: {"ip:puerto": [[t, jugadores, máximo], ...]}} en orden cronológico.
    pub data: IndexMap<String, IndexMap<String, Vec<[i64; 3]>>>,
}

impl History {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            data: IndexMap::new(),
        }
    }

    pub fn record(
        &mut self,
        game_id: &str,
        address: &str,
        players: i64,
        max_players: i64,
        now: f64,
    ) {
        let samples = self
            .data
            .entry(game_id.to_owned())
            .or_default()
            .entry(address.to_owned())
            .or_default();
        if samples
            .last()
            .is_some_and(|last| now - (last[0] as f64) < MIN_GAP_S as f64)
        {
            return;
        }
        samples.push([now as i64, players, max_players]);
        let cutoff = now - (KEEP_DAYS * 86400) as f64;
        // Como el `while ... pop(0)` de Python: solo las del principio.
        let old = samples
            .iter()
            .position(|s| (s[0] as f64) >= cutoff)
            .unwrap_or(samples.len());
        samples.drain(..old);
        if samples.len() > MAX_SAMPLES {
            samples.drain(..samples.len() - MAX_SAMPLES);
        }
    }

    pub fn samples(&self, game_id: &str, address: &str) -> Vec<[i64; 3]> {
        self.data
            .get(game_id)
            .and_then(|servers| servers.get(address))
            .cloned()
            .unwrap_or_default()
    }

    /// Promedio de jugadores por hora local (`None` si esa hora no tiene datos).
    pub fn hourly_average(
        &self,
        game_id: &str,
        address: &str,
        now: f64,
        tz_offset_s: i64,
    ) -> [Option<f64>; 24] {
        // i128: los enteros de Python no desbordan con valores del archivo.
        let mut sums = [0i128; 24];
        let mut counts = [0u32; 24];
        let cutoff = now - (KEEP_DAYS * 86400) as f64;
        let samples = self.data.get(game_id).and_then(|s| s.get(address));
        for &[t, players, _] in samples.into_iter().flatten() {
            if (t as f64) < cutoff {
                continue;
            }
            let hour = (i128::from(t) + i128::from(tz_offset_s))
                .div_euclid(3600)
                .rem_euclid(24) as usize;
            sums[hour] += i128::from(players);
            counts[hour] += 1;
        }
        std::array::from_fn(|h| (counts[h] > 0).then(|| sums[h] as f64 / f64::from(counts[h])))
    }

    /// Quita servidores que ya no son favoritos ni recientes.
    pub fn prune(&mut self, keep: &HashSet<(String, String)>) {
        for (game_id, servers) in &mut self.data {
            servers.retain(|address, _| keep.contains(&(game_id.clone(), address.clone())));
        }
        self.data.retain(|_, servers| !servers.is_empty());
    }

    /// `json.dumps(data, separators=(",", ":"))`, con el `ensure_ascii` por defecto.
    pub fn to_bytes(&self) -> Vec<u8> {
        let text = serde_json::to_string(&self.data).unwrap_or_default();
        let mut out = String::with_capacity(text.len());
        // Fuera de ASCII solo hay texto de las cadenas: se escapa en UTF-16, como Python.
        for c in text.chars() {
            // DEL (0x7f) también se escapa: `ensure_ascii` de Python.
            if c.is_ascii() && c != '\x7f' {
                out.push(c);
            } else {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
        out.into_bytes()
    }

    /// Carga tolerante: ausente, enorme, corrupto o con forma rara → historial vacío.
    pub fn load(&mut self, now: f64) {
        self.data = IndexMap::new();
        let limit = now + 86400.0;
        if !fs::metadata(&self.path).is_ok_and(|m| m.len() <= MAX_FILE_BYTES) {
            return;
        }
        let Some(Value::Object(raw)) = fs::read(&self.path).ok().and_then(|d| loads(&d)) else {
            return;
        };
        for (game_id, servers) in &raw {
            let Some(servers) = servers.as_object() else {
                continue;
            };
            for (address, samples) in servers {
                let Some(samples) = samples.as_array() else {
                    continue;
                };
                let mut clean: Vec<[i64; 3]> = samples
                    .iter()
                    .filter_map(|s| {
                        let [t, p, m] = s.as_array()?.as_slice() else {
                            return None;
                        };
                        let sample = [t.as_i64()?, p.as_i64()?, m.as_i64()?];
                        (sample[0] > 0 && sample[0] as f64 <= limit).then_some(sample)
                    })
                    .collect();
                clean.sort_by_key(|s| s[0]);
                if !clean.is_empty() {
                    let keep = clean.len().saturating_sub(MAX_SAMPLES);
                    self.data
                        .entry(game_id.clone())
                        .or_default()
                        .insert(address.clone(), clean.split_off(keep));
                }
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        atomic_write(&self.path, &self.to_bytes())
    }
}
