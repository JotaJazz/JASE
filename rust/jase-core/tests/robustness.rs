//! Ningún parser entra en pánico con bytes arbitrarios: todo error es Err(ProtocolError).
use indexmap::IndexMap;
use jase_core::models::ServerAddress;
use jase_core::protocols::gamespy::{self, GamespyOptions, StatusAssembler};
use jase_core::protocols::quake2::{self, Quake2Options};
use jase_core::protocols::quake3::{self, StatusOptions};
use jase_core::protocols::{huffman, steamweb, ut2004, valve, zandronum};
use proptest::prelude::*;

fn addr() -> ServerAddress {
    ServerAddress::new("192.0.2.1", 0)
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..2048)
}

/// Todos los parsers sobre `d`; el resultado no importa, solo que no haya pánico.
fn all_parsers(d: &[u8]) {
    let a = addr();
    let _ = quake3::parse_getservers_response(d);
    let _ = quake3::parse_status_response(d, &a, 1, &StatusOptions { version_key: "version", ..StatusOptions::default() });
    let _ = quake3::parse_status_response(d, &a, 1, &StatusOptions::default());
    let _ = quake2::parse_master_response(d);
    let _ = quake2::parse_status(d, &a, 1, &Quake2Options::default());
    let _ = gamespy::parse_master_list(d);
    let mut asm = StatusAssembler::new();
    if asm.feed(d).is_ok() {
        let _ = gamespy::parse_status(&asm.fields(), &a, 1, &GamespyOptions::default());
    }
    let _ = valve::parse_master_response(d);
    let _ = valve::parse_info(d, &a, 1, "x");
    let _ = valve::parse_players(d);
    let _ = valve::challenge_of(d);
    let _ = PacketAssemblerOnce::feed(d);
    let _ = ut2004::parse_info(d, &a, 1);
    let _ = ut2004::parse_rules(d, &mut IndexMap::new());
    let _ = ut2004::parse_players(d);
    let _ = ut2004::parse_list_entry(d);
    let _ = ut2004::parse_list_header(d);
    let _ = ut2004::check_reply(d, "APPROVED");
    let _ = huffman::decode(d);
    let _ = zandronum::parse_master_packet(d);
    let _ = zandronum::parse_response(d, &a, 1);
    let _ = steamweb::parse_response(d);
}

struct PacketAssemblerOnce;
impl PacketAssemblerOnce {
    fn feed(d: &[u8]) -> Result<Option<Vec<u8>>, jase_core::protocols::ProtocolError> {
        valve::PacketAssembler::new().feed(d)
    }
}

const PREFIXES: &[&[u8]] = &[
    b"\xff\xff\xff\xffstatusResponse\n",
    b"\xff\xff\xff\xffgetserversResponse\\",
    b"\xff\xff\xff\xffprint\n",
    b"\xff\xff\xff\xffservers ",
    b"\xff\xff\xff\xff\x66\x0a",
    b"\xff\xff\xff\xffI",
    b"\xff\xff\xff\xffm",
    b"\xff\xff\xff\xffD",
    b"\xfe\xff\xff\xff",
    b"\x80\x00\x00\x00\x00",
    b"\x80\x00\x00\x00\x01",
    b"\x80\x00\x00\x00\x02",
    b"\xff\x77\x5d\x56\x00", // Huffman sin comprimir + SERVER_ACCEPTED
    b"\xff\x06\x00\x00\x00", // Huffman sin comprimir + MSC_BEGINSERVERLISTPART
    b"{\"response\":{\"servers\":",
    b"\\queryid\\1.1\\final\\\\",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_bytes_never_panic(d in bytes()) {
        all_parsers(&d);
    }

    #[test]
    fn valid_header_then_garbage_never_panics(i in 0..PREFIXES.len(), d in bytes()) {
        all_parsers(&[PREFIXES[i], &d].concat());
    }

    #[test]
    fn valve_assembler_sequence_never_panics(pkts in prop::collection::vec(
        (any::<i32>(), any::<u8>(), any::<u8>(), prop::collection::vec(any::<u8>(), 0..64)), 0..40))
    {
        let mut asm = valve::PacketAssembler::new();
        for (id, b8, b9, rest) in pkts {
            let pkt = [&b"\xfe\xff\xff\xff"[..], &id.to_le_bytes(), &[b8, b9], &rest].concat();
            if let Ok(Some(msg)) = asm.feed(&pkt) {
                all_parsers(&msg);
            }
            prop_assert!(asm.held_parts() <= 2 * valve::MAX_SPLIT_PACKETS);
        }
    }

    #[test]
    fn gamespy_assembler_sequence_never_panics(pkts in prop::collection::vec(bytes(), 0..10)) {
        let mut asm = StatusAssembler::new();
        for p in pkts {
            let _ = asm.feed(&[&b"\\queryid\\1."[..], &p].concat());
        }
        let _ = gamespy::parse_status(&asm.fields(), &addr(), 1, &GamespyOptions::default());
    }
}
