//! Descargas HTTP contra un servidor local de un solo uso (sin Internet).
use jase_core::net::fetch_blocking;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

/// Atiende una conexión: `respond` recibe la petición cruda y el socket.
fn serve(respond: impl FnOnce(String, std::net::TcpStream) + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/x", listener.local_addr().unwrap());
    thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let n = s.read(&mut buf).unwrap_or(0);
        respond(String::from_utf8_lossy(&buf[..n]).into_owned(), s);
    });
    url
}

fn reply(status: &str, body: &[u8]) -> impl FnOnce(String, std::net::TcpStream) + Send + 'static {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let body = body.to_vec();
    move |_, mut s| {
        let _ = s.write_all(head.as_bytes());
        let _ = s.write_all(&body);
    }
}

const D: Duration = Duration::from_secs(2);

#[test]
fn fetch_ok_sends_user_agent() {
    let (tx, rx) = std::sync::mpsc::channel();
    let url = serve(move |req, s| {
        tx.send(req).unwrap();
        reply("200 OK", b"hola")(String::new(), s);
    });
    assert_eq!(
        fetch_blocking(&url, 1024, D, &[("X-Test", "1")]),
        Some(b"hola".to_vec())
    );
    let req = rx.recv().unwrap().to_lowercase();
    assert!(req.contains("user-agent: jase\r\n"), "{req}");
    assert!(req.contains("x-test: 1\r\n"), "{req}");
}

#[test]
fn fetch_404_is_none() {
    let url = serve(reply("404 Not Found", b"no"));
    assert_eq!(fetch_blocking(&url, 1024, D, &[]), None);
}

#[test]
fn fetch_over_max_bytes_is_none() {
    let url = serve(reply("200 OK", &[b'x'; 11]));
    assert_eq!(fetch_blocking(&url, 10, D, &[]), None);
    let url = serve(reply("200 OK", &[b'x'; 10]));
    assert_eq!(fetch_blocking(&url, 10, D, &[]), Some(vec![b'x'; 10]));
}

#[test]
fn fetch_slow_body_hits_deadline() {
    let url = serve(|_, mut s| {
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n");
        for _ in 0..100 {
            if s.write_all(b"x").is_err() {
                return;
            }
            thread::sleep(Duration::from_millis(300));
        }
    });
    let t = Instant::now();
    assert_eq!(
        fetch_blocking(&url, 1024, Duration::from_millis(500), &[]),
        None
    );
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn fetch_unreachable_is_none() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert_eq!(
        fetch_blocking(&format!("http://127.0.0.1:{port}/"), 1024, D, &[]),
        None
    );
}

#[tokio::test]
async fn async_fetch_matches_blocking() {
    let url = serve(reply("200 OK", b"ok"));
    assert_eq!(
        jase_core::net::fetch(url, 1024, D).await,
        Some(b"ok".to_vec())
    );
}

/// El proxy acepta http, https y socks; rechaza otros esquemas o sin esquema.
#[test]
fn proxy_url_validation() {
    use jase_core::net::parse_proxy;
    for ok in [
        "http://127.0.0.1:8080",
        "https://proxy.lan:3128",
        "socks5://user:pw@10.0.0.1:1080",
        "SOCKS4://10.0.0.1:1080",
    ] {
        assert!(parse_proxy(ok).is_ok(), "{ok}");
    }
    for bad in ["127.0.0.1:8080", "ftp://x:21", "", "socks5h://x:1"] {
        assert!(parse_proxy(bad).is_err(), "{bad}");
    }
}
