//! `fetch_master` y `scan` contra masters y servidores falsos locales. Traducción de
//! tests/test_scanner.py, test_steamweb.py, test_valve.py, test_zandronum.py, test_quake2.py
//! y test_ut2004.py. Donde Python hace `monkeypatch`, aquí se usa un master falso o el
//! `download` inyectado de `scan_with`.
mod support;
use jase_core::games::{Family, GameDef, game};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::protocols::{gamespy, huffman, quake2, quake3, ut2004, valve, zandronum};
use jase_core::scanner::MasterFetchError;
use jase_core::scanner::scan::*;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use support::fixture;
use support::net::fake_udp;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

type Download = Result<Vec<u8>, MasterFetchError>;

fn addr(host: &str, port: u16) -> ServerAddress {
    ServerAddress::new(host, port)
}

fn dead_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn ip_port(a: &ServerAddress) -> Vec<u8> {
    let mut d: Vec<u8> = a
        .host
        .split('.')
        .map(|p| p.parse::<u8>().unwrap())
        .collect();
    d.extend(a.port.to_be_bytes());
    d
}

fn q3_packet(addrs: &[ServerAddress]) -> Vec<u8> {
    let mut d = b"\xff\xff\xff\xffgetserversResponse".to_vec();
    for a in addrs {
        d.push(b'\\');
        d.extend(ip_port(a));
    }
    d.extend(b"\\EOT\0\0\0");
    d
}

fn valve_page(addrs: &[ServerAddress]) -> Vec<u8> {
    let mut d = valve::MASTER_HEADER.to_vec();
    for a in addrs {
        d.extend(ip_port(a));
    }
    d.extend([0u8; 6]);
    d
}

fn q3_status(d: &[u8]) -> Vec<Vec<u8>> {
    if d == quake3::GETSTATUS {
        vec![STATUS.to_vec()]
    } else {
        vec![]
    }
}

/// Master Quake 3 que lista `servers` ante cualquier getservers.
fn q3_master(servers: Vec<ServerAddress>) -> impl Fn(&[u8]) -> Vec<Vec<u8>> + Send + 'static {
    move |d| {
        if d.starts_with(b"\xff\xff\xff\xffgetservers ") {
            vec![q3_packet(&servers)]
        } else {
            vec![]
        }
    }
}

fn cod2_118() -> GameDef {
    GameDef {
        protocols: &[118],
        ..*game("cod2").unwrap()
    }
}

/// CS de prueba de test_valve.py: appid y gamedir, sin filtro propio ni API web.
fn cs() -> GameDef {
    GameDef {
        id: "cs16t",
        masters: &[],
        valve_master_filter: "",
        steam_webapi_optional: false,
        ..*game("cs16").unwrap()
    }
}

fn opts() -> ScanOptions {
    ScanOptions {
        master_timeout: T,
        query_timeout: T,
        ..ScanOptions::default()
    }
}

fn masters(list: &[(&str, u16)]) -> Vec<(String, u16)> {
    list.iter().map(|&(h, p)| (h.to_owned(), p)).collect()
}

#[derive(Default)]
struct Events {
    servers: Vec<ServerInfo>,
    status: Vec<String>,
    progress: Vec<(usize, usize)>,
}

async fn run_with<F, Fut>(
    game: &GameDef,
    list: &[(&str, u16)],
    opts: &ScanOptions,
    download: F,
) -> (ScanResult, Events)
where
    F: Fn(String) -> Fut + Sync,
    Fut: Future<Output = Download> + Send,
{
    let mut ev = Events::default();
    let mut on_event = |e| match e {
        ScanEvent::Server(i) => ev.servers.push(i),
        ScanEvent::Status(s) => ev.status.push(s),
        ScanEvent::Progress { done, total } => ev.progress.push((done, total)),
    };
    let result = scan_with(game, &masters(list), opts, &mut on_event, download).await;
    (result, ev)
}

