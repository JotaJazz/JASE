//! Listas de servidores por TCP: GameSpy (UT, BF1942...) y UT2004.
use super::{MasterFetchError, resolve_v4};
use crate::models::ServerAddress;
use crate::protocols::{ProtocolError, gamespy, ut2004};
use indexmap::IndexSet;
use std::io;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout_at;

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

async fn connect(host: &str, port: u16) -> io::Result<TcpStream> {
    TcpStream::connect(resolve_v4(host, port).await?).await
}

/// Lee `\secure\` + 6 bytes (o `\final\`, o EOF), manda validate + list y lee hasta
/// `\final\` o EOF. Si vence el plazo a mitad de la lista vale lo recibido.
pub async fn fetch_master_gamespy(
    host: &str,
    port: u16,
    gamename: &str,
    gamekey: &str,
    timeout: Duration,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    let mut buf = Vec::new();
    let run = async {
        let mut sock = connect(host, port).await?;
        let mut hello = Vec::new();
        loop {
            let secure_done = find(&hello, b"\\secure\\").is_some_and(|i| hello.len() - i - 8 >= 6);
            // Un hello real mide < 100 bytes; más de 4 KiB sin `\secure\` es basura.
            if secure_done || hello.len() > 4096 || find(&hello, b"\\final\\").is_some() {
                break;
            }
            let mut chunk = [0u8; 4096];
            match sock.read(&mut chunk).await? {
                0 => break,
                n => hello.extend_from_slice(&chunk[..n]),
            }
        }
        let secure = gamespy::parse_secure(&hello)?;
        let mut req = gamespy::build_validate(&secure, gamename, gamekey);
        req.extend(gamespy::build_list_request(gamename));
        sock.write_all(&req).await?;
        while find(&buf, b"\\final\\").is_none() {
            let mut chunk = vec![0u8; 65536];
            match sock.read(&mut chunk).await? {
                0 => break,
                n => buf.extend_from_slice(&chunk[..n]),
            }
        }
        Ok::<(), MasterFetchError>(())
    };
    let outcome = timeout_at((Instant::now() + timeout).into(), run).await;
    match outcome {
        Ok(r) => r?,
        Err(_) => {
            let partial = gamespy::parse_master_list(&buf);
            if partial.is_empty() {
                return Err(MasterFetchError::Timeout);
            }
            return Ok(partial);
        }
    }
    Ok(gamespy::parse_master_list(&buf))
}

/// Corte de la conversación UT2004: de E/S (se tolera con entradas ya recibidas) o fatal.
enum Cut {
    Io(io::Error),
    Fatal(MasterFetchError),
}

impl From<io::Error> for Cut {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<ProtocolError> for Cut {
    fn from(e: ProtocolError) -> Self {
        Self::Fatal(e.into())
    }
}

async fn read_frame(sock: &mut TcpStream) -> Result<Vec<u8>, Cut> {
    let mut size = [0u8; 4];
    sock.read_exact(&mut size).await?;
    let size = u32::from_le_bytes(size) as usize;
    if size > ut2004::MAX_FRAME {
        return Err(
            ProtocolError::Invalid(format!("Mensaje del master demasiado grande: {size}")).into(),
        );
    }
    let mut body = vec![0u8; size];
    sock.read_exact(&mut body).await?;
    Ok(body)
}

/// Challenge, respuesta fija, APPROVED, VERIFIED y lista. Un rechazo (DENIED) es
/// `Protocol` y no se reintenta. Un corte con entradas ya recibidas devuelve esas.
pub async fn fetch_master_ut2004(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    let mut entries: Vec<Vec<u8>> = Vec::new();
    let run = async {
        let mut sock = connect(host, port).await?;
        read_frame(&mut sock).await?; // challenge: la respuesta es fija
        sock.write_all(&ut2004::build_challenge_response()).await?;
        ut2004::check_reply(&read_frame(&mut sock).await?, "APPROVED")?;
        sock.write_all(&ut2004::verify_request()).await?;
        ut2004::check_reply(&read_frame(&mut sock).await?, "VERIFIED")?;
        sock.write_all(&ut2004::list_request()).await?;
        let count = ut2004::parse_list_header(&read_frame(&mut sock).await?)?;
        for _ in 0..count {
            entries.push(read_frame(&mut sock).await?);
        }
        Ok::<(), Cut>(())
    };
    match timeout_at((Instant::now() + timeout).into(), run).await {
        Ok(Err(Cut::Fatal(e))) => return Err(e),
        Ok(Err(Cut::Io(e))) if entries.is_empty() => {
            return Err(if e.kind() == io::ErrorKind::UnexpectedEof {
                ProtocolError::Invalid(format!("El master {host} cerró la conexión")).into()
            } else {
                e.into()
            });
        }
        Err(_) if entries.is_empty() => return Err(MasterFetchError::Timeout),
        _ => {}
    }
    let servers: IndexSet<ServerAddress> = entries
        .iter()
        .filter_map(|b| ut2004::parse_list_entry(b).ok())
        .collect();
    Ok(servers.into_iter().collect())
}
