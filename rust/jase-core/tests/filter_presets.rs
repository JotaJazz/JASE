//! Port de tests/test_filter_presets.py.
use jase_core::config::{Config, load_config, save_config};
use jase_core::filter_presets::{FilterPreset, MAX_PRESETS, parse, remove, to_json, upsert};
use jase_core::models::QuickFilters;
use serde_json::json;

fn preset(name: &str, quick: QuickFilters, text: &str) -> FilterPreset {
    FilterPreset {
        name: name.to_owned(),
        quick,
        text: text.to_owned(),
    }
}

#[test]
fn parse_tolerant() {
    let raw = json!([
        {"name": "A", "quick": {"hide_empty": true, "max_ping": "x"}, "text": "dm"},
        "basura", {"name": ""}, {"name": 3}, {"name": "a"},  // duplicado casefold
        {"name": " B ", "quick": 5, "text": 7},
    ]);
    let out = parse(&raw);
    let names: Vec<_> = out.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["A", "B"]);
    assert_eq!(
        out[0].quick,
        QuickFilters {
            hide_empty: true,
            ..QuickFilters::default()
        }
    );
    assert_eq!(out[0].text, "dm");
    assert_eq!(out[1].quick, QuickFilters::default());
    assert_eq!(out[1].text, "");
    assert!(parse(&json!("no")).is_empty() && parse(&json!(null)).is_empty());
}

#[test]
fn parse_cap() {
    let raw: Vec<_> = (0..50).map(|i| json!({"name": format!("p{i}")})).collect();
    assert_eq!(parse(&json!(raw)).len(), MAX_PRESETS);
}

#[test]
fn upsert_replaces_keeping_order_and_remove() {
    let a = preset("A", QuickFilters::default(), "");
    let b = preset("B", QuickFilters::default(), "");
    let a2 = preset(
        "a",
        QuickFilters {
            hide_full: true,
            ..QuickFilters::default()
        },
        "x",
    );
    let out = upsert(&[a, b.clone()], a2.clone());
    assert_eq!(out, [a2.clone(), b]);
    assert_eq!(remove(&out, "B"), [a2]);
    let full: Vec<_> = (0..MAX_PRESETS)
        .map(|i| preset(&format!("p{i}"), QuickFilters::default(), ""))
        .collect();
    let out = upsert(&full, preset("new", QuickFilters::default(), ""));
    assert_eq!(out.len(), MAX_PRESETS);
}

#[test]
fn config_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("c.json");
    let cfg = Config {
        filter_presets: vec![preset(
            "A",
            QuickFilters {
                max_ping: 80,
                ..QuickFilters::default()
            },
            "t",
        )],
        ..Config::default()
    };
    save_config(&cfg, &p).unwrap();
    assert_eq!(load_config(&p).filter_presets, cfg.filter_presets);
    assert_eq!(to_json(&cfg.filter_presets)[0]["quick"]["max_ping"], 80);
}
