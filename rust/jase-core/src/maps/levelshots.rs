//! Levelshots de los .pk3 de los juegos de la familia Quake 3 (no CoD).
//! Traducción de `src/jase/maps/levelshots.py`.
use super::cod::{archives_in, read_first};
use crate::paths::{child, resolve_game_dir};
use crate::pystr::casefold;

pub const LEVELSHOT_EXTS: [&str; 3] = [".jpg", ".tga", ".png"];

/// Ruta de mapa segura: segmentos con `/`, sin vacíos, `.`, `..`, `\` ni `:`.
pub fn valid_map_path(map_name: &str) -> bool {
    !map_name.is_empty()
        && !map_name.contains(['\\', ':'])
        && map_name
            .split('/')
            .all(|seg| !matches!(seg, "" | "." | ".."))
}

/// Bytes de levelshots/<map>.<ext> en los pk3 de <juego>/<base_dir>, o `None`.
/// Gana el último pk3 en orden de carga; los que no son zip válidos se ignoran.
pub fn find_levelshot(
    install_dir: Option<&str>,
    base_dir: &str,
    map_name: &str,
) -> Option<Vec<u8>> {
    if !valid_map_path(map_name) {
        return None;
    }
    let game_dir = resolve_game_dir(install_dir, base_dir)?;
    let folder = child(&game_dir, base_dir)?;
    let wanted: Vec<String> = LEVELSHOT_EXTS
        .iter()
        .map(|ext| casefold(&format!("levelshots/{map_name}{ext}")))
        .collect();
    archives_in(&folder)
        .iter()
        .rev()
        .filter(|archive| casefold(archive).ends_with(".pk3"))
        .find_map(|archive| read_first(archive, &wanted))
}
