//! Masters UDP contra servidores falsos locales. Traducción de tests/test_scanner.py,
//! test_quake2.py y test_valve.py.
mod support;
use jase_core::models::ServerAddress;
use jase_core::protocols::{huffman, quake3, valve};
use jase_core::scanner::MasterFetchError;
use jase_core::scanner::masters_udp::*;
use std::time::{Duration, Instant};
use support::{fixture, net::fake_udp};

const T: Duration = Duration::from_millis(300);
const LO: &str = "127.0.0.1";

fn addr(host: &str, port: u16) -> ServerAddress {
    ServerAddress::new(host, port)
}

fn q3_packet(addrs: &[ServerAddress], eot: bool) -> Vec<u8> {
    let mut d = b"\xff\xff\xff\xffgetserversResponse".to_vec();
    for a in addrs {
        d.push(b'\\');
        d.extend(a.host.split('.').map(|p| p.parse::<u8>().unwrap()));
        d.extend(a.port.to_be_bytes());
    }
    if eot {
        d.extend(b"\\EOT\0\0\0");
    }
    d
}

fn valve_page(addrs: &[ServerAddress], end: bool) -> Vec<u8> {
    let mut d = valve::MASTER_HEADER.to_vec();
    for a in addrs {
        d.extend(a.host.split('.').map(|p| p.parse::<u8>().unwrap()));
        d.extend(a.port.to_be_bytes());
    }
    if end {
        d.extend([0u8; 6]);
    }
    d
}

fn zan_packet(last: bool, ip: u8) -> Vec<u8> {
    let mut d = 6i32.to_le_bytes().to_vec();
    d.push(0);
    d.extend([8, 1, ip, ip, ip, ip, 0x2A, 0x2A, 0]);
    d.push(if last { 2 } else { 7 });
    huffman::encode_raw(&d)
}

fn sorted(mut v: Vec<ServerAddress>) -> Vec<String> {
    v.sort_by_key(ToString::to_string);
    v.iter().map(ToString::to_string).collect()
}

#[tokio::test]
async fn quake3_master_multi_packet() {
    let (a1, a2, a3) = (
        addr("1.2.3.4", 28960),
        addr("5.6.7.92", 28961),
        addr("9.9.9.9", 28962),
    );
    let (p1, p2) = (vec![a1.clone(), a2.clone()], vec![a3.clone()]);
    let srv = fake_udp(move |d| {
        if d == quake3::build_getservers(118, "") {
            vec![q3_packet(&p1, false), q3_packet(&p2, true)]
        } else {
            vec![]
        }
    })
    .await;
    let got = fetch_master_quake3(LO, srv.port, &[118], T, "")
        .await
        .unwrap();
    assert_eq!(sorted(got), sorted(vec![a1, a2, a3]));
}

#[tokio::test]
async fn quake3_master_dedupes_across_protocols() {
    let (a, b) = (addr("1.2.3.4", 28960), addr("4.3.2.1", 28960));
    let (a2, ab) = (vec![a.clone()], vec![a.clone(), b.clone()]);
    let srv = fake_udp(move |d| {
        if d == quake3::build_getservers(117, "") {
            vec![q3_packet(&a2, true)]
        } else if d == quake3::build_getservers(118, "") {
            vec![q3_packet(&ab, true)]
        } else {
            vec![]
        }
    })
    .await;
    let got = fetch_master_quake3(LO, srv.port, &[117, 118], T, "")
        .await
        .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(sorted(got), sorted(vec![a, b]));
}

#[tokio::test]
async fn quake3_master_sends_gamename() {
    let a = vec![addr("1.2.3.4", 26000)];
    let srv = fake_udp(move |_| vec![q3_packet(&a, true)]).await;
    let got = fetch_master_quake3(LO, srv.port, &[3], T, "Xonotic")
        .await
        .unwrap();
    assert_eq!(got, vec![addr("1.2.3.4", 26000)]);
    assert_eq!(
        srv.received(),
        vec![b"\xff\xff\xff\xffgetservers Xonotic 3 full empty".to_vec()]
    );
}

#[tokio::test]
async fn quake3_master_real_cod2_packets_collects_all_until_eof() {
    let packets: Vec<Vec<u8>> = (1..=4)
        .map(|i| fixture(&format!("cod2_master_118_{i}.bin")))
        .collect();
    let mut expected = std::collections::HashSet::new();
    for p in &packets {
        expected.extend(quake3::parse_getservers_response(p).unwrap().0);
    }
    let srv = fake_udp(move |d| {
        if d == quake3::build_getservers(118, "") {
            packets.clone()
        } else {
            vec![]
        }
    })
    .await;
    let t0 = Instant::now();
    let got = fetch_master_quake3(LO, srv.port, &[118], Duration::from_secs(2), "")
        .await
        .unwrap();
    assert!(expected.len() > 300);
    assert_eq!(
        got.iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>(),
        expected
    );
    assert!(t0.elapsed() < Duration::from_secs(1)); // \EOF corta la espera
}

#[tokio::test]
async fn quake3_master_silent_is_timeout() {
    let srv = fake_udp(|_| vec![]).await;
    let err = fetch_master_quake3(LO, srv.port, &[3], Duration::from_millis(100), "").await;
    assert!(matches!(err, Err(MasterFetchError::Timeout)));
}

const Q2_A: &[u8] = b"\xff\xff\xff\xffservers \x01\x01\x01\x01\x6b\x86";

