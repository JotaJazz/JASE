//! Port de tests/test_masters.py (los de la config y la ventana quedan para fases 4/5).
use indexmap::IndexMap;
use jase_core::fsutil::atomic_write;
use jase_core::games;
use jase_core::masters::{
    self, CustomMasters, Master, bundled_official, effective_masters, load_custom, load_official,
    master_key, official_for, parse_official, save_custom,
};
use proptest::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn doc(games: Value, schema: Value, updated: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"schema": schema, "updated": updated, "games": games})).unwrap()
}
fn d(games: Value) -> Vec<u8> {
    doc(games, json!(1), json!("2026-10-20"))
}
fn entry(host: &str, port: u16) -> Value {
    json!({"host": host, "port": port})
}
fn m(host: &str, port: u16) -> Master {
    (host.to_owned(), port)
}
fn game_masters(id: &str) -> Vec<Master> {
    games::game(id)
        .unwrap()
        .masters
        .iter()
        .map(|(h, p)| m(h, *p))
        .collect()
}

#[test]
fn repo_masters_json_matches_bundled_and_gamedefs() {
    let root = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../masters.json")).unwrap();
    let official = parse_official(&root).unwrap();
    assert_eq!(
        (official.schema, official.updated.as_str()),
        (1, "2026-11-05")
    );
    assert_eq!(official.games.len(), games::GAMES.len());
    for g in games::GAMES {
        assert_eq!(official.games[g.id], game_masters(g.id), "{}", g.id);
    }
    assert_eq!(bundled_official().unwrap(), official);
}

#[test]
fn parse_official_ok() {
    let o = parse_official(&d(
        json!({"cod2": [entry("m.example.com", 1234), entry("M.Example.com", 1234)]}),
    ))
    .unwrap();
    assert_eq!(
        o.games,
        IndexMap::from([("cod2".to_owned(), vec![m("m.example.com", 1234)])])
    );
    assert_eq!(o.updated, "2026-10-20");
}

#[test]
fn parse_official_rejects_bad_json() {
    let bad: [Vec<u8>; 4] = [
        b"{nope".to_vec(),
        b"[]".to_vec(),
        b"\xff\xfe".to_vec(),
        doc(json!({}), json!(1), json!(3)),
    ];
    for b in bad {
        assert!(parse_official(&b).is_err());
    }
}

#[test]
fn parse_official_rejects_unknown_schema() {
    for schema in [
        json!(2),
        json!(0),
        json!("1"),
        json!(true),
        Value::Null,
        json!(1.5),
    ] {
        assert!(
            parse_official(&doc(
                json!({"cod2": [entry("m.example.com", 1)]}),
                schema.clone(),
                json!("2026-10-20")
            ))
            .is_err(),
            "{schema}"
        );
    }
}

#[test]
fn parse_official_rejects_bad_host() {
    let long = "x".repeat(254);
    for host in [
        json!("http://x"),
        json!("a b"),
        json!(""),
        json!("a/b"),
        json!("u@h"),
        json!(long),
        json!(5),
        Value::Null,
    ] {
        assert!(parse_official(&d(json!({"cod2": [{"host": host, "port": 1234}]}))).is_err());
    }
}

#[test]
fn parse_official_rejects_bad_port() {
    for port in [
        json!(0),
        json!(70000),
        json!("x"),
        json!(true),
        json!(1.5),
        Value::Null,
        json!(-1),
        json!(1.0),
    ] {
        assert!(
            parse_official(&d(
                json!({"cod2": [{"host": "m.example.com", "port": port}]})
            ))
            .is_err(),
            "{port}"
        );
    }
}

#[test]
fn parse_official_rejects_all_games_empty() {
    let all: serde_json::Map<String, Value> = games::GAMES
        .iter()
        .map(|g| (g.id.to_owned(), json!([])))
        .collect();
    assert!(parse_official(&d(Value::Object(all))).is_err());
}

