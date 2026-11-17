//! Consultas a servidores en paralelo por un socket UDP compartido.
use super::resolve_v4;
use crate::games::{Family, GameDef};
use crate::models::{ServerAddress, ServerInfo};
use crate::protocols::quake3::VersionAlias;
use crate::protocols::{ProtocolError, gamespy, quake2, quake3, ut2004, zandronum};
use futures::{StreamExt, stream};
use indexmap::{IndexMap, IndexSet};
use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::{timeout, timeout_at};

/// Tras la información de UT2004, espera sin datagramas nuevos por reglas y jugadores.
pub const UT2004_QUERY_GRACE: Duration = Duration::from_millis(250);

/// Esperas fijas del protocolo, parametrizables para los tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryTiming {
    /// Pausa antes de repetir una consulta que Zandronum ignoró.
    pub zandronum_ignore_wait: Duration,
}

impl Default for QueryTiming {
    fn default() -> Self {
        Self {
            zandronum_ignore_wait: Duration::from_secs_f64(zandronum::IGNORE_WAIT_S),
        }
    }
}

/// Opciones de `query_servers` que dependen del juego; ver `game_query_options`.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryOptions {
    pub connect_port_fallback: i32,
    pub version_key: String,
    pub version_aliases: Vec<VersionAlias>,
    pub exclude_mods: Vec<String>,
    pub expected_gamename: String,
    pub expected_gamedir: String,
    pub timing: QueryTiming,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            connect_port_fallback: -1,
            version_key: "shortversion".into(),
            version_aliases: Vec::new(),
            exclude_mods: Vec::new(),
            expected_gamename: String::new(),
            expected_gamedir: String::new(),
            timing: QueryTiming::default(),
        }
    }
}

pub fn game_query_options(game: &GameDef) -> QueryOptions {
    QueryOptions {
        connect_port_fallback: game.connect_port_fallback,
        version_key: game.version_key.into(),
        version_aliases: game.version_aliases(),
        exclude_mods: game.status_exclude_mods.iter().map(|&m| m.into()).collect(),
        expected_gamename: game.status_gamename.into(),
        expected_gamedir: if game.family == Family::Valve {
            game.valve_gamedir
        } else {
            game.status_gamedir
        }
        .into(),
        timing: QueryTiming::default(),
    }
}

type Datagram = (Vec<u8>, Instant);

/// Socket compartido: el lector reparte cada datagrama por (ip, puerto) de origen a la
/// consulta registrada.
struct Router {
    sock: UdpSocket,
    /// Origen de `zandronum::build_query` (cualquier reloj monótono sirve).
    epoch: Instant,
    routes: Mutex<HashMap<SocketAddrV4, mpsc::UnboundedSender<Datagram>>>,
    locks: Mutex<HashMap<SocketAddrV4, Arc<tokio::sync::Mutex<()>>>>,
}

impl Router {
    fn register(&self, key: SocketAddrV4) -> mpsc::UnboundedReceiver<Datagram> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.routes.lock().unwrap().insert(key, tx);
        rx
    }

    fn unregister(&self, key: SocketAddrV4) {
        self.routes.lock().unwrap().remove(&key);
    }

    async fn send(&self, data: &[u8], key: SocketAddrV4) {
        // Un fallo de envío se resuelve como un servidor que no responde.
        let _ = self.sock.send_to(data, key).await;
    }

    async fn read_forever(&self) {
        let mut buf = vec![0u8; 65535];
        loop {
            // Los errores (ICMP sueltos, ConnectionReset en Windows) se resuelven por timeout.
            if let Ok((n, SocketAddr::V4(from))) = self.sock.recv_from(&mut buf).await
                && let Some(tx) = self.routes.lock().unwrap().get(&from)
            {
                let _ = tx.send((buf[..n].to_vec(), Instant::now()));
            }
        }
    }
}

fn ms(d: Duration) -> u32 {
    (d.as_secs_f64() * 1000.0).round() as u32
}

