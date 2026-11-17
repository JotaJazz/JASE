//! Configuración de la app. Traducción de `src/jase/config.py` (por ahora, solo la ruta).
use std::path::PathBuf;

/// `%APPDATA%\JimmysASE\config.json`; si no, `~/.config/jimmys-ase/config.json`.
pub fn default_config_path() -> PathBuf {
    match std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        Some(appdata) => PathBuf::from(appdata).join("JimmysASE").join("config.json"),
        None => std::env::home_dir()
            .unwrap_or_default()
            .join(".config/jimmys-ase/config.json"),
    }
}
