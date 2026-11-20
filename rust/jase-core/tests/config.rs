//! Port de tests/test_config.py, más los de la config en test_wine.py, test_masters.py,
//! test_friends.py y test_party.py, y la compatibilidad de bytes con Python 1.3.0.
mod support;

use indexmap::IndexMap;
use jase_core::config::{
    AUTO_REFRESH_CHOICES, Config, TERMINAL_COLORS, THEMES, config_from_json, config_to_json,
    load_config, save_config, version_for,
};
use jase_core::filter_presets::FilterPreset;
use jase_core::games::{self, GAMES};
use jase_core::masters::{
    CustomMasters, effective_masters, migrate_config_masters, parse_official,
};
use jase_core::models::QuickFilters;
use jase_core::{friends, party};
use proptest::prelude::*;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/data");

fn golden(name: &str) -> Vec<u8> {
    fs::read(Path::new(GOLDEN).join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

struct Tmp {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

/// Carpeta temporal con la ruta de `config.json` dentro.
fn tmp() -> Tmp {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    Tmp { _dir: dir, path }
}

fn load_text(t: &Tmp, text: &str) -> Config {
    fs::write(&t.path, text).unwrap();
    load_config(&t.path)
}

fn load_json(t: &Tmp, v: Value) -> Config {
    load_text(t, &v.to_string())
}

fn s(v: &str) -> String {
    v.to_owned()
}

fn smap(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (s(k), s(v))).collect()
}

fn vfilter<'a>(id: &str, cfg: &'a Config) -> &'a str {
    version_for(games::game(id).unwrap(), cfg)
}

fn qf() -> QuickFilters {
    QuickFilters::default()
}

#[test]
fn defaults_when_missing() {
    let t = tmp();
    let cfg = load_config(&t.path.with_file_name("nope.json"));
    assert_eq!(
        cfg.game_paths,
        smap(&[
            ("cod2", r"D:\Juegos\CoD2"),
            ("ut99", r"D:\Juegos\UnrealTournament"),
            ("mohaa", r"D:\Juegos\MOHAA_STOCK"),
        ])
    );
    assert_eq!(cfg.theme, "goldsrc");
    assert_eq!(vfilter("cod2", &cfg), "1.3");
    assert_eq!(vfilter("ut99", &cfg), "Todas");
}

#[test]
fn corrupt_file_gives_defaults() {
    let t = tmp();
    assert_eq!(load_text(&t, "{not json").theme, "goldsrc");
}

#[test]
fn roundtrip() {
    let t = tmp();
    let p = t.path.with_file_name("sub").join("c.json");
    let mut cfg = load_config(&p);
    cfg.theme = s("light");
    cfg.version_filter.insert(s("cod2"), s("1.0"));
    cfg.masters.insert(s("ut99"), vec![(s("localhost"), 28900)]);
    save_config(&cfg, &p).unwrap();
    let cfg2 = load_config(&p);
    assert_eq!(cfg2.theme, "light");
    assert_eq!(vfilter("cod2", &cfg2), "1.0");
    assert_eq!(
        cfg2.masters,
        IndexMap::from([(s("ut99"), vec![(s("localhost"), 28900)])])
    );
}

#[test]
fn bad_masters_keeps_other_fields() {
    let t = tmp();
    let cfg = load_json(
        &t,
        json!({"theme": "dark", "game_paths": {"cod2": "X"},
               "version_filter": {"cod2": "1.0"}, "masters": {"ut99": [["only-host"]]}}),
    );
    assert_eq!(cfg.theme, "dark");
    assert_eq!(cfg.game_paths["cod2"], "X");
    assert_eq!(cfg.version_filter, smap(&[("cod2", "1.0")]));
    assert!(cfg.masters.is_empty());
}

#[test]
fn toplevel_list_gives_defaults() {
    let t = tmp();
    let cfg = load_text(&t, "[1, 2]");
    assert_eq!(cfg.theme, "goldsrc");
    assert!(cfg.game_paths.contains_key("cod2"));
}

