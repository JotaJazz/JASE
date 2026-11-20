//! Paridad con Python 1.3.0: cada fixture de tests/fixtures/ debe dar el mismo resultado
//! que scripts/protocol_golden.py escribió en tests/golden/<fixture>.json.
mod support;

use indexmap::IndexMap;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::protocols::gamespy::{self, GamespyOptions, StatusAssembler};
use jase_core::protocols::quake2::{self, Quake2Options};
use jase_core::protocols::quake3::{self, StatusOptions};
use jase_core::protocols::{ProtocolError, steamweb, ut2004, valve, zandronum};
use serde_json::{Value, json};
use support::{fixture, fixtures_dir};

const EXCLUDED: &[&str] = &["ut2004_master_1.bin"];
const Q3_MASTERS: &[&str] = &[
    "cod1", "cod2", "cod4", "coduo", "et", "jka", "oa", "q3", "rtcw", "sof2", "urt", "xonotic",
];
const Q3_STATUS: &[&str] = &[
    "cod2x",
    "cod4",
    "coduo",
    "et",
    "jk2",
    "jka",
    "oa",
    "q3",
    "rtcw",
    "smokinguns",
    "sof2",
    "tremulous",
    "urt",
    "warfork",
    "warsow",
    "xonotic",
];

fn addr() -> ServerAddress {
    ServerAddress::new("192.0.2.1", 27960)
}

/// Misma forma que `to_json` de scripts/protocol_golden.py.
trait ToJson {
    fn to_json(&self) -> Value;
}
impl ToJson for ServerAddress {
    fn to_json(&self) -> Value {
        json!(self.to_string())
    }
}
impl ToJson for Player {
    fn to_json(&self) -> Value {
        json!({"name": self.name, "score": self.score, "ping": self.ping, "raw_name": self.raw_name})
    }
}
impl ToJson for IndexMap<String, String> {
    fn to_json(&self) -> Value {
        Value::Object(self.iter().map(|(k, v)| (k.clone(), json!(v))).collect())
    }
}
impl ToJson for ServerInfo {
    fn to_json(&self) -> Value {
        json!({
            "address": self.address.to_json(),
            "connect_address": self.connect_address.to_json(),
            "name": self.name,
            "map": self.map,
            "gametype": self.gametype,
            "num_players": self.num_players,
            "max_players": self.max_players,
            "password": self.password,
            "version": self.version,
            "ping_ms": self.ping_ms,
            "players": self.players.to_json(),
            "rules": self.rules.to_json(),
            "raw_name": self.raw_name,
            "online": self.online,
            "bots": self.bots,
            "connect_port_deduced": self.connect_port_deduced,
        })
    }
}
impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self) -> Value {
        Value::Array(self.iter().map(ToJson::to_json).collect())
    }
}
impl<T: ToJson> ToJson for Option<T> {
    fn to_json(&self) -> Value {
        self.as_ref().map_or(Value::Null, ToJson::to_json)
    }
}
impl ToJson for bool {
    fn to_json(&self) -> Value {
        json!(self)
    }
}
impl ToJson for usize {
    fn to_json(&self) -> Value {
        json!(self)
    }
}
impl ToJson for [u8; 4] {
    fn to_json(&self) -> Value {
        json!(self.to_vec())
    }
}
impl<A: ToJson, B: ToJson> ToJson for (A, B) {
    fn to_json(&self) -> Value {
        json!([self.0.to_json(), self.1.to_json()])
    }
}

fn out<T: ToJson>(r: Result<T, ProtocolError>) -> Value {
    r.map_or_else(|_| json!({"error": true}), |v| v.to_json())
}

/// `fnmatch` mínimo: un solo `*` como comodín (es lo que usa la tabla).
fn matches(name: &str, pattern: &str) -> bool {
    match pattern.split_once('*') {
        None => name == pattern,
        Some((pre, rest)) => match rest.split_once('*') {
            None => {
                name.len() >= pre.len() + rest.len()
                    && name.starts_with(pre)
                    && name.ends_with(rest)
            }
            Some((mid, post)) => {
                name.starts_with(pre)
                    && name.ends_with(post)
                    && name.len() >= pre.len() + post.len()
                    && name[pre.len()..name.len() - post.len()].contains(mid)
            }
        },
    }
}

