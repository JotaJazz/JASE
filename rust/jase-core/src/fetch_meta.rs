//! Meta de comprobaciones periódicas (último intento/éxito). Traducción de
//! `fetch_meta.py`; `atomic_write` vive en `fsutil`.
use crate::fsutil::atomic_write;
use serde_json::{Value, json};
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MAX_META_BYTES: u64 = 256 * 1024 + 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub attempt: Option<f64>,
    pub ok: bool,
    pub success: Option<f64>,
}

/// Meta tolerante: cualquier fallo de lectura o de forma da los valores por defecto.
pub fn read_meta(path: &Path) -> Meta {
    let mut meta = Meta {
        attempt: None,
        ok: true,
        success: None,
    };
    let mut raw = Vec::new();
    if File::open(path)
        .and_then(|f| f.take(MAX_META_BYTES).read_to_end(&mut raw))
        .is_err()
    {
        return meta;
    }
    let Ok(Value::Object(obj)) = serde_json::from_slice::<Value>(&raw) else {
        return meta;
    };
    let num = |k: &str| obj.get(k).and_then(Value::as_f64); // bool y texto → None
    let legacy = num("last_check"); // formato anterior: un solo instante
    meta.success = num("last_success").or(legacy);
    meta.attempt = num("last_attempt").or(legacy);
    meta.ok = obj.get("last_attempt_ok") != Some(&Value::Bool(false));
    meta
}

/// Anota un intento; los errores de disco se ignoran.
pub fn write_meta(path: &Path, now: f64, ok: bool) {
    let success = if ok {
        Some(now)
    } else {
        read_meta(path).success
    };
    let data = json!({"last_attempt": now, "last_attempt_ok": ok, "last_success": success});
    let _ = atomic_write(path, data.to_string().as_bytes());
}

pub fn should_check(path: &Path, now: f64, interval_s: f64, retry_s: f64) -> bool {
    let meta = read_meta(path);
    if let (false, Some(attempt)) = (meta.ok, meta.attempt) {
        return !(0.0 <= now - attempt && now - attempt < retry_s); // reintento tras un fallo
    }
    match meta.success {
        None => true,
        Some(s) => !(0.0 <= now - s && now - s < interval_s), // reloj retrasado → revisar
    }
}
