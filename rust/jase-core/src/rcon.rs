//! Consola RCON: mandar comandos de administrador a un servidor. Traducción de
//! `src/jase/rcon.py`.
//!
//! Tres variantes según el juego (`RconKind`):
//! - Quake 3 y Quake II, UDP: «\xff\xff\xff\xffrcon <clave> <comando>», respuesta en uno o
//!   más «print\n…».
//! - GoldSrc (Half-Life 1), UDP: primero «challenge rcon», luego
//!   «rcon <challenge> "<clave>" <comando>»; la respuesta llega como «l<texto>».
//! - Source, TCP en el puerto del juego con paquetes [tamaño][id][tipo][cuerpo\0\0]: AUTH (3)
//!   y luego EXECCOMMAND (2).
//!
//! Un intercambio con un solo servidor: no usa el `Router` del escáner. Soltar el future
//! cancela. La clave nunca entra en un error.
use crate::games::{Family, GameDef, LaunchStyle};
use crate::models::ServerAddress;
use crate::pystr::{casefold, latin1, latin1_encode_lossy, py_splitlines, py_strip};
use crate::scanner::resolve_v4;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

pub const OOB: &[u8] = b"\xff\xff\xff\xff";
/// Tras la primera respuesta UDP, cuánto se espera por más trozos.
pub const GRACE: Duration = Duration::from_millis(400);
pub const MAX_COMMAND: usize = 1000;
pub const QUAKE_TIMEOUT: Duration = Duration::from_secs(3);
pub const SOURCE_TIMEOUT: Duration = Duration::from_secs(5);
pub const SERVERDATA_AUTH: i32 = 3;
pub const SERVERDATA_AUTH_RESPONSE: i32 = 2;
pub const SERVERDATA_EXECCOMMAND: i32 = 2;
pub const SERVERDATA_RESPONSE_VALUE: i32 = 0;
pub const MAX_PACKET: i32 = 4096 + 14;
/// Tope de bytes de una respuesta: un servidor que no para de mandar no la hace crecer
/// sin límite (Python no lo tiene).
pub const MAX_REPLY: usize = 1 << 20;
/// Tope de datagramas de una respuesta UDP: los vacíos no suman bytes.
pub const MAX_DATAGRAMS: usize = 1024;

pub const ERRORS: [&str; 8] = [
    "timeout",
    "bad_password",
    "no_challenge",
    "connect",
    "invalid",
    "closed",
    "unsupported",
    "network",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RconKind {
    Quake,
    Goldsrc,
    Source,
}

/// Fallo del RCON; `code()` es uno de `ERRORS` (la UI lo traduce). El detalle es solo el
/// texto del error del sistema: nunca la clave.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RconError {
    #[error("timeout")]
    Timeout,
    #[error("bad_password")]
    BadPassword,
    #[error("no_challenge")]
    NoChallenge,
    #[error("{}", with_detail("connect", .0))]
    Connect(String),
    #[error("invalid")]
    Invalid,
    #[error("closed")]
    Closed,
    #[error("unsupported")]
    Unsupported,
    #[error("{}", with_detail("network", .0))]
    Network(String),
}

/// Como `str()` del error de Python: `code` o `code: detalle`.
fn with_detail(code: &str, detail: &str) -> String {
    if detail.is_empty() {
        code.to_owned()
    } else {
        format!("{code}: {detail}")
    }
}

impl RconError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Timeout => ERRORS[0],
            Self::BadPassword => ERRORS[1],
            Self::NoChallenge => ERRORS[2],
            Self::Connect(_) => ERRORS[3],
            Self::Invalid => ERRORS[4],
            Self::Closed => ERRORS[5],
            Self::Unsupported => ERRORS[6],
            Self::Network(_) => ERRORS[7],
        }
    }
}

pub fn kind_of(game: &GameDef) -> Option<RconKind> {
    match game.family {
        Family::Quake3 | Family::Quake2 => Some(RconKind::Quake),
        Family::Valve if game.launch_style == LaunchStyle::Steam => Some(RconKind::Source),
        Family::Valve => Some(RconKind::Goldsrc),
        _ => None,
    }
}

/// Una sola línea (un salto de línea colaría otro comando) y con tope de largo.
pub fn clean_command(command: &str) -> String {
    py_strip(&py_splitlines(command).join(" "))
        .chars()
        .take(MAX_COMMAND)
        .collect()
}

// ---------------------------------------------------------------------- UDP