async fn run(game: &GameDef, list: &[(&str, u16)], opts: &ScanOptions) -> (ScanResult, Events) {
    let mut ev = Events::default();
    let mut on_event = |e| match e {
        ScanEvent::Server(i) => ev.servers.push(i),
        ScanEvent::Status(s) => ev.status.push(s),
        ScanEvent::Progress { done, total } => ev.progress.push((done, total)),
    };
    let result = scan(game, &masters(list), opts, &mut on_event).await;
    (result, ev)
}

fn kinds(r: &ScanResult) -> Vec<(String, &'static str)> {
    r.errors
        .iter()
        .map(|e| (e.master.clone(), e.kind))
        .collect()
}

// --- despacho de fetch_master ---------------------------------------------------

#[tokio::test]
async fn scan_sends_master_gamename_to_quake3_master() {
    let a = addr("1.2.3.4", 26000);
    let g = GameDef {
        protocols: &[3],
        master_gamename: "Xonotic",
        ..*game("oa").unwrap()
    };
    let a2 = a.clone();
    let srv = fake_udp(move |d| {
        if d == quake3::build_getservers(3, "Xonotic") {
            vec![q3_packet(std::slice::from_ref(&a2))]
        } else {
            vec![]
        }
    })
    .await;
    let found = fetch_master(&g, LO, srv.port, T, "", false).await.unwrap();
    assert_eq!(found, vec![a]);
    assert_eq!(
        srv.received(),
        vec![b"\xff\xff\xff\xffgetservers Xonotic 3 full empty".to_vec()]
    );
}

#[tokio::test]
async fn fetch_master_dispatches_to_quake2() {
    let srv = fake_udp(|_| vec![]).await;
    let _ = fetch_master(game("q2").unwrap(), LO, srv.port, T, "", false).await;
    assert_eq!(srv.received(), vec![quake2::MASTER_QUERY.to_vec()]);
}

#[tokio::test]
async fn fetch_master_dispatches_to_ut2004() {
    let l = TcpListener::bind((LO, 0)).await.unwrap();
    let port = l.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        // Challenge; el cliente contesta con un frame y se le rechaza.
        s.write_all(&ut2004::frame(b"\x0a111111111\0"))
            .await
            .unwrap();
        let mut n = [0u8; 4];
        s.read_exact(&mut n).await.unwrap();
        let mut body = vec![0u8; u32::from_le_bytes(n) as usize];
        s.read_exact(&mut body).await.unwrap();
        s.write_all(&ut2004::frame(b"\x07DENIED\0")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    });
    let got = fetch_master(game("ut2004").unwrap(), LO, port, T, "", false).await;
    task.await.unwrap();
    assert_eq!(got.unwrap_err().kind(), "protocol");
}

#[tokio::test]
async fn fetch_master_dispatch_uses_appid() {
    let srv = fake_udp(|_| vec![valve_page(&[])]).await;
    let got = fetch_master(&cs(), LO, srv.port, T, "", false).await;
    assert_eq!(got, Ok(vec![]));
    assert_eq!(
        srv.received(),
        vec![valve::build_master_query(None, "\\appid\\10")]
    );
    let no_appid = GameDef {
        valve_appid: 0,
        ..cs()
    };
    let err = fetch_master(&no_appid, "h", 1, T, "", false).await;
    assert_eq!(err.unwrap_err().kind(), "protocol");
}

#[tokio::test]
async fn fetch_master_dispatch_uses_custom_filter() {
    let g = GameDef {
        valve_appid: 0,
        valve_master_filter: "\\gamedir\\cstrike",
        ..cs()
    };
    let srv = fake_udp(|_| vec![valve_page(&[])]).await;
    fetch_master(&g, LO, srv.port, T, "", false).await.unwrap();
    assert_eq!(
        srv.received(),
        vec![valve::build_master_query(None, "\\gamedir\\cstrike")]
    );
}

#[tokio::test]
async fn fetch_master_requires_key_for_steam_games() {
    let err = fetch_master(
        game("l4d2").unwrap(),
        "api.steampowered.com",
        443,
        T,
        "",
        false,
    )
    .await
    .unwrap_err();
    assert_eq!(err.kind(), "missing_key");
    assert_eq!(
        err.to_string(),
        "Falta la clave de API de Steam; ponla en Ajustes → General"
    );
}