#[test]
fn invalid_theme_gives_default() {
    let t = tmp();
    assert_eq!(load_text(&t, r#"{"theme": "disco"}"#).theme, "goldsrc");
}

#[test]
fn config_theme_tolerant() {
    let t = tmp();
    for tid in THEMES {
        assert_eq!(load_json(&t, json!({"theme": tid})).theme, tid);
    }
    for bad in [
        json!(null),
        json!(3),
        json!(["neon"]),
        json!({"a": 1}),
        json!("NEON"),
    ] {
        assert_eq!(load_json(&t, json!({"theme": bad})).theme, "goldsrc");
    }
}

#[test]
fn unhashable_theme_and_non_dict_fields_do_not_crash() {
    let t = tmp();
    for theme in [json!([]), json!({}), json!(5), json!(null)] {
        let cfg = load_json(
            &t,
            json!({"theme": theme, "game_paths": [1], "version_filter": "x", "masters": 3}),
        );
        assert_eq!(cfg.theme, "goldsrc");
        assert!(cfg.game_paths.contains_key("cod2"));
        assert!(cfg.version_filter.is_empty() && cfg.masters.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn save_config_is_atomic() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("c.json");
    let mut cfg = load_config(&p);
    cfg.theme = s("light");
    save_config(&cfg, &p).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
    // Como root la carpeta de solo lectura no impide escribir: no hay nada que probar.
    let probe = dir.path().join("probe");
    if fs::write(&probe, b"").is_ok() {
        let _ = fs::remove_file(&probe);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("como root: se salta save_config_is_atomic");
        return;
    }
    cfg.theme = s("dark");
    let result = save_config(&cfg, &p);
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err(), "el error de escritura debe propagarse");
    assert_eq!(load_config(&p).theme, "light"); // el archivo previo queda intacto
    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["c.json"]); // sin temporales
}

#[test]
fn quick_filters_roundtrip() {
    let t = tmp();
    let mut cfg = load_config(&t.path);
    assert_eq!(cfg.quick_filters, qf());
    cfg.quick_filters = QuickFilters {
        hide_empty: true,
        hide_password: true,
        max_ping: 150,
        ..qf()
    };
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).quick_filters, cfg.quick_filters);
}

#[test]
fn old_config_without_quick_filters_loads_defaults() {
    let t = tmp();
    let cfg = load_json(
        &t,
        json!({"theme": "dark", "game_paths": {"cod2": "X"}, "version_filter": {"cod2": "1.0"}}),
    );
    assert_eq!(cfg.quick_filters, qf());
    assert_eq!(cfg.theme, "dark");
    assert_eq!(cfg.game_paths["cod2"], "X");
}

#[test]
fn invalid_quick_filter_fields_fall_back_per_field() {
    let t = tmp();
    let raw = json!({"quick_filters": {"hide_empty": true, "hide_full": "si",
                                       "hide_password": 1, "max_ping": -5}});
    assert_eq!(
        load_json(&t, raw).quick_filters,
        QuickFilters {
            hide_empty: true,
            ..qf()
        }
    );
    let raw = json!({"quick_filters": {"max_ping": true}});
    assert_eq!(load_json(&t, raw).quick_filters, qf());
    assert_eq!(
        load_json(&t, json!({"quick_filters": [1, 2]})).quick_filters,
        qf()
    );
}

#[test]
fn ui_state_roundtrip() {
    let t = tmp();
    let mut cfg = load_config(&t.path);
    assert!(cfg.ui_state.is_empty());
    cfg.ui_state = smap(&[("geometry", "AAEC"), ("last_game", "ut99")]);
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).ui_state, cfg.ui_state);
}

#[test]
fn v1_config_without_ui_state_loads() {
    let t = tmp();
    let cfg = load_json(&t, json!({"theme": "dark", "game_paths": {"cod2": "X"}}));
    assert!(cfg.ui_state.is_empty());
    assert_eq!(cfg.theme, "dark");
}

#[test]
fn invalid_ui_state_is_ignored_per_entry() {
    let t = tmp();
    let raw = json!({"ui_state": {"geometry": 5, "last_game": "cod2", "x": null}});
    assert_eq!(load_json(&t, raw).ui_state, smap(&[("last_game", "cod2")]));
    let cfg = load_json(&t, json!({"ui_state": ["nope"], "theme": "light"}));
    assert!(cfg.ui_state.is_empty());
    assert_eq!(cfg.theme, "light");
}