#[tokio::test]
async fn quake2_master_continues_after_bad_packet() {
    let srv = fake_udp(|_| vec![b"garbage".to_vec(), Q2_A.to_vec()]).await;
    let got = fetch_master_quake2(LO, srv.port, T).await.unwrap();
    assert_eq!(got, vec![addr("1.1.1.1", 27526)]);
}

#[tokio::test]
async fn quake2_master_raises_first_error_if_nothing_valid() {
    let srv = fake_udp(|_| vec![b"garbage".to_vec()]).await;
    let err = fetch_master_quake2(LO, srv.port, Duration::from_millis(50)).await;
    assert!(matches!(err, Err(MasterFetchError::Protocol(_))));
}

#[tokio::test]
async fn quake2_master_accumulates_datagrams() {
    let srv = fake_udp(|d| {
        assert_eq!(d, b"query\n\0");
        vec![
            b"\xff\xff\xff\xffservers \x01\x01\x01\x01\x6b\x86".to_vec(),
            b"\xff\xff\xff\xffservers \x02\x02\x02\x02\x6b\x86".to_vec(),
        ]
    })
    .await;
    let got = fetch_master_quake2(LO, srv.port, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(got, vec![addr("1.1.1.1", 27526), addr("2.2.2.2", 27526)]);
}

#[tokio::test]
async fn zandronum_master_stops_on_bad_packet_keeping_what_arrived() {
    let replies = vec![
        zan_packet(false, 1),
        b"garbage".to_vec(),
        zan_packet(true, 2),
    ];
    let srv = fake_udp(move |_| replies.clone()).await;
    let got = fetch_master_zandronum(LO, srv.port, T).await.unwrap();
    assert_eq!(got, vec![addr("1.1.1.1", 0x2A2A)]);
}

#[tokio::test]
async fn zandronum_master_last_packet_ends_the_loop() {
    let replies = vec![zan_packet(true, 1), zan_packet(true, 2)];
    let srv = fake_udp(move |_| replies.clone()).await;
    let got = fetch_master_zandronum(LO, srv.port, T).await.unwrap();
    assert_eq!(got, vec![addr("1.1.1.1", 0x2A2A)]);
}

#[tokio::test]
async fn zandronum_master_busy_is_raised() {
    let busy = huffman::encode_raw(&4i32.to_le_bytes());
    let srv = fake_udp(move |_| vec![busy.clone()]).await;
    let err = fetch_master_zandronum(LO, srv.port, T).await;
    assert!(matches!(err, Err(MasterFetchError::Busy)));
}

#[tokio::test]
async fn valve_paginates_until_end() {
    let first = vec![addr("10.0.0.1", 27015), addr("10.0.0.2", 27015)];
    let second = vec![addr("10.0.0.3", 27016)];
    let (f, s) = (first.clone(), second.clone());
    let srv = fake_udp(move |d| {
        assert!(d.starts_with(b"\x31\xff"));
        if d.windows(10).any(|w| w == b"0.0.0.0:0\0") {
            vec![valve_page(&f, false)]
        } else {
            assert!(d.windows(15).any(|w| w == b"10.0.0.2:27015\0"));
            vec![valve_page(&s, true)]
        }
    })
    .await;
    let got = fetch_master_valve(LO, srv.port, "\\appid\\10", Duration::from_secs(1), 20)
        .await
        .unwrap();
    assert_eq!(got, [first, second].concat());
    assert_eq!(srv.received().len(), 2);
}

#[tokio::test]
async fn valve_stops_at_max_pages() {
    let n = std::sync::atomic::AtomicU32::new(0);
    let srv = fake_udp(move |_| {
        let i = n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        vec![valve_page(&[addr(&format!("10.0.0.{i}"), 27015)], false)]
    })
    .await;
    let got = fetch_master_valve(LO, srv.port, "\\appid\\10", Duration::from_secs(1), 3)
        .await
        .unwrap();
    let hosts: Vec<_> = got.iter().map(|a| a.host.as_str()).collect();
    assert_eq!(hosts, ["10.0.0.1", "10.0.0.2", "10.0.0.3"]);
}

#[tokio::test]
async fn valve_partial_on_timeout() {
    let page = valve_page(&[addr("10.0.0.1", 27015)], false);
    let srv = fake_udp(move |d| {
        if d.windows(10).any(|w| w == b"0.0.0.0:0\0") {
            vec![page.clone()]
        } else {
            vec![]
        }
    })
    .await;
    let got = fetch_master_valve(LO, srv.port, "\\appid\\10", T, 20)
        .await
        .unwrap();
    assert_eq!(got, vec![addr("10.0.0.1", 27015)]);
}

#[tokio::test]
async fn valve_silent_raises() {
    let srv = fake_udp(|_| vec![]).await;
    let err = fetch_master_valve(LO, srv.port, "\\appid\\10", T, 20).await;
    assert!(matches!(err, Err(MasterFetchError::Timeout)));
}

#[tokio::test]
async fn valve_stops_when_seed_repeats() {
    let page = valve_page(&[addr("10.0.0.1", 27015)], false);
    let srv = fake_udp(move |_| vec![page.clone()]).await;
    let got = fetch_master_valve(LO, srv.port, "\\appid\\10", T, 20)
        .await
        .unwrap();
    assert_eq!(got, vec![addr("10.0.0.1", 27015)]);
    assert_eq!(srv.received().len(), 2);
}
