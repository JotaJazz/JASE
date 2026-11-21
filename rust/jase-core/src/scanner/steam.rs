//! Lista de servidores por la API web de Steam (HTTPS, con clave).
use super::MasterFetchError;
use crate::games::GameDef;
use crate::i18n::{tr, tr_fmt};
use crate::models::ServerAddress;
use crate::protocols::steamweb;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;
use std::io::Read;
use std::time::Duration;

// La API devuelve como mucho 10 000 servidores por consulta. Sondeo 2026-10-29 (CS 1.6):
// unas 200 IPs publican 200 servidores falsos cada una («This is a dummy CS 1.6 server!»
// o nombres copiados de servidores reales) y llenan ese tope, dejando fuera a los reales.
pub const STEAMWEB_LIMIT: usize = 10000;
pub const STEAMWEB_SPAM_PER_IP: usize = 100; // ningún hosting real visto pasa de ~20 por IP
pub const STEAMWEB_MAX_PASSES: usize = 6;
pub const STEAMWEB_MAX_EXCLUDED: usize = 400; // IPs excluidas (la URL crece ~27 B por IP)
pub const STEAMWEB_SPAM_NAMES: [&str; 1] = ["This is a dummy*"];
const STEAMWEB_MAX_BYTES: u64 = 16 * 1024 * 1024;
/// `limit` que pide Python (el valor por defecto de `steamweb.build_url`).
const URL_LIMIT: u32 = 20000;

/// Filtro de la API web: exige el appid (solo `\gamedir\` devuelve 0 servidores).
pub fn steam_filter(game: &GameDef, include_empty: bool) -> String {
    let mut flt = format!("\\appid\\{}", game.valve_appid);
    if !game.valve_gamedir.is_empty() {
        let _ = write!(flt, "\\gamedir\\{}", game.valve_gamedir);
    }
    if !include_empty {
        flt.push_str("\\empty\\1");
    }
    flt
}

fn steam_error(kind: &'static str, message: String) -> MasterFetchError {
    MasterFetchError::SteamWeb { kind, message }
}

pub(super) fn unsupported_host() -> MasterFetchError {
    let host = steamweb::OFFICIAL_HOST;
    steam_error(
        "unsupported_host",
        tr_fmt(
            "Solo se admite {host} como fuente de Steam",
            &[("host", &host)],
        ),
    )
}

/// Lista sin el spam. La clave solo va a `steamweb::OFFICIAL_HOST`: otro host se rechaza
/// sin enviar nada. Ver [`fetch_master_steamweb_with`].
pub async fn fetch_master_steamweb(
    host: &str,
    port: u16,
    filter: &str,
    key: &str,
    timeout: Duration,
) -> Result<Vec<ServerAddress>, MasterFetchError> {
    fetch_master_steamweb_with(host, port, filter, key, |url| {
        steamweb_download(url, timeout)
    })
    .await
}

