//! Núcleo de JASE sin UI. Traducción de `src/jase/` (Python 1.3.0).
pub mod browse;
pub mod config;
pub mod detect;
pub mod favorites;
pub mod favorites_io;
pub mod fetch_meta;
pub mod filter_presets;
pub mod friends;
pub mod fsutil;
pub mod games;
pub mod geo;
pub mod history;
pub mod i18n;
pub mod jase_url;
pub mod masters;
pub mod models;
pub mod net;
pub mod party;
pub mod paths;
pub mod player_search;
pub mod protocols;
pub mod pyjson;
pub mod pystr;
pub mod recent;
pub mod scanner;
pub mod server_cache;
#[cfg(windows)]
mod winreg;