#[test]
fn saved_game_paths_gain_new_defaults_without_overwriting() {
    let t = tmp();
    let cfg = load_text(
        &t,
        r#"{"game_paths": {"cod2": "E:\\CoD2", "cod1": "C:\\CoD", "ut99": ""}}"#,
    );
    let expected = smap(&[
        ("cod2", r"E:\CoD2"),
        ("cod1", r"C:\CoD"),
        ("ut99", ""),
        ("mohaa", r"D:\Juegos\MOHAA_STOCK"),
    ]);
    // Igualdad de dict de Python: sin orden.
    assert_eq!(cfg.game_paths.len(), expected.len());
    assert!(
        expected
            .iter()
            .all(|(k, v)| cfg.game_paths.get(k) == Some(v))
    );
}

#[test]
fn config_find_games_offered_tolerant() {
    let t = tmp();
    assert!(!load_config(&t.path).find_games_offered);
    let mut cfg = load_json(&t, json!({"find_games_offered": "si", "theme": "dark"}));
    assert!(!cfg.find_games_offered);
    assert_eq!(cfg.theme, "dark");
    cfg.find_games_offered = true;
    save_config(&cfg, &t.path).unwrap();
    assert!(load_config(&t.path).find_games_offered);
}

#[test]
fn config_language_tolerant() {
    let t = tmp();
    assert_eq!(load_config(&t.path).language, "auto");
    for raw in [
        "{}",
        r#"{"language": "xx"}"#,
        r#"{"language": 3}"#,
        r#"{"language": [1]}"#,
    ] {
        assert_eq!(load_text(&t, raw).language, "auto");
    }
    let cfg = load_text(&t, r#"{"language": "en"}"#);
    assert_eq!(cfg.language, "en");
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).language, "en");
    assert!(
        fs::read_to_string(&t.path)
            .unwrap()
            .contains(r#""language": "en""#)
    );
}

#[test]
fn config_themes_match_registry() {
    let constants: Value = serde_json::from_slice(&golden("config_constants.json")).unwrap();
    let mut themes = THEMES.to_vec();
    themes.sort_unstable();
    assert_eq!(constants["themes"], json!(themes));
    assert_eq!(constants["terminal_colors"], json!(TERMINAL_COLORS));
    assert_eq!(constants["auto_refresh"], json!(AUTO_REFRESH_CHOICES));
}

#[test]
fn recent_roundtrip_and_tolerance() {
    let t = tmp();
    let raw = json!({"recent": {"cod2": ["1.1.1.1:28960", 5], "ut99": "x"}});
    let expected = IndexMap::from([(s("cod2"), vec![s("1.1.1.1:28960")])]);
    assert_eq!(load_json(&t, raw).recent, expected);
    let mut cfg = load_json(&t, json!({"recent": [1, 2]}));
    assert!(cfg.recent.is_empty());
    cfg.recent = expected.clone();
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).recent, expected);
}

#[test]
fn auto_refresh_minutes_tolerant() {
    let t = tmp();
    for (raw, expected) in [
        (json!(7), 0),
        (json!(true), 0),
        (json!("5"), 0),
        (json!(5), 5),
        (json!(15), 15),
        (json!(0), 0),
        (json!(5.0), 0),
    ] {
        let cfg = load_json(&t, json!({"auto_refresh_minutes": raw}));
        assert_eq!(cfg.auto_refresh_minutes, expected, "{raw}");
    }
    let mut cfg = load_config(&t.path);
    cfg.auto_refresh_minutes = 10;
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).auto_refresh_minutes, 10);
}

/// Campo booleano: solo `true`/`false` de JSON valen; lo demás deja el default.
fn check_bool_field(key: &str, default: bool, get: fn(&Config) -> bool, set: fn(&mut Config)) {
    let t = tmp();
    assert_eq!(get(&load_config(&t.path)), default); // ausente
    for raw in [json!(true), json!(false)] {
        assert_eq!(Value::Bool(get(&load_json(&t, json!({key: raw})))), raw);
    }
    for raw in [json!(0), json!(1), json!("no"), json!("yes"), json!(null)] {
        assert_eq!(
            get(&load_json(&t, json!({key: raw}))),
            default,
            "{key}: {raw}"
        );
    }
    let mut cfg = load_config(&t.path);
    set(&mut cfg);
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(get(&load_config(&t.path)), !default);
}

#[test]
fn check_app_updates_tolerant() {
    check_bool_field(
        "check_app_updates",
        true,
        |c| c.check_app_updates,
        |c| c.check_app_updates = false,
    );
}

#[test]
fn steam_include_empty_tolerant() {
    check_bool_field(
        "steam_include_empty",
        false,
        |c| c.steam_include_empty,
        |c| c.steam_include_empty = true,
    );
}

