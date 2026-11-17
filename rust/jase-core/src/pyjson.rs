//! `json.loads` de Python sobre serde_json, para leer lo que escribe o acepta Python.
use serde_json::Value;

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

/// `json.loads(bytes)`: acepta el BOM UTF-8. Surrogates sueltos → U+FFFD; más de 128
/// niveles de anidamiento (el RecursionError de Python) y NaN/Infinity → `None`. Un
/// entero fuera de i64/u64 llega como decimal: quien pida un entero lo ve inválido.
pub fn loads(data: &[u8]) -> Option<Value> {
    let data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    serde_json::from_slice(&replace_lone_surrogates(data)).ok()
}

/// `json.loads(path.read_text("utf-8"))`: UTF-8 estricto y el BOM es inválido.
pub fn loads_str(data: &[u8]) -> Option<Value> {
    if data.starts_with(b"\xef\xbb\xbf") || std::str::from_utf8(data).is_err() {
        return None;
    }
    serde_json::from_slice(&replace_lone_surrogates(data)).ok()
}