#[test]
fn parse_official_rejects_too_big() {
    let pad = "x".repeat(masters::MAX_OFFICIAL_BYTES + 1);
    assert!(
        parse_official(&doc(
            json!({"cod2": [entry("m.example.com", 1)]}),
            json!(1),
            json!(pad)
        ))
        .is_err()
    );
}

#[test]
fn parse_official_rejects_bad_date_or_games() {
    assert!(
        parse_official(&doc(
            json!({"cod2": [entry("a.b", 1)]}),
            json!(1),
            json!("2026-1-01")
        ))
        .is_err()
    );
    assert!(parse_official(&doc(json!([]), json!(1), json!("2026-10-20"))).is_err());
    assert!(parse_official(&d(json!({"cod2": {"host": "a"}}))).is_err());
    assert!(parse_official(&d(json!({"cod2": [3]}))).is_err());
}

#[test]
fn parse_official_ignores_unknown_game() {
    let o = parse_official(&d(
        json!({"cod2": [entry("m.example.com", 1)], "zzz": [{"host": "http://bad", "port": 0}]}),
    ))
    .unwrap();
    assert!(!o.games.contains_key("zzz"));
}

#[test]
fn official_for_absent_vs_empty() {
    let o = parse_official(&d(
        json!({"cod2": [], "ut99": [entry("m.example.com", 1234)]}),
    ))
    .unwrap();
    assert!(official_for("cod2", &o).is_empty());
    assert_eq!(official_for("ut99", &o), [m("m.example.com", 1234)]);
    assert_eq!(official_for("cod1", &o), game_masters("cod1"));
    assert!(official_for("nope", &o).is_empty());
}

#[test]
fn load_official_fallback_order() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let bundled = bundled_official().unwrap();
    assert_eq!(load_official(&cache), bundled);
    std::fs::create_dir(&cache).unwrap();
    std::fs::write(cache.join("masters.json"), b"{garbage").unwrap();
    assert_eq!(load_official(&cache), bundled);
    std::fs::write(
        cache.join("masters.json"),
        doc(
            json!({"cod2": [entry("m.example.com", 1)]}),
            json!(1),
            json!("2030-01-01"),
        ),
    )
    .unwrap();
    assert_eq!(load_official(&cache).updated, "2030-01-01");
}

#[test]
fn load_official_prefers_newer_bundled_over_stale_cache() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("masters.json"),
        doc(
            json!({"cod2": [entry("m.example.com", 1)]}),
            json!(1),
            json!("2000-01-01"),
        ),
    )
    .unwrap();
    assert_eq!(load_official(tmp.path()), bundled_official().unwrap());
}

#[test]
fn load_official_deeply_nested_cache_falls_back() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("masters.json"), b"[".repeat(100_000)).unwrap();
    assert_eq!(load_official(tmp.path()), bundled_official().unwrap());
    assert!(parse_official(&b"[".repeat(100_000)).is_err());
}

#[test]
fn custom_tolerant_load() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("c.json");
    assert_eq!(load_custom(&p), CustomMasters::default());
    for bad in ["{bad", "[1]"] {
        std::fs::write(&p, bad).unwrap();
        assert_eq!(load_custom(&p), CustomMasters::default());
    }
    let data = json!({
        "added": {"cod2": [entry("m.example.com", 1234), {"host": "x y", "port": 1}, {"host": "ok", "port": 0}, 3, entry("M.EXAMPLE.com", 1234)], "ut99": "x", "5": []},
        "disabled": {"cod2": ["a:1", 7, ""], "ut99": "x"}});
    std::fs::write(&p, data.to_string()).unwrap();
    let c = load_custom(&p);
    assert_eq!(
        c.added,
        IndexMap::from([("cod2".to_owned(), vec![m("m.example.com", 1234)])])
    );
    assert_eq!(
        c.disabled,
        IndexMap::from([("cod2".to_owned(), BTreeSet::from(["a:1".to_owned()]))])
    );
    std::fs::write(&p, json!({"added": 3, "disabled": []}).to_string()).unwrap();
    assert_eq!(load_custom(&p), CustomMasters::default());
}

