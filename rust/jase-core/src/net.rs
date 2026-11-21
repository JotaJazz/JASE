//! Descargas HTTP pequeñas con plazo y tope de tamaño (traducción de `net.py`) y el proxy
//! HTTP de toda la app.
use std::sync::RwLock;
use std::time::Duration;

use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5); // por operación de socket
pub const FETCH_DEADLINE: Duration = Duration::from_secs(8); // total de la descarga
pub const USER_AGENT: &str = "JASE";
pub const MAX_DOWNLOAD_BYTES: usize = 2 * 1024 * 1024;

/// Esquemas de proxy que acepta JASE (los que entiende ureq con `socks-proxy`).
pub const PROXY_SCHEMES: [&str; 5] = ["http", "https", "socks4", "socks4a", "socks5"];

/// El proxy elegido en Ajustes → Red; `None` = el del entorno (`HTTP_PROXY`…) o ninguno.
static PROXY: RwLock<Option<ureq::Proxy>> = RwLock::new(None);

/// Valida `esquema://[usuario:clave@]host:puerto` con un esquema de [`PROXY_SCHEMES`].
pub fn parse_proxy(url: &str) -> Result<ureq::Proxy, String> {
    let scheme = url.split_once("://").map(|(s, _)| s.to_ascii_lowercase());
    if !scheme.is_some_and(|s| PROXY_SCHEMES.contains(&s.as_str())) {
        return Err(format!(
            "esquema no válido (usa {})",
            PROXY_SCHEMES.join(", ")
        ));
    }
    ureq::Proxy::new(url).map_err(|e| e.to_string())
}

/// Fija el proxy de las conexiones HTTP siguientes; `""` vuelve al del entorno. Si `url` no
/// es válido, no cambia nada.
pub fn set_proxy(url: &str) -> Result<(), String> {
    let url = url.trim();
    let proxy = if url.is_empty() {
        None
    } else {
        Some(parse_proxy(url)?)
    };
    *PROXY.write().unwrap_or_else(|e| e.into_inner()) = proxy;
    Ok(())
}

/// El constructor de agente de toda la app: User-Agent "JASE" y el proxy de Ajustes, si hay.
pub fn agent_builder() -> ConfigBuilder<AgentScope> {
    let builder = ureq::Agent::config_builder().user_agent(USER_AGENT);
    match PROXY.read().unwrap_or_else(|e| e.into_inner()).clone() {
        Some(proxy) => builder.proxy(Some(proxy)),
        None => builder,
    }
}

/// GET con User-Agent "JASE"; cualquier error (4xx/5xx, sin red, lentitud, más de
/// `max_bytes`) → `None`. Bloqueante: desde async usar [`fetch`]. No registra la URL
/// (puede llevar la clave de Steam).
pub fn fetch_blocking(
    url: &str,
    max_bytes: usize,
    deadline: Duration,
    headers: &[(&str, &str)],
) -> Option<Vec<u8>> {
    let agent: ureq::Agent = agent_builder()
        .timeout_global(Some(deadline))
        .timeout_connect(Some(FETCH_TIMEOUT))
        .build()
        .into();
    let mut req = agent.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.call().ok()?;
    // El tope falla si el cuerpo lo excede, no lo trunca.
    resp.body_mut()
        .with_config()
        .limit(max_bytes as u64)
        .read_to_vec()
        .ok()
}

/// Como [`fetch_blocking`] sin cabeceras extra, fuera del hilo async.
pub async fn fetch(url: String, max_bytes: usize, deadline: Duration) -> Option<Vec<u8>> {
    tokio::task::spawn_blocking(move || fetch_blocking(&url, max_bytes, deadline, &[]))
        .await
        .ok()
        .flatten()
}
