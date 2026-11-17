//! Descargas HTTP pequeñas con plazo y tope de tamaño. Traducción de `src/jase/net.py`.
use std::time::Duration;

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5); // por operación de socket
pub const FETCH_DEADLINE: Duration = Duration::from_secs(8); // total de la descarga
pub const USER_AGENT: &str = "JASE";
pub const MAX_DOWNLOAD_BYTES: usize = 2 * 1024 * 1024;

/// GET con User-Agent "JASE"; cualquier error (4xx/5xx, sin red, lentitud, más de
/// `max_bytes`) → `None`. Bloqueante: desde async usar [`fetch`]. No registra la URL
/// (puede llevar la clave de Steam).
pub fn fetch_blocking(
    url: &str,
    max_bytes: usize,
    deadline: Duration,
    headers: &[(&str, &str)],
) -> Option<Vec<u8>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(deadline))
        .timeout_connect(Some(FETCH_TIMEOUT))
        .user_agent(USER_AGENT)
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
