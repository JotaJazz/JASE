//! Escaneo completo: masters en paralelo, deduplicado y consulta de cada servidor.
use super::MasterFetchError;
use super::masters_tcp::{fetch_master_gamespy, fetch_master_ut2004};
use super::masters_udp::{
    VALVE_MASTER_MAX_PAGES, fetch_master_quake2, fetch_master_quake3, fetch_master_valve,
    fetch_master_zandronum,
};
use super::query::{game_query_options, query_servers};
use super::steam::{fetch_master_steamweb_with, steam_filter, steamweb_download};
use crate::games::{Family, GameDef};
use crate::i18n::{tr, tr_fmt, tr_n};
use crate::masters::Master;
use crate::models::{ServerAddress, ServerInfo};
use crate::protocols::{ProtocolError, steamweb};
use futures::future::join_all;
use indexmap::IndexSet;
use std::sync::Mutex;
use std::time::Duration;

/// Fallo de un master. `kind` es estable (la CLI lo publica en su JSON): ver
/// [`MasterFetchError::kind`]. `message` es el texto traducido que ve el usuario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasterError {
    pub master: String, // "host:puerto"
    pub kind: &'static str,
    pub message: String,
}

/// Resumen de un escaneo; `ok()` es false si ningún master respondió.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub masters_ok: usize,
    pub servers_listed: usize,
    pub servers_responded: usize,
    pub errors: Vec<MasterError>,
}

impl ScanResult {
    pub fn ok(&self) -> bool {
        self.masters_ok > 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScanOptions {
    pub master_timeout: Duration,
    pub query_timeout: Duration,
    pub steam_key: String,
    pub steam_include_empty: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            master_timeout: Duration::from_secs(3),
            query_timeout: Duration::from_millis(1500),
            steam_key: String::new(),
            steam_include_empty: false,
        }
    }
}

// Cada evento se mueve al callback y no se guarda: Box solo añadiría una asignación.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum ScanEvent {
    Server(ServerInfo),
    Status(String),
    Progress { done: usize, total: usize },
}

/// Pide la lista al master según la familia del juego (o a la API web de Steam).
pub async fn fetch_master(
    game: &GameDef,
    host: &str,
    port: u16,
    timeout: Duration,
    steam_key: &str,
    steam_include_empty: bool,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    let download = |url| steamweb_download(url, timeout);
    fetch_master_with(
        game,
        host,
        port,
        timeout,
        steam_key,
        steam_include_empty,
        download,
    )
    .await
}

async fn fetch_master_with<F, Fut>(
    game: &GameDef,
    host: &str,
    port: u16,
    timeout: Duration,
    steam_key: &str,
    steam_include_empty: bool,
    download: F,
) -> Result<Vec<ServerAddress>, MasterFetchError>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, MasterFetchError>>,
{
    let official = host.to_lowercase() == steamweb::OFFICIAL_HOST;
    if game.steam_webapi || (game.steam_webapi_optional && official) {
        if steam_key.is_empty() {
            return Err(MasterFetchError::SteamWeb {
                kind: "missing_key",
                message: tr("Falta la clave de API de Steam; ponla en Ajustes → General"),
            });
        }
        // Por defecto solo servidores no vacíos (la lista completa es enorme).
        let filter = steam_filter(game, steam_include_empty);
        return fetch_master_steamweb_with(host, port, &filter, steam_key, download).await;
    }
    // Un GameDef incompleto es un fallo de la tabla (ValueError en Python): aquí, Protocol.
    let incomplete = |what: &str| {
        MasterFetchError::Protocol(ProtocolError::Invalid(format!("{}: {what}", game.id)))
    };
    match game.family {
        Family::Quake3 => {
            fetch_master_quake3(host, port, game.protocols, timeout, game.master_gamename).await
        }
        Family::Quake2 => fetch_master_quake2(host, port, timeout).await,
        Family::Ut2004 => fetch_master_ut2004(host, port, timeout).await,
        Family::Zandronum => fetch_master_zandronum(host, port, timeout).await,
        Family::Gamespy => {
            if game.gamespy_gamename.is_empty() || game.gamespy_key.is_empty() {
                return Err(incomplete("faltan gamespy_gamename/gamespy_key"));
            }
            let (name, key) = (game.gamespy_gamename, game.gamespy_key);
            fetch_master_gamespy(host, port, name, key, timeout).await
        }
        Family::Valve => {
            let filter = if !game.valve_master_filter.is_empty() {
                game.valve_master_filter.to_owned()
            } else if game.valve_appid != 0 {
                format!("\\appid\\{}", game.valve_appid)
            } else {
                return Err(incomplete("falta valve_appid o valve_master_filter"));
            };
            fetch_master_valve(host, port, &filter, timeout, VALVE_MASTER_MAX_PAGES).await
        }
    }
}

