//! API web de Steam: IGameServersService/GetServerList (lista de servidores con clave).
//! Traducción de `src/jase/protocols/steamweb.py`. La respuesta es JSON:
//! `{"response": {"servers": [{"addr": "ip:puerto", ...}]}}`.
use super::{ProtocolError, invalid};
use crate::models::ServerAddress;
use indexmap::IndexSet;
use serde_json::Value;
use std::fmt::Write;

/// Único host al que se envía la clave: nunca se toma de masters.json ni de un master propio.
pub const OFFICIAL_HOST: &str = "api.steampowered.com";
pub const API_URL: &str = "https://api.steampowered.com/IGameServersService/GetServerList/v1/";

/// `urllib.parse.quote_plus`: sin escapar `A-Za-z0-9_.-~`, espacio → `+`, el resto `%XX`.
fn quote_plus(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                out.push(char::from(b))
            }
            b' ' => out.push('+'),
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

pub fn build_url(host: &str, key: &str, filter: &str, limit: u32) -> Result<String, ProtocolError> {
    if host.to_lowercase() != OFFICIAL_HOST {
        return Err(invalid(format!(
            "La clave de Steam solo se envía a {OFFICIAL_HOST}"
        )));
    }
    Ok(format!(
        "{API_URL}?key={}&filter={}&limit={limit}",
        quote_plus(key),
        quote_plus(filter)
    ))
}

/// Valor de un escape `\uXXXX` en `data[i..]` (donde `data[i]` es la barra).
fn unicode_escape(data: &[u8], i: usize) -> Option<u16> {
    let hex = data.get(i + 2..i + 6)?;
    if data.get(i + 1) != Some(&b'u') {
        return None;
    }
    u16::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()
}

/// `json.loads` acepta escapes `\uD800`-`\uDFFF` sueltos (quedan en la cadena) y serde_json
/// no: se cambian por `\uFFFD` para que una entrada rara no tumbe la lista entera.
fn replace_lone_surrogates(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] != b'\\' {
            out.push(data[i]);
            i += 1;
            continue;
        }
        match unicode_escape(data, i) {
            Some(0xD800..=0xDBFF)
                if matches!(unicode_escape(data, i + 6), Some(0xDC00..=0xDFFF)) =>
            {
                out.extend(&data[i..i + 12]); // par válido
                i += 12;
            }
            Some(0xD800..=0xDFFF) => {
                out.extend(b"\\uFFFD");
                i += 6;
            }
            // Cualquier otro escape (incluido «\\»): se copian la barra y lo que sigue.
            _ => {
                out.extend(&data[i..(i + 2).min(data.len())]);
                i += 2;
            }
        }
    }
    out
}

/// (direcciones sin repetir, entradas de la respuesta): las entradas dicen si se llegó
/// al tope de la API aunque haya direcciones repetidas.
pub fn parse_response(data: &[u8]) -> Result<(Vec<ServerAddress>, usize), ProtocolError> {
    let bad = || invalid("Respuesta de la API web de Steam inválida");
    // serde_json corta a 128 niveles de anidamiento (el RecursionError de Python).
    let data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data); // BOM: json.loads lo acepta
    let root: Value = serde_json::from_slice(&replace_lone_surrogates(data)).map_err(|_| bad())?;
    let response = root
        .get("response")
        .and_then(Value::as_object)
        .ok_or_else(bad)?;
    let Some(Value::Array(entries)) = response.get("servers") else {
        return Ok((Vec::new(), 0)); // ausente, null o no-lista: Python itera sin encontrar nada
    };
    let mut servers = IndexSet::new();
    for entry in entries {
        // `addr` que no es cadena no tiene ":" en Python (str(5) == "5"): se salta.
        let Some(addr) = entry.get("addr").and_then(Value::as_str) else {
            continue;
        };
        let Some((host, port)) = addr.rsplit_once(':') else {
            continue;
        };
        // Divergencia: Python acepta dígitos no ASCII en isdigit() y puede caerse en int().
        if host.is_empty() || port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        if let Ok(port @ 1..=65535) = port.parse::<u32>() {
            servers.insert(ServerAddress::new(host, port as u16));
        }
    }
    Ok((servers.into_iter().collect(), entries.len()))
}
