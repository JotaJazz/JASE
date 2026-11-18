//! Opciones de los combos de la barra lateral y de la versión (`window_filters.py`).

mod support;

use std::collections::HashMap;

use jase::sidebar::{country_options, max_ping_options, ping_options, version_options};
use jase_core::games::game;
use support::lang::with_language;

#[test]
fn max_ping_options_are_pythons() {
    with_language("es", || {
        let labels: Vec<(String, i64)> = max_ping_options();
        assert_eq!(
            labels,
            [
                ("Sin límite".to_owned(), 0),
                ("50".to_owned(), 50),
                ("100".to_owned(), 100),
                ("150".to_owned(), 150),
                ("250".to_owned(), 250),
            ]
        );
    });
}

#[test]
fn unknown_saved_ping_is_kept() {
    with_language("es", || {
        let opts = ping_options(75);
        let labels: Vec<String> = opts.iter().map(ToString::to_string).collect();
        assert_eq!(labels, ["Sin límite", "50", "100", "150", "250", "75"]);
        assert_eq!(opts.last().unwrap().ms, 75);
        assert_eq!(ping_options(100).len(), 5);
    });
}

#[test]
fn country_options_sorted_by_name_with_counts() {
    with_language("es", || {
        let counts = HashMap::from([("cl".to_owned(), 1), ("ar".to_owned(), 2)]);
        let opts = country_options(&counts, "de");
        let labels: Vec<String> = opts.iter().map(ToString::to_string).collect();
        assert_eq!(
            labels,
            ["Todos los países", "Alemania", "Argentina (2)", "Chile (1)"]
        );
        let codes: Vec<&str> = opts.iter().map(|o| o.code.as_str()).collect();
        assert_eq!(codes, ["", "de", "ar", "cl"]);
        // Sin elegido, solo los que tienen servidores.
        assert_eq!(country_options(&counts, "").len(), 3);
    });
}

#[test]
fn version_options_include_saved_and_observed() {
    let cod2 = game("cod2").unwrap();
    let observed = ["1.3".to_owned(), "2.0".to_owned(), String::new()];
    let opts = version_options(cod2, "1.01b", &observed);
    let keys: Vec<&str> = opts.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["Todas", "1.0", "1.2", "1.3", "1.4", "1.01b", "2.0"]);
    let labels: Vec<String> = opts.iter().map(ToString::to_string).collect();
    assert!(labels.contains(&"1.4 (CoD2x)".to_owned()));
    assert!(!labels.contains(&"1.4".to_owned()));
}