/// Consulta los masters en paralelo, deduplica y consulta cada servidor. Emite `Status`,
/// `Progress { done: 0, total }` antes de consultar y uno por servidor terminado, y
/// `Server` por cada respuesta que `game.lists_server` acepta. Soltar el future cancela.
pub async fn scan(
    game: &GameDef,
    masters: &[Master],
    opts: &ScanOptions,
    events: &mut (dyn FnMut(ScanEvent) + Send),
) -> ScanResult {
    let timeout = opts.master_timeout;
    scan_with(game, masters, opts, events, |url| {
        steamweb_download(url, timeout)
    })
    .await
}

/// Como [`scan`] con la descarga de la API de Steam inyectada (los tests no usan Internet).
pub async fn scan_with<F, Fut>(
    game: &GameDef,
    masters: &[Master],
    opts: &ScanOptions,
    events: &mut (dyn FnMut(ScanEvent) + Send),
    download: F,
) -> ScanResult
where
    F: Fn(String) -> Fut + Sync,
    Fut: Future<Output = Result<Vec<u8>, MasterFetchError>> + Send,
{
    let (key, include_empty) = (opts.steam_key.as_str(), opts.steam_include_empty);
    let fetches = masters.iter().map(|(host, port)| {
        fetch_master_with(
            game,
            host,
            *port,
            opts.master_timeout,
            key,
            include_empty,
            &download,
        )
    });
    let results = join_all(fetches).await;

    let mut addresses: IndexSet<ServerAddress> = IndexSet::new();
    let mut result = ScanResult::default();
    for ((host, port), fetched) in masters.iter().zip(results) {
        let e = match fetched {
            Ok(list) => {
                result.masters_ok += 1;
                addresses.extend(list);
                continue;
            }
            Err(e) => e,
        };
        let message = match &e {
            MasterFetchError::SteamWeb { message, .. } => message.clone(),
            MasterFetchError::Busy => tr_fmt(
                "El master {host} pide esperar unos segundos entre consultas",
                &[("host", host)],
            ),
            _ => tr_fmt("Master {host} no responde", &[("host", host)]),
        };
        result.errors.push(MasterError {
            master: format!("{host}:{port}"),
            kind: e.kind(),
            message: message.clone(),
        });
        events(ScanEvent::Status(message));
    }
    if result.masters_ok == 0 {
        events(ScanEvent::Status(tr("Ningún master respondió")));
        return result;
    }
    let total = addresses.len();
    result.servers_listed = total;
    events(ScanEvent::Status(tr_n(
        "{n} servidor en el master, consultando…",
        "{n} servidores en el master, consultando…",
        total as i64,
        &[],
    )));
    events(ScanEvent::Progress { done: 0, total });

    // Los dos callbacks emiten eventos: comparten `events` tras un Mutex (nunca a
    // través de un await: se toma y se suelta dentro de cada llamada).
    let events = Mutex::new(events);
    let mut responded = 0;
    let mut done = 0;
    let mut on_server = |info: ServerInfo| {
        if game.lists_server(&info.rules) {
            responded += 1;
            (events.lock().unwrap())(ScanEvent::Server(info));
        }
    };
    let mut on_done = || {
        done += 1;
        (events.lock().unwrap())(ScanEvent::Progress { done, total });
    };
    let query_opts = game_query_options(game);
    let addresses = addresses.into_iter().collect();
    // Sin socket local no hay consultas: el resumen queda con 0 respuestas.
    let _ = query_servers(
        game.family,
        addresses,
        &query_opts,
        opts.query_timeout,
        1,
        64,
        &mut on_server,
        Some(&mut on_done),
    )
    .await;
    result.servers_responded = responded;
    result
}
