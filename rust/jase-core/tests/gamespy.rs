//! Port de tests/test_gamespy.py.
mod support;

use indexmap::IndexMap;
use jase_core::models::{Player, ServerAddress};
use jase_core::protocols::gamespy::{self as gs, GamespyOptions, StatusAssembler};

fn fields(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn addr(port: u16) -> ServerAddress {
    ServerAddress::new("1.1.1.1", port)
}

fn parse(
    f: &IndexMap<String, String>,
    port: u16,
) -> Result<jase_core::models::ServerInfo, jase_core::protocols::ProtocolError> {
    gs::parse_status(f, &addr(port), 5, &GamespyOptions::default())
}

#[test]
fn parse_secure() {
    assert_eq!(
        gs::parse_secure(b"\\basic\\\\secure\\ABCDEF").unwrap(),
        "ABCDEF"
    );
}

#[test]
fn gsmsalg_shape_and_determinism() {
    let out = gs::gsmsalg("ABCDEF", gs::UT_GAMEKEY);
    assert_eq!(out.len(), 8);
    assert!(
        out.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
    );
    assert_eq!(out, gs::gsmsalg("ABCDEF", gs::UT_GAMEKEY));
    assert_ne!(out, gs::gsmsalg("ABCDEG", gs::UT_GAMEKEY));
}

#[test]
fn gsmsalg_known_answer() {
    // Lo que daba gs.gsmsalg("ABCDEF", "Z5Nfb0") en Python 1.3.0.
    assert_eq!(gs::gsmsalg("ABCDEF", gs::UT_GAMEKEY), GSMSALG_ABCDEF);
}

const GSMSALG_ABCDEF: &str = include_str!("golden/gsmsalg_ABCDEF.txt");

#[test]
fn build_validate() {
    let v = gs::build_validate("ABCDEF", gs::UT_GAMENAME, gs::UT_GAMEKEY);
    assert!(v.starts_with(b"\\gamename\\ut\\enctype\\0\\validate\\") && v.ends_with(b"\\final\\"));
}

#[test]
fn parse_master_list() {
    let data = b"\\ip\\1.2.3.4:7778\\ip\\5.6.7.8:7790\\final\\";
    assert_eq!(
        gs::parse_master_list(data),
        [
            ServerAddress::new("1.2.3.4", 7778),
            ServerAddress::new("5.6.7.8", 7790)
        ]
    );
}

const P1: &[u8] =
    b"\\hostname\\Jimmy UT\\hostport\\7777\\mapname\\DM-Deck16][\\gametype\\DeathMatchPlus\
\\numplayers\\1\\maxplayers\\16\\password\\True\\gamever\\469\\queryid\\7.1";
const P2: &[u8] = b"\\player_0\\Jimmy\\frags_0\\12\\ping_0\\40\\queryid\\7.2\\final\\";

#[test]
fn assembler_out_of_order() {
    let mut a = StatusAssembler::new();
    assert!(!a.feed(P2).unwrap());
    assert!(a.feed(P1).unwrap());
    let f = a.fields();
    assert_eq!(f["hostname"], "Jimmy UT");
    assert_eq!(f["player_0"], "Jimmy");
}

#[test]
fn parse_status() {
    let mut a = StatusAssembler::new();
    assert!(!a.feed(P1).unwrap());
    assert!(a.feed(P2).unwrap());
    let info = gs::parse_status(
        &a.fields(),
        &ServerAddress::new("1.2.3.4", 7778),
        33,
        &GamespyOptions::default(),
    )
    .unwrap();
    assert_eq!(
        (
            info.name.as_str(),
            info.map.as_str(),
            info.gametype.as_str()
        ),
        ("Jimmy UT", "DM-Deck16][", "DeathMatchPlus")
    );
    assert_eq!(
        (
            info.num_players,
            info.max_players,
            info.password,
            info.version.as_str()
        ),
        (1, 16, true, "469")
    );
    assert_eq!(info.players, [Player::new("Jimmy", 12, 40)]);
    assert_eq!(info.connect_address, ServerAddress::new("1.2.3.4", 7777));
    assert!(!info.rules.contains_key("queryid") && !info.rules.contains_key("player_0"));
}

#[test]
fn parse_status_drops_control_characters() {
    let f = fields(&[
        ("hostname", "Jim\x01my\tUT"),
        ("numplayers", "1"),
        ("maxplayers", "8"),
        ("player_0", "\x02Ji\tmmy"),
        ("frags_0", "1"),
        ("ping_0", "2"),
    ]);
    let info = parse(&f, 7778).unwrap();
    assert_eq!(info.name, "JimmyUT");
    assert_eq!(info.players, [Player::new("Jimmy", 1, 2)]);
}

#[test]
fn parse_status_without_hostport_uses_query_port_minus_one() {
    let info = parse(
        &fields(&[("hostname", "X"), ("numplayers", "0"), ("maxplayers", "8")]),
        7778,
    )
    .unwrap();
    assert_eq!(info.connect_address.port, 7777);
    assert!(!info.password);
}

#[test]
fn parse_status_bad_numbers() {
    assert!(parse(&fields(&[("hostname", "X"), ("numplayers", "lots")]), 7778).is_err());
}

#[test]
fn master_list_skips_malformed_entries() {
    let data = b"\\ip\\1.2.3.4:7\xb2\\ip\\:7777\\ip\\1.2.3.4:0\\ip\\1.2.3.4:70000\\ip\\nocolon\\ip\\9.9.9.9:7778\\final\\";
    assert_eq!(
        gs::parse_master_list(data),
        [ServerAddress::new("9.9.9.9", 7778)]
    );
}

#[test]
fn master_list_garbage_is_empty() {
    for data in [&b""[..], b"garbage", b"\\\\\\"] {
        assert!(gs::parse_master_list(data).is_empty());
    }
}

#[test]
fn parse_secure_missing() {
    assert!(gs::parse_secure(b"\\basic\\\\final\\").is_err());
}

#[test]
fn assembler_rejects_packet_without_valid_queryid() {
    for data in [
        &b"\\hostname\\X\\final\\"[..],
        b"\\queryid\\abc\\final\\",
        b"\\queryid\\7.0\\final\\",
    ] {
        assert!(StatusAssembler::new().feed(data).is_err());
    }
}

#[test]
fn assembler_never_completes_with_missing_packet() {
    let mut a = StatusAssembler::new();
    assert!(!a.feed(P1).unwrap());
    assert!(!a.is_complete());
    assert_eq!(a.fields()["hostname"], "Jimmy UT");
}

#[test]
fn assembler_in_order_completes() {
    let mut a = StatusAssembler::new();
    assert!(!a.feed(P1).unwrap());
    assert!(a.feed(P2).unwrap());
}

#[test]
fn parse_status_bad_player_numbers() {
    for bad in ["frags_0", "ping_0"] {
        let mut f = fields(&[
            ("hostname", "X"),
            ("player_0", "J"),
            ("frags_0", "1"),
            ("ping_0", "2"),
        ]);
        f.insert(bad.into(), "x".into());
        assert!(parse(&f, 7778).is_err());
    }
}

#[test]
fn parse_status_bad_hostport() {
    assert!(parse(&fields(&[("hostname", "X"), ("hostport", "abc")]), 7778).is_err());
}

#[test]
fn parse_status_hostport_out_of_range_uses_fallback() {
    let info = parse(&fields(&[("hostname", "X"), ("hostport", "999999")]), 7778).unwrap();
    assert_eq!(info.connect_address.port, 7777);
    assert!(info.connect_port_deduced);
}

#[test]
fn parse_status_huge_numbers_are_bounded() {
    let huge = "9".repeat(400);
    let f = fields(&[
        ("hostname", "X"),
        ("numplayers", &huge),
        ("maxplayers", "8"),
        ("player_0", "J"),
        ("frags_0", &huge),
        ("ping_0", "1"),
    ]);
    assert_eq!(parse(&f, 7778).unwrap().players[0].score, i32::MAX);
}

#[test]
fn parse_status_overlong_digits_still_protocol_error() {
    assert!(
        parse(
            &fields(&[("hostname", "X"), ("numplayers", &"9".repeat(5000))]),
            7778
        )
        .is_err()
    );
}

// Extra.
#[test]
fn deduced_port_saturates() {
    let info = parse(&fields(&[("hostname", "X")]), 0).unwrap();
    assert_eq!(info.connect_address.port, 0);
    assert!(info.connect_port_deduced);
}

#[test]
fn player_index_without_matching_key_is_error() {
    // Python lanza KeyError; Rust descarta el servidor.
    assert!(parse(&fields(&[("hostname", "X"), ("player_01", "J")]), 7778).is_err());
}

#[test]
fn options_gamename_version_key_and_aliases() {
    let f = fields(&[
        ("hostname", "X"),
        ("gamename", "TacOps"),
        ("gametype", "TO340"),
        ("gamever", "436"),
    ]);
    let o = GamespyOptions {
        expected_gamename: "ut",
        ..GamespyOptions::default()
    };
    assert!(gs::parse_status(&f, &addr(7778), 1, &o).is_err());
    let aliases = [jase_core::protocols::quake3::VersionAlias {
        prefix: "TO340".into(),
        key: "3.4".into(),
        label: "3.4".into(),
    }];
    let o = GamespyOptions {
        version_key: "gametype",
        version_aliases: &aliases,
        ..GamespyOptions::default()
    };
    assert_eq!(
        gs::parse_status(&f, &addr(7778), 1, &o).unwrap().version,
        "3.4"
    );
    let o = GamespyOptions {
        version_key: "missing",
        ..GamespyOptions::default()
    };
    assert_eq!(
        gs::parse_status(&f, &addr(7778), 1, &o).unwrap().version,
        "436"
    );
}

#[test]
fn rules_drop_any_indexed_field() {
    let f = fields(&[
        ("hostname", "X"),
        ("team_0", "red"),
        ("queryid", "1.1"),
        ("final", ""),
    ]);
    let info = parse(&f, 7778).unwrap();
    assert_eq!(info.rules.keys().collect::<Vec<_>>(), ["hostname"]);
}

#[test]
fn huge_index_on_non_player_field_is_ignored() {
    let key = format!("team_{}", "9".repeat(30));
    let info = parse(&fields(&[("hostname", "X"), (&key, "red")]), 7778).unwrap();
    assert!(!info.rules.contains_key(&key));
}
