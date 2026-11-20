//! Port de tests/test_quake3.py.
mod support;

use jase_core::models::{Player, ServerAddress};
use jase_core::protocols::is_bot;
use jase_core::protocols::quake3::{self as q3, StatusOptions, VersionAlias};
use support::fixture;

fn addr(host: &str, port: u16) -> ServerAddress {
    ServerAddress::new(host, port)
}

fn parse(data: &[u8], a: &ServerAddress, ping: u32) -> jase_core::models::ServerInfo {
    q3::parse_status_response(data, a, ping, &StatusOptions::default()).unwrap()
}

#[test]
fn build_getservers() {
    assert_eq!(
        q3::build_getservers(118, ""),
        b"\xff\xff\xff\xffgetservers 118 full empty"
    );
}

#[test]
fn build_getservers_with_gamename() {
    assert_eq!(
        q3::build_getservers(3, "Xonotic"),
        b"\xff\xff\xff\xffgetservers Xonotic 3 full empty"
    );
}

#[test]
fn parse_getservers_fixed_stride_and_eot() {
    let mut data = b"\xff\xff\xff\xffgetserversResponse".to_vec();
    data.extend(b"\\");
    data.extend([1, 2, 3, 4]);
    data.extend(28960u16.to_be_bytes());
    data.extend(b"\\");
    data.extend([5, 6, 7, 92]);
    data.extend(28961u16.to_be_bytes());
    data.extend(b"\\EOT\x00\x00\x00");
    let (servers, eot) = q3::parse_getservers_response(&data).unwrap();
    assert_eq!(servers, [addr("1.2.3.4", 28960), addr("5.6.7.92", 28961)]);
    assert!(eot);
}

#[test]
fn parse_getservers_without_eot() {
    let data = b"\xff\xff\xff\xffgetserversResponse\\\x01\x02\x03\x04\x71\x20";
    assert_eq!(
        q3::parse_getservers_response(data).unwrap(),
        (vec![addr("1.2.3.4", 28960)], false)
    );
}

#[test]
fn parse_getservers_without_header_fails() {
    assert!(q3::parse_getservers_response(b"garbage").is_err());
}

const STATUS: &[u8] = b"\xff\xff\xff\xffstatusResponse\n\
\\sv_hostname\\^1Jimmy^7 TDM\\mapname\\mp_carentan\\g_gametype\\tdm\
\\sv_maxclients\\20\\pswrd\\1\\shortversion\\1.3\n\
24 41 \"Sgt.^2Jimmy\"\n0 999 \"Kowalski \\\"K\\\"\"\n";

#[test]
fn parse_status() {
    let a = addr("1.2.3.4", 28960);
    let info = parse(STATUS, &a, 52);
    assert_eq!(
        (
            info.name.as_str(),
            info.map.as_str(),
            info.gametype.as_str()
        ),
        ("Jimmy TDM", "mp_carentan", "tdm")
    );
    assert_eq!((info.num_players, info.max_players), (2, 20));
    assert!(info.password);
    assert_eq!(info.version, "1.3");
    assert_eq!(info.ping_ms, 52);
    assert_eq!(info.players[0], Player::new("Sgt.Jimmy", 24, 41));
    assert_eq!(info.connect_address, a);
    assert_eq!(info.rules["sv_hostname"], "^1Jimmy^7 TDM");
}

#[test]
fn parse_status_empty_server_and_needpass() {
    let data =
        b"\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\Empty\\sv_maxclients\\16\\g_needpass\\0\n";
    let info = parse(data, &addr("1.1.1.1", 1), 10);
    assert_eq!(info.num_players, 0);
    assert!(info.players.is_empty());
    assert!(!info.password);
    assert_eq!(info.version, "");
    assert_eq!(info.map, "");
}

#[test]
fn parse_status_malformed_raises() {
    let bad: [&[u8]; 3] = [
        b"",
        b"garbage",
        b"\xff\xff\xff\xffstatusResponse\n\\sv_maxclients\\abc\n",
    ];
    for b in bad {
        assert!(
            q3::parse_status_response(b, &addr("1.1.1.1", 1), 10, &StatusOptions::default())
                .is_err()
        );
    }
}

#[test]
fn strip_colors() {
    assert_eq!(q3::strip_colors("^1Red^7White^"), "RedWhite^");
}

#[test]
fn strip_colors_removes_control_characters() {
    assert_eq!(q3::strip_colors("^1Jim\x01my\tTDM\x7f\x00"), "JimmyTDM");
    assert_eq!(q3::strip_colors("^\x011Red"), "Red");
}

#[test]
fn status_names_drop_control_characters() {
    let data = b"\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\^1Jim\x01my\tCoD1\\sv_maxclients\\20\n3 50 \"\x01Sgt.\t^2Jimmy\"\n";
    let info = parse(data, &addr("1.1.1.1", 1), 10);
    assert_eq!(info.name, "JimmyCoD1");
    assert_eq!(info.players, [Player::new("Sgt.Jimmy", 3, 50)]);
}

