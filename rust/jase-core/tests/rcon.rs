//! RCON contra servidores falsos locales. Traducción de tests/test_rcon.py (sin Qt).
mod support;
use jase_core::games::game;
use jase_core::models::ServerAddress;
use jase_core::pystr::{latin1_encode_lossy, py_splitlines};
use jase_core::rcon::*;
use std::future::Future;
use std::time::Duration;
use support::net::fake_udp;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

const T: Duration = Duration::from_millis(500);
const PW: &str = "s3cr3t-pw";

fn assert_send<T: Send>(_: T) {}

fn lo(port: u16) -> ServerAddress {
    ServerAddress::new("127.0.0.1", port)
}

fn oob(body: &[u8]) -> Vec<u8> {
    [OOB, body].concat()
}

/// Puerto local sin nadie escuchando.
fn dead_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Acepta una conexión y corre `handler`; el test hace `await` del handle para ver sus panics.
async fn fake_tcp<F, Fut>(handler: F) -> (u16, JoinHandle<()>)
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let (s, _) = l.accept().await.unwrap();
        handler(s).await;
    });
    (port, task)
}

/// Un paquete Source: (id, tipo, cuerpo sin los dos nulos).
async fn read_packet(s: &mut TcpStream) -> (i32, i32, String) {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).await.unwrap();
    let mut data = vec![0u8; i32::from_le_bytes(n) as usize];
    s.read_exact(&mut data).await.unwrap();
    let id = i32::from_le_bytes(data[0..4].try_into().unwrap());
    let kind = i32::from_le_bytes(data[4..8].try_into().unwrap());
    (
        id,
        kind,
        String::from_utf8(data[8..data.len() - 2].to_vec()).unwrap(),
    )
}

/// Servidor Source que acepta la clave `PW` y contesta `answer` al comando.
async fn source_answering(answer: Vec<u8>) -> (u16, JoinHandle<()>) {
    fake_tcp(move |mut s| async move {
        let (rid, _, _) = read_packet(&mut s).await;
        s.write_all(&source_packet(rid, SERVERDATA_AUTH_RESPONSE, ""))
            .await
            .unwrap();
        read_packet(&mut s).await;
        read_packet(&mut s).await; // el vacío del final: sin leerlo, cerrar manda un RST
        let _ = s.write_all(&answer).await;
    })
    .await
}

async fn quake_bad_password(reply: &[u8]) -> RconError {
    let reply = oob(&[b"print\n", reply].concat());
    let server = fake_udp(move |_| vec![reply.clone()]).await;
    quake(&lo(server.port), PW, "status", T).await.unwrap_err()
}

async fn quake_timeout() -> RconError {
    let server = fake_udp(|_| vec![]).await;
    let e = quake(&lo(server.port), PW, "status", Duration::from_millis(300)).await;
    e.unwrap_err()
}

async fn quake_closed_port() -> RconError {
    let addr = lo(dead_udp_port());
    let e = quake(&addr, PW, "status", Duration::from_millis(300)).await;
    e.unwrap_err()
}

async fn goldsrc_no_challenge() -> RconError {
    let server = fake_udp(|_| vec![oob(b"lUnknown command\n")]).await;
    goldsrc(&lo(server.port), PW, "status", T)
        .await
        .unwrap_err()
}

async fn source_bad_size(size: i32) -> RconError {
    let (port, task) = source_answering([&size.to_le_bytes()[..], &[0; 16]].concat()).await;
    let e = source(&lo(port), PW, "status", T).await.unwrap_err();
    task.await.unwrap();
    e
}

async fn source_connect_refused() -> RconError {
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    source(&lo(port), PW, "status", T).await.unwrap_err()
}

async fn source_eof_mid_answer() -> RconError {
    let mut answer = source_packet(2, SERVERDATA_RESPONSE_VALUE, "hostname: x\n");
    answer.extend_from_slice(&source_packet(2, SERVERDATA_RESPONSE_VALUE, "mitad")[..9]);
    let (port, task) = source_answering(answer).await;
    let e = source(&lo(port), PW, "status", T).await.unwrap_err();
    task.await.unwrap();
    e
}

async fn source_bad_password() -> RconError {
    let (port, task) = fake_tcp(|mut s| async move {
        let (rid, _, _) = read_packet(&mut s).await;
        let reply = [
            source_packet(rid, SERVERDATA_RESPONSE_VALUE, ""),
            source_packet(-1, SERVERDATA_AUTH_RESPONSE, ""),
        ];
        s.write_all(&reply.concat()).await.unwrap();
    })
    .await;
    let e = source(&lo(port), "mala", "status", T).await.unwrap_err();
    task.await.unwrap();
    e
}

