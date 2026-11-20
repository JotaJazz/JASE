//! Port de tests/test_valve.py (los del escáner y el lanzamiento quedan para la fase 2).
mod support;

use jase_core::models::ServerAddress;
use jase_core::protocols::valve::{self, PacketAssembler};
use std::time::Instant;
use support::fixture;

fn info() -> Vec<u8> {
    fixture("cs16_info.bin")
}
fn info_goldsrc() -> Vec<u8> {
    fixture("cs16_info_goldsrc.bin")
}
fn player_challenge() -> Vec<u8> {
    fixture("cs16_player_challenge.bin")
}
fn player() -> Vec<u8> {
    fixture("cs16_player.bin")
}
fn addr() -> ServerAddress {
    ServerAddress::new("139.162.153.162", 27015)
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn master_page(addrs: &[ServerAddress], end: bool) -> Vec<u8> {
    let mut out = valve::MASTER_HEADER.to_vec();
    for a in addrs {
        out.extend(a.host.split('.').map(|p| p.parse::<u8>().unwrap()));
        out.extend(a.port.to_be_bytes());
    }
    if end {
        out.extend([0u8; 6]);
    }
    out
}

// --- master ---

#[test]
fn build_master_query_first_page() {
    assert_eq!(
        valve::build_master_query(None, valve::DEFAULT_FILTER),
        b"\x31\xff0.0.0.0:0\x00\\appid\\10\x00"
    );
}

#[test]
fn build_master_query_seed_and_filter() {
    let q = valve::build_master_query(
        Some(&ServerAddress::new("1.2.3.4", 27015)),
        "\\gamedir\\cstrike",
    );
    assert_eq!(q, b"\x31\xff1.2.3.4:27015\x00\\gamedir\\cstrike\x00");
}

#[test]
fn parse_master_page_continues_from_last() {
    let (a, b) = (
        ServerAddress::new("1.2.3.4", 27015),
        ServerAddress::new("5.6.7.8", 27016),
    );
    let (servers, seed) =
        valve::parse_master_response(&master_page(&[a.clone(), b.clone()], false)).unwrap();
    assert_eq!(servers, [a, b.clone()]);
    assert_eq!(seed, Some(b));
}

#[test]
fn parse_master_page_end_marker() {
    let a = ServerAddress::new("1.2.3.4", 27015);
    let (servers, seed) =
        valve::parse_master_response(&master_page(std::slice::from_ref(&a), true)).unwrap();
    assert_eq!(servers, [a]);
    assert_eq!(seed, None);
}

#[test]
fn parse_master_empty_page_is_end() {
    assert_eq!(
        valve::parse_master_response(valve::MASTER_HEADER).unwrap(),
        (vec![], None)
    );
}

#[test]
fn parse_master_bad_header() {
    assert!(valve::parse_master_response(b"\xff\xff\xff\xffgetserversResponse").is_err());
}

// --- A2S_INFO ---

#[test]
fn parse_info_source_format_real() {
    let i = valve::parse_info(&info(), &addr(), 42, "").unwrap();
    assert_eq!(i.name, "ROMANIA.FULLBOOST.RO V.I.P FREE");
    assert_eq!(i.map, "de_dust2");
    assert_eq!(i.gametype, "Hosted by STRONG-HOST.DE");
    assert_eq!((i.num_players, i.max_players), (30, 32));
    assert!(!i.password);
    assert_eq!(i.version, "1.1.2.7/Stdio");
    assert_eq!(i.ping_ms, 42);
    assert_eq!(
        (i.address.clone(), i.connect_address.clone()),
        (addr(), addr())
    );
    assert_eq!(i.rules["gamedir"], "cstrike");
    assert_eq!(i.rules["appid"], "10");
}

#[test]
fn parse_info_goldsrc_format_real() {
    let i = valve::parse_info(
        &info_goldsrc(),
        &ServerAddress::new("51.38.60.53", 27015),
        5,
        "",
    )
    .unwrap();
    assert!(i.name.starts_with("EMIRATES.KiNGS \u{4e97} GAMEAREA"));
    assert_eq!(i.map, "de_dust2");
    assert_eq!(i.gametype, "Counter-Strike");
    assert_eq!((i.num_players, i.max_players), (31, 32));
    assert!(!i.password);
    assert_eq!(i.rules["gamedir"], "cstrike");
}

fn source_info(name: &[u8], password: u8, players: u8, bots: u8) -> Vec<u8> {
    cat(&[
        valve::OOB,
        b"I\x30",
        name,
        b"\0de_nuke\0cstrike\0Counter-Strike\0",
        &10i16.to_le_bytes(),
        &[players, 16, bots],
        b"dw",
        &[password, 0],
        b"1.1.2.7\0",
    ])
}

#[test]
fn parse_info_password_and_bots() {
    let i = valve::parse_info(&source_info(b"Srv", 1, 3, 1), &addr(), 1, "").unwrap();
    assert!(i.password);
    assert_eq!((i.num_players, i.max_players), (3, 16));
    assert_eq!(i.rules["bots"], "1");
    assert_eq!((i.bots, i.humans()), (1, 2));
}

#[test]
fn parse_info_gametype_falls_back_to_gamedir() {
    let data = cat(&[
        valve::OOB,
        b"I\x30Srv\0de_nuke\0cstrike\0\0",
        &10i16.to_le_bytes(),
        &[0, 16, 0],
        b"dl\x00\x001.0\0",
    ]);
    assert_eq!(
        valve::parse_info(&data, &addr(), 1, "").unwrap().gametype,
        "cstrike"
    );
}

#[test]
fn parse_info_latin1_fallback() {
    assert_eq!(
        valve::parse_info(&source_info(b"Caf\xe9", 1, 3, 1), &addr(), 1, "")
            .unwrap()
            .name,
        "Caf\u{e9}"
    );
}

#[test]
fn parse_info_malformed() {
    let full = info();
    let bad: [Vec<u8>; 5] = [
        vec![],
        cat(&[valve::OOB, b"I"]),
        cat(&[valve::OOB, b"I\x30abc"]),
        cat(&[valve::OOB, b"Z\x00"]),
        full[..20].to_vec(),
    ];
    for b in bad {
        assert!(valve::parse_info(&b, &addr(), 1, "").is_err(), "{b:?}");
    }
}

#[test]
fn parse_info_expected_gamedir() {
    assert!(valve::parse_info(&info(), &addr(), 1, "CSTRIKE").is_ok());
    assert!(valve::parse_info(&info(), &addr(), 1, "tf").is_err());
}

#[test]
fn info_challenge_packet() {
    assert_eq!(valve::A2S_INFO, b"\xff\xff\xff\xffTSource Engine Query\x00");
    let pc = player_challenge();
    assert_eq!(
        valve::challenge_of(&pc).map(|c| c.to_vec()),
        Some(pc[5..9].to_vec())
    );
    assert_eq!(valve::challenge_of(&info()), None);
    assert_eq!(
        valve::build_info_query(Some(b"\x01\x02\x03\x04")),
        cat(&[valve::A2S_INFO, b"\x01\x02\x03\x04"])
    );
    assert_eq!(
        valve::build_player_query(None),
        b"\xff\xff\xff\xffU\xff\xff\xff\xff"
    );
}

// --- A2S_PLAYER ---

#[test]
fn parse_players_real() {
    let players = valve::parse_players(&player()).unwrap();
    assert_eq!(players.len(), 30);
    assert_eq!(
        (players[0].name.as_str(), players[0].score),
        ("IUBESC CS", 0)
    );
    assert_eq!(
        (players[1].name.as_str(), players[1].score),
        ("\u{0274}\u{1d0f}\u{1d0f}\u{0299}\u{026a}\u{1d07}", 14)
    );
    assert_eq!((players[2].name.as_str(), players[2].score), ("rip", 19));
    assert!(players.iter().all(|p| p.ping == 0));
}

#[test]
fn parse_players_truncated() {
    assert!(valve::parse_players(&player()[..40]).is_err());
    assert!(valve::parse_players(&info()).is_err());
}

// --- multipaquete ---

fn chunks(payload: &[u8], n: usize) -> Vec<&[u8]> {
    let size = payload.len().div_ceil(n);
    (0..n)
        .map(|i| &payload[(i * size).min(payload.len())..((i + 1) * size).min(payload.len())])
        .collect()
}

fn goldsrc_split(payload: &[u8], n: usize, req_id: i32, order: Option<&[usize]>) -> Vec<Vec<u8>> {
    let pkts: Vec<Vec<u8>> = chunks(payload, n)
        .iter()
        .enumerate()
        .map(|(i, c)| {
            cat(&[
                valve::SPLIT,
                &req_id.to_le_bytes(),
                &[((i << 4) | n) as u8],
                c,
            ])
        })
        .collect();
    match order {
        Some(o) => o.iter().map(|&i| pkts[i].clone()).collect(),
        None => pkts,
    }
}

fn source_split(payload: &[u8], n: usize, req_id: i32) -> Vec<Vec<u8>> {
    chunks(payload, n)
        .iter()
        .enumerate()
        .map(|(i, c)| {
            cat(&[
                valve::SPLIT,
                &req_id.to_le_bytes(),
                &[n as u8, i as u8],
                &1248i16.to_le_bytes(),
                c,
            ])
        })
        .collect()
}

#[test]
fn assembler_single_packet() {
    assert_eq!(PacketAssembler::new().feed(&info()).unwrap(), Some(info()));
}

#[test]
fn assembler_goldsrc_split_out_of_order() {
    let mut asm = PacketAssembler::new();
    let p = player();
    let pkts = goldsrc_split(&p, 3, 7, Some(&[2, 0, 1]));
    assert_eq!(asm.feed(&pkts[0]).unwrap(), None);
    assert_eq!(asm.feed(&pkts[1]).unwrap(), None);
    assert_eq!(asm.feed(&pkts[2]).unwrap(), Some(p.clone()));
    assert_eq!(valve::parse_players(&p).unwrap().len(), 30);
}

#[test]
fn assembler_source_split() {
    let mut asm = PacketAssembler::new();
    let p = player();
    let pkts = source_split(&p, 2, 9);
    assert_eq!(asm.feed(&pkts[1]).unwrap(), None);
    assert_eq!(asm.feed(&pkts[0]).unwrap(), Some(p));
}

#[test]
fn assembler_compressed_source_split_rejected() {
    let pkt = cat(&[
        valve::SPLIT,
        &(-0x7FFF_FFFFi32).to_le_bytes(),
        &[1, 0],
        &1248i16.to_le_bytes(),
        &[0u8; 8],
        &info(),
    ]);
    assert!(PacketAssembler::new().feed(&pkt).is_err());
}

/// Generador determinista (sin dependencia de `rand`).
struct Lcg(u64);
impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

#[test]
fn assembler_flood_is_fast_and_bounded() {
    let mut asm = PacketAssembler::new();
    let mut rng = Lcg(1);
    let start = Instant::now();
    for _ in 0..20000 {
        let idx = 1 + rng.below(14);
        let pkt = cat(&[
            valve::SPLIT,
            &7i32.to_le_bytes(),
            &[((idx << 4) | 15) as u8],
            &[b'x'; 1200],
        ]);
        if let Ok(r) = asm.feed(&pkt) {
            assert_eq!(r, None);
        }
    }
    assert!(start.elapsed().as_secs_f64() < 0.5);
    assert!(asm.held_parts() <= 2 * valve::MAX_SPLIT_PACKETS);
}

#[test]
fn assembler_duplicates_overwrite() {
    let mut asm = PacketAssembler::new();
    let p = player();
    let pkts = goldsrc_split(&p, 3, 7, None);
    for _ in 0..100 {
        assert_eq!(asm.feed(&pkts[1]).unwrap(), None);
    }
    assert!(asm.held_parts() <= 2);
    assert_eq!(asm.feed(&pkts[0]).unwrap(), None);
    assert_eq!(asm.feed(&pkts[2]).unwrap(), Some(p));
}

#[test]
fn assembler_rejects_too_many_parts() {
    let pkt = cat(&[
        valve::SPLIT,
        &9i32.to_le_bytes(),
        &[200, 1],
        &1248i16.to_le_bytes(),
        &[b'x'; 10],
    ]);
    assert!(PacketAssembler::new().feed(&pkt).is_err());
}

#[test]
fn assembler_new_request_id_resets() {
    let mut asm = PacketAssembler::new();
    let (p, i) = (player(), info());
    let old = goldsrc_split(&p, 2, 1, None);
    let new = goldsrc_split(&i, 2, 2, None);
    assert_eq!(asm.feed(&old[0]).unwrap(), None);
    assert_eq!(asm.feed(&new[1]).unwrap(), None);
    assert_eq!(asm.feed(&old[1]).unwrap(), None);
    assert_eq!(asm.feed(&old[0]).unwrap(), Some(p));
    assert!(asm.held_parts() <= 4);
}

#[test]
fn assembler_fuzz_only_protocol_error() {
    let mut rng = Lcg(1234);
    let p = player();
    let mut seeds = vec![info(), info_goldsrc(), p.clone(), player_challenge()];
    seeds.extend(goldsrc_split(&p, 3, 7, None));
    seeds.extend(source_split(&p, 2, 9));
    for _ in 0..4000 {
        let mut data = seeds[rng.below(seeds.len())].clone();
        for _ in 0..1 + rng.below(5) {
            match rng.below(3) {
                0 if !data.is_empty() => {
                    let i = rng.below(data.len());
                    data[i] = rng.below(256) as u8;
                }
                1 if !data.is_empty() => {
                    let i = rng.below(data.len());
                    data.truncate(i);
                }
                _ => {
                    let i = rng.below(data.len() + 1);
                    data.insert(i, rng.below(256) as u8);
                }
            }
        }
        if let Ok(Some(msg)) = PacketAssembler::new().feed(&data)
            && msg.len() >= 5
        {
            match msg[4] {
                valve::INFO_SOURCE | valve::INFO_GOLDSRC => {
                    drop(valve::parse_info(&msg, &addr(), 1, ""))
                }
                valve::PLAYER_RESPONSE => drop(valve::parse_players(&msg)),
                _ => {}
            }
        }
    }
}

#[test]
fn assembler_garbage() {
    assert!(
        PacketAssembler::new()
            .feed(b"\x01\x02\x03\x04\x05")
            .is_err()
    );
}

#[test]
fn parse_info_source_edf_game_port() {
    let body = cat(&[
        b"\x11",
        b"Srv\0",
        b"c1m1\0",
        b"left4dead2\0",
        b"L4D2\0",
        &550u16.to_le_bytes(),
        &[1, 4, 0],
        b"d",
        b"l",
        b"\x00\x01",
        b"2.2.4.3\0",
        b"\x80",
        &27016u16.to_le_bytes(),
    ]);
    let i = valve::parse_info(
        &cat(&[valve::OOB, b"\x49", &body]),
        &ServerAddress::new("1.2.3.4", 27015),
        5,
        "",
    )
    .unwrap();
    assert_eq!(i.address, ServerAddress::new("1.2.3.4", 27015));
    assert_eq!(i.connect_address, ServerAddress::new("1.2.3.4", 27016));
}
