//! Port de tests/test_zandronum.py (GameDef, lanzamiento, identify y escáner: fase 2).
mod support;

use jase_core::models::ServerAddress;
use jase_core::protocols::ProtocolError;
use jase_core::protocols::{huffman, zandronum as z};
use support::fixture;

fn addr() -> ServerAddress {
    ServerAddress::new("1.2.3.4", 10666)
}

fn i32_at(b: &[u8], pos: usize) -> i32 {
    i32::from_le_bytes(b[pos..pos + 4].try_into().unwrap())
}

#[test]
fn huffman_raw_and_compressed() {
    assert_eq!(huffman::decode(b"\xffhola"), b"hola");
    assert_eq!(huffman::decode(b""), b"");
    let body = huffman::decode(&fixture("zandronum_players.bin"));
    assert_eq!(i32_at(&body, 0), z::SERVER_ACCEPTED);
    assert_eq!(huffman::encode_raw(b"x"), b"\xffx");
}

#[test]
fn master_packet_lists_servers_and_end_marker() {
    let (found, last) = z::parse_master_packet(&fixture("zandronum_master.bin")).unwrap();
    assert!(last);
    assert_eq!(found.len(), 78);
    assert!(found.iter().all(|a| a.port != 0));
}

#[test]
fn master_errors_and_truncation() {
    let err = z::parse_master_packet(&huffman::encode_raw(&3i32.to_le_bytes())).unwrap_err();
    assert!(err.to_string().contains("bloqueada"));
    let truncated = [&6i32.to_le_bytes()[..], &[0], b"\x08\x02\x01"].concat();
    assert!(z::parse_master_packet(&huffman::encode_raw(&truncated)).is_err());
}

#[test]
fn parse_team_game_with_bots() {
    let info = z::parse_response(&fixture("zandronum_team.bin"), &addr(), 30).unwrap();
    assert_eq!(
        (info.name.as_str(), info.map.as_str()),
        ("[TSPG-PK] Ultimate Railgun", "BUR08")
    );
    assert_eq!(
        (info.gametype.as_str(), info.version.as_str()),
        ("teamplay", "3.3")
    );
    assert_eq!(
        (info.num_players, info.max_players, info.bots, info.humans()),
        (4, 32, 4, 0)
    );
    let names: Vec<&str> = info.players.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Pluto", "KnightMan", "Neptune", "Sunstar"]);
    assert_eq!(info.rules["iwad"], "megagame.wad");
}

#[test]
fn parse_coop_players() {
    let info = z::parse_response(&fixture("zandronum_players.bin"), &addr(), 30).unwrap();
    assert_eq!(
        (info.gametype.as_str(), info.map.as_str()),
        ("cooperative", "DARK_27")
    );
    let got: Vec<(&str, i32, i32)> = info
        .players
        .iter()
        .map(|p| (p.name.as_str(), p.score, p.ping))
        .collect();
    assert_eq!(got, [("Player", 0, 76), ("leg", 757, 45)]);
    assert_eq!(info.bots, 0);
    assert!(!info.password);
    assert_eq!(info.rules["iwad"], "doom2.wad");
}

#[test]
fn parse_rejects_bad_answers() {
    let ignoring = [z::SERVER_IGNORING.to_le_bytes(), 0i32.to_le_bytes()].concat();
    assert!(matches!(
        z::parse_response(&huffman::encode_raw(&ignoring), &addr(), 1),
        Err(ProtocolError::ServerIgnoring(_))
    ));
    let body = huffman::decode(&fixture("zandronum_team.bin"));
    assert!(z::parse_response(&huffman::encode_raw(&body[..body.len() - 5]), &addr(), 1).is_err());
}

#[test]
fn strip_doom_colors() {
    assert_eq!(z::strip_colors("\x1cGRed\x1c[Gold]Gold\x1c-x"), "RedGoldx");
    assert_eq!(z::strip_colors("a\x1c[sin cierre"), "a");
    assert_eq!(z::strip_colors("fin\x1c"), "fin");
}

#[test]
fn busy_master_is_its_own_error() {
    let r = z::parse_master_packet(&huffman::encode_raw(&z::MASTER_IGNORING.to_le_bytes()));
    assert!(matches!(r, Err(ProtocolError::MasterBusy(_))));
}

#[test]
fn player_data_fallback_when_first_pass_raises() {
    let team = fixture("zandronum_team.bin");
    let body = huffman::decode(&team);
    let info = z::parse_response(&team, &addr(), 1).unwrap();
    let find = |needle: &[u8], from: usize| {
        body[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .unwrap()
            + from
    };
    let mut pos = find(b"BUR08\0", 0) + 6 + 2;
    let count = body[pos];
    pos += 1;
    for _ in 0..count {
        pos = find(b"\0", pos) + 1;
    }
    assert_eq!(body[pos], 4); // teamplay
    let mut patched = body.clone();
    patched[pos] = 3; // deathmatch: sin byte de equipo
    let again = z::parse_response(&huffman::encode_raw(&patched), &addr(), 1).unwrap();
    let names = |i: &jase_core::models::ServerInfo| {
        i.players.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
    };
    assert_eq!(names(&again), names(&info));
}

#[test]
fn requests_are_raw_packets() {
    let q = z::build_query(0x1_2345_6789);
    assert_eq!(q[0], 0xff);
    assert_eq!(i32_at(&q, 1), z::SERVER_CHALLENGE);
    assert_eq!(
        u32::from_le_bytes(q[5..9].try_into().unwrap()),
        z::QUERY_FLAGS
    );
    assert_eq!(i32_at(&q, 9), 0x2345_6789);
    assert_eq!(
        z::master_request(),
        [
            &[0xff][..],
            &z::MASTER_CHALLENGE.to_le_bytes(),
            &z::MASTER_PROTOCOL.to_le_bytes()
        ]
        .concat()
    );
}