#[test]
fn load_custom_nested_or_oversize_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("c.json");
    std::fs::write(&p, b"[".repeat(100_000)).unwrap();
    assert_eq!(load_custom(&p), CustomMasters::default());
    let pad = "x".repeat(masters::MAX_OFFICIAL_BYTES + 1);
    std::fs::write(
        &p,
        json!({"added": {"cod2": [entry("m.example.com", 1)]}, "pad": pad}).to_string(),
    )
    .unwrap();
    assert_eq!(load_custom(&p), CustomMasters::default());
}

fn sample_custom() -> CustomMasters {
    CustomMasters {
        added: IndexMap::from([("cod2".to_owned(), vec![m("h.example", 1)])]),
        disabled: IndexMap::from([(
            "cod2".to_owned(),
            BTreeSet::from(["b:2".to_owned(), "a:1".to_owned()]),
        )]),
    }
}

#[test]
fn custom_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("sub").join("c.json");
    let c = sample_custom();
    save_custom(&c, &p).unwrap();
    assert_eq!(load_custom(&p), c);
    let raw: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    assert_eq!(raw["schema"], 1);
    assert_eq!(raw["disabled"]["cod2"], json!(["a:1", "b:2"]));
    let names: Vec<_> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["c.json"]);
}

#[test]
fn effective_union_disabled_dedupe() {
    let o = parse_official(&d(json!({"cod2": [entry("a.com", 1), entry("b.com", 2)]}))).unwrap();
    let c = CustomMasters {
        added: IndexMap::from([("cod2".to_owned(), vec![m("B.com", 2), m("c.com", 3)])]),
        disabled: IndexMap::from([(
            "cod2".to_owned(),
            BTreeSet::from([master_key(&m("a.com", 1))]),
        )]),
    };
    assert_eq!(
        effective_masters("cod2", &o, &c),
        [m("b.com", 2), m("c.com", 3)]
    );
    assert_eq!(
        effective_masters("cod1", &o, &CustomMasters::default()),
        game_masters("cod1")
    );
}

#[test]
fn valid_host() {
    let a63 = "a".repeat(63);
    let cases = [
        ("example.com", true),
        ("example.com.", true),
        ("1.2.3.4", true),
        ("a..b", false),
        (".a", false),
        (a63.as_str(), true),
        (&"a".repeat(64), false),
        (&[a63.as_str(); 4].join("."), false),
        ("é.com", false),
        ("a.", true),
        (".", false),
    ];
    for (host, ok) in cases {
        assert_eq!(masters::valid_host(host), ok, "{host}");
    }
}

#[test]
fn atomic_write_keeps_old_file_on_error() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("dir");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep"), b"x").unwrap();
    assert!(atomic_write(&target, b"new").is_err()); // destino es una carpeta
    assert!(target.join("keep").exists());
    let names: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["dir"]); // sin temporales sueltos
}

#[cfg(unix)]
#[test]
fn atomic_write_read_only_dir_keeps_old_file() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("c.json");
    std::fs::write(&p, b"old").unwrap();
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let r = atomic_write(&p, b"new");
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(r.is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"old");
}

proptest! {
    #[test]
    fn parse_official_never_panics(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = parse_official(&data);
    }

    #[test]
    fn parse_official_odd_types_never_panic(host in prop_oneof![Just(json!(null)), Just(json!(1)), Just(json!([])), "[a-z.]{0,20}".prop_map(Value::from)],
                                            port in prop_oneof![Just(json!(null)), Just(json!("1")), any::<i64>().prop_map(Value::from), any::<f64>().prop_map(Value::from)]) {
        let _ = parse_official(&d(json!({"cod2": [{"host": host, "port": port}]})));
    }
}

#[test]
fn schema_one_point_zero_is_accepted() {
    // En Python 1.0 == 1: el schema 1.0 pasa.
    assert!(
        parse_official(&doc(
            json!({"cod2": [entry("m.example.com", 1)]}),
            json!(1.0),
            json!("2026-10-20")
        ))
        .is_ok()
    );
}
