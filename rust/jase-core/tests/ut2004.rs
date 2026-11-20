//! Port de tests/test_ut2004.py (los de GameDef, lanzamiento y escáner quedan para la fase 2).
mod support;

use indexmap::IndexMap;
use jase_core::models::ServerAddress;
use jase_core::protocols::ProtocolError;
use jase_core::protocols::ut2004;
use support::fixture;

fn addr() -> ServerAddress {
    ServerAddress::new("1.2.3.4", 7778) // puerto de consulta
}

fn split_frames(data: &[u8]) -> Result<(Vec<Vec<u8>>, Vec<u8>), ProtocolError> {
    let (mut frames, mut pos) = (Vec::new(), 0);
    while data.len() - pos >= 4 {
        let size = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        if size > ut2004::MAX_FRAME {
            return Err(ProtocolError::Invalid(format!(
                "Mensaje del master demasiado grande: {size}"
            )));
        }
        if data.len() - pos - 4 < size {
            break;
        }
        frames.push(data[pos + 4..pos + 4 + size].to_vec());
        pos += 4 + size;
    }
    Ok((frames, data[pos..].to_vec()))
}

fn master_frames() -> Vec<Vec<u8>> {
    let (frames, rest) = split_frames(&fixture("ut2004_master_1.bin")).unwrap();
    assert!(rest.is_empty());
    frames
}

fn fs(text: &str) -> Vec<u8> {
    let mut raw: Vec<u8> = text.chars().map(|c| c as u8).collect();
    raw.push(0);
    [vec![raw.len() as u8], raw].concat()
}

fn le(v: i32) -> [u8; 4] {
    v.to_le_bytes()
}

#[test]
fn fake_hashes_are_fixed_fictitious_hex() {
    assert_eq!(ut2004::FAKE_CDKEY_HASH, "0".repeat(32));
    assert_eq!(ut2004::FAKE_RESPONSE_HASH, "0".repeat(32));
}

#[test]
fn challenge_response_exact_bytes() {
    let zeros = [&b"\x21"[..], &[b'0'; 32], b"\0"].concat();
    let body = [
        &zeros[..],
        &zeros,
        b"\x0cUT2K4CLIENT\0",
        &3369u32.to_le_bytes(),
        b"\x03",
        b"\x04int\0",
        &[0u8; 12],
        b"\0",
    ]
    .concat();
    assert_eq!(
        ut2004::build_challenge_response(),
        [&(body.len() as u32).to_le_bytes()[..], &body].concat()
    );
}

#[test]
fn list_request_has_no_filters() {
    assert_eq!(ut2004::list_request(), b"\x02\0\0\0\0\0");
}

#[test]
fn verify_request_is_framed_fstring() {
    assert_eq!(
        ut2004::verify_request(),
        ut2004::frame(&ut2004::fstring(ut2004::FAKE_RESPONSE_HASH))
    );
}

#[test]
fn compact_index_round_trip() {
    for value in [0, 1, 63, 64, 76, 417, 8191, 8192, 1_000_000, -5] {
        let enc = ut2004::compact_int(value);
        assert_eq!(
            ut2004::read_compact_int(&enc, 0).unwrap(),
            (i64::from(value), enc.len())
        );
    }
    assert_eq!(ut2004::read_compact_int(b"\x61\x06", 0).unwrap(), (417, 2));
}

#[test]
fn compact_index_too_long_or_truncated() {
    assert!(ut2004::read_compact_int(b"\x40\x80\x80\x80\x80\x01", 0).is_err());
    assert!(ut2004::read_compact_int(b"\x40", 0).is_err());
    assert!(ut2004::read_compact_int(b"", 0).is_err());
}

#[test]
fn read_fstring_latin1_utf16_and_truncated() {
    assert_eq!(
        ut2004::read_fstring(b"\x05J\xf6rg\0", 0).unwrap(),
        ("J\u{f6}rg".to_owned(), 6)
    );
    let wide: Vec<u8> = "Añ\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert_eq!(
        ut2004::read_fstring(&[&b"\x83"[..], &wide].concat(), 0).unwrap(),
        ("Añ".to_owned(), 7)
    );
    assert!(ut2004::read_fstring(b"\x09abc", 0).is_err());
}

#[test]
fn master_fixture_handshake_frames() {
    let frames = master_frames();
    assert!(!ut2004::read_fstring(&frames[0], 0).unwrap().0.is_empty());
    ut2004::check_reply(&frames[1], "APPROVED").unwrap();
    ut2004::check_reply(&frames[2], "VERIFIED").unwrap();
    assert_eq!(
        ut2004::parse_list_header(&frames[3]).unwrap() as usize,
        frames.len() - 4
    );
}

#[test]
fn master_fixture_entries() {
    let entries: Vec<_> = master_frames()[4..]
        .iter()
        .map(|f| ut2004::parse_list_entry(f).unwrap())
        .collect();
    assert!(entries.len() > 500);
    assert!(entries.iter().all(|e| e.port > 0));
    assert_eq!(entries[0], ServerAddress::new("79.24.244.55", 7780));
}

#[test]
fn check_reply_denied_raises() {
    assert!(ut2004::check_reply(&fs("DENIED"), "APPROVED").is_err());
}

