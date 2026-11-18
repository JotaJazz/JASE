mod support;

use std::collections::HashMap;

use jase::startup::{DEFAULT_GAME, force_x11, initial_game, locale_name_from_env, prepare};
use jase_core::config::Config;
use jase_core::i18n::current_language;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |k| map.get(k).cloned()
}

#[test]
fn main_resolves_language_before_window() {
    // `prepare` cambia el idioma global; el candado lo devuelve al español
    support::lang::with_language("es", || {
        let en = Config {
            language: "en".to_owned(),
            ..Config::default()
        };
        prepare(&en, "es_CL");
        assert_eq!(current_language(), "en");
        let auto = Config {
            language: "auto".to_owned(),
            ..Config::default()
        };
        prepare(&auto, "de_DE");
        assert_eq!(current_language(), "de");
    });
}

#[test]
fn system_locale_name_prefers_ui_languages() {
    let both = env(&[("LANGUAGE", "pt_BR:en"), ("LANG", "fr_FR.UTF-8")]);
    assert_eq!(locale_name_from_env(&both), "pt_BR");
    assert_eq!(
        locale_name_from_env(&env(&[("LANG", "fr_FR.UTF-8")])),
        "fr_FR.UTF-8"
    );
    assert_eq!(
        locale_name_from_env(&env(&[("LC_ALL", "es_CL"), ("LANG", "fr_FR")])),
        "es_CL"
    );
    assert_eq!(locale_name_from_env(&env(&[("LANG", "C")])), "");
    assert_eq!(
        locale_name_from_env(&env(&[("LC_ALL", "POSIX"), ("LANG", "it_IT")])),
        "it_IT"
    );
    assert_eq!(locale_name_from_env(&env(&[])), "");
}

#[test]
fn force_x11_only_in_wsl() {
    assert!(force_x11("5.15.167.4-microsoft-standard-WSL2\n", None));
    assert!(force_x11("4.4.0-19041-Microsoft", None));
    assert!(force_x11("6.8.0-45-generic", Some("Ubuntu")));
    assert!(!force_x11("6.8.0-45-generic", None));
}

#[test]
fn initial_game_falls_back_to_cod2() {
    let mut cfg = Config::default();
    assert_eq!(initial_game(&cfg), DEFAULT_GAME);
    cfg.ui_state
        .insert("last_game".to_owned(), "cod4".to_owned());
    assert_eq!(initial_game(&cfg), "cod4");
    cfg.ui_state
        .insert("last_game".to_owned(), "no-existe".to_owned());
    assert_eq!(initial_game(&cfg), "cod2");
}