/// Escanea con la API de Steam por un `download` falso; devuelve los filtros pedidos.
async fn steam_filters(game: &GameDef, host: &str, include_empty: bool) -> Vec<String> {
    let urls = Mutex::new(Vec::new());
    let download = |url: String| {
        urls.lock().unwrap().push(url);
        async { Ok(b"{\"response\":{}}".to_vec()) }
    };
    let o = ScanOptions {
        steam_key: "K".into(),
        steam_include_empty: include_empty,
        ..opts()
    };
    run_with(game, &[(host, 443)], &o, download).await;
    let urls = urls.into_inner().unwrap();
    urls.iter()
        .map(|u| {
            let f = u.split_once("filter=").unwrap().1;
            f.split('&').next().unwrap().replace("%5C", "\\")
        })
        .collect()
}

#[tokio::test]
async fn fetch_master_steam_game_adds_empty_filter() {
    let f = steam_filters(game("l4d2").unwrap(), "api.steampowered.com", false).await;
    assert_eq!(f.len(), 1);
    assert!(f[0].starts_with("\\appid\\550\\gamedir\\left4dead2\\empty\\1\\nand"));
}

#[tokio::test]
async fn fetch_master_filter_with_and_without_empty() {
    let tf2 = game("tf2").unwrap();
    let without = steam_filters(tf2, "api.steampowered.com", false).await;
    let with = steam_filters(tf2, "api.steampowered.com", true).await;
    assert!(without[0].starts_with("\\appid\\440\\gamedir\\tf\\empty\\1\\nand"));
    assert!(with[0].starts_with("\\appid\\440\\gamedir\\tf\\nand"));
}

#[tokio::test]
async fn scan_passes_include_empty_to_fetch_master() {
    let f = steam_filters(game("tf2").unwrap(), "API.steampowered.com", true).await;
    assert!(!f[0].contains("\\empty\\1"));
}

#[tokio::test]
async fn cs16_lists_through_steam_only_with_the_official_host() {
    let cs16 = game("cs16").unwrap();
    assert!(cs16.masters.contains(&("api.steampowered.com", 443)));
    assert!(cs16.uses_steam_api() && !cs16.steam_webapi);
    let f = steam_filters(cs16, "api.steampowered.com", false).await;
    assert!(f[0].starts_with("\\appid\\10\\gamedir\\cstrike\\empty\\1\\nand"));
    // Otro host: master Valve por UDP con el filtro propio del juego.
    let srv = fake_udp(|_| vec![valve_page(&[])]).await;
    fetch_master(cs16, LO, srv.port, T, "K", false)
        .await
        .unwrap();
    assert_eq!(
        srv.received(),
        vec![valve::build_master_query(None, "\\gamedir\\cstrike")]
    );
}

// --- scan ------------------------------------------------------------------------

#[tokio::test]
async fn scan_survives_dead_master() {
    let srv = fake_udp(q3_status).await;
    let server = addr(LO, srv.port);
    let dead = addr(LO, dead_udp_port());
    let master = fake_udp(q3_master(vec![server, dead])).await;
    let (result, ev) = run(
        &cod2_118(),
        &[("nonexistent.invalid", 20710), (LO, master.port)],
        &opts(),
    )
    .await;
    assert!(result.ok());
    assert_eq!(
        (
            result.masters_ok,
            result.servers_listed,
            result.servers_responded
        ),
        (1, 2, 1)
    );
    assert_eq!(
        kinds(&result),
        vec![("nonexistent.invalid:20710".into(), "network")]
    );
    assert!(
        ev.status
            .contains(&"Master nonexistent.invalid no responde".into())
    );
    assert!(
        ev.status
            .contains(&"2 servidores en el master, consultando…".into())
    );
    let names: Vec<&str> = ev.servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Jimmy TDM"]);
}