async fn udp_socket(address: &ServerAddress) -> Result<UdpSocket, RconError> {
    let net = |e: std::io::Error| RconError::Network(e.to_string());
    let target = resolve_v4(&address.host, address.port).await.map_err(net)?;
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(net)?;
    sock.connect(target).await.map_err(net)?;
    Ok(sock)
}

/// El siguiente datagrama antes de `wait`. Los errores de `recv` (el ICMP de un puerto
/// cerrado: `ConnectionRefused`, o `ConnectionReset` en Windows) se ignoran.
async fn recv_within(sock: &UdpSocket, buf: &mut [u8], wait: Duration) -> Option<usize> {
    tokio::time::timeout(wait, async {
        loop {
            if let Ok(n) = sock.recv(buf).await {
                return n;
            }
        }
    })
    .await
    .ok()
}

/// El primer datagrama (antes de `timeout`) y los que sigan llegando antes de `GRACE`.
async fn replies(
    sock: &UdpSocket,
    timeout: Duration,
    max_datagrams: usize,
) -> Result<Vec<Vec<u8>>, RconError> {
    let mut buf = vec![0u8; 65535];
    let n = recv_within(sock, &mut buf, timeout)
        .await
        .ok_or(RconError::Timeout)?;
    let (mut packets, mut total) = (vec![buf[..n].to_vec()], n);
    while total < MAX_REPLY
        && packets.len() < max_datagrams
        && let Some(n) = recv_within(sock, &mut buf, GRACE).await
    {
        packets.push(buf[..n].to_vec());
        total += n;
    }
    Ok(packets)
}

fn starts_with_bad_password(text: &str, prefixes: &[&str]) -> bool {
    let text = casefold(py_strip(text));
    prefixes.iter().any(|p| text.starts_with(p))
}

