//! Búsqueda en la red local contra servidores falsos en 127.0.0.1 (una difusión real no
//! se puede probar en CI: los destinos se inyectan). Traducción de tests/test_lan.py
//! (sin Qt) y de las partes de LAN de test_games_tacops.py.
mod support;
use jase_core::games::{Family, GAMES, game};
use jase_core::lan::*;
use jase_core::models::ServerAddress;
use jase_core::protocols::{gamespy, quake2, quake3, ut2004, valve, zandronum};
use jase_core::scanner::scan::ScanEvent;
use std::net::Ipv4Addr;
use std::time::Duration;
use support::fixture;
use support::net::fake_udp;

const T: Duration = Duration::from_millis(300);
const LO: Ipv4Addr = Ipv4Addr::LOCALHOST;

fn assert_send<T: Send>(_: T) {}

/// Puerto local sin nadie escuchando.
fn dead_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn local_opts(ports: Vec<u16>) -> LanOptions {
    LanOptions {
        discover_timeout: T,
        query_timeout: T,
        targets: Some(vec![LO]),
        ports: Some(ports),
    }
}

#[test]
fn lan_ports_per_game_and_family() {
    let g = |id| game(id).unwrap();
    assert_eq!(lan_ports(g("cod2")), [28960, 28961, 28962, 28963]);
    assert_eq!(lan_ports(g("cs16")), (27015..27021).collect::<Vec<_>>());
    assert_eq!(lan_ports(g("mohaa"))[0], 12300);
    assert_eq!(GAME_PORTS.len(), 27);
    assert!(GAME_PORTS.iter().all(|(id, _)| game(id).is_some()));
    assert!(
        GAMES
            .iter()
            .all(|g| FAMILY_PORTS.iter().any(|(f, _)| *f == g.family))
    );
    assert_eq!(RANGE, 4);
}

#[test]
fn tacops_lan_port_matches_ut99() {
    assert_eq!(
        lan_ports(game("tacops").unwrap()),
        lan_ports(game("ut99").unwrap())
    );
}

#[test]
fn probes_match_scanner_queries() {
    assert_eq!(probe(Family::Quake3), quake3::GETSTATUS);
    assert_eq!(probe(Family::Quake2), quake2::STATUS_QUERY);
    assert_eq!(probe(Family::Gamespy), gamespy::STATUS_QUERY);
    assert_eq!(probe(Family::Valve), valve::A2S_INFO);
    assert_eq!(probe(Family::Ut2004), ut2004::INFO_QUERY);
    assert_eq!(probe(Family::Zandronum), zandronum::build_query(0));
}

#[tokio::test]
async fn broadcast_addresses_always_include_limited_broadcast() {
    let targets = broadcast_addresses().await;
    assert_eq!(targets[0], Ipv4Addr::BROADCAST);
    assert!(targets.iter().all(|t| t.octets()[3] == 255));
    let mut unique = targets.clone();
    unique.dedup();
    assert_eq!(unique, targets);
}

#[tokio::test]
async fn discover_finds_a_server_that_answers() {
    let fake = fake_udp(|_| vec![b"\xff\xff\xff\xffstatusResponse\n".to_vec()]).await;
    let found = discover(game("cod2").unwrap(), T, &[LO], &[fake.port])
        .await
        .unwrap();
    assert_eq!(found, [ServerAddress::new("127.0.0.1", fake.port)]);
    assert_eq!(fake.received(), [probe(Family::Quake3)]);
}

#[tokio::test]
async fn discover_ignores_closed_ports() {
    // En Windows el ICMP del puerto cerrado llega como ConnectionReset al socket.
    let fake = fake_udp(|_| vec![b"x".to_vec()]).await;
    let ports = [dead_udp_port(), fake.port, dead_udp_port()];
    let found = discover(game("cod2").unwrap(), T, &[LO], &ports)
        .await
        .unwrap();
    assert_eq!(found, [ServerAddress::new("127.0.0.1", fake.port)]);
}

#[tokio::test]
async fn discover_dedupes_and_skips_empty() {
    let twice = fake_udp(|_| vec![b"a".to_vec(), b"b".to_vec()]).await;
    let empty = fake_udp(|_| vec![Vec::new()]).await;
    let found = discover(game("q3").unwrap(), T, &[LO], &[empty.port, twice.port])
        .await
        .unwrap();
    assert_eq!(found, [ServerAddress::new("127.0.0.1", twice.port)]);
    assert_eq!(empty.received().len(), 1);
}

#[tokio::test]
async fn scan_lan_queries_the_responders() {
    let status = fixture("cod2x_status.bin");
    let fake = fake_udp(move |d| {
        if d == quake3::GETSTATUS {
            vec![status.clone()]
        } else {
            vec![]
        }
    })
    .await;
    let mut events = Vec::new();
    let result = scan_lan(
        game("cod2").unwrap(),
        &local_opts(vec![fake.port]),
        &mut |e| events.push(e),
    )
    .await
    .unwrap();
    assert_eq!(
        events[0],
        ScanEvent::Status("1 servidor en la red local, consultando…".into())
    );
    assert_eq!(events.len(), 2);
    let ScanEvent::Server(info) = &events[1] else {
        panic!("{events:?}")
    };
    assert_eq!(info.address, ServerAddress::new("127.0.0.1", fake.port));
    assert_eq!((result.servers_listed, result.servers_responded), (1, 1));
    assert_eq!(result.masters_ok, 1);
}

#[tokio::test]
async fn scan_lan_applies_the_same_split() {
    fn server(gametype: &'static str) -> impl Fn(&[u8]) -> Vec<Vec<u8>> {
        move |d| {
            if d != gamespy::STATUS_QUERY {
                return vec![];
            }
            let p = format!(
                "\\hostname\\{gametype}\\gamename\\ut\\gametype\\{gametype}\\hostport\\7777\
                 \\mapname\\TO-Scope\\numplayers\\0\\maxplayers\\8\\queryid\\1.1\\final\\"
            );
            vec![p.into_bytes()]
        }
    }
    let to = fake_udp(server("TO340")).await;
    let ut = fake_udp(server("CTFGame")).await;
    let mut events = Vec::new();
    let result = scan_lan(
        game("tacops").unwrap(),
        &local_opts(vec![to.port, ut.port]),
        &mut |e| events.push(e),
    )
    .await
    .unwrap();
    let names: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            ScanEvent::Server(i) => Some(i.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["TO340"]);
    assert_eq!((result.servers_listed, result.servers_responded), (2, 1));
}

#[test]
fn lan_and_scan_lan_futures_are_send() {
    let g = game("cod2").unwrap();
    let opts = LanOptions::default();
    assert_eq!(opts.discover_timeout, Duration::from_millis(1500));
    assert_eq!(opts.query_timeout, Duration::from_millis(1500));
    assert!(opts.targets.is_none() && opts.ports.is_none());
    assert_send(broadcast_addresses());
    assert_send(discover(g, T, &[LO], &[1]));
    let mut sink = |_: ScanEvent| {};
    assert_send(scan_lan(g, &opts, &mut sink));
}
