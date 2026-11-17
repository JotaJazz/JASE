//! País de un servidor por su IPv4: base DB-IP «IP to Country Lite» (CC BY 4.0).
//! Traducción de `src/jase/geo.py` sin los nombres de países. Todo es bloqueante.
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::read::MultiGzDecoder;

use crate::i18n::tr;
use crate::net::fetch_blocking;

pub const DB_URL: &str = "https://download.db-ip.com/free/dbip-country-lite-{month}.csv.gz";
pub const DB_FILENAME: &str = "dbip-country-lite.csv";
pub const MAX_GZ_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_CSV_BYTES: u64 = 256 * 1024 * 1024;
pub const DOWNLOAD_DEADLINE: Duration = Duration::from_secs(180);
/// Tope por línea del CSV (las reales miden < 100 bytes).
const MAX_LINE: u64 = 4096;
/// DB-IP marca así los rangos reservados / sin asignar.
const UNKNOWN_CODES: [&str; 1] = ["zz"];

/// Entero de una IPv4 en notación decimal con puntos estricta.
pub fn ipv4_int(text: &str) -> Option<u32> {
    let mut parts = text.split('.');
    let mut value = 0u32;
    for _ in 0..4 {
        let part = parts.next()?;
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        value = (value << 8) | u8::try_from(part.parse::<u16>().ok()?).ok()? as u32;
    }
    parts.next().is_none().then_some(value)
}

#[derive(Debug)]
/// Rangos IPv4 ordenados → código ISO alfa-2 (minúsculas).
pub struct CountryDB {
    /// (inicio, fin, índice en `codes`), ordenado por inicio.
    rows: Vec<(u32, u32, u16)>,
    codes: Vec<String>,
}

impl CountryDB {
    /// Filas IPv6, mal formadas o desconocidas (ZZ) se omiten.
    pub fn from_csv(path: &Path) -> io::Result<CountryDB> {
        let mut reader = BufReader::new(fs::File::open(path)?);
        let mut line = Vec::new();
        let mut rows = Vec::new();
        let mut codes: Vec<String> = Vec::new();
        loop {
            line.clear();
            // Línea acotada: una sin saltos no agota la memoria; se descarta el resto.
            if (&mut reader).take(MAX_LINE).read_until(b'\n', &mut line)? == 0 {
                break;
            }
            if line.last() != Some(&b'\n') && line.len() as u64 == MAX_LINE {
                reader.skip_until(b'\n')?;
                continue;
            }
            let line = String::from_utf8_lossy(&line);
            if line.contains(':') {
                continue;
            }
            let fields: Vec<&str> = line.trim().split(',').collect();
            let [start, end, code] = fields[..] else {
                continue;
            };
            let code = code.trim().trim_matches('"').to_ascii_lowercase();
            let (Some(start), Some(end)) = (ipv4_int(start), ipv4_int(end)) else {
                continue;
            };
            if end < start
                || code.len() != 2
                || !code.bytes().all(|b| b.is_ascii_lowercase())
                || UNKNOWN_CODES.contains(&code.as_str())
            {
                continue;
            }
            let pos = match codes.iter().position(|c| *c == code) {
                Some(p) => p,
                None => {
                    codes.push(code);
                    codes.len() - 1
                }
            };
            rows.push((start, end, pos as u16));
        }
        rows.sort_unstable();
        Ok(CountryDB { rows, codes })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn lookup(&self, ip: &str) -> Option<&str> {
        let value = ipv4_int(ip)?;
        let i = self.rows.partition_point(|r| r.0 <= value).checked_sub(1)?;
        let (_, end, code) = self.rows[i];
        (value <= end).then(|| self.codes[code as usize].as_str())
    }
}

// ------------------------------------------------------------------ descarga
/// Mes actual y anterior como `YYYY-MM`.
fn months(today: (i32, u32)) -> [String; 2] {
    let (y, m) = today;
    let (py, pm) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
    [format!("{y:04}-{m:02}"), format!("{py:04}-{pm:02}")]
}

/// Primera fila con forma `ipv4,ipv4,cc` (descarta páginas de error comprimidas).
fn looks_like_db(path: &Path) -> bool {
    let mut head = Vec::new();
    if fs::File::open(path)
        .and_then(|f| f.take(4096).read_to_end(&mut head))
        .is_err()
    {
        return false;
    }
    let text = String::from_utf8_lossy(&head);
    let line = text.split('\n').next().unwrap_or("");
    let fields: Vec<&str> = line.trim().split(',').collect();
    matches!(fields[..], [a, b, c]
        if ipv4_int(a).is_some() && ipv4_int(b).is_some() && c.chars().count() == 2)
}

fn unpack(data: &[u8], tmp: &Path, limit: u64) -> io::Result<()> {
    let mut out = fs::File::create(tmp)?;
    // limit + 1 para distinguir «justo el tope» de «lo pasa».
    let written = io::copy(&mut MultiGzDecoder::new(data).take(limit + 1), &mut out)?;
    if written > limit {
        return Err(io::Error::other("base de países demasiado grande"));
    }
    out.sync_all()
}

/// Descomprime `data` en `dest` de forma atómica; `false` si no es una base válida.
pub fn install_with_limit(data: &[u8], dest: &Path, limit: u64) -> bool {
    let Some(parent) = dest.parent() else {
        return false;
    };
    let name = dest.file_name().unwrap_or_default().to_string_lossy();
    let tmp: PathBuf = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let ok = fs::create_dir_all(parent).is_ok()
        && unpack(data, &tmp, limit).is_ok()
        && looks_like_db(&tmp)
        && fs::rename(&tmp, dest).is_ok();
    if !ok {
        let _ = fs::remove_file(&tmp);
    }
    ok
}

/// Descarga la base del mes actual (o, si falla, la del anterior) a `dest`.
/// El archivo existente solo se reemplaza si la descarga es válida.
pub fn download_db(
    dest: &Path,
    today: (i32, u32),
    fetch: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> bool {
    months(today).iter().any(|month| {
        fetch(&DB_URL.replace("{month}", month))
            .is_some_and(|data| !data.is_empty() && install_with_limit(&data, dest, MAX_CSV_BYTES))
    })
}

/// La base del archivo en caché o, si falta (o se pide), descargándola.
pub fn load_country_db(
    path: &Path,
    refresh: bool,
    today: (i32, u32),
    fetch: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<CountryDB, String> {
    if (refresh || !path.is_file()) && !download_db(path, today, fetch) {
        return Err(tr("No se pudo descargar la base de países"));
    }
    CountryDB::from_csv(path).map_err(|_| tr("No se pudo leer la base de países"))
}

/// (año, mes) de un día contado desde 1970-01-01 (días civiles de Howard Hinnant).
pub fn month_of_unix_days(days: i64) -> (i32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as i32, month as u32)
}

/// Mes actual en UTC.
pub fn current_month() -> (i32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    month_of_unix_days((secs / 86_400) as i64)
}

pub fn default_fetch(url: &str) -> Option<Vec<u8>> {
    fetch_blocking(url, MAX_GZ_BYTES, DOWNLOAD_DEADLINE, &[])
}
