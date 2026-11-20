//! Consultas Valve (A2S) en el escáner contra servidores falsos locales.
//! Traducción de los tests del escáner de tests/test_valve.py.
mod support;
use jase_core::games::Family;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::protocols::valve;
use jase_core::scanner::query::*;
use std::time::Duration;
use support::fixture;
use support::net::{FakeUdp, fake_udp, fake_udp_delayed};

const T: Duration = Duration::from_millis(300);

fn info() -> Vec<u8> {
    fixture("cs16_info.bin")
}
fn info_goldsrc() -> Vec<u8> {
    fixture("cs16_info_goldsrc.bin")
}
fn player() -> Vec<u8> {
    fixture("cs16_player.bin")
}

fn challenge_packet(c: &[u8]) -> Vec<u8> {
    [valve::OOB, b"A", c].concat()
}

fn goldsrc_split(payload: &[u8], n: usize) -> Vec<Vec<u8>> {
    payload
        .chunks(payload.len().div_ceil(n))
        .enumerate()
        .map(|(i, c)| {
            [
                valve::SPLIT,
                &7i32.to_le_bytes(),
                &[((i << 4) | n) as u8],
                c,
            ]
            .concat()
        })
        .collect()
}

/// (servidores que respondieron, veces que se llamó a on_done).
async fn run(srv: &FakeUdp, timeout: Duration, retries: u32) -> (Vec<ServerInfo>, usize) {
    let mut got = Vec::new();
    let mut done = 0;
    let mut on_server = |i| got.push(i);
    let mut on_done = || done += 1;
    query_servers(
        Family::Valve,
        vec![ServerAddress::new("127.0.0.1", srv.port)],
        &QueryOptions::default(),
        timeout,
        retries,
        64,
        &mut on_server,
        Some(&mut on_done),
    )
    .await
    .unwrap();
    (got, done)
}

#[tokio::test]
async fn query_info_and_players_with_challenge() {
    let c = b"\xaa\xbb\xcc\xdd";
    let srv = fake_udp(move |d| {
        if d == valve::A2S_INFO || d == valve::build_player_query(None) {
            vec![challenge_packet(c)]
        } else if d == valve::build_info_query(Some(c)) {
            vec![info()]
        } else if d == valve::build_player_query(Some(c)) {
            goldsrc_split(&player(), 2)
        } else {
            vec![]
        }
    })
    .await;
    let (got, done) = run(&srv, T, 1).await;
    assert_eq!((got.len(), done), (1, 1));
    assert_eq!(got[0].name, "ROMANIA.FULLBOOST.RO V.I.P FREE");
    assert_eq!(got[0].players.len(), 30);
    assert_eq!(
        srv.received(),
        vec![
            valve::A2S_INFO.to_vec(),
            valve::build_info_query(Some(c)),
            valve::build_player_query(None),
            valve::build_player_query(Some(c)),
        ]
    );
}

/// HLDS que responde 0x6D y 0x49 a la vez y no contesta A2S_PLAYER.
#[tokio::test]
async fn query_direct_info_double_reply_and_no_players() {
    let srv = fake_udp(|d| {
        if d == valve::A2S_INFO {
            vec![info_goldsrc(), info()]
        } else {
            vec![]
        }
    })
    .await;
    let (got, done) = run(&srv, T, 1).await;
    assert_eq!((got.len(), done), (1, 1));
    assert_eq!(got[0].version, "1.1.2.7/Stdio"); // la 0x49 completa a la 0x6D
    assert!(got[0].players.is_empty());
    let infos = srv
        .received()
        .iter()
        .filter(|d| *d == valve::A2S_INFO)
        .count();
    assert_eq!(infos, 1); // sin reintentos de INFO
}

#[tokio::test]
async fn query_goldsrc_first_completed_by_late_source_reply() {
    let c = b"\x01\x02\x03\x04";
    let srv = fake_udp(move |d| {
        if d == valve::A2S_INFO {
            vec![info_goldsrc(), info()]
        } else if d == valve::build_player_query(None) {
            vec![challenge_packet(c)]
        } else if d == valve::build_player_query(Some(c)) {
            vec![player()]
        } else {
            vec![]
        }
    })
    .await;
    let (got, _) = run(&srv, T, 1).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].version, "1.1.2.7/Stdio"); // de la 0x49 tardía
    assert_eq!(got[0].players.len(), 30);
}

/// La 0x49 llega tarde, ya en la fase de jugadores.
#[tokio::test]
async fn query_goldsrc_completed_by_source_reply_during_players() {
    let srv = fake_udp_delayed(|d| {
        if d == valve::A2S_INFO {
            vec![
                (Duration::ZERO, info_goldsrc()),
                (Duration::from_millis(50), info()),
            ]
        } else {
            vec![]
        }
    })
    .await;
    let (got, _) = run(&srv, T, 0).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].version, "1.1.2.7/Stdio");
}

#[tokio::test]
async fn query_info_retry_counts_once() {
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let srv = fake_udp(move |d| {
        if d == valve::A2S_INFO && calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0 {
            vec![info()]
        } else {
            vec![]
        }
    })
    .await;
    let (got, done) = run(&srv, T, 1).await;
    assert_eq!((got.len(), done), (1, 1));
    let infos = srv
        .received()
        .iter()
        .filter(|d| *d == valve::A2S_INFO)
        .count();
    assert_eq!(infos, 2);
}

#[tokio::test]
async fn query_silent_server() {
    let srv = fake_udp(|_| vec![]).await;
    let (got, done) = run(&srv, T, 1).await;
    assert_eq!((got.len(), done), (0, 1));
    // 1 intento + 1 reintento
    assert_eq!(srv.received(), vec![valve::A2S_INFO.to_vec(); 2]);
}

#[tokio::test]
async fn query_malformed_info() {
    let srv = fake_udp(|d| {
        if d == valve::A2S_INFO {
            vec![[valve::OOB, b"I\x30x"].concat()]
        } else {
            vec![]
        }
    })
    .await;
    let (got, done) = run(&srv, T, 1).await;
    assert_eq!((got.len(), done), (0, 1));
}

#[tokio::test]
async fn endless_challenges_give_up() {
    let srv = fake_udp(|_| vec![challenge_packet(b"1234")]).await;
    let (got, done) = run(&srv, Duration::from_millis(150), 1).await;
    assert_eq!((got.len(), done), (0, 1));
    // Por intento: la consulta y 2 reenvíos con challenge; nunca A2S_PLAYER.
    let with_challenge = valve::build_info_query(Some(b"1234"));
    let attempt = [
        valve::A2S_INFO.to_vec(),
        with_challenge.clone(),
        with_challenge,
    ];
    assert_eq!(srv.received(), [&attempt[..], &attempt[..]].concat());
}

/// Challenge a los 200 ms; INFO final 50 ms tras el reenvío: el ping no suma el challenge.
#[tokio::test]
async fn ping_measured_on_final_exchange() {
    let srv = fake_udp_delayed(|d| {
        if d == valve::A2S_INFO {
            vec![(Duration::from_millis(200), challenge_packet(b"1234"))]
        } else if d == valve::build_info_query(Some(b"1234")) {
            vec![(Duration::from_millis(50), info())]
        } else {
            vec![]
        }
    })
    .await;
    let (got, _) = run(&srv, Duration::from_millis(500), 0).await;
    assert_eq!(got.len(), 1);
    assert!((40..180).contains(&got[0].ping_ms), "{}", got[0].ping_ms);
}