/// Quita los colores `^[0-9A-Za-z]` (no `quake3::strip_colors`, que quita los saltos).
fn strip_colors(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '^' && chars.peek().is_some_and(char::is_ascii_alphanumeric) {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

pub async fn quake(
    address: &ServerAddress,
    password: &str,
    command: &str,
    timeout: Duration,
) -> Result<String, RconError> {
    let sock = udp_socket(address).await?;
    let request = [
        OOB,
        &latin1_encode_lossy(&format!("rcon {password} {command}")),
    ]
    .concat();
    let _ = sock.send(&request).await; // un fallo se ve como timeout, como en Python
    let text: String = replies(&sock, timeout, MAX_DATAGRAMS)
        .await?
        .iter()
        .filter_map(|p| p.strip_prefix(OOB))
        .map(|p| {
            let text = latin1(p);
            text.strip_prefix("print\n")
                .map(str::to_owned)
                .unwrap_or(text)
        })
        .collect();
    let text = strip_colors(&text);
    // Solo al principio: la salida de un comando puede mencionar «invalid password».
    let bad = ["bad rconpassword", "bad rcon_password", "invalid password"];
    if starts_with_bad_password(&text, &bad) {
        return Err(RconError::BadPassword);
    }
    Ok(text)
}

pub async fn goldsrc(
    address: &ServerAddress,
    password: &str,
    command: &str,
    timeout: Duration,
) -> Result<String, RconError> {
    let sock = udp_socket(address).await?;
    let _ = sock.send(&[OOB, b"challenge rcon\n"].concat()).await;
    let mut buf = vec![0u8; 65535];
    let n = recv_within(&sock, &mut buf, timeout)
        .await
        .ok_or(RconError::Timeout)?;
    let reply = latin1(buf.get(4..n).unwrap_or_default());
    let challenge: String = reply
        .strip_prefix("challenge rcon ")
        .unwrap_or_default()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if challenge.is_empty() {
        return Err(RconError::NoChallenge);
    }
    let line = format!("rcon {challenge} \"{password}\" {command}\n");
    let _ = sock
        .send(&[OOB, &latin1_encode_lossy(&line)].concat())
        .await;
    let text: String = replies(&sock, timeout, MAX_DATAGRAMS)
        .await?
        .iter()
        .filter_map(|p| p.strip_prefix(OOB)?.strip_prefix(b"l"))
        .map(latin1)
        .collect();
    if starts_with_bad_password(&text, &["bad rcon_password"]) {
        return Err(RconError::BadPassword);
    }
    Ok(text)
}

// ---------------------------------------------------------------------- Source (TCP)

pub fn source_packet(request_id: i32, kind: i32, body: &str) -> Vec<u8> {
    let mut payload = [request_id.to_le_bytes(), kind.to_le_bytes()].concat();
    payload.extend_from_slice(body.as_bytes());
    payload.extend_from_slice(b"\0\0");
    // Un cuerpo de más de 2 GiB no cabe en el tamaño: no llega (el comando tiene tope).
    let size = i32::try_from(payload.len()).unwrap_or(i32::MAX);
    [&size.to_le_bytes()[..], &payload].concat()
}

/// (id, tipo, cuerpo hasta el primer nulo).
async fn read_packet(stream: &mut TcpStream) -> Result<(i32, i32, String), RconError> {
    let closed = |_| RconError::Closed;
    let mut size = [0u8; 4];
    stream.read_exact(&mut size).await.map_err(closed)?;
    let size = i32::from_le_bytes(size);
    if !(10..=MAX_PACKET).contains(&size) {
        return Err(RconError::Invalid);
    }
    let mut data = vec![0u8; size as usize]; // 10..=4110: cabe
    stream.read_exact(&mut data).await.map_err(closed)?;
    let (head, body) = data.split_at(8);
    let request_id = i32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    let kind = i32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    let body = body.split(|&b| b == 0).next().unwrap_or_default();
    Ok((request_id, kind, String::from_utf8_lossy(body).into_owned()))
}

async fn source_exchange(
    stream: &mut TcpStream,
    password: &str,
    command: &str,
) -> Result<String, RconError> {
    let closed = |_| RconError::Closed;
    let auth = source_packet(1, SERVERDATA_AUTH, password);
    stream.write_all(&auth).await.map_err(closed)?;
    let request_id = loop {
        // Puede llegar antes un RESPONSE_VALUE vacío.
        let (request_id, kind, _) = read_packet(stream).await?;
        if kind == SERVERDATA_AUTH_RESPONSE {
            break request_id;
        }
    };
    if request_id == -1 {
        return Err(RconError::BadPassword);
    }
    // El comando y, detrás, un paquete vacío: su eco marca el final de la respuesta (las
    // largas llegan en varios paquetes).
    let request = [
        source_packet(2, SERVERDATA_EXECCOMMAND, command),
        source_packet(3, SERVERDATA_RESPONSE_VALUE, ""),
    ];
    stream.write_all(&request.concat()).await.map_err(closed)?;
    let mut text = String::new();
    loop {
        let (request_id, _, body) = read_packet(stream).await?;
        if request_id == 3 || text.len() + body.len() > MAX_REPLY {
            return Ok(text);
        }
        if request_id == 2 {
            text.push_str(&body);
        }
    }
}

pub async fn source(
    address: &ServerAddress,
    password: &str,
    command: &str,
    timeout: Duration,
) -> Result<String, RconError> {
    let connect = TcpStream::connect((address.host.as_str(), address.port));
    let mut stream = match tokio::time::timeout(timeout, connect).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(RconError::Connect(e.to_string())),
        Err(_) => return Err(RconError::Connect(String::new())), // como str(TimeoutError())
    };
    tokio::time::timeout(timeout, source_exchange(&mut stream, password, command))
        .await
        .unwrap_or(Err(RconError::Timeout))
}

/// Manda `command` y devuelve la respuesta.
pub async fn send(
    game: &GameDef,
    address: &ServerAddress,
    password: &str,
    command: &str,
) -> Result<String, RconError> {
    let kind = kind_of(game).ok_or(RconError::Unsupported)?;
    let command = clean_command(command);
    if command.is_empty() {
        return Ok(String::new());
    }
    match kind {
        RconKind::Quake => quake(address, password, &command, QUAKE_TIMEOUT).await,
        RconKind::Goldsrc => goldsrc(address, password, &command, QUAKE_TIMEOUT).await,
        RconKind::Source => source(address, password, &command, SOURCE_TIMEOUT).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replies_caps_datagram_count() {
        // Datagramas vacíos sin parar: el tope cuenta datagramas, no solo bytes. Todo se
        // encola antes de leer, así que no depende de temporizadores.
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.connect(server.local_addr().unwrap()).await.unwrap();
        let to = client.local_addr().unwrap();
        for _ in 0..8 {
            server.send_to(&[], to).await.unwrap();
        }
        server.send_to(b"fin", to).await.unwrap();
        let got = replies(&client, Duration::from_millis(500), 8)
            .await
            .unwrap();
        assert_eq!(got.len(), 8);
        assert!(got.iter().all(Vec::is_empty));
    }
}
