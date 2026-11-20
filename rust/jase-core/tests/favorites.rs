//! Port de tests/test_favorites.py (lógica pura), test_groups.py, test_recent.py y
//! test_favorites_io.py, más la compatibilidad de bytes con Python 1.3.0.
mod support;

use jase_core::config::{Config, load_config, save_config};
use jase_core::favorites::*;
use jase_core::favorites_io::*;
use jase_core::models::ServerAddress;
use jase_core::recent::*;
use serde_json::{Value, json};
use std::fs;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/data");

fn addr(host: &str, port: u16) -> ServerAddress {
    ServerAddress::new(host, port)
}

fn cfg_with(favs: &[(&str, &[&str])]) -> Config {
    let mut cfg = Config::default();
    for (g, list) in favs {
        cfg.favorites
            .insert((*g).into(), list.iter().map(|s| (*s).into()).collect());
    }
    cfg
}

fn sample_cfg() -> Config {
    let mut cfg = cfg_with(&[
        ("cod2", &["1.2.3.4:28960", "5.6.7.8:28961"]),
        ("ut99", &["9.9.9.9:7777"]),
    ]);
    set_note(&mut cfg, "cod2", "1.2.3.4:28960", "mi nota");
    cfg
}

fn golden_cfg() -> Config {
    let bytes = fs::read(format!("{GOLDEN}/config_sample.json")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, bytes).unwrap();
    load_config(&path)
}

#[test]
fn parse_address_cases() {
    assert_eq!(parse_address("1.2.3.4:28960"), Some(addr("1.2.3.4", 28960)));
    for bad in [
        "",
        "1.2.3.4",
        "1.2.3.4:",
        ":28960",
        "1.2.3.4:abc",
        "1.2.3.4:0",
        "1.2.3.4:70000",
        "1.2.3.4:١٢",
    ] {
        assert_eq!(parse_address(bad), None, "{bad}");
    }
    assert_eq!(
        parse_address("1.2.3.4:0028960"),
        Some(addr("1.2.3.4", 28960))
    );
    assert_eq!(parse_address("1.2.3.4:65535"), Some(addr("1.2.3.4", 65535)));
}

#[test]
fn parse_address_rejects_junk_hosts() {
    assert_eq!(
        parse_address("cod.example.com:28960"),
        Some(addr("cod.example.com", 28960))
    );
    let long = format!("{}:1", "x".repeat(254));
    for bad in [
        "a b:28960",
        "<x>:28960",
        "evil/../x:1",
        "-host:1",
        ".host:1",
        "hóst:1",
        &long,
    ] {
        assert_eq!(parse_address(bad), None, "{bad}");
    }
}

#[test]
fn add_remove_and_query() {
    let mut cfg = Config::default();
    let a = addr("1.2.3.4", 28960);
    assert!(!is_favorite(&cfg, "cod2", &a));
    assert!(add_favorite(&mut cfg, "cod2", &a));
    assert!(!add_favorite(&mut cfg, "cod2", &a));
    assert_eq!(cfg.favorites["cod2"], ["1.2.3.4:28960"]);
    assert!(is_favorite(&cfg, "cod2", &a));
    assert!(!is_favorite(&cfg, "ut99", &a));
    assert_eq!(favorite_addresses(&cfg, "cod2"), std::slice::from_ref(&a));
    assert!(remove_favorite(&mut cfg, "cod2", &a));
    assert!(!remove_favorite(&mut cfg, "cod2", &a));
    assert!(cfg.favorites.is_empty());
}

#[test]
fn favorite_addresses_skips_invalid_entries() {
    let cfg = cfg_with(&[("cod2", &["bogus", "5.5.5.5:1", "5.5.5.5:1"])]);
    assert_eq!(favorite_addresses(&cfg, "cod2"), [addr("5.5.5.5", 1)]);
    assert!(favorite_addresses(&cfg, "ut99").is_empty());
}

