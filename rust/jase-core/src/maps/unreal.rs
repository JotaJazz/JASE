//! Mapas de Unreal Tournament (UT99). Traducción de `src/jase/maps/unreal.py`.
use crate::paths::{child, resolve_game_dir};
use crate::pystr::casefold;
use std::path::Path;

/// Ruta de Maps/<map_name>.unr (sin distinguir mayúsculas), o `None`.
/// La caché (Cache/, archivos con nombre GUID) se ignora.
pub fn find_map_file(install_dir: Option<&str>, map_name: &str) -> Option<String> {
    if map_name.is_empty() || map_name.contains(['/', '\\']) {
        return None;
    }
    let game_dir = resolve_game_dir(install_dir, "Maps")?;
    let maps_dir = child(&game_dir, "Maps")?;
    let base: String = if casefold(map_name).ends_with(".unr") {
        let n = map_name.chars().count().saturating_sub(4);
        map_name.chars().take(n).collect()
    } else {
        map_name.to_owned()
    };
    child(&maps_dir, &format!("{base}.unr")).filter(|f| Path::new(f).is_file())
}
