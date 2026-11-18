//! Lo editable del diálogo de Ajustes, sin vista ni E/S (como `SettingsDialog` de
//! `ui/dialogs.py`).

use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use jase_core::config::{AUTO_REFRESH_CHOICES, Config};
use jase_core::games::{Family, GAMES, GameDef, LaunchStyle, game};
use jase_core::i18n::{LANGUAGES, tr, tr_fmt};
use jase_core::pystr::py_strip;

use crate::masters_tab::MastersTab;
use crate::theme::{TERMINAL_COLORS, THEME_IDS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Games,
    Masters,
    General,
    Appearance,
}

/// Estado de la carpeta de un juego.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStatus {
    Steam,
    Searching,
    /// El exe hallado.
    Found(String),
    Missing,
}

/// El botón «Actualizar base de países».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountryButton {
    Disabled,
    Ready,
    Downloading,
}

#[derive(Debug, Clone)]
pub struct Form {
    pub tab: Tab,
    pub paths: IndexMap<&'static str, String>,
    pub status: HashMap<&'static str, PathStatus>,
    pub download_maps: bool,
    pub language: String,
    pub auto_refresh: u32,
    pub check_updates: bool,
    pub tray: bool,
    pub wine: String,
    pub steam_key: String,
    pub steam_include_empty: bool,
    pub theme: String,
    pub terminal_color: String,
    pub masters: MastersTab,
    pub countries: CountryButton,
}

/// Sin carpeta no hay nada que leer del disco; con carpeta hay que buscar el exe.
pub fn initial_status(game: &GameDef, path: &str) -> PathStatus {
    if !py_strip(path).is_empty() {
        PathStatus::Searching
    } else if game.launch_style == LaunchStyle::Steam {
        PathStatus::Steam
    } else {
        PathStatus::Missing
    }
}

pub fn path_status_text(game: &GameDef, status: &PathStatus) -> String {
    match status {
        PathStatus::Steam => tr("Se conecta mediante Steam; no necesita carpeta"),
        PathStatus::Searching => tr("Buscando…"),
        PathStatus::Found(exe) => {
            let name = Path::new(exe)
                .file_name()
                .map_or(exe.clone(), |n| n.to_string_lossy().into_owned());
            format!("\u{2713} {name}")
        }
        PathStatus::Missing => {
            let wanted = if game.family == Family::Valve && !game.valve_gamedir.is_empty() {
                tr_fmt(
                    "{exe} con la carpeta {folder}",
                    &[("exe", &game.exe_relpath), ("folder", &game.valve_gamedir)],
                )
            } else {
                game.exe_relpath.to_owned()
            };
            format!(
                "\u{26a0} {}",
                tr_fmt("No se encontró {wanted}", &[("wanted", &wanted)])
            )
        }
    }
}

/// Como `findData` + `max(index, 0)`: un valor que no está entre las opciones abre en la
/// primera (`first`).
fn known_or(value: &str, first: &str, rest: &[&str]) -> String {
    if value == first || rest.contains(&value) {
        value
    } else {
        first
    }
    .to_owned()
}

impl Form {
    pub fn new(
        cfg: &Config,
        steam_key: &str,
        masters: MastersTab,
        countries: CountryButton,
    ) -> Form {
        let paths: IndexMap<&'static str, String> = GAMES
            .iter()
            .map(|g| (g.id, cfg.game_paths.get(g.id).cloned().unwrap_or_default()))
            .collect();
        let status = GAMES
            .iter()
            .map(|g| (g.id, initial_status(g, &paths[g.id])))
            .collect();
        Form {
            tab: Tab::Games,
            paths,
            status,
            download_maps: cfg.download_map_images,
            language: known_or(&cfg.language, "auto", &LANGUAGES),
            auto_refresh: Some(cfg.auto_refresh_minutes)
                .filter(|m| AUTO_REFRESH_CHOICES.contains(m))
                .unwrap_or(0),
            check_updates: cfg.check_app_updates,
            tray: cfg.minimize_to_tray,
            wine: cfg.wine_command.clone(),
            steam_key: steam_key.to_owned(),
            steam_include_empty: cfg.steam_include_empty,
            theme: known_or(&cfg.theme, "system", &THEME_IDS),
            terminal_color: known_or(&cfg.terminal_color, TERMINAL_COLORS[0], &TERMINAL_COLORS),
            masters,
            countries,
        }
    }

    pub fn apply_to(&self, cfg: &mut Config) {
        for (&id, path) in &self.paths {
            let path = py_strip(path);
            // Vaciar un juego con ruta por defecto la guarda como "" para que `load_config`
            // no la vuelva a proponer.
            if !path.is_empty() || game(id).is_some_and(|g| !g.default_path.is_empty()) {
                cfg.game_paths.insert(id.to_owned(), path.to_owned());
            } else {
                cfg.game_paths.shift_remove(id);
            }
        }
        cfg.download_map_images = self.download_maps;
        cfg.language = self.language.clone();
        cfg.auto_refresh_minutes = self.auto_refresh;
        cfg.check_app_updates = self.check_updates;
        cfg.minimize_to_tray = self.tray;
        cfg.steam_include_empty = self.steam_include_empty;
        cfg.wine_command = py_strip(&self.wine).to_owned();
        cfg.theme = self.theme.clone();
        cfg.terminal_color = self.terminal_color.clone();
    }

    pub fn steam_key(&self) -> String {
        py_strip(&self.steam_key).to_owned()
    }

    /// El color del fósforo solo cuenta con Terminal elegido.
    pub fn terminal_color_enabled(&self) -> bool {
        self.theme == "terminal"
    }
}
