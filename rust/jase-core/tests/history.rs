//! Port de tests/test_history.py, más la compatibilidad de bytes con Python 1.3.0.
use jase_core::history::*;
use serde_json::json;
use std::collections::HashSet;
use std::fs;

const NOW: f64 = 1_700_000_000.0;
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/data");

fn players(h: &History, game: &str, addr: &str) -> Vec<i64> {
    h.samples(game, addr).iter().map(|s| s[1]).collect()
}

#[test]
fn min_gap() {
    let mut h = History::new("h.json".into());
    h.record("q3", "1.2.3.4:27960", 3, 8, NOW);
    h.record("q3", "1.2.3.4:27960", 4, 8, NOW + MIN_GAP_S as f64 - 1.0);
    h.record("q3", "1.2.3.4:27960", 5, 8, NOW + MIN_GAP_S as f64);
    assert_eq!(players(&h, "q3", "1.2.3.4:27960"), [3, 5]);
}

#[test]
fn prune_by_days_and_cap() {
    let mut h = History::new("h.json".into());
    h.record("q3", "a:1", 1, 8, NOW);
    let later = NOW + (KEEP_DAYS * 86400) as f64 + 10.0;
    h.record("q3", "a:1", 2, 8, later);
    assert_eq!(players(&h, "q3", "a:1"), [2]);
    let mut h2 = History::new("h2.json".into());
    for i in 0..MAX_SAMPLES + 5 {
        h2.record(
            "q3",
            "a:1",
            (i % 7) as i64,
            8,
            NOW + (i as i64 * MIN_GAP_S) as f64,
        );
    }
    assert!(h2.samples("q3", "a:1").len() <= MAX_SAMPLES);
}

#[test]
fn hourly_average_timezone() {
    let mut h = History::new("h.json".into());
    let base = 1_700_006_400.0; // 00:00 UTC
    h.record("q3", "a:1", 4, 8, base);
    h.record("q3", "a:1", 6, 8, base + 600.0);
    h.record("q3", "a:1", 2, 8, base + 3.0 * 3600.0);
    let utc = h.hourly_average("q3", "a:1", base + 4000.0, 0);
    assert_eq!((utc[0], utc[3], utc[1]), (Some(5.0), Some(2.0), None));
    let shifted = h.hourly_average("q3", "a:1", base + 4000.0, -3 * 3600);
    assert_eq!((shifted[21], shifted[0]), (Some(5.0), Some(2.0)));
}

#[test]
fn hourly_average_huge_values_do_not_overflow() {
    let mut h = History::new("h.json".into());
    h.record("q3", "a:1", i64::MAX, 8, NOW);
    h.record("q3", "a:1", i64::MAX, 8, NOW + 600.0);
    let avg = h.hourly_average("q3", "a:1", NOW, i64::MIN);
    assert_eq!(avg.iter().flatten().count(), 1);
    assert_eq!(avg.iter().flatten().next(), Some(&(i64::MAX as f64)));
    let avg = h.hourly_average("q3", "a:1", NOW, i64::MAX);
    assert_eq!(avg.iter().flatten().count(), 1);
}

#[test]
fn roundtrip_and_prune() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache").join("h.json");
    let mut h = History::new(path.clone());
    h.record("q3", "a:1", 1, 8, NOW);
    h.record("q3", "b:2", 1, 8, NOW);
    h.record("cod", "c:3", 1, 8, NOW);
    h.prune(&HashSet::from([("q3".to_owned(), "a:1".to_owned())]));
    h.save().unwrap();
    let mut h2 = History::new(path);
    h2.load(NOW);
    assert!(!h2.samples("q3", "a:1").is_empty());
    assert!(h2.samples("q3", "b:2").is_empty() && h2.samples("cod", "c:3").is_empty());
    assert!(!h2.data.contains_key("cod"));
}

#[test]
fn corrupt_or_huge_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.json");
    fs::write(&path, "{no json").unwrap();
    let mut h = History::new(path.clone());
    h.load(NOW);
    assert!(h.samples("q3", "a:1").is_empty());
    fs::write(&path, json!({"q3": {"a:1": [[1, 2, 3]]}}).to_string()).unwrap();
    h.load(NOW);
    assert_eq!(h.samples("q3", "a:1"), [[1, 2, 3]]);
    fs::write(
        &path,
        json!({"q3": {"a:1": [["x", 2, 3], [1, 2]]}}).to_string(),
    )
    .unwrap();
    h.load(NOW);
    assert!(h.samples("q3", "a:1").is_empty());
    fs::write(&path, vec![b' '; MAX_FILE_BYTES as usize + 1]).unwrap();
    h.load(NOW);
    assert!(h.samples("q3", "a:1").is_empty());
}

#[test]
fn hostile_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.json");
    let mut h = History::new(path.clone());
    fs::write(&path, "[".repeat(100_000)).unwrap();
    h.load(NOW);
    assert!(h.samples("q3", "a:1").is_empty());
    fs::write(&path, "[1, 2]").unwrap();
    h.load(NOW);
    assert!(h.data.is_empty());
    fs::write(
        &path,
        r#"{"q3": {"a:1": [[NaN, 1, 2], [Infinity, 1, 2], [true, 1, 2]]}}"#,
    )
    .unwrap();
    h.load(NOW);
    assert!(h.samples("q3", "a:1").is_empty());
    let now = NOW as i64;
    let bad = json!([
        [-5, 1, 8],
        [0, 1, 8],
        [now + 3 * 86400, 1, 8],
        [now - 10, 2, 8],
        [now - 100, 3, 8]
    ]);
    fs::write(&path, json!({"q3": {"a:1": bad}}).to_string()).unwrap();
    h.load(NOW);
    assert_eq!(players(&h, "q3", "a:1"), [3, 2]);
}

#[test]
fn history_resave_matches_python_bytes() {
    let golden = fs::read(format!("{GOLDEN}/history.json")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");
    fs::write(&path, &golden).unwrap();
    let mut h = History::new(path.clone());
    h.load(NOW + 3600.0);
    assert_eq!(players(&h, "cod2", "1.2.3.4:28960"), [1, 3]);
    assert_eq!(players(&h, "et", "ñandú😀:27960"), [4]);
    assert_eq!(h.to_bytes(), golden);
    h.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), golden);
}
