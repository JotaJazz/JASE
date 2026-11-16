//! Tabla de juegos: paridad con src/jase/games.py y tests de sus métodos.
use indexmap::IndexMap;
use jase_core::games::{self, GAMES, ListFilter, is_tacops};
use serde_json::{Value, json};

fn rules(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn game_json(g: &games::GameDef) -> Value {
    let pairs = |v: &[(&str, &str)]| v.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>();
    json!({
        "id": g.id, "name": g.name, "family": g.family.as_str(),
        "masters": g.masters.iter().map(|(h, p)| json!([h, p])).collect::<Vec<_>>(),
        "protocols": g.protocols, "known_versions": g.known_versions,
        "default_version": g.default_version, "exe_relpath": g.exe_relpath,
        "exe_candidates": g.exe_candidates, "preferred_exes": g.preferred_exes,
        "gametracker_id": g.gametracker_id, "levelshot_dir": g.levelshot_dir,
        "cod_loadscreens": g.cod_loadscreens, "gamespy_gamename": g.gamespy_gamename,
        "gamespy_key": g.gamespy_key, "status_gamename": g.status_gamename,
        "status_exclude_mods": g.status_exclude_mods, "master_gamename": g.master_gamename,
        "status_gamedir": g.status_gamedir, "launch_style": g.launch_style.as_str(),
        "connect_port_fallback": g.connect_port_fallback, "version_key": g.version_key,
        "version_aliases": g.version_aliases.iter().map(|(a, b, c)| json!([a, b, c])).collect::<Vec<_>>(),
        "valve_appid": g.valve_appid, "steam_appid": g.steam_appid,
        "valve_gamedir": g.valve_gamedir, "valve_master_filter": g.valve_master_filter,
        "steam_webapi": g.steam_webapi, "steam_webapi_optional": g.steam_webapi_optional,
        "default_path": g.default_path, "short_name": g.short_name(), "mod_key": g.mod_key,
        "base_mods": g.base_mods, "badge": g.badge,
        "list_filter": match g.list_filter { ListFilter::None => Value::Null, ListFilter::Tacops => json!("is_tacops"), ListFilter::NotTacops => json!("not_tacops") },
        "version_dirs": pairs(g.version_dirs),
    })
}

#[test]
fn games_match_python() {
    let got = serde_json::to_string(&Value::Array(GAMES.iter().map(game_json).collect())).unwrap();
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/games.json"
    ))
    .unwrap();
    let want = serde_json::to_string(&serde_json::from_str::<Value>(&golden).unwrap()).unwrap();
    assert_eq!(got, want);
}

#[test]
fn is_tacops_cases() {
    assert!(is_tacops(&rules(&[("tostver", "")])));
    for gt in ["TO340", "tost_x", "TOST", "s_swatgame"] {
        assert!(is_tacops(&rules(&[("gametype", gt)])), "{gt}");
    }
    for gt in ["TO", "DeathMatchPlus", "", "TO340x", "xTO340"] {
        assert!(!is_tacops(&rules(&[("gametype", gt)])), "{gt}");
    }
}

#[test]
fn version_label_and_base_mods() {
    let cod2 = games::game("cod2").unwrap();
    assert_eq!(cod2.version_label("1.4"), "1.4 (CoD2x)");
    assert_eq!(cod2.version_label("1.3"), "1.3");
    assert_eq!(cod2.version_aliases()[0].label, "1.4 (CoD2x)");
    let q3 = games::game("q3").unwrap();
    assert!(q3.is_base_mod("BASEQ3"));
    assert!(!q3.is_base_mod("cpma"));
    assert_eq!(games::game("cod4").unwrap().short_name(), "CoD4");
    assert_eq!(games::game("cod2").unwrap().short_name(), "Call of Duty 2");
}

#[test]
fn lists_server_filters_tacops() {
    let to = rules(&[("gametype", "TO340")]);
    let dm = rules(&[("gametype", "DeathMatchPlus")]);
    let (ut, tacops) = (games::game("ut99").unwrap(), games::game("tacops").unwrap());
    assert!(!ut.lists_server(&to) && ut.lists_server(&dm));
    assert!(tacops.lists_server(&to) && !tacops.lists_server(&dm));
    assert!(games::game("q3").unwrap().lists_server(&to));
}

#[test]
fn ids_unique_and_masters_match_bundled_json() {
    let mut ids: Vec<&str> = GAMES.iter().map(|g| g.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), GAMES.len());
    let bundled: Value =
        serde_json::from_str(include_str!("../../../src/jase/assets/masters.json")).unwrap();
    for g in GAMES {
        let want: Vec<(String, u64)> = bundled["games"][g.id]
            .as_array()
            .unwrap_or_else(|| panic!("{} sin masters en masters.json", g.id))
            .iter()
            .map(|e| {
                (
                    e["host"].as_str().unwrap().to_owned(),
                    e["port"].as_u64().unwrap(),
                )
            })
            .collect();
        let got: Vec<(String, u64)> = g
            .masters
            .iter()
            .map(|(h, p)| ((*h).to_owned(), u64::from(*p)))
            .collect();
        assert_eq!(got, want, "{}", g.id);
    }
}

#[test]
fn uses_steam_api() {
    assert!(games::game("l4d2").unwrap().uses_steam_api());
    assert!(games::game("cs16").unwrap().uses_steam_api());
    assert!(!games::game("q3").unwrap().uses_steam_api());
}
