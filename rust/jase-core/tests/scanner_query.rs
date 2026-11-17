//! Consultas en paralelo por el socket compartido contra servidores falsos locales.
//! Traducción de tests/test_scanner.py, test_quake2.py, test_ut2004.py y test_zandronum.py.
mod support;
use jase_core::games::{Family, GAMES, game};
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::protocols::{gamespy, huffman, quake2, quake3, ut2004, zandronum};
use jase_core::scanner::query::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use support::fixture;
use support::net::{fake_udp, fake_udp_delayed};
use tokio::net::UdpSocket;

const T: Duration = Duration::from_millis(300);
const LO: &str = "127.0.0.1";

const STATUS: &[u8] = b"\xff\xff\xff\xffstatusResponse\n\
\\sv_hostname\\^1Jimmy^7 TDM\\mapname\\mp_carentan\\g_gametype\\tdm\
\\sv_maxclients\\20\\pswrd\\1\\shortversion\\1.3\n\
24 41 \"Sgt.^2Jimmy\"\n0 999 \"Kowalski \\\"K\\\"\"\n";
const P1: &[u8] =
    b"\\hostname\\Jimmy UT\\hostport\\7777\\mapname\\DM-Deck16][\\gametype\\DeathMatchPlus\
\\numplayers\\1\\maxplayers\\16\\password\\True\\gamever\\469\\queryid\\7.1";
const P2: &[u8] = b"\\player_0\\Jimmy\\frags_0\\12\\ping_0\\40\\queryid\\7.2\\final\\";

fn addr(host: &str, port: u16) -> ServerAddress {
    ServerAddress::new(host, port)
}

/// Puerto local sin nadie escuchando.
fn dead_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn q3_status(d: &[u8]) -> Vec<Vec<u8>> {
    if d == quake3::GETSTATUS {
        vec![STATUS.to_vec()]
    } else {
        vec![]
    }
}

fn gamespy_status(d: &[u8]) -> Vec<Vec<u8>> {
    if d == gamespy::STATUS_QUERY {
        vec![P2.to_vec(), P1.to_vec()]
    } else {
        vec![]
    }
}

fn ignoring() -> Vec<u8> {
    let raw = [zandronum::SERVER_IGNORING.to_le_bytes(), 0i32.to_le_bytes()].concat();
    huffman::encode_raw(&raw)
}

fn fast_ignore() -> QueryOptions {
    QueryOptions {
        timing: QueryTiming {
            zandronum_ignore_wait: Duration::from_millis(10),
        },
        ..QueryOptions::default()
    }
}

/// (servidores que respondieron, veces que se llamó a on_done).
async fn run_with(
    family: Family,
    addrs: Vec<ServerAddress>,
    opts: &QueryOptions,
    retries: u32,
) -> (Vec<ServerInfo>, usize) {
    let mut got = Vec::new();
    let mut done = 0;
    let mut on_server = |i| got.push(i);
    let mut on_done = || done += 1;
    query_servers(
        family,
        addrs,
        opts,
        T,
        retries,
        64,
        &mut on_server,
        Some(&mut on_done),
    )
    .await
    .unwrap();
    (got, done)
}

async fn run(family: Family, addrs: Vec<ServerAddress>, retries: u32) -> Vec<ServerInfo> {
    run_with(family, addrs, &QueryOptions::default(), retries)
        .await
        .0
}

#[tokio::test]
async fn query_servers_quake3() {
    let good = fake_udp(q3_status).await;
    let bad = fake_udp(|_| vec![b"garbage".to_vec()]).await;
    let addrs = vec![
        addr(LO, good.port),
        addr(LO, bad.port),
        addr(LO, dead_udp_port()),
    ];
    let retries = 1;
    let start = Instant::now();
    let got = run(Family::Quake3, addrs, retries).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].name, "Jimmy TDM");
    assert!(start.elapsed() < T * (retries + 1) + Duration::from_secs(1));
}

