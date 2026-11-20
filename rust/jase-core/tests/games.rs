//! Tabla de juegos: tests de sus métodos.
use indexmap::IndexMap;
use jase_core::games::{self, GAMES, is_tacops};
use serde_json::Value;

fn rules(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
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
    let bundled: Value = serde_json::from_str(include_str!("../../../masters.json")).unwrap();
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
