//! API web de Steam en el escáner: descarga, errores HTTP y spam. Traducción de los tests
//! del escáner de tests/test_steamweb.py (sin Internet: servidor HTTP local y `download`
//! inyectado en lugar del `monkeypatch` de `_steamweb_download`).
use jase_core::games::game;
use jase_core::models::ServerAddress;
use jase_core::protocols::steamweb::OFFICIAL_HOST;
use jase_core::scanner::MasterFetchError;
use jase_core::scanner::steam::*;
use std::future::Future;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const T: Duration = Duration::from_millis(500);

type Download = Result<Vec<u8>, MasterFetchError>;

/// Servidor HTTP local de un solo uso que responde `status` sin cuerpo; devuelve su URL base.
fn serve_status(status: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/x", listener.local_addr().unwrap());
    let head = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = s.read(&mut buf);
        let _ = s.write_all(head.as_bytes());
    });
    url
}

/// `download` que manda la consulta real (con la clave) al servidor local.
fn local(
    base: String,
) -> impl Fn(String) -> std::pin::Pin<Box<dyn Future<Output = Download> + Send>> {
    move |url: String| {
        let query = url.split_once('?').map_or("", |(_, q)| q).to_owned();
        Box::pin(steamweb_download(format!("{base}?{query}"), T))
    }
}

async fn steam_err(status: &str) -> MasterFetchError {
    let download = local(serve_status(status));
    fetch_master_steamweb_with(OFFICIAL_HOST, 443, "\\appid\\550", "SECRETKEY123", download)
        .await
        .unwrap_err()
}

#[tokio::test]
async fn steamweb_http_error_message_has_no_key() {
    for (code, kind) in [
        ("401 Unauthorized", "invalid_key"),
        ("403 Forbidden", "invalid_key"),
        ("429 Too Many Requests", "rate_limited"),
    ] {
        let err = steam_err(code).await;
        assert_eq!(err.kind(), kind);
        let msg = err.to_string();
        assert!(
            !msg.contains("SECRETKEY123") && !msg.contains("steampowered"),
            "{msg}"
        );
        assert!(!msg.contains("127.0.0.1"), "{msg}");
    }
    let invalid = steam_err("403 Forbidden").await.to_string();
    assert_eq!(
        invalid,
        "La clave de API de Steam no es válida; revísala en Ajustes → General"
    );
    let limited = steam_err("429 Too Many Requests").await.to_string();
    assert_eq!(
        limited,
        "Steam limitó las consultas; prueba en unos minutos"
    );
}

#[tokio::test]
async fn steamweb_other_http_error_is_network() {
    let err = steam_err("500 Internal Server Error").await;
    assert_eq!(err, MasterFetchError::Network("HTTP 500".into()));
}

#[tokio::test]
async fn steamweb_body_read_timeout_is_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/x", listener.local_addr().unwrap());
    thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = s.read(&mut buf);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n");
        thread::sleep(Duration::from_millis(1500)); // cabeceras y luego silencio
    });
    let err = steamweb_download(url, Duration::from_millis(300))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "timeout", "{err:?}");
}

#[tokio::test]
async fn steamweb_network_error_is_oserror() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port(); // se cierra al soltarlo: conexión rechazada
    let url = format!("http://127.0.0.1:{port}/x?key=SECRETKEY123");
    let err = steamweb_download(url, T).await.unwrap_err();
    let MasterFetchError::Network(msg) = &err else {
        panic!("{err:?}");
    };
    assert!(
        !msg.contains("SECRETKEY123") && !msg.contains("127.0.0.1"),
        "{msg}"
    );
}

#[tokio::test]
async fn steamweb_custom_host_never_sends_key() {
    let calls = Mutex::new(Vec::new());
    let download = |url: String| {
        calls.lock().unwrap().push(url);
        async { Ok(Vec::new()) }
    };
    let err = fetch_master_steamweb_with(
        "evil.example.com",
        443,
        "\\appid\\550",
        "SECRETKEY123",
        download,
    )
    .await
    .unwrap_err();
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(err.kind(), "unsupported_host");
    let msg = err.to_string();
    assert!(!msg.contains("SECRETKEY123") && !msg.contains("evil.example.com"));
    assert!(msg.contains(OFFICIAL_HOST));
}

#[tokio::test]
async fn steamweb_result_from_download() {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/steamweb_l4d2.json"
    ))
    .unwrap();
    let download = |_| {
        let data = data.clone();
        async move { Ok(data) }
    };
    let got = fetch_master_steamweb_with(OFFICIAL_HOST, 443, "\\appid\\550", "K", download)
        .await
        .unwrap();
    assert_eq!(got.len(), 20);
}

// --- spam en la API (sondeo 2026-10-29, CS 1.6): IPs con 200 servidores falsos cada una
// llenan el tope de 10 000 entradas. Python baja STEAMWEB_LIMIT con monkeypatch; aquí las
// páginas llegan al tope real.