#[test]
fn kind_of_each_family() {
    let kind = |id| kind_of(game(id).unwrap());
    assert_eq!(kind("cod2"), Some(RconKind::Quake));
    assert_eq!(kind("q2"), Some(RconKind::Quake));
    assert_eq!(kind("cs16"), Some(RconKind::Goldsrc));
    assert_eq!(kind("tf2"), Some(RconKind::Source));
    assert_eq!(kind("ut99"), None);
    assert_eq!(kind("zandronum"), None);
}

#[test]
fn clean_command_is_one_line() {
    assert_eq!(clean_command(" kick a\nquit "), "kick a quit");
    assert_eq!(clean_command(&"x".repeat(5000)).len(), MAX_COMMAND);
}

#[test]
fn clean_command_splits_like_python() {
    // Lo que da Python: 'a\rb\x0bc\x1cd e\r\n\nf\n'.splitlines()
    assert_eq!(
        py_splitlines("a\rb\x0bc\x1cd\u{2028}e\r\n\nf\n"),
        ["a", "b", "c", "d", "e", "", "f"]
    );
    assert_eq!(py_splitlines(" x\u{85}y\x1d"), [" x", "y"]);
    assert!(py_splitlines("").is_empty());
    assert_eq!(py_splitlines("\n"), [""]);
    assert_eq!(
        clean_command("say a\rquit\x0bx\x1cy\u{2028}z"),
        "say a quit x y z"
    );
    let long = clean_command(&"ñ".repeat(1500));
    assert_eq!(long.chars().count(), MAX_COMMAND);
}

#[test]
fn latin1_encode_lossy_replaces_like_python() {
    assert_eq!(latin1_encode_lossy("ñ€"), b"\xf1?");
    assert_eq!(latin1_encode_lossy("rcon x"), b"rcon x");
}

#[test]
fn errors_have_python_codes() {
    assert_eq!(ERRORS.len(), 8);
    assert_eq!(RconError::Timeout.code(), "timeout");
    assert_eq!(RconError::Network("x".into()).code(), "network");
}

#[tokio::test]
async fn quake_rcon_joins_print_packets_and_strips_colors() {
    let server = fake_udp(|_| {
        vec![
            oob(b"print\n^1map: ^7mp_toujane\n"),
            oob(b"print\nnum score\n"),
        ]
    })
    .await;
    let text = quake(&lo(server.port), "secreta", "status", T)
        .await
        .unwrap();
    assert_eq!(server.received()[0], oob(b"rcon secreta status"));
    assert_eq!(text, "map: mp_toujane\nnum score\n");
}

#[tokio::test]
async fn quake_rcon_bad_password_and_timeout() {
    let e = quake_bad_password(b"Bad rconpassword.\n").await;
    assert_eq!(e, RconError::BadPassword);
    assert_eq!(quake_timeout().await, RconError::Timeout);
}

#[tokio::test]
async fn quake_rcon_bad_password_variants() {
    for reply in [
        &b"Bad rcon_password.\n"[..],
        b"Bad rconpassword.\n",
        b"Invalid password.\n",
    ] {
        assert_eq!(quake_bad_password(reply).await, RconError::BadPassword);
    }
}

#[tokio::test]
async fn quake_rcon_output_mentioning_password_is_kept() {
    let server = fake_udp(|_| vec![oob(b"print\nmap: mp_x\nplayer: invalid password lol\n")]).await;
    let text = quake(&lo(server.port), "x", "status", T).await.unwrap();
    assert!(text.contains("invalid password lol"));
}

#[tokio::test]
async fn quake_rcon_closed_port_is_timeout() {
    assert_eq!(quake_closed_port().await, RconError::Timeout);
}

#[test]
fn error_display_is_python_str() {
    assert_eq!(RconError::Connect(String::new()).to_string(), "connect");
    assert_eq!(
        RconError::Network("sin red".into()).to_string(),
        "network: sin red"
    );
}

#[tokio::test]
async fn goldsrc_challenge_then_command() {
    let server = fake_udp(|d| {
        if d.starts_with(&oob(b"challenge rcon")) {
            vec![oob(b"challenge rcon 4242\n")]
        } else {
            vec![oob(b"lhostname: Pampa CS\n")]
        }
    })
    .await;
    let text = goldsrc(&lo(server.port), "clave", "status", T)
        .await
        .unwrap();
    assert_eq!(server.received()[1], oob(b"rcon 4242 \"clave\" status\n"));
    assert_eq!(text, "hostname: Pampa CS\n");
}

