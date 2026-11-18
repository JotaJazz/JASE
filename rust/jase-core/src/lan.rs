//! Búsqueda de servidores en la red local, sin master (como «LAN» en el ASE clásico).
//! Traducción de `src/jase/lan.py`.
//!
//! Se manda la consulta de la familia por difusión (255.255.255.255 y el .255 de las
//! direcciones IPv4 propias) a los puertos típicos del juego; quien contesta es un servidor
//! y después se consulta normalmente (`query_servers`), con los filtros del juego.
use crate::games::{Family, GameDef};
use crate::i18n::tr_n;
use crate::models::{ServerAddress, ServerInfo};
use crate::protocols::{gamespy, quake2, quake3, ut2004, valve, zandronum};
use crate::scanner::query::{game_query_options, query_servers};
use crate::scanner::scan::{ScanEvent, ScanResult};
use indexmap::IndexSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::{Instant, timeout_at};

/// Puertos de consulta por defecto (más los siguientes, para varios servidores en un equipo).
pub const RANGE: u16 = 4;
pub const GAME_PORTS: &[(&str, u16)] = &[
    ("cod2", 28960),
    ("cod1", 28960),
    ("coduo", 28960),
    ("cod4", 28960),
    ("q3", 27960),
    ("rtcw", 27960),
    ("et", 27960),
    ("urt", 27960),
    ("oa", 27960),
    ("smokinguns", 27960),
    ("jk2", 28070),
    ("jka", 29070),
    ("sof2", 20100),
    ("xonotic", 26000),
    ("warsow", 44400),
    ("warfork", 44400),
    ("tremulous", 30720),
    ("ut99", 7778),
    ("tacops", 7778),
    ("mohaa", 12300),
    ("mohaas", 12300),
    ("mohaab", 12300),
    ("bf1942", 23000),
    ("q2", 27910),
    ("dday", 27910),
    ("ut2004", 7778),
    ("zandronum", 10666),
];
pub const FAMILY_PORTS: &[(Family, u16)] = &[
    (Family::Valve, 27015),
    (Family::Quake3, 27960),
    (Family::Quake2, 27910),
    (Family::Gamespy, 7778),
    (Family::Ut2004, 7778),
    (Family::Zandronum, 10666),
];

/// La consulta que se difunde: la misma con la que el escáner pide el estado.
pub fn probe(family: Family) -> Vec<u8> {
    match family {
        Family::Quake3 => quake3::GETSTATUS.to_vec(),
        Family::Quake2 => quake2::STATUS_QUERY.to_vec(),
        Family::Gamespy => gamespy::STATUS_QUERY.to_vec(),
        Family::Valve => valve::A2S_INFO.to_vec(),
        Family::Ut2004 => ut2004::INFO_QUERY.to_vec(),
        Family::Zandronum => zandronum::build_query(0),
    }
}

pub fn lan_ports(game: &GameDef) -> Vec<u16> {
    let base = GAME_PORTS
        .iter()
        .find(|(id, _)| *id == game.id)
        .map(|(_, p)| *p)
        .or_else(|| {
            FAMILY_PORTS
                .iter()
                .find(|(f, _)| *f == game.family)
                .map(|(_, p)| *p)
        })
        .unwrap_or_default(); // toda familia está en FAMILY_PORTS (lo comprueba un test)
    let count = if game.family == Family::Valve {
        6
    } else {
        RANGE
    }; // Valve: 27015-27020
    (base..base.saturating_add(count)).collect()
}

