//! El formulario de Ajustes sin ventana (como los `test_settings_*` de `test_main_window.py`
//! y `test_maps_service.py`).
mod support;

use indexmap::IndexMap;
use jase::masters_tab::MastersTab;
use jase::settings::{CountryButton, Form, PathStatus, Tab, initial_status, path_status_text};
use jase_core::config::Config;
use jase_core::games::game;
use jase_core::masters::{CustomMasters, OfficialList};
use support::lang::with_language;

fn form_with(cfg: &Config, key: &str) -> Form {
    let official = OfficialList {
        schema: 1,
        updated: String::new(),
        games: IndexMap::new(),
    };
    let masters = MastersTab::new(official, CustomMasters::default(), false, false);
    Form::new(cfg, key, masters, CountryButton::Ready)
}

fn form(cfg: &Config) -> Form {
    form_with(cfg, "")
}

#[test]
fn settings_dialog_apply_to() {
    let mut cfg = Config::default();
    let mut d = form(&cfg);
    assert_eq!(d.tab, Tab::Games);
    assert_eq!(d.paths["cod2"], r"D:\Juegos\CoD2");
    d.paths.insert("cod1", r"C:\CoD".to_owned());
    d.paths.insert("ut99", String::new());
    d.paths.insert("coduo", String::new());
    d.apply_to(&mut cfg);
    // ut99 tiene ruta por defecto: se guarda vacía para que no vuelva al cargar.
    let expected: IndexMap<String, String> = [
        ("cod2", r"D:\Juegos\CoD2"),
        ("cod1", r"C:\CoD"),
        ("ut99", ""),
        ("mohaa", r"D:\Juegos\MOHAA_STOCK"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    assert_eq!(cfg.game_paths, expected);
}

#[test]
fn settings_status_steam_launch_game_needs_no_folder() {
    with_language("es", || {
        let l4d2 = game("l4d2").unwrap();
        let cfg = Config {
            game_paths: IndexMap::new(),
            ..Config::default()
        };
        assert_eq!(form(&cfg).status["l4d2"], PathStatus::Steam);
        assert_eq!(initial_status(l4d2, ""), PathStatus::Steam);
        assert_eq!(
            path_status_text(l4d2, &PathStatus::Steam),
            "Se conecta mediante Steam; no necesita carpeta"
        );
        assert_eq!(initial_status(l4d2, "/juegos/l4d2"), PathStatus::Searching);
        assert_eq!(
            initial_status(game("cod2").unwrap(), " "),
            PathStatus::Missing
        );
    });
}

#[test]
fn path_status_texts() {
    with_language("es", || {
        let g = |id| game(id).unwrap();
        assert_eq!(
            path_status_text(g("cod2"), &PathStatus::Found("C:/x/CoD2MP_s.exe".into())),
            "\u{2713} CoD2MP_s.exe"
        );
        let missing = path_status_text(g("cod2"), &PathStatus::Missing);
        assert!(missing.starts_with('\u{26a0}') && missing.contains("CoD2MP_s.exe"));
        assert!(path_status_text(g("cs16"), &PathStatus::Missing).contains("cstrike"));
        assert!(path_status_text(g("l4d2"), &PathStatus::Missing).contains("left4dead2"));
        assert_eq!(
            path_status_text(g("cod2"), &PathStatus::Searching),
            "Buscando…"
        );
    });
}

#[test]
fn settings_language_combo_saves() {
    let mut d = form(&Config {
        language: "en".to_owned(),
        ..Config::default()
    });
    assert_eq!(d.language, "en");
    d.language = "es".to_owned();
    let mut cfg = Config::default();
    d.apply_to(&mut cfg);
    assert_eq!(cfg.language, "es");
    let unknown = form(&Config {
        language: "xx".to_owned(),
        ..Config::default()
    });
    assert_eq!(unknown.language, "auto");
}

#[test]
fn settings_auto_refresh_combo_roundtrip() {
    let mut cfg = Config {
        auto_refresh_minutes: 10,
        ..Config::default()
    };
    let mut d = form(&cfg);
    assert_eq!(d.auto_refresh, 10);
    d.auto_refresh = 0;
    d.apply_to(&mut cfg);
    assert_eq!(cfg.auto_refresh_minutes, 0);
    cfg.auto_refresh_minutes = 7;
    assert_eq!(form(&cfg).auto_refresh, 0);
}

#[test]
fn settings_include_empty_checkbox_follows_config() {
    let cfg = Config {
        steam_include_empty: true,
        ..Config::default()
    };
    assert!(form(&cfg).steam_include_empty);
}

#[test]
fn settings_download_checkbox() {
    let mut cfg = Config::default();
    let mut d = form(&cfg);
    assert!(!d.download_maps);
    d.download_maps = true;
    d.apply_to(&mut cfg);
    assert!(cfg.download_map_images);
    assert!(form(&cfg).download_maps);
}

#[test]
fn settings_terminal_color_only_enabled_for_terminal() {
    let mut cfg = Config {
        theme: "dark".to_owned(),
        terminal_color: "white".to_owned(),
        ..Config::default()
    };
    let mut d = form(&cfg);
    assert_eq!(d.terminal_color, "white");
    assert!(!d.terminal_color_enabled());
    d.theme = "terminal".to_owned();
    assert!(d.terminal_color_enabled());
    d.terminal_color = "amber".to_owned();
    d.apply_to(&mut cfg);
    assert_eq!(
        (cfg.theme.as_str(), cfg.terminal_color.as_str()),
        ("terminal", "amber")
    );
    // Desconocidos: el tema abre en «Sistema» y el color en el primero.
    cfg.theme = "nope".to_owned();
    cfg.terminal_color = "nope".to_owned();
    let d = form(&cfg);
    assert_eq!(
        (d.theme.as_str(), d.terminal_color.as_str()),
        ("system", "green")
    );
}

#[test]
fn settings_steam_key_is_stripped() {
    let mut d = form_with(&Config::default(), "ABC");
    assert_eq!(d.steam_key(), "ABC");
    d.steam_key = " NEW ".to_owned();
    assert_eq!(d.steam_key(), "NEW");
}

#[test]
fn settings_wine_is_stripped() {
    let mut cfg = Config::default();
    let mut d = form(&cfg);
    assert_eq!(d.wine, "wine");
    d.wine = "  umu-run ".to_owned();
    d.apply_to(&mut cfg);
    assert_eq!(cfg.wine_command, "umu-run");
}