/// Envía y espera con reintentos: (payload, ping en ms), o None si se agotan los intentos
/// o el manejador falla con ProtocolError. `build_query` recibe la hora del envío;
/// `make_handler` da, por intento, uno nuevo que recibe cada datagrama y devuelve
/// `Some` al completar la respuesta (con la hora que cuenta para el ping).
async fn exchange<T, H>(
    router: &Router,
    key: SocketAddrV4,
    timeout_each: Duration,
    retries: u32,
    build_query: impl Fn(Instant) -> Vec<u8>,
    make_handler: impl Fn() -> H,
) -> Option<(T, u32)>
where
    H: FnMut(&[u8], Instant) -> Option<Result<(T, Instant), ProtocolError>>,
{
    let mut first_sent = None;
    for attempt in 0..=retries {
        let mut rx = router.register(key);
        let mut handler = make_handler();
        let sent = Instant::now();
        let first = *first_sent.get_or_insert(sent);
        router.send(&build_query(sent), key).await;
        let reply = timeout(timeout_each, async {
            while let Some((data, at)) = rx.recv().await {
                if let Some(r) = handler(&data, at) {
                    return Some(r);
                }
            }
            None
        })
        .await;
        router.unregister(key);
        match reply {
            Ok(Some(Ok((payload, received)))) => {
                // En un reintento la respuesta puede ser tardía del intento anterior:
                // medir desde el primer envío (un servidor lento nunca sale verde).
                let start = if attempt == 0 { sent } else { first };
                return Some((payload, ms(received.saturating_duration_since(start))));
            }
            Ok(Some(Err(_))) => return None,
            _ => continue,
        }
    }
    None
}

fn first_packet() -> impl FnMut(&[u8], Instant) -> Option<Result<Datagram, ProtocolError>> {
    |data, at| Some(Ok((data.to_vec(), at)))
}

type GamespyReply = Result<(IndexMap<String, String>, Instant), ProtocolError>;