#[tokio::test]
async fn same_server_under_two_names_both_answer() {
    // Demora para que las dos consultas se solapen siempre.
    let srv = fake_udp_delayed(|d| {
        q3_status(d)
            .into_iter()
            .map(|p| (Duration::from_millis(100), p))
            .collect()
    })
    .await;
    let addrs = vec![addr(LO, srv.port), addr("localhost", srv.port)];
    let got = run(Family::Quake3, addrs, 0).await;
    let mut hosts: Vec<_> = got.iter().map(|i| i.address.host.as_str()).collect();
    hosts.sort();
    assert_eq!(hosts, ["127.0.0.1", "localhost"]);
}

#[tokio::test]
async fn query_servers_gamespy_multipacket() {
    let srv = fake_udp(gamespy_status).await;
    let got = run(Family::Gamespy, vec![addr(LO, srv.port)], 1).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].players, [Player::new("Jimmy", 12, 40)]);
    assert_eq!(got[0].name, "Jimmy UT");
}

#[tokio::test]
async fn query_servers_survives_unexpected_parse_error() {
    let good = fake_udp(gamespy_status).await;
    let bad = fake_udp(|d| {
        if d == gamespy::STATUS_QUERY {
            vec![
                P1.to_vec(),
                b"\\player_007\\x\\queryid\\7.2\\final\\".to_vec(),
            ]
        } else {
            vec![]
        }
    })
    .await;
    let addrs = vec![addr(LO, bad.port), addr(LO, good.port)];
    let got = run(Family::Gamespy, addrs, 1).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].address.port, good.port);
}

#[tokio::test]
async fn query_one_returns_none_for_undecodable_host() {
    let (got, done) = run_with(
        Family::Quake3,
        vec![addr("a..b", 28960)],
        &QueryOptions::default(),
        0,
    )
    .await;
    assert!(got.is_empty());
    assert_eq!(done, 1);
}

#[tokio::test]
async fn query_servers_ignores_undecodable_host() {
    let good = fake_udp(q3_status).await;
    let addrs = vec![addr("a..b", 28960), addr(LO, good.port)];
    let got = run(Family::Quake3, addrs, 0).await;
    let names: Vec<_> = got.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Jimmy TDM"]);
}

#[tokio::test]
async fn retry_ping_measured_from_first_send() {
    // Calla al primer intento y responde tarde al segundo.
    let count = AtomicUsize::new(0);
    let srv = fake_udp_delayed(move |_| {
        if count.fetch_add(1, Ordering::SeqCst) == 1 {
            vec![(Duration::from_millis(50), STATUS.to_vec())]
        } else {
            vec![]
        }
    })
    .await;
    let got = run(Family::Quake3, vec![addr(LO, srv.port)], 1).await;
    assert_eq!(got.len(), 1);
    // ~350 ms: desde el primer envío, no ~50 ms.
    assert!(got[0].ping_ms >= T.as_millis() as u32, "{}", got[0].ping_ms);
}

#[tokio::test]
async fn zandronum_query_carries_send_clock() {
    let srv = fake_udp(|_| vec![]).await;
    run(Family::Zandronum, vec![addr(LO, srv.port)], 1).await;
    let times: Vec<i32> = srv
        .received()
        .iter()
        .map(|q| {
            let body = huffman::decode(q);
            assert_eq!(body[..4], zandronum::SERVER_CHALLENGE.to_le_bytes());
            i32::from_le_bytes(body[8..12].try_into().unwrap())
        })
        .collect();
    assert_eq!(times.len(), 2);
    // Cada intento lleva la hora de su propio envío (en ms).
    let gap = times[1] - times[0];
    let t = T.as_millis() as i32;
    assert!((t..t + 200).contains(&gap), "{gap}");
}

#[tokio::test]
async fn bad_reply_gives_none_without_retry() {
    for family in [
        Family::Quake3,
        Family::Quake2,
        Family::Gamespy,
        Family::Zandronum,
    ] {
        let srv = fake_udp(|_| vec![b"garbage".to_vec()]).await;
        let got = run(family, vec![addr(LO, srv.port)], 1).await;
        assert_eq!(srv.received().len(), 1, "{family:?}");
        assert!(got.is_empty(), "{family:?}");
    }
}