#[test]
fn minimize_to_tray_tolerant() {
    check_bool_field(
        "minimize_to_tray",
        false,
        |c| c.minimize_to_tray,
        |c| c.minimize_to_tray = true,
    );
}

#[test]
fn deeply_nested_config_gives_defaults() {
    let t = tmp();
    fs::write(&t.path, "[".repeat(100_000)).unwrap();
    assert_eq!(load_config(&t.path), Config::default());
}

#[test]
fn amber_theme_migrates_to_terminal_amber() {
    let t = tmp();
    let cfg = load_json(&t, json!({"theme": "amber"}));
    assert_eq!(
        (cfg.theme.as_str(), cfg.terminal_color.as_str()),
        ("terminal", "amber")
    );
}

#[test]
fn terminal_color_loaded_saved_and_invalid_ignored() {
    let t = tmp();
    let cfg = load_json(&t, json!({"theme": "terminal", "terminal_color": "sepia"}));
    assert_eq!(
        (cfg.theme.as_str(), cfg.terminal_color.as_str()),
        ("terminal", "sepia")
    );
    save_config(&cfg, &t.path).unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(&t.path).unwrap()).unwrap();
    assert_eq!(saved["terminal_color"], "sepia");
    let cfg = load_json(&t, json!({"theme": "dark", "terminal_color": "magenta"}));
    assert_eq!(
        (cfg.theme.as_str(), cfg.terminal_color.as_str()),
        ("dark", "green")
    );
}

#[test]
fn bots_and_country_quick_filters_roundtrip() {
    let t = tmp();
    let mut cfg = load_config(&t.path);
    cfg.quick_filters = QuickFilters {
        hide_bots: true,
        country: s("cl"),
        ..qf()
    };
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).quick_filters, cfg.quick_filters);
    for bad in [
        json!("chile"),
        json!("c1"),
        json!(5),
        json!(""),
        json!("ñu"),
    ] {
        let raw = json!({"quick_filters": {"country": bad, "hide_bots": "si"}});
        assert_eq!(load_json(&t, raw).quick_filters, qf());
    }
    let raw = json!({"quick_filters": {"country": "CL"}});
    assert_eq!(load_json(&t, raw).quick_filters.country, "cl");
}

fn bad_path(t: &Tmp) -> PathBuf {
    t.path.with_file_name("config.json.bad")
}

#[test]
fn damaged_config_is_kept() {
    let t = tmp();
    let text = r#"{"favorites": {"cod2": []},}"#;
    let cfg = load_text(&t, text);
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(fs::read_to_string(bad_path(&t)).unwrap(), text);
}

#[test]
fn non_dict_config_is_kept() {
    let t = tmp();
    load_text(&t, "[1, 2]");
    assert_eq!(fs::read_to_string(bad_path(&t)).unwrap(), "[1, 2]");
}

#[test]
fn first_run_creates_no_backup() {
    let t = tmp();
    load_config(&t.path);
    assert!(!bad_path(&t).exists());
}

#[test]
fn bom_or_non_utf8_config_is_damaged() {
    for data in [
        &b"\xef\xbb\xbf{\"theme\": \"dark\"}"[..],
        &b"{\"theme\": \"dark\", \"wine_command\": \"\xff\"}"[..],
    ] {
        let t = tmp();
        fs::write(&t.path, data).unwrap();
        assert_eq!(load_config(&t.path), Config::default());
        assert_eq!(fs::read(bad_path(&t)).unwrap(), data);
    }
}

