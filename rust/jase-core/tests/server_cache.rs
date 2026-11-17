//! Port de tests/test_server_cache.py, más la compatibilidad de bytes con Python 1.3.0.
use indexmap::IndexMap;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::server_cache::*;
use proptest::prelude::*;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/data");

fn info(host: &str, port: u16) -> ServerInfo {
    let mut s = ServerInfo::new(
        ServerAddress::new(host, port),
        ServerAddress::new(host, port - 1),
    );
    s.name = "Srv".into();
    s.map = "mp_toujane".into();
    s.gametype = "tdm".into();
    s.num_players = 3;
    s.max_players = 20;
    s.password = true;
    s.version = "1.3".into();
    s.ping_ms = 42;
    let mut p = Player::new("Jimmy", 10, 50);
    p.raw_name = "^1Jimmy".into();
    s.players = vec![p];
    s.rules = IndexMap::from([("g_gametype".to_owned(), "tdm".to_owned())]);
    s.raw_name = "^2Srv".into();
    s.bots = 1;
    s.connect_port_deduced = true;
    s
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn roundtrip_keeps_every_field() {
    let dir = tempfile::tempdir().unwrap();
    let mut second = info("5.6.7.8", 27015);
    second.players.clear();
    second.rules.clear();
    let servers = vec![info("1.2.3.4", 28960), second];
    save(dir.path(), "cod2", &servers, 1000.0).unwrap();
    let (saved_at, loaded) = load(dir.path(), "cod2").unwrap();
    assert_eq!(saved_at, 1000.0);
    assert_eq!(loaded, servers);
    assert_eq!(loaded[0].raw_name, "^2Srv");
    assert_eq!(loaded[0].bots, 1);
    assert!(loaded[0].connect_port_deduced && loaded[0].online);
    assert_eq!(loaded[0].players[0].raw_name, "^1Jimmy");
}

#[test]
fn missing_corrupt_or_foreign_files_give_none() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load(dir.path(), "cod2").is_none());
    let path = dir.path().join("cod2.json");
    fs::write(&path, b"{roto").unwrap();
    assert!(load(dir.path(), "cod2").is_none());
    fs::write(&path, r#"{"schema": 99, "saved_at": 1, "servers": []}"#).unwrap();
    assert!(load(dir.path(), "cod2").is_none());
    fs::write(&path, "[".repeat(100_000)).unwrap();
    assert!(load(dir.path(), "cod2").is_none());
}

#[test]
fn invalid_entries_are_dropped_one_by_one() {
    let dir = tempfile::tempdir().unwrap();
    let servers = [info("1.2.3.4", 28960), info("5.6.7.8", 28960)];
    save(dir.path(), "cod2", &servers, 5.0).unwrap();
    let path = dir.path().join("cod2.json");
    let mut data = read(&path);
    let list = data["servers"].as_array_mut().unwrap();
    list[0]["address"] = json!("no es una dirección");
    list.push(json!({"basura": true}));
    list.push(Value::Null);
    fs::write(&path, data.to_string()).unwrap();
    let (_, loaded) = load(dir.path(), "cod2").unwrap();
    let hosts: Vec<_> = loaded.iter().map(|s| s.address.host.as_str()).collect();
    assert_eq!(hosts, ["5.6.7.8"]);
}

#[test]
fn oversized_file_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), "cod2", &[info("1.2.3.4", 28960)], 5.0).unwrap();
    assert!(load_with_limit(dir.path(), "cod2", 10).is_none());
    assert!(load_with_limit(dir.path(), "cod2", MAX_BYTES).is_some());
}

#[test]
fn index_counts_and_forget() {
    let dir = tempfile::tempdir().unwrap();
    let two = [info("1.2.3.4", 28960), info("5.6.7.8", 28960)];
    save(dir.path(), "cod2", &two, 5.0).unwrap();
    save(dir.path(), "ut99", &[info("1.2.3.4", 28960)], 7.0).unwrap();
    let expected = IndexMap::from([("cod2".to_owned(), (2, 5.0)), ("ut99".to_owned(), (1, 7.0))]);
    assert_eq!(read_index(dir.path()), expected);
    forget(dir.path(), "cod2");
    let expected = IndexMap::from([("ut99".to_owned(), (1, 7.0))]);
    assert_eq!(read_index(dir.path()), expected);
    assert!(load(dir.path(), "cod2").is_none());
}