#[tokio::test]
async fn gamespy_assembler_is_fresh_on_each_attempt() {
    // Cada intento recibe media respuesta: no se juntan.
    let count = AtomicUsize::new(0);
    let srv = fake_udp(move |_| {
        let half = if count.fetch_add(1, Ordering::SeqCst) == 0 {
            P1
        } else {
            P2
        };
        vec![half.to_vec()]
    })
    .await;
    let got = run(Family::Gamespy, vec![addr(LO, srv.port)], 1).await;
    assert_eq!(srv.received().len(), 2);
    assert!(got.is_empty());
}

#[tokio::test]
async fn gamespy_ping_uses_first_packet() {
    let srv = fake_udp_delayed(|_| {
        vec![
            (Duration::ZERO, P2.to_vec()),
            (Duration::from_millis(150), P1.to_vec()),
        ]
    })
    .await;
    let got = run(Family::Gamespy, vec![addr(LO, srv.port)], 0).await;
    assert_eq!(got.len(), 1);
    assert!(got[0].ping_ms < 100, "{}", got[0].ping_ms);
}

#[tokio::test]
async fn zandronum_ignoring_twice_gives_none() {
    let srv = fake_udp(|_| vec![ignoring()]).await;
    let (got, _) = run_with(
        Family::Zandronum,
        vec![addr(LO, srv.port)],
        &fast_ignore(),
        1,
    )
    .await;
    assert_eq!(srv.received().len(), 2);
    assert!(got.is_empty());
}

#[tokio::test]
async fn query_servers_calls_on_done_per_server() {
    let srv = fake_udp(q3_status).await;
    let addrs = vec![addr(LO, srv.port), addr(LO, dead_udp_port())];
    let (_, done) = run_with(Family::Quake3, addrs, &QueryOptions::default(), 0).await;
    assert_eq!(done, 2);
}

async fn query_one(game_id: &str, address: ServerAddress) -> Option<ServerInfo> {
    let g = game(game_id).unwrap();
    let opts = game_query_options(g);
    run_with(g.family, vec![address], &opts, 1).await.0.pop()
}

#[tokio::test]
async fn query_one_server() {
    let srv = fake_udp(q3_status).await;
    let ok = query_one("cod2", addr(LO, srv.port)).await.unwrap();
    assert_eq!(ok.name, "Jimmy TDM");
    assert!(ok.address.port > 0);
    assert!(query_one("cod2", addr(LO, dead_udp_port())).await.is_none());
}

#[tokio::test]
async fn query_servers_accepts_quake2_family() {
    let data = fixture("q2_status.bin");
    let srv = fake_udp(move |req| {
        assert_eq!(req, quake2::STATUS_QUERY);
        vec![data.clone()]
    })
    .await;
    let found = query_one("q2", addr(LO, srv.port)).await.unwrap();
    assert_eq!(found.map, "act2r2");
}

#[tokio::test]
async fn query_servers_ut2004_combines_packets() {
    let info = fixture("ut2004_info.bin");
    let players = fixture("ut2004_players.bin");
    let rules = [
        ut2004::RULES_QUERY,
        &ut2004::fstring("ServerVersion"),
        &ut2004::fstring("3374"),
    ]
    .concat();
    let srv = fake_udp(move |req| {
        let reply = match req {
            r if r == ut2004::INFO_QUERY => &info,
            r if r == ut2004::RULES_QUERY => &rules,
            r if r == ut2004::PLAYERS_QUERY => &players,
            r => panic!("consulta inesperada {r:?}"),
        };
        vec![reply.clone()]
    })
    .await;
    let got = query_one("ut2004", addr(LO, srv.port)).await.unwrap();
    assert_eq!(got.map, "ONS-ArcticStronghold");
    assert_eq!(got.version, "3374");
    let names: Vec<_> = got.players.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Player1"]);
    assert_eq!(got.connect_address, addr(LO, 7777));
    assert_eq!(got.address, addr(LO, srv.port));
}