/// Reensambla el estado; el ping es el del primer paquete.
fn gamespy_status() -> impl FnMut(&[u8], Instant) -> Option<GamespyReply> {
    let mut assembler = gamespy::StatusAssembler::new();
    let mut first = None;
    move |data, at| {
        let first = *first.get_or_insert(at);
        match assembler.feed(data) {
            Ok(true) => Some(Ok((assembler.fields(), first))),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

async fn query_zandronum(
    router: &Router,
    address: &ServerAddress,
    key: SocketAddrV4,
    timeout_each: Duration,
    mut retries: u32,
    wait: Duration,
) -> Option<ServerInfo> {
    let epoch = router.epoch;
    let build =
        |sent: Instant| zandronum::build_query(sent.duration_since(epoch).as_millis() as u64);
    for ignored_before in [false, true] {
        let (payload, ping) =
            exchange(router, key, timeout_each, retries, build, first_packet).await?;
        match zandronum::parse_response(&payload, address, ping) {
            // Hubo otra consulta hace poco (escaneo, vigilancia, LAN): se espera y se
            // repite una vez, en vez de darlo por caído.
            Err(ProtocolError::ServerIgnoring(_)) if !ignored_before => {
                tokio::time::sleep(wait).await;
                retries = 0;
            }
            r => return r.ok(),
        }
    }
    None
}

/// Datagrama de respuesta de UT2004 (cabecera de consulta + tipo).
fn is_ut2004_reply(data: &[u8]) -> bool {
    data.len() >= 5 && data.starts_with(&ut2004::INFO_QUERY[..4])
}

/// Pide información, reglas y jugadores a la vez; tras la información espera
/// `UT2004_QUERY_GRACE` sin datagramas nuevos (o `timeout`) por el resto.
async fn query_ut2004(
    router: &Router,
    address: &ServerAddress,
    key: SocketAddrV4,
    timeout_each: Duration,
    retries: u32,
) -> Option<ServerInfo> {
    let (info_kind, rules_kind, players_kind) = (
        ut2004::INFO_QUERY[4],
        ut2004::RULES_QUERY[4],
        ut2004::PLAYERS_QUERY[4],
    );
    let mut first_sent = None;
    for attempt in 0..=retries {
        let mut rx = router.register(key);
        let sent = Instant::now();
        let first = *first_sent.get_or_insert(sent);
        for query in [
            ut2004::INFO_QUERY,
            ut2004::RULES_QUERY,
            ut2004::PLAYERS_QUERY,
        ] {
            router.send(query, key).await;
        }
        let mut packets: Vec<Vec<u8>> = Vec::new();
        let info_at = timeout(timeout_each, async {
            while let Some((data, at)) = rx.recv().await {
                if is_ut2004_reply(&data) {
                    let is_info = data[4] == info_kind;
                    packets.push(data);
                    if is_info {
                        return Some(at);
                    }
                }
            }
            None
        })
        .await;
        let Ok(Some(received)) = info_at else {
            router.unregister(key);
            continue;
        };
        let stop = Instant::now() + timeout_each;
        loop {
            let limit = stop.min(Instant::now() + UT2004_QUERY_GRACE);
            let next = timeout_at(limit.into(), async {
                while let Some((data, _)) = rx.recv().await {
                    if is_ut2004_reply(&data) {
                        return Some(data);
                    }
                }
                None
            });
            match next.await {
                Ok(Some(data)) => packets.push(data),
                _ => break,
            }
        }
        router.unregister(key);
        let start = if attempt == 0 { sent } else { first };
        let ping = ms(received.saturating_duration_since(start));
        let of_kind = |kind: u8| packets.iter().filter(move |p| p[4] == kind);
        let mut info = ut2004::parse_info(of_kind(info_kind).next()?, address, ping).ok()?;
        // Un datagrama de reglas o jugadores dañado no invalida el servidor.
        let mut rules = IndexMap::new();
        for data in of_kind(rules_kind) {
            let _ = ut2004::parse_rules(data, &mut rules);
        }
        let players = of_kind(players_kind)
            .filter_map(|d| ut2004::parse_players(d).ok())
            .flatten()
            .collect();
        ut2004::apply_details(&mut info, rules, players);
        return Some(info);
    }
    None
}

async fn query_one(
    router: &Router,
    family: Family,
    address: &ServerAddress,
    opts: &QueryOptions,
    timeout_each: Duration,
    retries: u32,
) -> Option<ServerInfo> {
    let ip = match address.host.parse::<Ipv4Addr>() {
        Ok(ip) => ip,
        Err(_) => *resolve_v4(&address.host, address.port).await.ok()?.ip(),
    };
    let key = SocketAddrV4::new(ip, address.port);
    // Una consulta a la vez por IP:puerto: dos nombres del mismo servidor (favorito por
    // nombre, reciente por IP) se pisarían la ruta del lector.
    let lock = Arc::clone(router.locks.lock().unwrap().entry(key).or_default());
    let _guard = lock.lock().await;
    let (t, r) = (timeout_each, retries);
    match family {
        Family::Quake3 => {
            let (payload, ping) = exchange(
                router,
                key,
                t,
                r,
                |_| quake3::GETSTATUS.to_vec(),
                first_packet,
            )
            .await?;
            let o = quake3::StatusOptions {
                version_key: &opts.version_key,
                version_aliases: &opts.version_aliases,
                exclude_mods: &opts.exclude_mods,
            };
            quake3::parse_status_response(&payload, address, ping, &o).ok()
        }
        Family::Quake2 => {
            let (payload, ping) = exchange(
                router,
                key,
                t,
                r,
                |_| quake2::STATUS_QUERY.to_vec(),
                first_packet,
            )
            .await?;
            let o = quake2::Quake2Options {
                version_key: &opts.version_key,
                expected_gamedir: &opts.expected_gamedir,
                exclude_mods: &opts.exclude_mods,
            };
            quake2::parse_status(&payload, address, ping, &o).ok()
        }
        Family::Gamespy => {
            let build = |_| gamespy::STATUS_QUERY.to_vec();
            let (fields, ping) = exchange(router, key, t, r, build, gamespy_status).await?;
            let o = gamespy::GamespyOptions {
                connect_port_fallback: opts.connect_port_fallback,
                expected_gamename: &opts.expected_gamename,
                version_key: &opts.version_key,
                version_aliases: &opts.version_aliases,
            };
            gamespy::parse_status(&fields, address, ping, &o).ok()
        }
        Family::Ut2004 => query_ut2004(router, address, key, t, r).await,
        Family::Zandronum => {
            let wait = opts.timing.zandronum_ignore_wait;
            query_zandronum(router, address, key, t, r, wait).await
        }
        // shortcut: Valve aún sin portar (fase 2b, tarea 5); sus servidores no responden.
        Family::Valve => None,
    }
}

/// Consulta servidores en paralelo; llama a `on_server` por cada respuesta válida y
/// `on_done` por cada servidor cuya consulta terminó (respondiera o no). Todo corre en
/// esta tarea: el lector del socket unido a las consultas; soltar el future cancela.
#[allow(clippy::too_many_arguments)]
pub async fn query_servers(
    family: Family,
    addresses: Vec<ServerAddress>,
    opts: &QueryOptions,
    timeout: Duration,
    retries: u32,
    concurrency: usize,
    on_server: &mut (dyn FnMut(ServerInfo) + Send),
    mut on_done: Option<&mut (dyn FnMut() + Send)>,
) -> io::Result<()> {
    let router = Router {
        sock: UdpSocket::bind("0.0.0.0:0").await?,
        epoch: Instant::now(),
        routes: Mutex::default(),
        locks: Mutex::default(),
    };
    let unique: IndexSet<ServerAddress> = addresses.into_iter().collect();
    let router = &router;
    let mut results = pin!(
        stream::iter(unique)
            .map(|a| async move { query_one(router, family, &a, opts, timeout, retries).await })
            .buffer_unordered(concurrency.max(1))
    );
    let queries = async {
        while let Some(info) = results.next().await {
            if let Some(info) = info {
                on_server(info);
            }
            if let Some(done) = on_done.as_mut() {
                done();
            }
        }
    };
    tokio::select! {
        () = queries => {}
        () = router.read_forever() => {}
    }
    Ok(())
}