fn api_json(addrs: &[String]) -> Vec<u8> {
    let entries: Vec<String> = addrs
        .iter()
        .map(|a| format!("{{\"addr\":\"{a}\"}}"))
        .collect();
    format!("{{\"response\":{{\"servers\":[{}]}}}}", entries.join(",")).into_bytes()
}

fn spam(ip: &str, n: u16) -> Vec<String> {
    (0..n).map(|i| format!("{ip}:{}", 27000 + i)).collect()
}

/// `unquote_plus` del parámetro `filter` de la URL.
fn filter_of(url: &str) -> String {
    let q = url.split_once("filter=").unwrap().1;
    let q = q.split('&').next().unwrap();
    let mut out = Vec::new();
    let b = q.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                out.push(u8::from_str_radix(&q[i + 1..i + 3], 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

/// Cada llamada devuelve la siguiente página (la última se repite); guarda los filtros.
async fn fake_steam(pages: Vec<Vec<String>>, filter: &str) -> (Vec<ServerAddress>, Vec<String>) {
    let filters = Mutex::new(Vec::new());
    let download = |url: String| {
        let mut f = filters.lock().unwrap();
        f.push(filter_of(&url));
        let page = api_json(&pages[f.len().min(pages.len()) - 1]);
        async move { Ok(page) }
    };
    let got = fetch_master_steamweb_with(OFFICIAL_HOST, 443, filter, "K", download)
        .await
        .unwrap();
    (got, filters.into_inner().unwrap())
}

#[tokio::test]
async fn steamweb_requeries_without_spam_ips_while_truncated() {
    let real: Vec<String> = ["1.1.1.1:27015", "2.2.2.2:27015", "2.2.2.2:27016"]
        .map(String::from)
        .to_vec();
    let pages = vec![
        [spam("9.9.9.9", 5000), spam("8.8.8.8", 5000)].concat(), // tope: puede faltar gente
        [spam("7.7.7.7", 4000), real.clone(), spam("6.6.6.6", 5997)].concat(), // tope otra vez
        [real.clone(), spam("5.5.5.5", 100)].concat(),           // bajo el tope: completa
    ];
    let (got, filters) = fake_steam(pages, "\\appid\\10\\gamedir\\cstrike").await;
    let mut got: Vec<String> = got.iter().map(ToString::to_string).collect();
    got.sort();
    assert_eq!(got, real);
    assert_eq!(filters.len(), 3);
    assert!(filters[0].starts_with("\\appid\\10\\gamedir\\cstrike\\nand\\1\\name_match\\"));
    assert!(filters[0].ends_with("\\name_match\\This is a dummy*"));
    assert!(filters[1].contains("\\nor\\2\\gameaddr\\8.8.8.8\\gameaddr\\9.9.9.9"));
    assert!(
        filters[2].contains("\\nor\\4\\gameaddr\\6.6.6.6\\gameaddr\\7.7.7.7\\gameaddr\\8.8.8.8")
    );
}

#[tokio::test]
async fn steamweb_single_query_when_not_truncated() {
    let page = [vec!["1.1.1.1:27015".to_owned()], spam("9.9.9.9", 100)].concat();
    let (got, filters) = fake_steam(vec![page], "\\appid\\10").await;
    assert_eq!(got, vec![ServerAddress::new("1.1.1.1", 27015)]);
    assert_eq!(filters.len(), 1);
}

#[tokio::test]
async fn steamweb_stops_after_max_passes() {
    let pages = (1..20)
        .map(|i| spam(&format!("9.9.9.{i}"), 10000))
        .collect();
    let (got, filters) = fake_steam(pages, "\\appid\\10").await;
    assert!(got.is_empty());
    assert_eq!(filters.len(), STEAMWEB_MAX_PASSES);
}

#[test]
fn steam_filter_uses_appid_and_gamedir() {
    assert_eq!(
        steam_filter(game("cs16").unwrap(), false),
        "\\appid\\10\\gamedir\\cstrike\\empty\\1"
    );
    assert_eq!(
        steam_filter(game("dod").unwrap(), true),
        "\\appid\\30\\gamedir\\dod"
    );
}

#[test]
fn steamweb_constants_known_answers() {
    assert_eq!(
        (
            STEAMWEB_LIMIT,
            STEAMWEB_SPAM_PER_IP,
            STEAMWEB_MAX_PASSES,
            STEAMWEB_MAX_EXCLUDED
        ),
        (10000, 100, 6, 400)
    );
    assert_eq!(STEAMWEB_SPAM_NAMES, ["This is a dummy*"]);
}

fn assert_send<T: Send>(_: T) {}

#[test]
fn fetch_master_steamweb_is_send() {
    assert_send(fetch_master_steamweb(OFFICIAL_HOST, 443, "", "K", T));
}