#[tokio::test]
async fn query_servers_against_local_fake_server() {
    let srv = fake_udp(|_| vec![fixture("zandronum_players.bin")]).await;
    let got = run(Family::Zandronum, vec![addr(LO, srv.port)], 0).await;
    let maps: Vec<_> = got.iter().map(|i| i.map.as_str()).collect();
    assert_eq!(maps, ["DARK_27"]);
    let asked = huffman::decode(&srv.received()[0]);
    assert_eq!(asked[..4], zandronum::SERVER_CHALLENGE.to_le_bytes());
}

#[tokio::test]
async fn ignored_query_waits_and_retries() {
    let count = AtomicUsize::new(0);
    let srv = fake_udp(move |_| {
        if count.fetch_add(1, Ordering::SeqCst) == 0 {
            vec![ignoring()]
        } else {
            vec![fixture("zandronum_players.bin")]
        }
    })
    .await;
    let mut opts = fast_ignore();
    let wait = Duration::from_millis(100);
    opts.timing.zandronum_ignore_wait = wait;
    let start = Instant::now();
    let (got, _) = run_with(Family::Zandronum, vec![addr(LO, srv.port)], &opts, 0).await;
    assert!(start.elapsed() >= wait);
    assert_eq!(srv.received().len(), 2);
    let maps: Vec<_> = got.iter().map(|i| i.map.as_str()).collect();
    assert_eq!(maps, ["DARK_27"]);
}

#[tokio::test]
async fn router_survives_icmp_unreachable() {
    let srv = fake_udp(q3_status).await;
    let addrs = vec![addr(LO, dead_udp_port()), addr(LO, srv.port)];
    let got = run(Family::Quake3, addrs, 0).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].address.port, srv.port);
}

#[tokio::test]
async fn datagram_from_other_ip_is_ignored() {
    // El servidor calla; un impostor en 127.0.0.2 con el mismo puerto responde.
    let srv = fake_udp(|_| vec![]).await;
    let impostor = UdpSocket::bind(("127.0.0.2", srv.port)).await.unwrap();
    let spoof = async {
        while srv.senders().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        impostor.send_to(STATUS, srv.senders()[0]).await.unwrap();
    };
    let (got, ()) = tokio::join!(run(Family::Quake3, vec![addr(LO, srv.port)], 0), spoof);
    assert!(got.is_empty());
}

fn assert_send<T: Send>(_: T) {}

#[test]
fn query_servers_is_send() {
    let opts = QueryOptions::default();
    let mut on_server = |_| {};
    let mut on_done = || {};
    assert_send(query_servers(
        Family::Quake3,
        vec![],
        &opts,
        T,
        0,
        1,
        &mut on_server,
        Some(&mut on_done),
    ));
}

#[test]
fn game_query_options_from_game_def() {
    let d = QueryOptions::default();
    assert_eq!(
        (d.connect_port_fallback, d.version_key.as_str()),
        (-1, "shortversion")
    );
    assert_eq!(
        d.timing.zandronum_ignore_wait,
        Duration::from_secs_f64(zandronum::IGNORE_WAIT_S)
    );
    for g in GAMES {
        let o = game_query_options(g);
        let gamedir = if g.family == Family::Valve {
            g.valve_gamedir
        } else {
            g.status_gamedir
        };
        assert_eq!(o.connect_port_fallback, g.connect_port_fallback, "{}", g.id);
        assert_eq!(o.version_key, g.version_key, "{}", g.id);
        assert_eq!(o.version_aliases, g.version_aliases(), "{}", g.id);
        assert_eq!(o.exclude_mods, g.status_exclude_mods, "{}", g.id);
        assert_eq!(o.expected_gamename, g.status_gamename, "{}", g.id);
        assert_eq!(o.expected_gamedir, gamedir, "{}", g.id);
    }
}
