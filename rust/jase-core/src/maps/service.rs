//! Nombres seguros para la caché y URL de GameTracker. Traducción de
//! `src/jase/maps/service.py`.
use crate::pystr::casefold;

pub const GAMETRACKER_URL: &str =
    "https://image.gametracker.com/images/maps/160x120/{gt_id}/{map}.jpg";

const WINDOWS_RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Nombre de archivo (sin extensión) válido en Windows para `map_name`, o `None`.
pub fn safe_map_filename(map_name: &str) -> Option<String> {
    let kept: String = map_name
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'..='\x1f'
            )
        })
        .collect();
    let mut name = casefold(kept.trim_matches([' ', '.']));
    if name.is_empty() {
        return None;
    }
    if WINDOWS_RESERVED.contains(&name.as_str()) {
        name.push('_');
    }
    Some(name)
}

/// GameTracker guarda los mapas en minúsculas y con `_` en todo carácter que no sea
/// alfanumérico (MOHAA «obj/obj_team2», UT99 «DM-Deck16][»).
pub fn gametracker_map_name(map_name: &str) -> String {
    casefold(map_name)
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn gametracker_url(gt_id: &str, map_name: &str) -> String {
    GAMETRACKER_URL
        .replace("{gt_id}", gt_id)
        .replace("{map}", &gametracker_map_name(map_name))
}
