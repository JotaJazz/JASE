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