#[tokio::test]
async fn scan_all_masters_dead() {
    let (result, ev) = run(game("cod2").unwrap(), &[(LO, dead_udp_port())], &opts()).await;
    assert_eq!(
        (
            result.masters_ok,
            result.servers_listed,
            result.servers_responded
        ),
        (0, 0, 0)
    );
    assert!(!result.ok());
    // Sin respuesta UDP: vence el plazo (o el ICMP de puerto cerrado da error de red).
    let k: Vec<&str> = result.errors.iter().map(|e| e.kind).collect();
    assert!(k == ["timeout"] || k == ["network"], "{k:?}");
    assert!(ev.status.contains(&"Ningún master respondió".into()));
    assert!(ev.servers.is_empty() && ev.progress.is_empty());
}

#[tokio::test]
async fn scan_gamespy_end_to_end() {
    let srv = fake_udp(|d| {
        if d == gamespy::STATUS_QUERY {
            vec![P1.to_vec(), P2.to_vec()]
        } else {
            vec![]
        }
    })
    .await;
    let l = TcpListener::bind((LO, 0)).await.unwrap();
    let port = l.local_addr().unwrap().port();
    let sport = srv.port;
    let task = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        s.write_all(b"\\basic\\\\secure\\ABCDEF").await.unwrap();
        let mut got: Vec<u8> = Vec::new();
        let mut buf = [0u8; 1024];
        while !got.windows(6).any(|w| w == b"\\list\\") {
            let n = s.read(&mut buf).await.unwrap();
            assert!(n > 0);
            got.extend(&buf[..n]);
        }
        let list = format!("\\ip\\127.0.0.1:{sport}\\ip\\127.0.0.1:{sport}");
        s.write_all(list.as_bytes()).await.unwrap(); // cierra sin \final\: vale lo recibido
    });
    let (_, ev) = run(game("ut99").unwrap(), &[(LO, port)], &opts()).await;
    task.await.unwrap();
    assert!(
        ev.status
            .contains(&"1 servidor en el master, consultando…".into())
    );
    let names: Vec<&str> = ev.servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Jimmy UT"]);
}

