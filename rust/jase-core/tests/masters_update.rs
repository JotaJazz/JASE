//! Port de tests/test_masters_update.py (sin Qt) con un `fetch` falso.
use jase_core::masters::{
    self, CHECK_INTERVAL_S, MAX_BYTES, check_official, fetch_official, parse_official, should_check,
};
use serde_json::{Value, json};
use std::fs;

fn doc(games: Value, updated: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"schema": 1, "updated": updated, "games": games})).unwrap()
}
fn new_doc() -> Vec<u8> {
    doc(
        json!({"cod2": [{"host": "new.example.com", "port": 20510}]}),
        "2030-01-01",
    )
}
fn ok(data: Vec<u8>) -> impl FnOnce(&str) -> Option<Vec<u8>> {
    move |_| Some(data)
}
fn meta_json(dir: &std::path::Path) -> Value {
    serde_json::from_slice(&fs::read(dir.join("masters.meta.json")).unwrap()).unwrap()
}

#[test]
fn should_check_interval() {
    let t = tempfile::tempdir().unwrap();
    let meta = t.path().join("masters.meta.json");
    assert!(should_check(&meta, 1000.0));
    fs::write(&meta, "basura").unwrap();
    assert!(should_check(&meta, 1000.0));
    fs::write(&meta, r#"{"last_check": 1000.0}"#).unwrap();
    let i = CHECK_INTERVAL_S as f64;
    assert!(!should_check(&meta, 1000.0 + i - 1.0));
    assert!(should_check(&meta, 1000.0 + i));
    assert!(should_check(&meta, 500.0));
}

#[test]
fn update_writes_cache_and_meta() {
    let t = tempfile::tempdir().unwrap();
    let (res, good) = check_official(t.path(), 123.0, ok(new_doc()));
    let res = res.unwrap();
    assert!(good);
    assert_eq!(
        res.games["cod2"],
        vec![("new.example.com".to_owned(), 20510)]
    );
    assert_eq!(fs::read(t.path().join("masters.json")).unwrap(), new_doc());
    assert_eq!(
        meta_json(t.path()),
        json!({"last_attempt": 123.0, "last_attempt_ok": true, "last_success": 123.0})
    );
    assert_eq!(masters::load_official(t.path()), res);
    assert!(
        fs::read_dir(t.path()).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
}

#[test]
fn update_keeps_previous_on_invalid() {
    let t = tempfile::tempdir().unwrap();
    check_official(t.path(), 1.0, ok(new_doc()));
    check_official(t.path(), 2.0, ok(b"{no json".to_vec()));
    assert_eq!(fs::read(t.path().join("masters.json")).unwrap(), new_doc());
    assert_eq!(meta_json(t.path())["last_attempt"], json!(2.0));
}

#[test]
fn update_network_error_writes_meta_only() {
    let t = tempfile::tempdir().unwrap();
    let (res, good) = check_official(t.path(), 5.0, |_| None);
    assert!(res.is_none() && !good);
    assert!(!t.path().join("masters.json").exists());
    assert_eq!(
        meta_json(t.path()),
        json!({"last_attempt": 5.0, "last_attempt_ok": false, "last_success": null})
    );
}

#[test]
fn update_returns_none_when_unchanged() {
    let t = tempfile::tempdir().unwrap();
    assert!(check_official(t.path(), 1.0, ok(new_doc())).0.is_some());
    assert!(check_official(t.path(), 2.0, ok(new_doc())).0.is_none());
}

#[test]
fn date_only_change_counts_as_updated() {
    let t = tempfile::tempdir().unwrap();
    check_official(t.path(), 1.0, ok(new_doc()));
    let same = doc(
        json!({"cod2": [{"host": "new.example.com", "port": 20510}]}),
        "2026-12-01",
    );
    let res = check_official(t.path(), 2.0, ok(same)).0.unwrap();
    assert_eq!(res.updated, "2026-12-01");
}

#[test]
fn parse_rejects_bad_updated_format() {
    for bad in [
        "",
        "ayer",
        "2026-1-01",
        "2026-11-01\nX",
        "<b>2026-11-01</b>",
    ] {
        let d = doc(json!({"cod2": [{"host": "a.b", "port": 1}]}), bad);
        assert!(parse_official(&d).is_err(), "{bad:?}");
    }
}

#[test]
fn should_check_retries_one_hour_after_failure() {
    let t = tempfile::tempdir().unwrap();
    let meta = t.path().join("masters.meta.json");
    check_official(t.path(), 1000.0, ok(new_doc()));
    check_official(t.path(), 2000.0, |_| None);
    let m = meta_json(t.path());
    assert_eq!(
        (m["last_success"].clone(), m["last_attempt"].clone()),
        (json!(1000.0), json!(2000.0))
    );
    assert!(!should_check(&meta, 2000.0 + 3599.0));
    assert!(should_check(&meta, 2000.0 + 3601.0));
    assert!(should_check(&meta, 1000.0 + CHECK_INTERVAL_S as f64 + 1.0));
    check_official(t.path(), 9000.0, ok(new_doc()));
    assert!(!should_check(&meta, 9000.0 + 3601.0));
}

#[test]
fn should_check_tolerates_odd_meta() {
    let t = tempfile::tempdir().unwrap();
    let meta = t.path().join("masters.meta.json");
    for bad in [
        "[]",
        r#"{"last_success": "x"}"#,
        r#"{"last_attempt_ok": false, "last_attempt": null}"#,
    ] {
        fs::write(&meta, bad).unwrap();
        assert!(should_check(&meta, 1000.0), "{bad}");
    }
}

#[test]
fn fetch_rejects_oversize() {
    assert!(fetch_official(|_| Some(vec![b'x'; MAX_BYTES + 1])).is_err());
    assert_eq!(
        fetch_official(|_| Some(vec![b'x'; MAX_BYTES]))
            .unwrap()
            .len(),
        MAX_BYTES
    );
    assert!(fetch_official(|_| None).is_err());
}
