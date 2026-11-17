//! Ida y vuelta de los datos del usuario: Python 1.3.0 escribe, Rust edita y guarda, Python relee.
mod support;

use jase_core::config::{load_config, save_config};
use jase_core::favorites::{add_favorite, set_group, set_note};
use jase_core::history::History;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::recent::record_recent;
use jase_core::server_cache;
use serde_json::{Value, json};
use support::python::python;

const WRITE: &str = r#"
import sys, json
from pathlib import Path
sys.path.insert(0, sys.argv[2])
from core_golden import config_sample, cache_servers
from jase.config import save_config
from jase import server_cache
from jase.history import History

d = Path(sys.argv[1])
save_config(config_sample(), d / "config.json")
raw = json.loads((d / "config.json").read_text(encoding="utf-8"))
raw["python14_key"] = {"a": [1, 2]}
(d / "config.json").write_text(json.dumps(raw), encoding="utf-8")
server_cache.save(d / "cache" / "servers", "cod2", cache_servers(), 1700000000.125)
h = History(d / "cache" / "history.json")
h.record("cod2", "1.2.3.4:28960", 3, 20, 1700000000)
h.save()
"#;

const READ: &str = r#"
import sys, json
from pathlib import Path
from jase.config import load_config
from jase import server_cache
from jase.history import History

d = Path(sys.argv[1])
load_config(d / "config.json")
servers = d / "cache" / "servers"
h = History(d / "cache" / "history.json")
h.load(1700001000)
print(json.dumps({
    "config": json.loads((d / "config.json").read_text(encoding="utf-8")),
    "bad": sorted(p.name for p in d.rglob("*.bad")),
    "index": {g: n for g, (n, _) in server_cache.read_index(servers).items()},
    "cod2": [s.name for s in server_cache.load(servers, "cod2")[1]],
    "q3": [s.name for s in server_cache.load(servers, "q3")[1]],
    "history": h.samples("cod2", "1.2.3.4:28960"),
}, ensure_ascii=False))
"#;

fn run(code: &str, dir: &std::path::Path, extra: &[&str]) -> Option<std::process::Output> {
    let mut cmd = python()?;
    let out = cmd
        .args(["-c", code])
        .arg(dir)
        .args(extra)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(out)
}

#[test]
fn python_data_survives_rust_edits() {
    let t = tempfile::tempdir().unwrap();
    let scripts = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");
    if run(WRITE, t.path(), &[scripts]).is_none() {
        return;
    }
    let (config_path, cache) = (t.path().join("config.json"), t.path().join("cache/servers"));

    let mut cfg = load_config(&config_path);
    let new = ServerAddress::new("9.9.9.9", 28960);
    assert!(add_favorite(&mut cfg, "cod2", &new));
    set_note(&mut cfg, "cod2", "9.9.9.9:28960", "nota nueva");
    set_group(&mut cfg, "cod2", "9.9.9.9:28960", "Grupo nuevo");
    record_recent(&mut cfg, "cod2", &new);
    record_recent(&mut cfg, "q3", &ServerAddress::new("a.b", 27960));
    save_config(&cfg, &config_path).unwrap();

    let (saved_at, servers) = server_cache::load(&cache, "cod2").unwrap();
    server_cache::save(&cache, "cod2", &servers, saved_at).unwrap();
    let mut q3 = ServerInfo::new(
        ServerAddress::new("a.b", 27960),
        ServerAddress::new("a.b", 27960),
    );
    q3.name = "Servidor q3".into();
    server_cache::save(&cache, "q3", &[q3], 1_700_000_100.0).unwrap();

    let mut history = History::new(t.path().join("cache/history.json"));
    history.load(1_700_000_500.0);
    history.record("cod2", "1.2.3.4:28960", 5, 20, 1_700_000_600.0);
    history.record("cod2", "1.2.3.4:28960", 7, 20, 1_700_001_000.0);
    history.save().unwrap();

    let out = run(READ, t.path(), &[]).unwrap();
    let got: Value = serde_json::from_slice(&out.stdout).unwrap();
    let c = &got["config"];
    // Lo que escribió Python sigue...
    assert_eq!(
        c["favorites"]["cod2"],
        json!(["1.2.3.4:28960", "5.6.7.8:28961", "9.9.9.9:28960"])
    );
    assert_eq!(c["favorites"]["et"], json!(["9.9.9.9:27960"]));
    assert_eq!(
        c["favorite_notes"]["cod2"]["1.2.3.4:28960"],
        "Clan «Ñandú»\tnoche"
    );
    assert_eq!(c["favorite_groups"]["cod2"]["5.6.7.8:28961"], "Públicos");
    assert_eq!(c["filter_presets"].as_array().unwrap().len(), 2);
    assert_eq!(c["friends"], json!(["^1JoTa", "Ñandú"]));
    assert_eq!(c["party_alerts"]["et"]["9.9.9.9:27960"], 4);
    assert_eq!(c["ui_state"]["last_game"], "cod2");
    assert_eq!(c["python14_key"], json!({"a": [1, 2]}));
    // ...y lo que agregó Rust aparece.
    assert_eq!(c["favorite_notes"]["cod2"]["9.9.9.9:28960"], "nota nueva");
    assert_eq!(c["favorite_groups"]["cod2"]["9.9.9.9:28960"], "Grupo nuevo");
    assert_eq!(c["recent"]["cod2"][0], "9.9.9.9:28960");
    assert_eq!(c["recent"]["q3"], json!(["a.b:27960"]));
    assert_eq!(got["bad"], json!([]));
    assert_eq!(got["index"], json!({"cod2": 2, "q3": 1}));
    assert_eq!(got["cod2"].as_array().unwrap().len(), 2);
    assert_eq!(got["q3"], json!(["Servidor q3"]));
    assert_eq!(
        got["history"],
        json!([
            [1_700_000_000, 3, 20],
            [1_700_000_600, 5, 20],
            [1_700_001_000, 7, 20]
        ])
    );
}