#[test]
fn list_entry_truncated_raises() {
    assert!(ut2004::parse_list_entry(b"\x01\x02\x03").is_err());
}

#[test]
fn parse_info_fixture() {
    let info = ut2004::parse_info(&fixture("ut2004_info.bin"), &addr(), 42).unwrap();
    assert_eq!(info.name, "UT2004 3374p23 - Brasil - Multimode");
    assert_eq!(info.map, "ONS-ArcticStronghold");
    assert_eq!(info.gametype, "ONSOnslaughtGame");
    assert_eq!((info.num_players, info.max_players), (10, 16));
    assert_eq!(info.ping_ms, 42);
    assert!(!info.password);
    assert_eq!(info.address, addr());
    assert_eq!(info.connect_address, ServerAddress::new("1.2.3.4", 7777));
}

#[test]
fn parse_info_strips_colour_codes() {
    let data = [
        &b"\x80\0\0\0\0"[..],
        &[0u8; 4],
        b"\x01\0",
        &le(7777),
        &[0u8; 4],
        b"\x0d\x1b\xff\x01\x01Red\x1b\x01\x01\xffX\0",
        &fs("DM-Rankin"),
        &fs("xDeathMatch"),
        &le(1),
        &le(8),
        &[0u8; 8],
    ]
    .concat();
    let info = ut2004::parse_info(&data, &addr(), 1).unwrap();
    assert_eq!(
        (info.name.as_str(), info.map.as_str()),
        ("RedX", "DM-Rankin")
    );
}

#[test]
fn parse_players_names_utf8_or_latin1() {
    let data = [
        &b"\x80\0\0\0\x02"[..],
        &le(1),
        b"\x0eRetro\xc2\xa0Heaven\0",
        &[0u8; 12],
        &le(2),
        b"\x05J\xf6rg\0",
        &[0u8; 12],
    ]
    .concat();
    let names: Vec<String> = ut2004::parse_players(&data)
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Retro\u{a0}Heaven", "J\u{f6}rg"]);
}

#[test]
fn parse_info_truncated_raises() {
    let data = fixture("ut2004_info.bin");
    assert!(ut2004::parse_info(&data[..40], &addr(), 1).is_err());
    assert!(
        ut2004::parse_info(&[&b"\x80\0\0\0\x02"[..], &data[5..]].concat(), &addr(), 1).is_err()
    );
}

#[test]
fn parse_players_fixture_skips_team_scores() {
    let players = ut2004::parse_players(&fixture("ut2004_players.bin")).unwrap();
    let got: Vec<(&str, i32, i32)> = players
        .iter()
        .map(|p| (p.name.as_str(), p.score, p.ping))
        .collect();
    assert_eq!(got, [("Player1", 0, 96)]);
}

#[test]
fn parse_players_truncated_raises() {
    assert!(ut2004::parse_players(&fixture("ut2004_players.bin")[..20]).is_err());
}

#[test]
fn parse_rules_joins_repeated_keys_and_password() {
    let data = [
        &b"\x80\0\0\0\x01"[..],
        &fs("ServerVersion"),
        &fs("3369"),
        &fs("Mutator"),
        &fs("MutA"),
        &fs("Mutator"),
        &fs("MutB"),
        &fs("GamePassword"),
        &fs("True"),
    ]
    .concat();
    let mut rules = IndexMap::new();
    ut2004::parse_rules(&data, &mut rules).unwrap();
    let want: IndexMap<String, String> = [
        ("ServerVersion", "3369"),
        ("Mutator", "MutA, MutB"),
        ("GamePassword", "True"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    assert_eq!(rules, want);
    let mut info = ut2004::parse_info(&fixture("ut2004_info.bin"), &addr(), 1).unwrap();
    ut2004::apply_details(&mut info, rules, vec![]);
    assert!(info.password);
    assert_eq!(info.version, "3369");
    assert_eq!(info.rules, want);
}

// Extra.
#[test]
fn deduced_port_saturates() {
    let data = [
        &b"\x80\0\0\0\0"[..],
        &[0u8; 4],
        b"\x01\0",
        &le(0),
        &[0u8; 4],
        &fs("X"),
        &fs("M"),
        &fs("G"),
        &le(-3),
        &le(8),
    ]
    .concat();
    let info = ut2004::parse_info(&data, &ServerAddress::new("1.2.3.4", 0), 1).unwrap();
    assert_eq!(info.connect_address.port, 0);
    assert_eq!(info.num_players, 0); // negativo → 0
}

#[test]
fn wide_colour_code_counts_characters_not_utf16_units() {
    // ESC + 😀 (2 unidades UTF-16) + "ab": Python quita ESC, 😀, a y b (4 caracteres).
    let text: Vec<u16> = "\u{1b}\u{1F600}abZ\0".encode_utf16().collect();
    let raw: Vec<u8> = text.iter().flat_map(|u| u.to_le_bytes()).collect();
    let n = -(text.len() as i32);
    let data = [
        &b"\x80\0\0\0\0"[..],
        &[0u8; 4],
        b"\x01\0",
        &le(7777),
        &[0u8; 4],
        &ut2004::compact_int(n),
        &raw,
        &fs("M"),
        &fs("G"),
        &le(0),
        &le(8),
    ]
    .concat();
    assert_eq!(ut2004::parse_info(&data, &addr(), 1).unwrap().name, "Z");
}
