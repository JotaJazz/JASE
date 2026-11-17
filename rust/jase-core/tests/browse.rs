//! `mod_of`. Traducción de los casos de tests/test_browse.py.
mod support;
use jase_core::browse::{BASE_MOD, mod_of};
use jase_core::games::game;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::protocols::gamespy::{self as gs, GamespyOptions, StatusAssembler};
use jase_core::protocols::quake3::{self as q3, StatusOptions};
use support::fixture;

fn addr() -> ServerAddress {
    ServerAddress::new("1.2.3.4", 28960)
}

fn q3_info(name: &str, game_id: &str) -> ServerInfo {
    let opts = StatusOptions {
        version_key: game(game_id).unwrap().version_key,
        ..StatusOptions::default()
    };
    q3::parse_status_response(&fixture(name), &addr(), 50, &opts).unwrap()
}

#[test]
fn mod_of_cod4_strips_mods_prefix() {
    let info = q3_info("cod4_status_17.bin", "cod4");
    assert_eq!(mod_of(game("cod4").unwrap(), &info), "1stmsob_openwarfare");
}

#[test]
fn mod_of_cod4x_without_fs_game_is_base() {
    let info = q3_info("cod4_status_cod4x.bin", "cod4");
    assert_eq!(mod_of(game("cod4").unwrap(), &info), BASE_MOD);
}

#[test]
fn mod_of_et_uses_gamename() {
    assert_eq!(
        mod_of(game("et").unwrap(), &q3_info("et_status.bin", "et")),
        "nq"
    );
}

#[test]
fn mod_of_rtcw_strips_colors() {
    let info = q3_info("rtcw_status.bin", "rtcw");
    assert_eq!(mod_of(game("rtcw").unwrap(), &info), "ffmod");
}

#[test]
fn mod_of_q3_baseq3_is_base() {
    assert_eq!(
        mod_of(game("q3").unwrap(), &q3_info("q3_status.bin", "q3")),
        BASE_MOD
    );
}

#[test]
fn mod_of_mohaa_always_base() {
    let mut asm = StatusAssembler::new();
    assert!(asm.feed(&fixture("mohaa_status.bin")).unwrap());
    let g = game("mohaa").unwrap();
    let opts = GamespyOptions {
        connect_port_fallback: g.connect_port_fallback,
        expected_gamename: g.status_gamename,
        ..GamespyOptions::default()
    };
    let info = gs::parse_status(
        &asm.fields(),
        &ServerAddress::new("1.2.3.4", 12300),
        50,
        &opts,
    )
    .unwrap();
    assert_eq!(mod_of(g, &info), BASE_MOD);
}

#[test]
fn mod_of_prefix_and_case_insensitive() {
    let mut info = ServerInfo::new(addr(), addr());
    info.rules.insert("fs_game".into(), "Mods/PAM".into());
    assert_eq!(mod_of(game("cod2").unwrap(), &info), "pam");
}

// ---- Desglose del árbol (resto de tests/test_browse.py) ----
use jase_core::browse::{TOP_MAPS, breakdown, map_of, matches};
use std::cell::Cell;
use std::collections::HashSet;

fn srv(map: &str, fs_game: Option<&str>) -> ServerInfo {
    let mut s = ServerInfo::new(addr(), addr());
    s.map = map.into();
    if let Some(m) = fs_game {
        s.rules.insert("fs_game".into(), m.into());
    }
    s
}

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn pairs(v: &[(&str, usize)]) -> Vec<(String, usize)> {
    v.iter().map(|(k, n)| ((*k).to_owned(), *n)).collect()
}

#[test]
fn breakdown_hides_mods_when_all_base() {
    let servers = [srv("mohaa1", None), srv("mohaa2", None)];
    assert!(
        breakdown(game("mohaa").unwrap(), &servers, None)
            .mods
            .is_empty()
    );
}

#[test]
fn breakdown_mods_sorted_by_count_then_name() {
    let servers = [
        srv("m", Some("mods/b")),
        srv("m", Some("mods/a")),
        srv("m", Some("mods/c")),
        srv("m", Some("mods/c")),
        srv("m", None),
    ];
    let b = breakdown(game("cod2").unwrap(), &servers, None);
    assert_eq!(
        b.mods,
        pairs(&[("c", 2), (BASE_MOD, 1), ("a", 1), ("b", 1)])
    );
}

#[test]
fn breakdown_top_maps_and_other_count() {
    let mut servers = vec![srv("mp_a", None); 3];
    servers.extend("bcdefghijkl".chars().map(|c| srv(&format!("mp_{c}"), None)));
    let b = breakdown(game("cod2").unwrap(), &servers, None);
    assert_eq!(b.maps[0], ("mp_a".to_owned(), 3));
    assert_eq!(b.maps.len(), TOP_MAPS);
    assert_eq!(b.other_maps, 2);
}

#[test]
fn breakdown_groups_case_insensitive() {
    let servers = [srv("MP_Toujane", None), srv("mp_toujane", None)];
    let b = breakdown(game("cod2").unwrap(), &servers, None);
    assert_eq!(b.maps, pairs(&[("mp_toujane", 2)]));
    assert_eq!(map_of(&servers[0]), "mp_toujane");
}

#[test]
fn matches_or_within_and_across() {
    let g = game("cod2").unwrap();
    let toujane = srv("mp_toujane", Some("mods/pam"));
    let carentan = srv("mp_carentan", Some("mods/pam"));
    assert!(matches(
        g,
        &toujane,
        &set(&["pam", "zom"]),
        &set(&["mp_toujane"])
    ));
    assert!(!matches(
        g,
        &carentan,
        &set(&["pam"]),
        &set(&["mp_toujane"])
    ));
    assert!(matches(g, &carentan, &set(&[]), &set(&[])));
    assert!(matches(g, &carentan, &set(&[]), &set(&["mp_carentan"])));
}

#[test]
fn breakdown_counts_countries_with_lookup() {
    let servers = [srv("a", None), srv("b", None), srv("c", None)];
    let codes = ["cl", "ar", "cl"];
    let i = Cell::new(0);
    let lookup = move |_: &ServerInfo| {
        i.set(i.get() + 1);
        Some(codes[i.get() - 1].to_owned())
    };
    let b = breakdown(game("cod2").unwrap(), &servers, Some(&lookup));
    assert_eq!(b.countries, pairs(&[("cl", 2), ("ar", 1)]));
}

#[test]
fn breakdown_skips_unknown_country_and_needs_lookup() {
    let g = game("cod2").unwrap();
    let servers = [srv("a", None)];
    assert!(breakdown(g, &servers, None).countries.is_empty());
    let none = |_: &ServerInfo| None;
    assert!(breakdown(g, &servers, Some(&none)).countries.is_empty());
}