#[test]
fn offline_server_row() {
    let a = addr("9.9.9.9", 7778);
    let info = offline_server(&a);
    assert!(!info.online);
    assert_eq!((&info.address, &info.connect_address), (&a, &a));
    assert_eq!(info.name, "9.9.9.9:7778");
    assert_eq!((&*info.map, &*info.gametype, &*info.version), ("", "", ""));
    assert_eq!(
        (info.num_players, info.max_players, info.ping_ms),
        (0, 0, 0)
    );
    assert!(!info.password && info.players.is_empty() && info.rules.is_empty());
}

#[test]
fn online_does_not_affect_equality() {
    // Rust: la igualdad es de `ServerAddress`; `online` vive en `ServerInfo`.
    let a = addr("1.1.1.1", 1);
    let mut info = offline_server(&a);
    info.online = true;
    assert_eq!(info.address, a);
}

fn tmp_config() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.json");
    (dir, path)
}

#[test]
fn favorites_roundtrip() {
    let (_d, path) = tmp_config();
    let cfg = cfg_with(&[("cod2", &["1.2.3.4:28960"]), ("ut99", &["9.9.9.9:7777"])]);
    save_config(&cfg, &path).unwrap();
    assert_eq!(load_config(&path).favorites, cfg.favorites);
}