#[tokio::test]
async fn scan_cancel_closes_cleanly() {
    let master = fake_udp(|_| vec![]).await; // nunca responde
    let o = ScanOptions {
        master_timeout: Duration::from_secs(5),
        query_timeout: Duration::from_secs(5),
        ..ScanOptions::default()
    };
    let t0 = Instant::now();
    let r = tokio::time::timeout(
        Duration::from_millis(100),
        run(&cod2_118(), &[(LO, master.port)], &o),
    )
    .await;
    assert!(r.is_err()); // soltar el future cancela el escaneo
    assert!(t0.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn scan_survives_undecodable_master() {
    let (result, _) = run(&cod2_118(), &[("a..b", 20810)], &opts()).await;
    assert_eq!(kinds(&result), vec![("a..b:20810".into(), "network")]);
}

#[tokio::test]
async fn scan_cancel_during_queries() {
    let srv = fake_udp(|_| vec![]).await; // servidor mudo
    let master = fake_udp(q3_master(vec![addr(LO, srv.port)])).await;
    let o = ScanOptions {
        master_timeout: Duration::from_secs(5),
        query_timeout: Duration::from_secs(5),
        ..ScanOptions::default()
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut on_event = move |e| {
        if let ScanEvent::Status(s) = e {
            let _ = tx.send(s);
        }
    };
    let g = cod2_118();
    let list = masters(&[(LO, master.port)]);
    let t0 = Instant::now();
    let mut fut = std::pin::pin!(scan(&g, &list, &o, &mut on_event));
    tokio::select! {
        _ = &mut fut => panic!("el escaneo no debía terminar"),
        () = async {
            while rx.recv().await.as_deref() != Some("1 servidor en el master, consultando…") {}
        } => {}
    }
    // Ya en fase de consulta: sigue sin terminar y se suelta.
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut fut)
            .await
            .is_err()
    );
    assert!(t0.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn scan_reports_progress() {
    let s1 = fake_udp(q3_status).await;
    let s2 = fake_udp(q3_status).await;
    let servers = vec![
        addr(LO, s1.port),
        addr(LO, s2.port),
        addr(LO, dead_udp_port()),
    ];
    let master = fake_udp(q3_master(servers)).await;
    let (result, ev) = run(&cod2_118(), &[(LO, master.port)], &opts()).await;
    assert_eq!(result.servers_responded, 2);
    assert_eq!(ev.servers.len(), 2);
    assert_eq!(ev.progress.first(), Some(&(0, 3)));
    assert_eq!(ev.progress.last(), Some(&(3, 3)));
    let done: Vec<usize> = ev.progress.iter().map(|p| p.0).collect();
    assert_eq!(done, [0, 1, 2, 3]);
}

#[tokio::test]
async fn scan_without_on_progress_still_works() {
    // En Rust el progreso siempre llega como evento: el resultado es el mismo sin mirarlo.
    let mut on_event = |_| {};
    let list = masters(&[(LO, dead_udp_port())]);
    let result = scan(game("cod2").unwrap(), &list, &opts(), &mut on_event).await;
    assert!(!result.ok());
}

#[tokio::test]
async fn scan_valve_end_to_end() {
    let info = fixture("cs16_info.bin");
    let srv = fake_udp(move |d| {
        if d == valve::A2S_INFO {
            vec![info.clone()]
        } else {
            vec![]
        }
    })
    .await;
    let page = valve_page(&[addr(LO, srv.port)]);
    let master = fake_udp(move |_| vec![page.clone()]).await;
    let o = ScanOptions {
        master_timeout: Duration::from_secs(1),
        ..opts()
    };
    let (result, ev) = run(&cs(), &[(LO, master.port)], &o).await;
    assert_eq!((result.servers_listed, result.servers_responded), (1, 1));
    assert_eq!(ev.servers[0].map, "de_dust2");
    assert_eq!(ev.progress, [(0, 1), (1, 1)]);
}

#[tokio::test]
async fn busy_master_gets_its_own_status() {
    let busy = huffman::encode_raw(&zandronum::MASTER_IGNORING.to_le_bytes());
    let master = fake_udp(move |_| vec![busy.clone()]).await;
    let (result, ev) = run(game("zandronum").unwrap(), &[(LO, master.port)], &opts()).await;
    assert!(!result.ok());
    assert_eq!(
        ev.status[0],
        "El master 127.0.0.1 pide esperar unos segundos entre consultas"
    );
    assert_eq!(
        kinds(&result),
        vec![(format!("{LO}:{}", master.port), "busy")]
    );
    assert_eq!(master.received(), vec![zandronum::master_request()]);
}

#[tokio::test]
async fn scan_lists_server_filters_before_emitting() {
    // Tactical Ops comparte master con UT99: `lists_server` decide antes de emitir.
    let srv = fake_udp(|d| {
        if d == gamespy::STATUS_QUERY {
            vec![P1.to_vec(), P2.to_vec()]
        } else {
            vec![]
        }
    })
    .await;
    let g = game("tacops").unwrap();
    assert_eq!(g.family, Family::Gamespy);
    let l = TcpListener::bind((LO, 0)).await.unwrap();
    let port = l.local_addr().unwrap().port();
    let sport = srv.port;
    tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        s.write_all(b"\\basic\\\\secure\\ABCDEF").await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = s.read(&mut buf).await;
        let list = format!("\\ip\\127.0.0.1:{sport}\\final\\");
        let _ = s.write_all(list.as_bytes()).await;
    });
    let (result, ev) = run(g, &[(LO, port)], &opts()).await;
    assert_eq!((result.servers_listed, result.servers_responded), (1, 0));
    assert!(ev.servers.is_empty());
    assert_eq!(ev.progress, [(0, 1), (1, 1)]);
}

// --- errores de la API web de Steam en el escaneo ---------------------------------

fn l4d2_key() -> ScanOptions {
    ScanOptions {
        steam_key: "K".into(),
        ..opts()
    }
}

/// Servidor HTTP local de un solo uso que responde `status` sin cuerpo.
fn serve_status(status: &str) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/x", listener.local_addr().unwrap());
    let head = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = s.read(&mut buf);
        let _ = s.write_all(head.as_bytes());
    });
    url
}

