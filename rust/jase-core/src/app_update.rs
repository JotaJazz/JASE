//! Aviso de versión nueva de JASE desde GitHub Releases y descarga verificada del
//! instalador. Traducción de `app_update.py`. Todo bloqueante (la UI usa
//! `spawn_blocking`).
//!
//! Se usa el listado y no `/releases/latest`, que ignora las prerelease: en el listado
//! cuentan las releases y las prerelease, y los borradores no.
use crate::fetch_meta;
use crate::masters::FETCH_TIMEOUT_S;
use crate::net::{FETCH_DEADLINE, fetch_blocking};
use crate::pyjson::loads_str;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const RELEASES_URL: &str = "https://api.github.com/repos/JotaJazz/JASE/releases?per_page=10";
pub const RELEASE_PAGE_PREFIX: &str = "https://github.com/JotaJazz/JASE/releases/";
pub const CHECK_INTERVAL_S: u64 = 86400;
pub const RETRY_AFTER_FAILURE_S: u64 = 3600;
pub const META_FILENAME: &str = "app_update.meta.json";
pub const MAX_BYTES: usize = 512 * 1024;

pub const DOWNLOAD_PREFIX: &str = "https://github.com/JotaJazz/JASE/releases/download/";
pub const MAX_INSTALLER_BYTES: u64 = 300 * 1024 * 1024;
pub const CHUNK_BYTES: usize = 64 * 1024;

// Python no pone tope total (10 s por lectura); aquí, uno amplio para 300 MiB lentos.
const RECV_BODY_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub installer: Option<Asset>,
    pub portable: Option<Asset>, // JASE-<versión>-win64.zip (actualizar el portátil)
}

/// Códigos de error de la descarga (la UI los traduce; nunca texto aquí).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadCode {
    Size,      // el archivo terminó antes del tamaño declarado
    TooMuch,   // llegaron más bytes de los declarados
    Hash,      // el SHA-256 no coincide
    Insecure,  // URL o redirección que no es https
    Cancelled, // el usuario canceló
    Network,   // red o disco
}

impl DownloadCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Size => "size",
            Self::TooMuch => "too_much",
            Self::Hash => "hash",
            Self::Insecure => "insecure",
            Self::Cancelled => "cancelled",
            Self::Network => "network",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{}", if .detail.is_empty() { .code.as_str() } else { .detail.as_str() })]
pub struct DownloadError {
    pub code: DownloadCode,
    pub detail: String,
}

fn fail(code: DownloadCode, detail: impl ToString) -> DownloadError {
    DownloadError {
        code,
        detail: detail.to_string(),
    }
}

fn network(e: impl ToString) -> DownloadError {
    fail(DownloadCode::Network, e)
}

