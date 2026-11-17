//! Búsqueda de jugadores por nombre en las listas ya cargadas. Traducción de
//! `src/jase/player_search.py`.
use crate::games::GAMES;
use crate::models::{Player, ServerInfo};
use crate::pystr::{casefold, py_strip};
use indexmap::IndexMap;

pub struct PlayerHit<'a> {
    pub game_id: &'static str,
    pub server: &'a ServerInfo,
    pub player: &'a Player,
}

/// Jugadores cuyo nombre plano contiene `query` (sin distinguir mayúsculas), ordenados por
/// juego (según GAMES), servidor y jugador; con `limit` quedan los primeros de ese orden.
pub fn find_players<'a>(
    lists: &'a IndexMap<String, Vec<ServerInfo>>,
    query: &str,
    limit: usize,
) -> Vec<PlayerHit<'a>> {
    let needle = casefold(py_strip(query));
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, String, String, PlayerHit<'a>)> = Vec::new();
    for (order, game) in GAMES.iter().enumerate() {
        for server in lists
            .get(game.id)
            .into_iter()
            .flatten()
            .filter(|s| s.online)
        {
            for player in &server.players {
                let name = casefold(&player.name);
                if name.contains(&needle) {
                    let hit = PlayerHit {
                        game_id: game.id,
                        server,
                        player,
                    };
                    hits.push((order, casefold(&server.name), name, hit));
                }
            }
        }
    }
    // Estable, como `heapq.nsmallest`.
    hits.sort_by(|a, b| (a.0, &a.1, &a.2).cmp(&(b.0, &b.1, &b.2)));
    hits.into_iter().take(limit).map(|h| h.3).collect()
}