#[test]
fn v1_config_without_favorites_loads_empty() {
    let (_d, path) = tmp_config();
    fs::write(&path, r#"{"theme": "dark"}"#).unwrap();
    let cfg = load_config(&path);
    assert!(cfg.favorites.is_empty() && cfg.theme == "dark");
}

#[test]
fn invalid_favorites_are_dropped_per_entry() {
    let (_d, path) = tmp_config();
    let raw = json!({"theme": "light", "favorites": {
        "cod2": ["1.2.3.4:28960", 5, null, "1.2.3.4:28960"], "ut99": "no-es-lista",
        "mohaa": [], "cod1": ["7.7.7.7:28960"]}});
    fs::write(&path, raw.to_string()).unwrap();
    let cfg = load_config(&path);
    assert_eq!(cfg.favorites.len(), 2);
    assert_eq!(cfg.favorites["cod2"], ["1.2.3.4:28960"]);
    assert_eq!(cfg.favorites["cod1"], ["7.7.7.7:28960"]);
    assert_eq!(cfg.theme, "light");
    fs::write(&path, r#"{"theme": "light", "favorites": ["x"]}"#).unwrap();
    assert!(load_config(&path).favorites.is_empty());
}

#[test]
fn notes_load_tolerant() {
    let (_d, path) = tmp_config();
    let raw = json!({
        "favorites": {"cod2": ["1.1.1.1:28960"]},
        "favorite_notes": {"cod2": {"1.1.1.1:28960": "hola", "9.9.9.9:1": "no favorito",
                                    "2.2.2.2:1": 5, "3.3.3.3:1": ""},
                           "ut": "mal"}});
    fs::write(&path, raw.to_string()).unwrap();
    let cfg = load_config(&path);
    assert_eq!(cfg.favorite_notes.len(), 1);
    let notes = &cfg.favorite_notes["cod2"];
    assert_eq!(notes.len(), 2);
    assert_eq!(notes["1.1.1.1:28960"], "hola");
    assert_eq!(notes["9.9.9.9:1"], "no favorito");
    fs::write(&path, r#"{"favorite_notes": [1]}"#).unwrap();
    assert!(load_config(&path).favorite_notes.is_empty());
}

#[test]
fn notes_roundtrip() {
    let (_d, path) = tmp_config();
    let mut cfg = Config::default();
    set_note(&mut cfg, "cod2", "1.1.1.1:28960", "hola");
    save_config(&cfg, &path).unwrap();
    assert_eq!(load_config(&path).favorite_notes, cfg.favorite_notes);
}

#[test]
fn set_note_clears_trims_and_cleans_game() {
    let mut cfg = Config::default();
    let a = "1.1.1.1:1";
    set_note(&mut cfg, "g", a, "linea 1\nlinea 2");
    assert_eq!(get_note(&cfg, "g", a), "linea 1 linea 2");
    set_note(&mut cfg, "g", a, &"x".repeat(300));
    assert_eq!(get_note(&cfg, "g", a).len(), NOTE_MAX);
    assert_eq!(NOTE_MAX, 200);
    set_note(&mut cfg, "g", a, "  ");
    assert!(cfg.favorite_notes.is_empty());
    assert_eq!(get_note(&cfg, "g", a), "");
}

#[test]
fn note_cut_counts_chars() {
    let mut cfg = Config::default();
    set_note(&mut cfg, "g", "1.1.1.1:1", &"ñ".repeat(300));
    assert_eq!(get_note(&cfg, "g", "1.1.1.1:1").chars().count(), 200);
    // El corte cae sobre un espacio: el `strip` final lo quita, como en Python.
    set_note(
        &mut cfg,
        "g",
        "1.1.1.1:1",
        &format!("{} z", "a".repeat(199)),
    );
    assert_eq!(get_note(&cfg, "g", "1.1.1.1:1"), "a".repeat(199));
}

#[test]
fn game_notes_keeps_valid_addresses() {
    let mut cfg = Config::default();
    set_note(&mut cfg, "g", "1.1.1.1:1", "ok");
    set_note(&mut cfg, "g", "basura", "no");
    let notes = game_notes(&cfg, "g");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[&addr("1.1.1.1", 1)], "ok");
}

#[test]
fn remove_favorite_removes_note_and_group() {
    let mut cfg = Config::default();
    let a = addr("1.1.1.1", 1);
    add_favorite(&mut cfg, "g", &a);
    set_note(&mut cfg, "g", "1.1.1.1:1", "nota");
    set_group(&mut cfg, "g", "1.1.1.1:1", "grupo");
    remove_favorite(&mut cfg, "g", &a);
    assert!(cfg.favorite_notes.is_empty() && cfg.favorite_groups.is_empty());
}

#[test]
fn set_group_cleans_and_group_names_only_count_current_favorites() {
    let (a, b) = ("1.1.1.1:28960", "2.2.2.2:28960");
    let mut cfg = cfg_with(&[("cod2", &[a, b])]);
    set_group(&mut cfg, "cod2", a, "  Clan   Nox ");
    set_group(&mut cfg, "cod2", b, "lan");
    set_group(&mut cfg, "cod2", "9.9.9.9:1", "Viejo");
    assert_eq!(get_group(&cfg, "cod2", a), "Clan Nox");
    assert_eq!(group_names(&cfg, "cod2"), ["Clan Nox", "lan"]);
    set_group(&mut cfg, "cod2", b, "   ");
    assert_eq!(get_group(&cfg, "cod2", b), "");
    remove_favorite(&mut cfg, "cod2", &addr("1.1.1.1", 28960));
    assert_eq!(get_group(&cfg, "cod2", a), "");
    assert_eq!(clean_group(&"x".repeat(100)).len(), GROUP_MAX);
    assert_eq!(GROUP_MAX, 40);
}

#[test]
fn group_names_sort_by_casefold_then_name() {
    let mut cfg = cfg_with(&[("g", &["1.1.1.1:1", "2.2.2.2:2", "3.3.3.3:3"])]);
    set_group(&mut cfg, "g", "1.1.1.1:1", "b");
    set_group(&mut cfg, "g", "2.2.2.2:2", "B");
    set_group(&mut cfg, "g", "3.3.3.3:3", "a");
    assert_eq!(group_names(&cfg, "g"), ["a", "B", "b"]);
}

#[test]
fn groups_roundtrip_config_and_export() {
    let (_d, path) = tmp_config();
    let mut cfg = cfg_with(&[("cod2", &["1.1.1.1:28960"])]);
    set_group(&mut cfg, "cod2", "1.1.1.1:28960", "Clan");
    save_config(&cfg, &path).unwrap();
    assert_eq!(load_config(&path).favorite_groups, cfg.favorite_groups);
    let data = export_favorites(&cfg);
    assert_eq!(
        data["favorites"]["cod2"],
        json!([{"address": "1.1.1.1:28960", "group": "Clan"}])
    );
    let mut fresh = Config::default();
    import_favorites(&mut fresh, &data).unwrap();
    assert_eq!(get_group(&fresh, "cod2", "1.1.1.1:28960"), "Clan");
}

// ------------------------------------------------------------------- recientes
#[test]
fn record_puts_first_without_duplicates() {
    let (a, b) = (addr("1.1.1.1", 28960), addr("2.2.2.2", 28960));
    let mut cfg = Config::default();
    for x in [&a, &b, &a] {
        record_recent(&mut cfg, "cod2", x);
    }
    assert_eq!(recent_addresses(&cfg, "cod2"), [a.clone(), b]);
    assert_eq!(cfg.recent["cod2"], ["1.1.1.1:28960", "2.2.2.2:28960"]);
}

#[test]
fn record_trims_to_max() {
    let mut cfg = Config::default();
    let mut last = addr("10.0.0.1", 0);
    for i in 0..25 {
        last = addr("10.0.0.1", 1000 + i);
        record_recent(&mut cfg, "cod2", &last);
    }
    let result = recent_addresses(&cfg, "cod2");
    assert_eq!((result.len(), RECENT_MAX), (20, 20));
    assert_eq!(result[0], last);
}

#[test]
fn remove_last_entry_drops_game_key() {
    let a = addr("1.1.1.1", 28960);
    let mut cfg = Config::default();
    record_recent(&mut cfg, "cod2", &a);
    assert!(remove_recent(&mut cfg, "cod2", &a));
    assert!(!cfg.recent.contains_key("cod2"));
    assert!(!remove_recent(&mut cfg, "cod2", &a));
}

#[test]
fn garbage_entries_ignored() {
    let mut cfg = Config::default();
    cfg.recent
        .insert("cod2".into(), vec!["basura".into(), "1.1.1.1:28960".into()]);
    assert_eq!(recent_addresses(&cfg, "cod2"), [addr("1.1.1.1", 28960)]);
}

// ----------------------------------------------------------- exportar/importar
#[test]
fn export_shape() {
    let data = export_favorites(&sample_cfg());
    assert_eq!(data["format"], FORMAT);
    assert_eq!(FORMAT, "jase-favorites");
    assert_eq!(data["version"], 1);
    assert_eq!(
        data["favorites"]["cod2"],
        json!([{"address": "1.2.3.4:28960", "note": "mi nota"}, {"address": "5.6.7.8:28961"}])
    );
}

#[test]
fn roundtrip() {
    let src = sample_cfg();
    let mut dst = Config::default();
    let result = import_favorites(&mut dst, &export_favorites(&src)).unwrap();
    assert_eq!(
        result,
        ImportResult {
            added: 3,
            skipped: 0
        }
    );
    assert_eq!(dst.favorites, src.favorites);
    assert_eq!(dst.favorite_notes, src.favorite_notes);
}

#[test]
fn import_merges_without_overwriting() {
    let mut cfg = cfg_with(&[("cod2", &["1.2.3.4:28960"])]);
    set_note(&mut cfg, "cod2", "1.2.3.4:28960", "mía");
    cfg.favorites
        .get_mut("cod2")
        .unwrap()
        .push("5.6.7.8:28961".into());
    let data = json!({"format": FORMAT, "version": 1, "favorites": {"cod2": [
        {"address": "1.2.3.4:28960", "note": "ajena"},
        {"address": "5.6.7.8:28961", "note": "nueva"},
        {"address": "7.7.7.7:1"}]}});
    let result = import_favorites(&mut cfg, &data).unwrap();
    assert_eq!(
        result,
        ImportResult {
            added: 1,
            skipped: 0
        }
    );
    assert_eq!(
        cfg.favorites["cod2"],
        ["1.2.3.4:28960", "5.6.7.8:28961", "7.7.7.7:1"]
    );
    assert_eq!(get_note(&cfg, "cod2", "1.2.3.4:28960"), "mía");
    assert_eq!(get_note(&cfg, "cod2", "5.6.7.8:28961"), "nueva");
}

#[test]
fn import_skips_invalid() {
    let mut cfg = Config::default();
    let data = json!({"format": FORMAT, "version": 1, "favorites": {
        "nogame": [{"address": "1.1.1.1:1"}],
        "cod2": [{"address": "bad"}, "1.2.3.4:5", 7, {"address": "2.2.2.2:2"}],
        "ut99": "oops"}});
    let result = import_favorites(&mut cfg, &data).unwrap();
    assert_eq!(
        result,
        ImportResult {
            added: 1,
            skipped: 5
        }
    );
    assert_eq!(cfg.favorites.len(), 1);
    assert_eq!(cfg.favorites["cod2"], ["2.2.2.2:2"]);
}

#[test]
fn import_wrong_format() {
    for data in [
        json!([]),
        json!({"format": "x", "version": 1, "favorites": {}}),
        json!({"format": FORMAT, "version": 2, "favorites": {}}),
        json!({"format": FORMAT, "version": 1, "favorites": []}),
    ] {
        let mut cfg = Config::default();
        assert!(matches!(
            import_favorites(&mut cfg, &data),
            Err(ImportError::Invalid)
        ));
        assert!(cfg.favorites.is_empty());
    }
}

#[test]
fn import_version_equal_to_one() {
    for v in [json!(1.0), json!(true), json!(1)] {
        let data = json!({"format": FORMAT, "version": v, "favorites": {}});
        assert!(
            import_favorites(&mut Config::default(), &data).is_ok(),
            "{v}"
        );
    }
    let data = json!({"format": FORMAT, "version": "1", "favorites": {}});
    assert!(import_favorites(&mut Config::default(), &data).is_err());
}

#[test]
fn read_import_file_too_big() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.json");
    fs::write(&big, vec![b' '; MAX_IMPORT_BYTES as usize + 1]).unwrap();
    assert!(matches!(read_import_file(&big), Err(ImportError::TooLarge)));
    let ok = dir.path().join("ok.json");
    fs::write(&ok, export_favorites(&sample_cfg()).to_string()).unwrap();
    assert_eq!(read_import_file(&ok).unwrap()["format"], FORMAT);
    assert!(matches!(
        read_import_file(&dir.path().join("falta.json")),
        Err(ImportError::Io(_))
    ));
}

#[test]
fn deeply_nested_json_is_value_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep.json");
    fs::write(&path, vec![b'['; 200_000]).unwrap();
    assert!(matches!(read_import_file(&path), Err(ImportError::Invalid)));
}

#[test]
fn export_matches_python_bytes() {
    let export = export_favorites(&golden_cfg());
    let expected = fs::read(format!("{GOLDEN}/favorites_export.json")).unwrap();
    assert_eq!(serde_json::to_vec_pretty(&export).unwrap(), expected);
}

#[test]
fn import_accepts_python_export_with_bom() {
    let mut bytes = b"\xef\xbb\xbf".to_vec();
    bytes.extend(fs::read(format!("{GOLDEN}/favorites_export.json")).unwrap());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fav.json");
    fs::write(&path, bytes).unwrap();
    let data: Value = read_import_file(&path).unwrap();
    let mut cfg = Config::default();
    let result = import_favorites(&mut cfg, &data).unwrap();
    assert_eq!(
        result,
        ImportResult {
            added: 3,
            skipped: 0
        }
    );
    assert_eq!(
        get_note(&cfg, "cod2", "1.2.3.4:28960"),
        "Clan «Ñandú» noche"
    );
}