/// 255.255.255.255 y el .255 (como /24) de cada IPv4 privada del equipo.
pub async fn broadcast_addresses() -> Vec<Ipv4Addr> {
    let mut own: Vec<Ipv4Addr> = Vec::new();
    #[cfg(windows)]
    if let Ok(name) = std::env::var("COMPUTERNAME")
        && let Ok(addrs) = tokio::net::lookup_host((name.as_str(), 0)).await
    {
        own.extend(addrs.filter_map(|a| match a.ip() {
            IpAddr::V4(ip) => Some(ip),
            IpAddr::V6(_) => None,
        }));
    }
    // IP de la interfaz con ruta por defecto (connect en UDP no envía nada).
    let routed = std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").and_then(|()| s.local_addr()));
    if let Ok(addr) = routed
        && let IpAddr::V4(ip) = addr.ip()
    {
        own.push(ip);
    }
    let mut result = vec![Ipv4Addr::BROADCAST];
    for ip in own {
        if (ip.is_private() || ip.is_link_local()) && !ip.is_loopback() {
            let [a, b, c, _] = ip.octets();
            let broadcast = Ipv4Addr::new(a, b, c, 255);
            if !result.contains(&broadcast) {
                result.push(broadcast);
            }
        }
    }
    result
}

/// Direcciones (IP:puerto de consulta) que contestan a la difusión, en orden de llegada.
pub async fn discover(
    game: &GameDef,
    timeout: Duration,
    targets: &[Ipv4Addr],
    ports: &[u16],
) -> io::Result<Vec<ServerAddress>> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.set_broadcast(true)?;
    let probe = probe(game.family);
    for &target in targets {
        for &port in ports {
            let _ = sock.send_to(&probe, (target, port)).await; // sin ruta: se sigue
        }
    }
    let deadline = Instant::now() + timeout;
    let mut found: IndexSet<ServerAddress> = IndexSet::new();
    let mut buf = vec![0u8; 65535];
    // Los errores de `recv_from` (en Windows, el ICMP de un puerto cerrado) se ignoran.
    while let Ok(received) = timeout_at(deadline, sock.recv_from(&mut buf)).await {
        if let Ok((n, from)) = received
            && n > 0
        {
            found.insert(ServerAddress::new(from.ip().to_string(), from.port()));
        }
    }
    Ok(found.into_iter().collect())
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanOptions {
    pub discover_timeout: Duration,
    pub query_timeout: Duration,
    /// `None` = `broadcast_addresses()`.
    pub targets: Option<Vec<Ipv4Addr>>,
    /// `None` = `lan_ports(game)`.
    pub ports: Option<Vec<u16>>,
}

impl Default for LanOptions {
    fn default() -> Self {
        Self {
            discover_timeout: Duration::from_millis(1500),
            query_timeout: Duration::from_millis(1500),
            targets: None,
            ports: None,
        }
    }
}

/// Igual que `scan`, pero buscando en la red local en vez de en los masters. Emite un
/// `Status` y un `Server` por respuesta que `game.lists_server` acepta (sin `Progress`,
/// como Python). Soltar el future cancela.
pub async fn scan_lan(
    game: &GameDef,
    opts: &LanOptions,
    events: &mut (dyn FnMut(ScanEvent) + Send),
) -> io::Result<ScanResult> {
    let targets = match &opts.targets {
        Some(t) => t.clone(),
        None => broadcast_addresses().await,
    };
    let ports = opts.ports.clone().unwrap_or_else(|| lan_ports(game));
    let found = discover(game, opts.discover_timeout, &targets, &ports).await?;
    events(ScanEvent::Status(tr_n(
        "{n} servidor en la red local, consultando…",
        "{n} servidores en la red local, consultando…",
        found.len() as i64,
        &[],
    )));
    let listed = found.len();
    let mut responded = 0;
    let mut on_server = |info: ServerInfo| {
        if game.lists_server(&info.rules) {
            responded += 1;
            events(ScanEvent::Server(info));
        }
    };
    let query_opts = game_query_options(game);
    query_servers(
        game.family,
        found,
        &query_opts,
        opts.query_timeout,
        1,
        64,
        &mut on_server,
        None,
    )
    .await?;
    Ok(ScanResult {
        masters_ok: 1,
        servers_listed: listed,
        servers_responded: responded,
        ..ScanResult::default()
    })
}