#[test]
fn unknown_game_id_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("cache");
    fs::create_dir(&dir).unwrap();
    save(&dir, "../fuera", &[info("1.2.3.4", 28960)], 1.0).unwrap();
    let names: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["cache"]);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    assert!(load(&dir, "../fuera").is_none());
}

#[test]
fn corrupt_index_reads_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(INDEX), "{roto").unwrap();
    assert!(read_index(dir.path()).is_empty());
}

#[test]
fn non_finite_or_huge_date_is_not_a_list() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), "cod2", &[info("1.2.3.4", 28960)], 5.0).unwrap();
    let path = dir.path().join("cod2.json");
    let good = fs::read_to_string(&path).unwrap();
    // json.dumps de Python escribe NaN/Infinity tal cual.
    for bad in ["NaN", "Infinity", "1e300", "-5.0"] {
        fs::write(
            &path,
            good.replace("\"saved_at\":5.0", &format!("\"saved_at\":{bad}")),
        )
        .unwrap();
        assert!(load(dir.path(), "cod2").is_none(), "{bad}");
    }
}

#[test]
fn cache_resave_matches_python_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let list = fs::read(format!("{GOLDEN}/cache_cod2.json")).unwrap();
    let index = fs::read(format!("{GOLDEN}/cache_index.json")).unwrap();
    fs::write(dir.path().join("cod2.json"), &list).unwrap();
    fs::write(dir.path().join(INDEX), &index).unwrap();
    let (saved_at, servers) = load(dir.path(), "cod2").unwrap();
    assert_eq!(saved_at, 1700000000.125);
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[1].name, "Русский сервер");
    assert_eq!(servers[0].players[0].raw_name, "^1JoTa");
    save(dir.path(), "cod2", &servers, saved_at).unwrap();
    assert_eq!(fs::read(dir.path().join("cod2.json")).unwrap(), list);
    assert_eq!(fs::read(dir.path().join(INDEX)).unwrap(), index);
}

#[test]
fn cache_schema_true_or_float_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cod2.json");
    for schema in ["1", "1.0", "true"] {
        fs::write(
            &path,
            format!(r#"{{"schema":{schema},"saved_at":3,"servers":[]}}"#),
        )
        .unwrap();
        assert_eq!(load(dir.path(), "cod2"), Some((3.0, vec![])), "{schema}");
    }
    for schema in ["false", "\"1\"", "2", "null"] {
        fs::write(
            &path,
            format!(r#"{{"schema":{schema},"saved_at":3,"servers":[]}}"#),
        )
        .unwrap();
        assert!(load(dir.path(), "cod2").is_none(), "{schema}");
    }
}

#[test]
fn integers_are_bounded_like_python() {
    let mut raw = server_to_json(&info("1.2.3.4", 28960));
    raw["num_players"] = json!(i64::MAX);
    raw["bots"] = json!(i64::MIN);
    raw["ping_ms"] = json!(-7);
    raw["address"] = json!("1.2.3.4:00080");
    let s = server_from_json(&raw).unwrap();
    assert_eq!((s.num_players, s.bots, s.ping_ms), (i32::MAX, i32::MIN, 0));
    assert_eq!(s.address.port, 80);
    for bad in [
        "1.2.3.4:+80",
        "1.2.3.4:0",
        "1.2.3.4:65536",
        "1.2.3.4",
        "bad host:1",
    ] {
        raw["address"] = json!(bad);
        assert!(server_from_json(&raw).is_none(), "{bad}");
    }
    raw["address"] = json!("1.2.3.4:80");
    raw["bots"] = json!(true);
    assert!(server_from_json(&raw).is_none());
    raw["bots"] = json!(1.0);
    assert!(server_from_json(&raw).is_none());
}

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        (-1e300f64..1e300).prop_map(Value::from),
        ".{0,8}".prop_map(Value::String),
        Just(json!("1.2.3.4:28960")),
    ];
    leaf.prop_recursive(3, 24, 5, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::vec((".{0,6}", inner), 0..5)
                .prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn server_from_json_never_panics(key in 0usize..15, value in arb_json()) {
        let mut raw = server_to_json(&info("1.2.3.4", 28960));
        let obj = raw.as_object_mut().unwrap();
        let k = obj.keys().nth(key).unwrap().clone();
        obj.insert(k, value);
        if let Some(s) = server_from_json(&raw) {
            // Lo que se acepta se vuelve a leer igual.
            prop_assert_eq!(server_from_json(&server_to_json(&s)), Some(s));
        }
    }
}