/// Como [`fetch_master_steamweb`] con la descarga inyectada (los tests no usan Internet).
///
/// Spam: se excluyen por nombre los servidores de `STEAMWEB_SPAM_NAMES` y se descartan las
/// IPs con `STEAMWEB_SPAM_PER_IP` servidores o más. Si la respuesta llegó al tope (puede
/// faltar gente) y apareció spam nuevo, se repite la consulta excluyendo esas IPs con
/// `\nor\`, como mucho `STEAMWEB_MAX_PASSES` veces.
pub async fn fetch_master_steamweb_with<F, Fut>(
    host: &str,
    _port: u16,
    filter: &str,
    key: &str,
    download: F,
) -> Result<Vec<ServerAddress>, MasterFetchError>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, MasterFetchError>>,
{
    if host.to_lowercase() != steamweb::OFFICIAL_HOST {
        return Err(unsupported_host());
    }
    let mut base = filter.to_owned();
    for name in STEAMWEB_SPAM_NAMES {
        let _ = write!(base, "\\nand\\1\\name_match\\{name}");
    }
    let mut spam: BTreeSet<String> = BTreeSet::new();
    let mut servers = Vec::new();
    for _ in 0..STEAMWEB_MAX_PASSES {
        let mut flt = base.clone();
        let excluded: Vec<&String> = spam.iter().take(STEAMWEB_MAX_EXCLUDED).collect();
        if !excluded.is_empty() {
            let _ = write!(flt, "\\nor\\{}", excluded.len());
            for ip in excluded {
                let _ = write!(flt, "\\gameaddr\\{ip}");
            }
        }
        let url = steamweb::build_url(host, key, &flt, URL_LIMIT)?;
        let entries;
        (servers, entries) = steamweb::parse_response(&download(url).await?)?;
        let mut per_ip: HashMap<&str, usize> = HashMap::new();
        for a in &servers {
            *per_ip.entry(&a.host).or_default() += 1;
        }
        let new: Vec<String> = per_ip
            .into_iter()
            .filter(|(ip, n)| *n >= STEAMWEB_SPAM_PER_IP && !spam.contains(*ip))
            .map(|(ip, _)| ip.to_owned())
            .collect();
        let found_new = !new.is_empty();
        spam.extend(new);
        let truncated = entries >= STEAMWEB_LIMIT - STEAMWEB_LIMIT / 100;
        if !(truncated && found_new) || spam.len() > STEAMWEB_MAX_EXCLUDED {
            break;
        }
    }
    servers.retain(|a| !spam.contains(&a.host));
    Ok(servers)
}

/// GET bloqueante en `spawn_blocking`. 401/403 y 429 son errores de Steam con texto para
/// el usuario; el resto, `Network` con el código o el tipo de fallo. Nunca la URL: lleva
/// la clave. Como `urlopen`, `timeout` vale por operación de socket; se leen como mucho
/// 16 MiB (lo que pase de ahí corta el JSON y da error de protocolo).
pub async fn steamweb_download(
    url: String,
    timeout: Duration,
) -> Result<Vec<u8>, MasterFetchError> {
    tokio::task::spawn_blocking(move || download_blocking(&url, timeout))
        .await
        .unwrap_or_else(|_| Err(MasterFetchError::Network("JoinError".into())))
}

fn download_blocking(url: &str, timeout: Duration) -> Result<Vec<u8>, MasterFetchError> {
    let t = Some(timeout);
    let agent: ureq::Agent = crate::net::agent_builder()
        .timeout_resolve(t)
        .timeout_connect(t)
        .timeout_per_read(t)
        .timeout_per_write(t)
        .build()
        .into();
    let resp = agent.get(url).call().map_err(http_error)?;
    let mut data = Vec::new();
    resp.into_body()
        .into_reader()
        .take(STEAMWEB_MAX_BYTES)
        .read_to_end(&mut data)
        .map_err(|e| {
            // Como Python: el timeout leyendo el cuerpo es `timeout`; conectar o las
            // cabeceras, `network`. ureq lo envuelve en un `io::Error` de tipo `Other`.
            let wrapped = e.get_ref().and_then(|i| i.downcast_ref::<ureq::Error>());
            if e.kind() == std::io::ErrorKind::TimedOut
                || matches!(wrapped, Some(ureq::Error::Timeout(_)))
            {
                MasterFetchError::Timeout
            } else {
                MasterFetchError::Network(format!("{:?}", e.kind()))
            }
        })?;
    Ok(data)
}

fn http_error(e: ureq::Error) -> MasterFetchError {
    match e {
        ureq::Error::StatusCode(401 | 403) => steam_error(
            "invalid_key",
            tr("La clave de API de Steam no es válida; revísala en Ajustes → General"),
        ),
        ureq::Error::StatusCode(429) => steam_error(
            "rate_limited",
            tr("Steam limitó las consultas; prueba en unos minutos"),
        ),
        ureq::Error::StatusCode(code) => MasterFetchError::Network(format!("HTTP {code}")),
        ureq::Error::Io(e) => MasterFetchError::Network(format!("{:?}", e.kind())),
        // Solo el nombre de la variante: el texto de algunas lleva la URL (y la clave).
        e => {
            let debug = format!("{e:?}");
            let name = debug.split(['(', ' ', '{']).next().unwrap_or_default();
            MasterFetchError::Network(name.to_owned())
        }
    }
}
