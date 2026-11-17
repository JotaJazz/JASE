//! Enlaces `jase://connect/<juego>/<host>:<puerto>`: construcción y análisis estricto.
//! Traducción de `src/jase/jase_url.py`. El enlace llega de fuera: `parse` rechaza todo lo que
//! no sea exactamente esa forma.
use crate::games::game;
use crate::masters::valid_host;
use crate::models::ServerAddress;

pub const SCHEME: &str = "jase";
pub const ACTION: &str = "connect";
pub const MAX_LENGTH: usize = 300;

pub fn build(game_id: &str, address: &ServerAddress) -> String {
    format!(
        "{SCHEME}://{ACTION}/{game_id}/{}:{}",
        address.host, address.port
    )
}

/// Compara los primeros 7 `char`s (nunca un corte por bytes: el texto puede ser multibyte).
pub fn is_link(text: &str) -> bool {
    let prefix = format!("{SCHEME}://");
    text.chars()
        .take(prefix.len())
        .flat_map(|c| c.to_lowercase())
        .eq(prefix.chars())
}

pub fn parse(text: &str) -> Option<(&'static str, ServerAddress)> {
    // Solo ASCII imprimible sin espacio (0x21..=0x7e).
    if text.is_empty() || text.chars().count() > MAX_LENGTH {
        return None;
    }
    if !text.bytes().all(|b| (0x21..=0x7e).contains(&b)) || !is_link(text) {
        return None;
    }
    let rest = &text[SCHEME.len() + 3..];
    if rest.contains(['?', '#', '@', '\\', '%']) {
        return None;
    }
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let parts: Vec<&str> = rest.split('/').collect();
    let [action, game_id, target] = parts[..] else {
        return None;
    };
    if !action.eq_ignore_ascii_case(ACTION) {
        return None;
    }
    let valid_id = (1..=32).contains(&game_id.len())
        && game_id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    let game = game(game_id).filter(|_| valid_id)?;
    let (host, port_text) = target.rsplit_once(':')?;
    if !valid_host(host) || host.starts_with(['.', '-']) {
        return None;
    }
    if !(1..=5).contains(&port_text.len()) || !port_text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port: u16 = port_text.parse().ok().filter(|&p| p >= 1)?;
    Some((game.id, ServerAddress::new(host, port)))
}