#[test]
fn parse_real_cod2_intermediate_packet_ends_with_bare_eot() {
    let (servers, last) = q3::parse_getservers_response(&fixture("cod2_master_118_1.bin")).unwrap();
    assert_eq!(servers.len(), 111);
    assert!(!last);
    assert!(servers.iter().all(|s| s.port >= 1));
}

#[test]
fn parse_real_cod2_last_packet_ends_with_eof() {
    let (servers, last) = q3::parse_getservers_response(&fixture("cod2_master_118_4.bin")).unwrap();
    assert_eq!(servers.len(), 46);
    assert!(last);
}

#[test]
fn parse_real_cod1_single_packet() {
    let (servers, last) = q3::parse_getservers_response(&fixture("cod1_master_1.bin")).unwrap();
    assert_eq!(servers.len(), 54);
    assert!(last);
    assert!(servers[0].host.starts_with("15."));
}

#[test]
fn parse_status_keeps_raw_names() {
    let info = parse(STATUS, &addr("1.2.3.4", 28960), 52);
    assert_eq!(info.raw_name, "^1Jimmy^7 TDM");
    assert_eq!(info.players[0].raw_name, "Sgt.^2Jimmy");
}

#[test]
fn strip_colors_double_caret_codes() {
    assert_eq!(q3::strip_colors("^^7Sugus"), "Sugus");
    assert_eq!(q3::strip_colors("^^00B E N D E R"), "B E N D E R");
    assert_eq!(q3::strip_colors("^^71337"), "1337");
}

#[test]
fn parse_status_counts_bots() {
    let data = b"\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\B\\sv_maxclients\\16\n\
3 45 \"Persona\"\n0 0 \"Orbb\"\n2 999 \"bot12\"\n1 30 \"[BOT]Shadow\"\n5 999 \"Interrumpido\"\n";
    let info = parse(data, &addr("1.1.1.1", 1), 10);
    assert_eq!((info.num_players, info.bots, info.humans()), (5, 3, 2));
}

#[test]
fn is_bot_heuristic() {
    assert!(is_bot(&Player::new("Xaero", 0, 0)) && is_bot(&Player::new(" [bot]Rush", 3, 50)));
    assert!(is_bot(&Player::new("BOT581", 0, 999)));
    assert!(!is_bot(&Player::new("Jimmy", 0, 999)) && !is_bot(&Player::new("robot", 0, 40)));
}

#[test]
fn parse_status_huge_maxclients_is_bounded() {
    let mut data = b"\xff\xff\xff\xffstatusResponse\n\\sv_maxclients\\".to_vec();
    data.extend(std::iter::repeat_n(b'9', 400));
    data.extend(b"\n999999999999999 5 \"X\"\n");
    let info = parse(&data, &addr("1.1.1.1", 1), 10);
    assert_eq!(info.max_players, i32::MAX);
    assert_eq!(info.players[0].score, i32::MAX);
}

// Extra (opciones que el golden no cubre; en Python se prueban vía games.py).
#[test]
fn short_engine_version_examples() {
    assert_eq!(
        q3::short_engine_version("ioq3 1.36+u20221123/Debian linux-x86_64"),
        "ioq3 1.36"
    );
    assert_eq!(
        q3::short_engine_version("Wolf 1.41b-MP win-x86 Dec  4 2002"),
        "Wolf 1.41b"
    );
    assert_eq!(
        q3::short_engine_version("ET Legacy v2.86.2 linux-x86_64"),
        "ET Legacy v2.86.2"
    );
    assert_eq!(q3::short_engine_version("  sin  numeros "), "sin numeros");
}

#[test]
fn status_options_version_key_aliases_and_excluded_mods() {
    let data = b"\xff\xff\xff\xffstatusResponse\n\\hostname\\X\\version\\ioq3 1.36_GIT linux\\protocol\\2004\\gamename\\Q3UT4\n";
    let a = addr("1.1.1.1", 1);
    let o = StatusOptions {
        version_key: "version",
        ..StatusOptions::default()
    };
    let info = q3::parse_status_response(data, &a, 1, &o).unwrap();
    assert_eq!(
        (info.name.as_str(), info.version.as_str()),
        ("X", "ioq3 1.36")
    );
    let o = StatusOptions {
        version_key: "protocol",
        ..StatusOptions::default()
    };
    assert_eq!(
        q3::parse_status_response(data, &a, 1, &o).unwrap().version,
        "1.03"
    );
    let aliases = [VersionAlias {
        prefix: "1.03".into(),
        key: "sof2".into(),
        label: "SoF2".into(),
    }];
    let o = StatusOptions {
        version_key: "protocol",
        version_aliases: &aliases,
        ..StatusOptions::default()
    };
    assert_eq!(
        q3::parse_status_response(data, &a, 1, &o).unwrap().version,
        "sof2"
    );
    let excluded = ["q3ut4".to_string()];
    let o = StatusOptions {
        exclude_mods: &excluded,
        ..StatusOptions::default()
    };
    assert!(q3::parse_status_response(data, &a, 1, &o).is_err());
}
