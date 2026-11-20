//! Listas de servidores por UDP: Quake 3, Quake II, Zandronum y Valve.
use super::{MasterFetchError, resolve_v4};
use crate::models::ServerAddress;
use crate::protocols::{ProtocolError, quake2, quake3, valve, zandronum};
use futures::future::join_all;
use indexmap::IndexSet;
use std::io;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::{timeout, timeout_at};

/// Espera tras el último datagrama válido por si llegan más del mismo master.
pub const QUAKE2_MASTER_GRACE: Duration = Duration::from_millis(500);
/// Valve limita las consultas al master: pocas páginas (~230 servidores cada una).
pub const VALVE_MASTER_MAX_PAGES: u32 = 20;

/// Socket UDP conectado al master (resuelto y con la consulta ya enviada).
async fn connect_and_send(
    host: &str,
    port: u16,
    query: &[u8],
) -> Result<UdpSocket, MasterFetchError> {
    let remote = resolve_v4(host, port).await?;
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(remote).await?;
    sock.send(query).await?;
    Ok(sock)
}

async fn recv(sock: &UdpSocket) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; 65535];
    let n = sock.recv(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

#[derive(Default)]
struct Collected {
    servers: IndexSet<ServerAddress>,
    got_valid: bool,
    first_error: Option<MasterFetchError>,
}

impl Collected {
    fn into_result(self) -> Result<Vec<ServerAddress>, MasterFetchError> {
        if self.got_valid {
            Ok(self.servers.into_iter().collect())
        } else {
            Err(self.first_error.unwrap_or(MasterFetchError::Timeout))
        }
    }
}

/// Un socket por protocolo; recoge paquetes hasta el último (\EOF) en todos o `timeout`.
/// `gamename` va en la consulta para los masters que mezclan juegos (dpmaster).
pub async fn fetch_master_quake3(
    host: &str,
    port: u16,
    protocols: &[u32],
    timeout: Duration,
    gamename: &str,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    let state = Mutex::new(Collected::default());
    let one = |proto: u32| {
        let state = &state;
        async move {
            let query = quake3::build_getservers(proto, gamename);
            let fail = |e: MasterFetchError| {
                state.lock().unwrap().first_error.get_or_insert(e);
            };
            let sock = match connect_and_send(host, port, &query).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            loop {
                let data = match recv(&sock).await {
                    Ok(d) => d,
                    Err(e) => return fail(e.into()),
                };
                match quake3::parse_getservers_response(&data) {
                    Err(e) => fail(e.into()),
                    Ok((found, eot)) => {
                        let mut st = state.lock().unwrap();
                        st.got_valid = true;
                        st.servers.extend(found);
                        if eot {
                            return;
                        }
                    }
                }
            }
        }
    };
    // Un único plazo: DNS + envío + lectura. Al vencer vale lo recogido.
    let _ = timeout_at(
        (Instant::now() + timeout).into(),
        join_all(protocols.iter().map(|&p| one(p))),
    )
    .await;
    state.into_inner().unwrap().into_result()
}

type MasterPacket = (Vec<ServerAddress>, bool);

/// Un datagrama `query`; la lista llega en uno o más datagramas (se acumulan los que
/// lleguen dentro del plazo, tras el primero válido se espera poco por más). `parse`
/// devuelve (direcciones, es_el_último). Un paquete malo corta si `stop_on_bad`.
async fn fetch_master_datagrams(
    host: &str,
    port: u16,
    timeout: Duration,
    query: &[u8],
    parse: impl Fn(&[u8]) -> Result<MasterPacket, ProtocolError>,
    stop_on_bad: bool,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    let deadline = Instant::now() + timeout;
    let mut st = Collected::default();
    let sock = match timeout_at(deadline.into(), connect_and_send(host, port, query)).await {
        Ok(r) => r?,
        Err(_) => return Err(MasterFetchError::Timeout),
    };
    loop {
        let mut limit = deadline;
        if st.got_valid {
            limit = limit.min(Instant::now() + QUAKE2_MASTER_GRACE);
        }
        let data = match timeout_at(limit.into(), recv(&sock)).await {
            Err(_) => break,
            Ok(Err(e)) => {
                st.first_error.get_or_insert(e.into());
                break;
            }
            Ok(Ok(d)) => d,
        };
        match parse(&data) {
            Err(e) => {
                st.first_error.get_or_insert(e.into());
                if stop_on_bad {
                    break;
                }
            }
            Ok((found, last)) => {
                st.servers.extend(found);
                st.got_valid = true;
                if last {
                    break;
                }
            }
        }
    }
    st.into_result()
}

/// Sin marca de final: se acumula hasta el plazo; un paquete malo no corta.
pub async fn fetch_master_quake2(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    fetch_master_datagrams(
        host,
        port,
        timeout,
        quake2::MASTER_QUERY,
        |d| quake2::parse_master_response(d).map(|f| (f, false)),
        false,
    )
    .await
}

/// Datagramas numerados; el último trae MSC_ENDSERVERLIST. Si no llega el final dentro
/// del plazo, vale lo recibido (como en Quake II).
pub async fn fetch_master_zandronum(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    fetch_master_datagrams(
        host,
        port,
        timeout,
        &zandronum::master_request(),
        zandronum::parse_master_packet,
        true,
    )
    .await
}

/// Master Server Query de Valve: una página por ida y vuelta, cada una sembrada con la
/// última dirección de la anterior, hasta 0.0.0.0:0 o `max_pages`. `timeout` es por
/// página (incluye el DNS en la primera). Si una página posterior falla (Valve limita),
/// se devuelve lo ya recibido.
pub async fn fetch_master_valve(
    host: &str,
    port: u16,
    filter: &str,
    page_timeout: Duration,
    max_pages: u32,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    let mut servers: IndexSet<ServerAddress> = IndexSet::new();
    let mut sock: Option<UdpSocket> = None;
    let mut seed: Option<ServerAddress> = None;
    let mut seen: IndexSet<ServerAddress> = IndexSet::new();
    for page in 0..max_pages {
        let step = async {
            if sock.is_none() {
                let s = UdpSocket::bind("0.0.0.0:0").await?;
                s.connect(resolve_v4(host, port).await?).await?;
                sock = Some(s);
            }
            let s = sock.as_ref().expect("socket creado arriba");
            s.send(&valve::build_master_query(seed.as_ref(), filter))
                .await?;
            let data = recv(s).await?;
            Ok::<_, MasterFetchError>(valve::parse_master_response(&data)?)
        };
        let (found, next) = match timeout(page_timeout, step).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) if page == 0 => return Err(e),
            Err(_) if page == 0 => return Err(MasterFetchError::Timeout),
            _ => break, // página tardía o inválida: quedarse con lo recibido
        };
        servers.extend(found);
        match next {
            Some(n) if seen.insert(n.clone()) => seed = Some(n),
            _ => break,
        }
    }
    Ok(servers.into_iter().collect())
}
