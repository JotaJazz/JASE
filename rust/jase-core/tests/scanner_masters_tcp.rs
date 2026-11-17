//! Masters TCP contra servidores falsos locales. Traducción de tests/test_scanner.py y
//! tests/test_ut2004.py.
mod support;
use jase_core::models::ServerAddress;
use jase_core::protocols::{gamespy, ut2004};
use jase_core::scanner::MasterFetchError;
use jase_core::scanner::masters_tcp::*;
use std::future::Future;
use std::time::Duration;
use support::fixture;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

const T: Duration = Duration::from_millis(500);
const LO: &str = "127.0.0.1";

/// Acepta una conexión y corre `handler`; el test hace `await` del handle para ver sus panics.
async fn fake_tcp<F, Fut>(handler: F) -> (u16, JoinHandle<()>)
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    let l = TcpListener::bind((LO, 0)).await.unwrap();
    let port = l.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let (s, _) = l.accept().await.unwrap();
        handler(s).await;
    });
    (port, task)
}

fn split_frames(data: &[u8]) -> Vec<Vec<u8>> {
    let (mut frames, mut pos) = (Vec::new(), 0);
    while pos < data.len() {
        let n = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        frames.push(data[pos + 4..pos + 4 + n].to_vec());
        pos += 4 + n;
    }
    frames
}

fn master_frames() -> Vec<Vec<u8>> {
    split_frames(&fixture("ut2004_master_1.bin"))
}

fn fs(text: &str) -> Vec<u8> {
    [
        vec![text.len() as u8 + 1],
        text.as_bytes().to_vec(),
        vec![0],
    ]
    .concat()
}

async fn read_frame(s: &mut TcpStream) -> Vec<u8> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).await.unwrap();
    let mut body = vec![0u8; u32::from_le_bytes(n) as usize];
    s.read_exact(&mut body).await.unwrap();
    body
}

/// Como `_serve` de Python: challenge, y por cada petición del cliente las respuestas
/// fijadas; `entries` limita cuántos registros manda tras la cabecera.
async fn serve_ut2004(mut s: TcpStream, frames: Vec<Vec<u8>>, entries: usize) -> Vec<Vec<u8>> {
    let mut received = Vec::new();
    s.write_all(&ut2004::frame(&frames[0])).await.unwrap();
    for reply in [&frames[1..2], &frames[2..3], &frames[3..4 + entries]] {
        received.push(read_frame(&mut s).await);
        for body in reply {
            s.write_all(&ut2004::frame(body)).await.unwrap();
        }
    }
    received
}

#[tokio::test]
async fn gamespy_master_handshake() {
    let (port, task) = fake_tcp(|mut s| async move {
        s.write_all(b"\\basic\\\\secure\\ABCDEF").await.unwrap();
        let mut got = Vec::new();
        while !got.ends_with(b"\\final\\") {
            let mut c = [0u8; 1024];
            let n = s.read(&mut c).await.unwrap();
            assert!(n > 0);
            got.extend_from_slice(&c[..n]);
        }
        let validate = [
            b"\\validate\\".as_slice(),
            gamespy::gsmsalg("ABCDEF", "Z5Nfb0").as_bytes(),
        ]
        .concat();
        assert!(got.windows(validate.len()).any(|w| w == validate));
        assert!(got.windows(18).any(|w| w == b"\\list\\\\gamename\\ut"));
        s.write_all(b"\\ip\\127.0.0.1:7778\\final\\").await.unwrap();
    })
    .await;
    let got = fetch_master_gamespy(LO, port, "ut", "Z5Nfb0", T).await;
    task.await.unwrap();
    assert_eq!(got.unwrap(), [ServerAddress::new(LO, 7778)]);
}

#[tokio::test]
async fn gamespy_master_timeout_mid_list_keeps_partial() {
    let (port, task) = fake_tcp(|mut s| async move {
        s.write_all(b"\\secure\\ABCDEF").await.unwrap();
        let mut c = [0u8; 1024];
        assert!(s.read(&mut c).await.unwrap() > 0);
        s.write_all(b"\\ip\\127.0.0.1:7778").await.unwrap();
        tokio::time::sleep(Duration::from_millis(900)).await; // sin \final\
    })
    .await;
    let got = fetch_master_gamespy(LO, port, "ut", "Z5Nfb0", Duration::from_millis(300)).await;
    assert_eq!(got.unwrap(), [ServerAddress::new(LO, 7778)]);
    task.abort();
}

