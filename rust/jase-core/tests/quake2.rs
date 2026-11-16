//! Port de tests/test_quake2.py (los de GameDef y escáner quedan para la fase 2).
mod support;

use jase_core::models::ServerAddress;
use jase_core::protocols::quake2::{self, Quake2Options};
use support::fixture;

fn addr() -> ServerAddress {
    ServerAddress::new("1.2.3.4", 27910)
}

#[test]
fn master_fixture_parses() {
    let servers = quake2::parse_master_response(&fixture("q2_master_1.bin")).unwrap();
    assert!(servers.len() >= 100);
    assert!(servers.iter().all(|s| s.port > 0));
}

#[test]
fn master_without_header_raises() {
    assert!(
        quake2::parse_master_response(b"\xff\xff\xff\xffprint\n\x01\x02\x03\x04\x6b\x86").is_err()
    );
}

#[test]
fn master_decodes_entries() {
    let got =
        quake2::parse_master_response(b"\xff\xff\xff\xffservers \x01\x02\x03\x04\x6b\x86\x05")
            .unwrap();
    assert_eq!(got, [ServerAddress::new("1.2.3.4", 27526)]);
}

#[test]
fn status_fixture() {
    let info = quake2::parse_status(
        &fixture("q2_status.bin"),
        &addr(),
        42,
        &Quake2Options::default(),
    )
    .unwrap();
    assert_eq!(info.name, "Feast of Frags [US] [DM] [Bots]");
    assert_eq!(info.map, "act2r2");
    assert_eq!((info.num_players, info.max_players), (6, 18));
    assert_eq!(
        (info.gametype.as_str(), info.version.as_str()),
        ("action", "q2proded r1~277d2ef")
    );
    assert!(!info.password);
    assert_eq!(info.ping_ms, 42);
    let first: Vec<(&str, i32, i32)> = info.players[..2]
        .iter()
        .map(|p| (p.name.as_str(), p.score, p.ping))
        .collect();
    assert_eq!(first, [("Paril", 25, 9), ("Mekhi", 6, 23)]);
    assert_eq!(info.players[3].name, "[DoS] Xavier");
}

#[test]
fn status_needpass_and_high_bit_name() {
    let data = b"\xff\xff\xff\xffprint\n\\hostname\\X\\needpass\\1\\maxclients\\8\\version\\3.20 x86\n1 5 \"J\xf6rg\"\n";
    let info = quake2::parse_status(data, &addr(), 1, &Quake2Options::default()).unwrap();
    assert!(info.password);
    assert_eq!(info.version, "3.20");
    assert_eq!(info.players[0].name, "J\u{f6}rg");
}

#[test]
fn status_without_header_raises() {
    assert!(
        quake2::parse_status(
            b"\xff\xff\xff\xffinfo\n",
            &addr(),
            1,
            &Quake2Options::default()
        )
        .is_err()
    );
}

// Extra: opciones que en Python se prueban vía games.py.
#[test]
fn status_options_gamedir_and_excluded_mods() {
    let data = b"\xff\xff\xff\xffprint\n\\hostname\\X\\gamedir\\DDay\\maxclients\\8\\needpass\\\n";
    let a = addr();
    let o = Quake2Options {
        expected_gamedir: "dday",
        ..Quake2Options::default()
    };
    let info = quake2::parse_status(data, &a, 1, &o).unwrap();
    assert!(!info.password); // needpass vacío = 0
    let o = Quake2Options {
        expected_gamedir: "action",
        ..Quake2Options::default()
    };
    assert!(quake2::parse_status(data, &a, 1, &o).is_err());
    let excluded = ["dday".to_string()];
    let o = Quake2Options {
        exclude_mods: &excluded,
        ..Quake2Options::default()
    };
    assert!(quake2::parse_status(data, &a, 1, &o).is_err());
}

#[test]
fn short_version_examples() {
    assert_eq!(quake2::short_version("3.20 x86_64 Linux"), "3.20");
    assert_eq!(
        quake2::short_version("q2proded r1~277d2ef Mar 24 2026"),
        "q2proded r1~277d2ef"
    );
    assert_eq!(quake2::short_version("\u{b2}x y"), "\u{b2}x"); // isdigit() de Python acepta ²
    assert_eq!(quake2::short_version("  "), "");
}