/// `sha256:<64 hex>` (cualquier caja) → el hex en minúsculas.
fn sha256_of(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// El asset `wanted` si todos sus campos son válidos; el primero con ese nombre decide.
fn asset(item: &serde_json::Map<String, Value>, wanted: &str) -> Option<Asset> {
    let a = item
        .get("assets")?
        .as_array()?
        .iter()
        .filter_map(Value::as_object)
        .find(|a| a.get("name").and_then(Value::as_str) == Some(wanted))?;
    let url = a
        .get("browser_download_url")
        .and_then(Value::as_str)
        .filter(|u| u.starts_with(DOWNLOAD_PREFIX))?;
    // Solo un entero JSON: `9.0`, `true` y `"9"` no valen (como `type(size) is int`).
    let size = a
        .get("size")
        .and_then(Value::as_u64)
        .filter(|s| (1..=MAX_INSTALLER_BYTES).contains(s))?;
    let sha256 = sha256_of(a.get("digest").and_then(Value::as_str)?)?;
    Some(Asset {
        name: wanted.to_owned(),
        url: url.to_owned(),
        size,
        sha256,
    })
}

/// «v0.10.0» o «0.10.0» → [0, 10, 0]; cualquier otra cosa (o un componente que no cabe
/// en u64) → None. Solo dígitos ASCII.
pub fn parse_version(text: &str) -> Option<Vec<u64>> {
    let digits = text.strip_prefix('v').unwrap_or(text);
    digits
        .split('.')
        .map(|p| {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        })
        .collect()
}

/// La release publicada (no borrador) de mayor versión con enlace a nuestra página.
pub fn newest_release(data: &[u8]) -> Option<Release> {
    let raw = loads_str(data)?;
    let mut best: Option<(Vec<u64>, Release)> = None;
    for item in raw.as_array()?.iter().filter_map(Value::as_object) {
        if item.get("draft") != Some(&Value::Bool(false)) {
            continue;
        }
        let Some(version) = item
            .get("tag_name")
            .and_then(Value::as_str)
            .and_then(parse_version)
        else {
            continue;
        };
        let Some(url) = item
            .get("html_url")
            .and_then(Value::as_str)
            .filter(|u| u.starts_with(RELEASE_PAGE_PREFIX))
        else {
            continue;
        };
        if best.as_ref().is_none_or(|(b, _)| version > *b) {
            let ver = version
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(".");
            let release = Release {
                installer: asset(item, &format!("JASE-{ver}-setup.exe")),
                portable: asset(item, &format!("JASE-{ver}-win64.zip")),
                version: ver,
                url: url.to_owned(),
            };
            best = Some((version, release));
        }
    }
    best.map(|(_, r)| r)
}

pub fn should_check(meta_path: &Path, now: f64) -> bool {
    fetch_meta::should_check(
        meta_path,
        now,
        CHECK_INTERVAL_S as f64,
        RETRY_AFTER_FAILURE_S as f64,
    )
}

/// None si falla o pasa de `MAX_BYTES`.
pub fn default_fetch(url: &str) -> Option<Vec<u8>> {
    fetch_blocking(
        url,
        MAX_BYTES,
        FETCH_DEADLINE,
        &[("Accept", "application/vnd.github+json")],
    )
}

/// La release solo si es más nueva que `current`.
pub fn check(
    cache_dir: &Path,
    now: f64,
    current: &str,
    fetch: impl FnOnce(&str) -> Option<Vec<u8>>,
) -> Option<Release> {
    check_detailed(cache_dir, now, current, fetch).0
}

/// (release más nueva o None, ¿se pudo consultar?).
pub fn check_detailed(
    cache_dir: &Path,
    now: f64,
    current: &str,
    fetch: impl FnOnce(&str) -> Option<Vec<u8>>,
) -> (Option<Release>, bool) {
    let meta_path = cache_dir.join(META_FILENAME);
    let Some(data) = fetch(RELEASES_URL).filter(|d| d.len() <= MAX_BYTES) else {
        fetch_meta::write_meta(&meta_path, now, false); // se reintenta tras 1 h
        return (None, false);
    };
    let release = newest_release(&data);
    fetch_meta::write_meta(&meta_path, now, true);
    let (Some(release), Some(mine)) = (release, parse_version(current)) else {
        return (None, true);
    };
    let newer = parse_version(&release.version).is_some_and(|theirs| theirs > mine);
    (newer.then_some(release), true)
}

/// Abre `url` (solo https, también en cada redirección) para leerla por trozos.
pub fn default_fetch_stream(url: &str) -> Result<Box<dyn Read + Send>, DownloadError> {
    if !url
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
    {
        return Err(fail(DownloadCode::Insecure, "solo https"));
    }
    stream_with_agent(url)
}

/// El agente de [`default_fetch_stream`] sin el chequeo de prefijo (para tests).
#[doc(hidden)]
pub fn stream_with_agent(url: &str) -> Result<Box<dyn Read + Send>, DownloadError> {
    let timeout = Some(Duration::from_secs(FETCH_TIMEOUT_S));
    let agent: ureq::Agent = crate::net::agent_builder()
        .https_only(true) // ureq lo aplica también a cada redirección
        .timeout_connect(timeout)
        .timeout_recv_response(timeout)
        .timeout_recv_body(Some(RECV_BODY_TIMEOUT))
        .build()
        .into();
    match agent.get(url).call() {
        Ok(resp) => Ok(Box::new(resp.into_body().into_reader())), // el tope lo pone `size`
        Err(e @ ureq::Error::RequireHttpsOnly(_)) => Err(fail(DownloadCode::Insecure, e)),
        Err(e) => Err(network(e)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Descarga el instalador, verifica tamaño y SHA-256 y lo deja en `dest_dir`. Ante
/// cualquier fallo (red, disco, tamaño, hash, cancelación) borra el `.part`.
pub fn download_installer(
    asset: &Asset,
    dest_dir: &Path,
    fetch_stream: impl FnOnce(&str) -> Result<Box<dyn Read + Send>, DownloadError>,
    progress: &mut dyn FnMut(u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<PathBuf, DownloadError> {
    let final_path = dest_dir.join(&asset.name);
    let tmp = dest_dir.join(format!("{}.part", asset.name));
    let result = (|| {
        fs::create_dir_all(dest_dir).map_err(network)?;
        let mut stream = fetch_stream(&asset.url)?;
        let mut out = File::create(&tmp).map_err(network)?;
        let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
        let mut buf = vec![0u8; CHUNK_BYTES];
        let mut done: u64 = 0;
        loop {
            if cancelled() {
                return Err(fail(DownloadCode::Cancelled, ""));
            }
            let n = match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(network(e)),
            };
            done += n as u64;
            if done > asset.size {
                return Err(fail(DownloadCode::TooMuch, ""));
            }
            let chunk = &buf[..n];
            digest.update(chunk);
            out.write_all(chunk).map_err(network)?;
            progress(done, asset.size);
        }
        drop(out); // cerrado antes de renombrar (Windows)
        if done != asset.size {
            return Err(fail(DownloadCode::Size, ""));
        }
        if hex(digest.finish().as_ref()) != asset.sha256 {
            return Err(fail(DownloadCode::Hash, ""));
        }
        fs::rename(&tmp, &final_path).map_err(network)
    })();
    match result {
        Ok(()) => Ok(final_path),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}