#[tokio::test]
async fn gamespy_master_timeout_without_list_is_timeout() {
    let (port, task) = fake_tcp(|mut s| async move {
        s.write_all(b"\\secure\\ABCDEF").await.unwrap();
        tokio::time::sleep(Duration::from_millis(900)).await;
    })
    .await;
    let got = fetch_master_gamespy(LO, port, "ut", "Z5Nfb0", Duration::from_millis(200)).await;
    assert_eq!(got, Err(MasterFetchError::Timeout));
    task.abort();
}

#[tokio::test]
async fn fetch_master_ut2004_replays_fixture() {
    let frames = master_frames();
    let (port, task) = fake_tcp(|s| async move {
        let n = frames.len() - 4;
        let received = serve_ut2004(s, frames, n).await;
        assert_eq!(received[0], ut2004::build_challenge_response()[4..]);
        assert_eq!(received[2], ut2004::list_request()[4..]);
    })
    .await;
    let servers = fetch_master_ut2004(LO, port, Duration::from_secs(3))
        .await
        .unwrap();
    task.await.unwrap();
    let uniq: std::collections::HashSet<_> = servers.iter().collect();
    assert!(servers.len() > 500 && uniq.len() == servers.len());
}

#[tokio::test]
async fn fetch_master_ut2004_denied_raises() {
    let (port, task) = fake_tcp(|mut s| async move {
        s.write_all(&ut2004::frame(&fs("111111111"))).await.unwrap();
        read_frame(&mut s).await;
        s.write_all(&ut2004::frame(&fs("DENIED"))).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    })
    .await;
    let got = fetch_master_ut2004(LO, port, T).await;
    task.await.unwrap();
    assert_eq!(got.unwrap_err().kind(), "protocol");
}

#[tokio::test]
async fn fetch_master_ut2004_reset_mid_list_keeps_partial() {
    let frames = master_frames();
    let (port, task) = fake_tcp(|s| async move {
        serve_ut2004(s, frames, 3).await;
        // Se suelta el socket con la lista a medias (la cabecera anuncia más).
    })
    .await;
    let got = fetch_master_ut2004(LO, port, T).await.unwrap();
    task.await.unwrap();
    assert_eq!(got.len(), 3);
}

#[tokio::test]
async fn fetch_master_ut2004_reset_before_list_raises() {
    let (port, task) = fake_tcp(|s| async move {
        // SO_LINGER 0: al soltar el socket se envía RST en vez de FIN.
        #[allow(deprecated)] // tokio no ofrece otra forma sin socket2
        s.set_linger(Some(Duration::ZERO)).unwrap();
        drop(s);
    })
    .await;
    let got = fetch_master_ut2004(LO, port, T).await;
    task.await.unwrap();
    assert_eq!(got.unwrap_err().kind(), "network");
}

#[tokio::test]
async fn fetch_master_ut2004_eof_before_list_is_protocol_error() {
    let (port, task) = fake_tcp(|s| async move { drop(s) }).await;
    let got = fetch_master_ut2004(LO, port, T).await;
    task.await.unwrap();
    assert_eq!(got.unwrap_err().kind(), "protocol");
}

#[tokio::test]
async fn ut2004_timeout_without_entries_is_timeout() {
    let (port, task) = fake_tcp(|s| async move {
        tokio::time::sleep(Duration::from_millis(900)).await;
        drop(s);
    })
    .await;
    let got = fetch_master_ut2004(LO, port, Duration::from_millis(200)).await;
    assert_eq!(got, Err(MasterFetchError::Timeout));
    task.abort();
}

#[tokio::test]
async fn ut2004_frame_too_big_is_protocol_error() {
    let (port, task) = fake_tcp(|mut s| async move {
        s.write_all(&((ut2004::MAX_FRAME as u32 + 1).to_le_bytes()))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    })
    .await;
    let got = fetch_master_ut2004(LO, port, T).await;
    task.await.unwrap();
    assert_eq!(got.unwrap_err().kind(), "protocol");
}

fn assert_send<T: Send>(_: T) {}

#[test]
fn tcp_master_fetches_are_send() {
    let t = Duration::from_millis(1);
    assert_send(fetch_master_gamespy(LO, 1, "ut", "Z5Nfb0", t));
    assert_send(fetch_master_ut2004(LO, 1, t));
}