#[tokio::test]
async fn goldsrc_without_challenge_is_no_challenge() {
    assert_eq!(goldsrc_no_challenge().await, RconError::NoChallenge);
}

#[tokio::test]
async fn source_rcon_auth_and_multi_packet_answer() {
    let (port, task) = fake_tcp(|mut s| async move {
        let auth = read_packet(&mut s).await;
        assert_eq!((auth.1, auth.2.as_str()), (SERVERDATA_AUTH, "buena"));
        let reply = [
            source_packet(auth.0, SERVERDATA_RESPONSE_VALUE, ""),
            source_packet(auth.0, SERVERDATA_AUTH_RESPONSE, ""),
        ];
        s.write_all(&reply.concat()).await.unwrap();
        let (rid, kind, body) = read_packet(&mut s).await;
        assert_eq!((kind, body.as_str()), (SERVERDATA_EXECCOMMAND, "status"));
        let reply = [
            source_packet(rid, SERVERDATA_RESPONSE_VALUE, "hostname: TF2 "),
            source_packet(rid, SERVERDATA_RESPONSE_VALUE, "Pampa\n"),
        ];
        s.write_all(&reply.concat()).await.unwrap();
        let (rid, _, _) = read_packet(&mut s).await; // el vacío que marca el final
        s.write_all(&source_packet(rid, SERVERDATA_RESPONSE_VALUE, ""))
            .await
            .unwrap();
    })
    .await;
    let text = source(&lo(port), "buena", "status", T).await.unwrap();
    task.await.unwrap();
    assert_eq!(text, "hostname: TF2 Pampa\n");
    assert_eq!(source_bad_password().await, RconError::BadPassword);
}

#[tokio::test]
async fn source_bad_size_is_invalid() {
    assert_eq!(source_bad_size(9).await, RconError::Invalid);
    assert_eq!(source_bad_size(5000).await, RconError::Invalid);
}

#[tokio::test]
async fn source_connect_refused_is_connect() {
    assert!(matches!(
        source_connect_refused().await,
        RconError::Connect(_)
    ));
}

#[tokio::test]
async fn source_eof_mid_answer_is_closed() {
    assert_eq!(source_eof_mid_answer().await, RconError::Closed);
}

#[tokio::test]
async fn source_long_answer_is_capped() {
    // Un servidor que no para de mandar: se corta en el tope, no crece sin límite.
    let chunk = source_packet(2, SERVERDATA_RESPONSE_VALUE, &"x".repeat(4000));
    let (port, task) = source_answering(chunk.repeat(400)).await;
    let text = source(&lo(port), PW, "status", T).await.unwrap();
    task.await.unwrap();
    assert!(text.len() <= MAX_REPLY && text.len() > MAX_REPLY - 8000);
}

#[tokio::test]
async fn send_rejects_unsupported_games() {
    let addr = ServerAddress::new("1.1.1.1", 7778);
    let e = send(game("ut99").unwrap(), &addr, "x", "status").await;
    assert_eq!(e, Err(RconError::Unsupported));
}

#[tokio::test]
async fn send_empty_command_skips_network() {
    let server = fake_udp(|_| vec![oob(b"print\nhola\n")]).await;
    let text = send(game("cod2").unwrap(), &lo(server.port), PW, " \n\r ").await;
    assert_eq!(text, Ok(String::new()));
    assert!(server.received().is_empty());
}

#[tokio::test]
async fn rcon_errors_never_contain_password() {
    let addr = lo(1);
    let errors = [
        quake_bad_password(b"Bad rconpassword.\n").await,
        quake_timeout().await,
        quake_closed_port().await,
        goldsrc_no_challenge().await,
        source_bad_size(9).await,
        source_connect_refused().await,
        source_eof_mid_answer().await,
        send(game("ut99").unwrap(), &addr, PW, "status")
            .await
            .unwrap_err(),
    ];
    for e in errors {
        assert!(!format!("{e}").contains(PW), "{e}");
        assert!(!format!("{e:?}").contains(PW), "{e:?}");
    }
}

#[test]
fn rcon_futures_are_send() {
    let (g, addr) = (game("cod2").unwrap(), lo(1));
    assert_send(send(g, &addr, PW, "status"));
    assert_send(quake(&addr, PW, "status", T));
    assert_send(goldsrc(&addr, PW, "status", T));
    assert_send(source(&addr, PW, "status", T));
}