fn any(name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| matches(name, p))
}

/// Resultado de Rust para un fixture, con la misma tabla que el script de Python.
fn parse(name: &str, d: &[u8]) -> Option<Value> {
    let a = addr();
    let q3m: Vec<String> = Q3_MASTERS
        .iter()
        .map(|g| format!("{g}_master_*.bin"))
        .collect();
    let mut q3s: Vec<String> = Q3_STATUS
        .iter()
        .map(|g| format!("{g}_status*.bin"))
        .collect();
    q3s.push("oa_master_mixed_baseq3_status.bin".into());
    let p = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    Some(if any(name, &q3m) {
        out(quake3::parse_getservers_response(d))
    } else if any(name, &q3s) {
        out(quake3::parse_status_response(
            d,
            &a,
            1,
            &StatusOptions::default(),
        ))
    } else if name == "q2_master_1.bin" {
        out(quake2::parse_master_response(d))
    } else if any(name, &p(&["q2_status.bin", "dday_status.bin"])) {
        out(quake2::parse_status(d, &a, 1, &Quake2Options::default()))
    } else if any(
        name,
        &p(&["bf1942_master_openspy.bin", "mohaa*_master_333.bin"]),
    ) {
        gamespy::parse_master_list(d).to_json()
    } else if any(
        name,
        &p(&["bf1942_status.bin", "mohaa*_status*.bin", "to*_status.bin"]),
    ) {
        let mut asm = StatusAssembler::new();
        out(asm
            .feed(d)
            .and_then(|_| gamespy::parse_status(&asm.fields(), &a, 1, &GamespyOptions::default())))
    } else if name == "cs16_master_gamedir_1.bin" {
        out(valve::parse_master_response(d))
    } else if any(name, &p(&["cs16_info*.bin"])) {
        out(valve::parse_info(d, &a, 1, ""))
    } else if name == "cs16_player.bin" {
        out(valve::parse_players(d))
    } else if name == "cs16_player_challenge.bin" {
        valve::challenge_of(d).to_json()
    } else if name == "ut2004_info.bin" {
        out(ut2004::parse_info(d, &a, 1))
    } else if name == "ut2004_players.bin" {
        out(ut2004::parse_players(d))
    } else if name == "zandronum_master.bin" {
        out(zandronum::parse_master_packet(d))
    } else if any(name, &p(&["zandronum_players.bin", "zandronum_team.bin"])) {
        out(zandronum::parse_response(d, &a, 1))
    } else if name == "steamweb_l4d2.json" {
        out(steamweb::parse_response(d))
    } else {
        return None;
    })
}

fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn golden_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden"))
        .join(format!("{name}.json"))
}

#[test]
fn every_fixture_matches_golden() {
    let mut diffs = Vec::new();
    for name in fixture_names()
        .iter()
        .filter(|n| !EXCLUDED.contains(&n.as_str()))
    {
        let got = parse(name, &fixture(name)).unwrap_or(Value::Null);
        let want: Value =
            serde_json::from_str(&std::fs::read_to_string(golden_path(name)).unwrap()).unwrap();
        // Texto, no Value: con preserve_order el orden de claves (rules) también cuenta.
        if serde_json::to_string(&got).unwrap() != serde_json::to_string(&want).unwrap() {
            diffs.push(format!("{name}:\n  rust:   {got}\n  golden: {want}"));
        }
    }
    assert!(
        diffs.is_empty(),
        "{} diferencias:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}

#[test]
fn every_fixture_has_golden_or_is_excluded() {
    for name in fixture_names() {
        let known = EXCLUDED.contains(&name.as_str())
            || (parse(&name, b"").is_some() && golden_path(&name).is_file());
        assert!(
            known,
            "fixture nuevo sin golden ni exclusión: {name} (correr scripts/protocol_golden.py)"
        );
    }
}

#[test]
fn glob_matches_like_fnmatch() {
    assert!(matches("mohaa_status_hostport.bin", "mohaa*_status*.bin"));
    assert!(matches("mohaas_status.bin", "mohaa*_status*.bin"));
    assert!(!matches("mohaa_master_333.bin", "mohaa*_status*.bin"));
    assert!(matches("cod2_master_118_1.bin", "cod2_master_*.bin"));
    assert!(!matches("cod2x_status.bin", "cod2_status*.bin"));
}