async fn scan_steam_http(status: &str) -> (ScanResult, Events) {
    let base = serve_status(status);
    let download = |url: String| {
        let query = url.split_once('?').unwrap().1.to_owned();
        jase_core::scanner::steam::steamweb_download(format!("{base}?{query}"), T)
    };
    let g = game("l4d2").unwrap();
    run_with(g, g.masters, &l4d2_key(), download).await
}

#[tokio::test]
async fn scan_reports_steamweb_error() {
    let (result, ev) = scan_steam_http("403 Forbidden").await;
    assert_eq!(result.masters_ok, 0);
    assert!(ev.status.iter().any(|m| m.to_lowercase().contains("clave")));
    assert!(!ev.status.iter().any(|m| m.contains("no responde")));
}

#[tokio::test]
async fn scan_error_kinds_for_steam_http() {
    for (code, kind) in [
        ("401 Unauthorized", "invalid_key"),
        ("403 Forbidden", "invalid_key"),
        ("429 Too Many Requests", "rate_limited"),
    ] {
        let (result, _) = scan_steam_http(code).await;
        assert_eq!(
            kinds(&result),
            vec![("api.steampowered.com:443".into(), kind)]
        );
        assert!(!result.errors[0].message.split(' ').any(|w| w == "K"));
    }
}

#[tokio::test]
async fn scan_error_kind_missing_key() {
    let g = game("l4d2").unwrap();
    let (result, _) = run(g, g.masters, &opts()).await;
    let k: Vec<&str> = result.errors.iter().map(|e| e.kind).collect();
    assert_eq!(k, ["missing_key"]);
    assert_eq!(result.masters_ok, 0);
}

#[tokio::test]
async fn scan_error_kind_unsupported_host() {
    let (result, _) = run(
        game("l4d2").unwrap(),
        &[("evil.example.com", 443)],
        &l4d2_key(),
    )
    .await;
    let k: Vec<&str> = result.errors.iter().map(|e| e.kind).collect();
    assert_eq!(k, ["unsupported_host"]);
}

#[tokio::test]
async fn scan_reports_custom_steam_host() {
    let calls = Mutex::new(0);
    let download = |_: String| {
        *calls.lock().unwrap() += 1;
        async { Ok(Vec::new()) }
    };
    let (result, ev) = run_with(
        game("l4d2").unwrap(),
        &[("evil.example.com", 443)],
        &l4d2_key(),
        download,
    )
    .await;
    assert_eq!(result.masters_ok, 0);
    assert_eq!(*calls.lock().unwrap(), 0);
    assert!(ev.status.iter().any(|m| m.contains("api.steampowered.com")));
}

#[tokio::test]
async fn cs16_without_key_still_scans_udp_master() {
    let page = valve_page(&[addr(LO, dead_udp_port())]);
    let master = fake_udp(move |_| vec![page.clone()]).await;
    let cs16 = game("cs16").unwrap();
    let (result, _) = run(
        cs16,
        &[("api.steampowered.com", 443), (LO, master.port)],
        &opts(),
    )
    .await;
    assert!(result.ok());
    assert_eq!(result.servers_listed, 1);
    let k: Vec<&str> = result.errors.iter().map(|e| e.kind).collect();
    assert_eq!(k, ["missing_key"]);
}

fn assert_send<T: Send>(_: T) {}

#[test]
fn scan_futures_are_send() {
    let g = game("cod2").unwrap();
    let o = ScanOptions::default();
    let mut on_event = |_| {};
    assert_send(scan(g, &[], &o, &mut on_event));
    assert_send(fetch_master(g, LO, 1, T, "", false));
}

#[test]
fn scan_options_defaults() {
    let o = ScanOptions::default();
    assert_eq!(o.master_timeout, Duration::from_secs(3));
    assert_eq!(o.query_timeout, Duration::from_millis(1500));
    assert!(o.steam_key.is_empty() && !o.steam_include_empty);
}