// ---------------------------------------------------------------- test_wine.py
#[test]
fn wine_command_config_roundtrip() {
    let t = tmp();
    assert_eq!(load_config(&t.path).wine_command, "wine");
    let cfg = Config {
        wine_command: s("umu-run"),
        ..Config::default()
    };
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).wine_command, "umu-run");
    assert_eq!(load_text(&t, r#"{"wine_command": 5}"#).wine_command, "wine");
    // strip() y a lo sumo 300 caracteres (no bytes).
    let long = format!("  {}  ", "ñ".repeat(400));
    let cfg = load_json(&t, json!({ "wine_command": long }));
    assert_eq!(cfg.wine_command, "ñ".repeat(300));
}

// ---------------------------------------------------------------- test_masters.py
fn official(games: Value) -> jase_core::masters::OfficialList {
    let doc = json!({"schema": 1, "updated": "2026-10-20", "games": games});
    parse_official(&serde_json::to_vec(&doc).unwrap()).unwrap()
}

fn entry(host: &str, port: u16) -> Value {
    json!({"host": host, "port": port})
}

#[test]
fn migration_preserves_effective_list_and_runs_once() {
    let o = official(json!({"cod2": [entry("a.com", 1), entry("b.com", 2)]}));
    let mut cfg = Config {
        masters: IndexMap::from([
            (s("cod2"), vec![(s("b.com"), 2), (s("mine.lan"), 9)]),
            (s("ut99"), vec![]),
        ]),
        ..Config::default()
    };
    let mut custom = CustomMasters::default();
    assert!(migrate_config_masters(&mut cfg, &mut custom, &o));
    assert!(cfg.masters.is_empty());
    assert_eq!(
        custom.added,
        IndexMap::from([(s("cod2"), vec![(s("mine.lan"), 9)])])
    );
    assert_eq!(
        custom.disabled,
        IndexMap::from([(s("cod2"), [s("a.com:1")].into())])
    );
    let mut eff = effective_masters("cod2", &o, &custom);
    eff.sort();
    assert_eq!(eff, [(s("b.com"), 2), (s("mine.lan"), 9)]);
    let ut99: Vec<_> = GAMES
        .iter()
        .find(|g| g.id == "ut99")
        .unwrap()
        .masters
        .iter()
        .map(|(h, p)| ((*h).to_owned(), *p))
        .collect();
    assert_eq!(effective_masters("ut99", &o, &custom), ut99);
    assert!(!migrate_config_masters(&mut cfg, &mut custom, &o));
}

#[test]
fn migration_drops_invalid_entries() {
    let o = official(json!({"cod2": [entry("a.com", 1)]}));
    let mut cfg = Config {
        masters: IndexMap::from([(
            s("cod2"),
            vec![
                (s("a.com"), 1),
                (s("bad host!"), 5),
                (s("ok.lan"), 70000),
                (s("ok.lan"), 0),
                (s("ok.lan"), 7),
            ],
        )]),
        ..Config::default()
    };
    let mut custom = CustomMasters::default();
    assert!(migrate_config_masters(&mut cfg, &mut custom, &o));
    assert_eq!(
        custom.added,
        IndexMap::from([(s("cod2"), vec![(s("ok.lan"), 7)])])
    );
}

// ---------------------------------------------------------------- test_friends.py
#[test]
fn normalize_strips_colors_case_and_space() {
    assert_eq!(friends::normalize("  ^1Jo^7TA "), "jota");
}

#[test]
fn parse_limits_and_dedupes() {
    assert_eq!(
        friends::parse(&json!(["A", "a", " ", 3, "^1A", "B"])),
        ["A", "B"]
    );
    let many: Vec<String> = (0..300).map(|i| format!("n{i}")).collect();
    assert_eq!(friends::parse(&json!(many)).len(), friends::MAX_FRIENDS);
    assert!(friends::parse(&json!("x")).is_empty());
}

#[test]
fn config_roundtrip_and_tolerant() {
    let t = tmp();
    let cfg = Config {
        friends: vec![s("a"), s("b")],
        ..Config::default()
    };
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).friends, ["a", "b"]);
    assert!(load_text(&t, r#"{"friends": 5}"#).friends.is_empty());
}

// ---------------------------------------------------------------- test_party.py
#[test]
fn parse_is_tolerant() {
    // La clave numérica `5` de Python no existe en JSON: la cubre `"q3": []`.
    let raw = json!({"cod2": {"1.2.3.4:28960": 4, "bad": 3, "5.6.7.8:1": 0, "9.9.9.9:2": true,
                              "8.8.8.8:3": 999, "7.7.7.7:4": 4.0},
                     "q3": [], "et": {"2.2.2.2:27960": 6}});
    let expected = IndexMap::from([
        (s("cod2"), IndexMap::from([(s("1.2.3.4:28960"), 4)])),
        (s("et"), IndexMap::from([(s("2.2.2.2:27960"), 6)])),
    ]);
    assert_eq!(party::parse(&raw), expected);
    assert!(party::parse(&json!("x")).is_empty());
    assert_eq!(party::MAX_THRESHOLD, 128);
    assert_eq!(party::DEFAULT_THRESHOLD, 4);
}

#[test]
fn party_config_roundtrip() {
    let t = tmp();
    let alerts = IndexMap::from([(s("cod2"), IndexMap::from([(s("1.2.3.4:28960"), 5)]))]);
    let cfg = Config {
        party_alerts: alerts.clone(),
        ..Config::default()
    };
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(load_config(&t.path).party_alerts, alerts);
}

// ---------------------------------------------------------------- compatibilidad con Python
#[test]
fn config_default_matches_python_bytes() {
    let t = tmp();
    save_config(&Config::default(), &t.path).unwrap();
    assert_eq!(
        String::from_utf8(fs::read(&t.path).unwrap()).unwrap(),
        String::from_utf8(golden("config_default.json")).unwrap()
    );
}

#[test]
fn config_sample_resave_matches_python_bytes() {
    let t = tmp();
    let data = golden("config_sample.json");
    fs::write(&t.path, &data).unwrap();
    let cfg = load_config(&t.path);
    assert_eq!(cfg.game_paths["cod2"], "E:\\Juegos\\Ñandú \"CoD2\"");
    assert_eq!(cfg.game_paths["q3"], "/opt/q3\ttab");
    assert_eq!(
        (cfg.theme.as_str(), cfg.terminal_color.as_str()),
        ("terminal", "amber")
    );
    assert_eq!(
        cfg.filter_presets[0],
        FilterPreset {
            name: s("Ñandú «rápido»"),
            quick: QuickFilters {
                hide_empty: true,
                max_ping: 80,
                ..qf()
            },
            text: s("dm"),
        }
    );
    assert_eq!(
        cfg.favorite_notes["cod2"]["1.2.3.4:28960"],
        "Clan «Ñandú»\tnoche"
    );
    assert_eq!(cfg.masters["cod2"][1], (s("mine.lan"), 9));
    assert!(cfg.extra.is_empty());
    save_config(&cfg, &t.path).unwrap();
    assert_eq!(
        String::from_utf8(fs::read(&t.path).unwrap()).unwrap(),
        String::from_utf8(data).unwrap()
    );
}

/// La muestra de Python con una clave que Python no conoce.
fn sample_with_future(t: &Tmp) -> Config {
    let mut data: Map<String, Value> =
        serde_json::from_slice(&golden("config_sample.json")).unwrap();
    data.insert(s("futuro"), json!({"a": [1]}));
    load_json(t, Value::Object(data))
}

#[test]
fn unknown_keys_survive_rust_save() {
    let t = tmp();
    let cfg = sample_with_future(&t);
    assert_eq!(cfg.extra.get("futuro"), Some(&json!({"a": [1]})));
    save_config(&cfg, &t.path).unwrap();
    let saved: Map<String, Value> = serde_json::from_slice(&fs::read(&t.path).unwrap()).unwrap();
    assert_eq!(saved.keys().next_back().map(String::as_str), Some("futuro"));
    assert_eq!(saved["futuro"], json!({"a": [1]}));
    assert_eq!(load_config(&t.path), cfg);
}

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        (-1e300f64..1e300).prop_map(Value::from),
        ".{0,8}".prop_map(Value::String),
        prop::sample::select(vec![
            "cod2",
            "terminal",
            "amber",
            "en",
            "1.2.3.4:28960",
            "a"
        ])
        .prop_map(|s| Value::String(s.to_owned())),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((".{0,6}", inner), 0..6)
                .prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ]
    })
}

const KEYS: [&str; 25] = [
    "game_paths",
    "theme",
    "terminal_color",
    "language",
    "version_filter",
    "masters",
    "quick_filters",
    "filter_presets",
    "download_map_images",
    "find_games_offered",
    "only_installed_games",
    "ui_state",
    "favorites",
    "favorite_notes",
    "favorite_groups",
    "recent",
    "auto_refresh_minutes",
    "steam_include_empty",
    "wine_command",
    "minimize_to_tray",
    "friends",
    "watch_friends",
    "party_alerts",
    "check_app_updates",
    "futuro",
];

proptest! {
    #[test]
    fn config_from_json_never_panics(values in prop::collection::vec(arb_json(), KEYS.len())) {
        let data: Map<String, Value> = KEYS.iter().map(|k| s(k)).zip(values).collect();
        let cfg = config_from_json(&data);
        // Lo que sale se vuelve a leer igual.
        let again = config_from_json(config_to_json(&cfg).as_object().unwrap());
        prop_assert_eq!(again, cfg);
    }
}
