//! Parsers puros por familia (sin red). Traducción de `src/jase/protocols/__init__.py`.
pub mod gamespy;
pub mod huffman;
pub mod quake2;
pub mod quake3;
pub mod steamweb;
pub mod ut2004;
pub mod valve;
pub mod zandronum;
use crate::models::Player;
use crate::pystr::py_strip;
use indexmap::IndexMap;

/// Respuesta de servidor malformada o inesperada.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("{0}")]
    Invalid(String),
    /// El master de Zandronum ignoró la consulta por llegar demasiado pronto.
    #[error("{0}")]
    MasterBusy(String),
    /// El servidor de Zandronum ignora consultas de esta IP durante ~10 s.
    #[error("{0}")]
    ServerIgnoring(String),
}

pub(crate) fn invalid(msg: impl Into<String>) -> ProtocolError {
    ProtocolError::Invalid(msg.into())
}

/// `int(text)` de Python recortado a i32: un servidor puede mandar cualquier cifra.
/// Acepta espacios alrededor, signo y `_` entre dígitos, como `int()`.
pub fn bounded_int(text: &str) -> Result<i32, ProtocolError> {
    let err = || invalid(format!("número inválido: {text:?}"));
    let s = py_strip(text);
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
    {
        return Err(err());
    }
    // int() de Python rechaza más de 4300 dígitos (sys.int_info.str_digits_check_threshold).
    if digits.bytes().filter(|&b| b != b'_').count() > 4300 {
        return Err(err());
    }
    let mut value: i64 = 0;
    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        let d = c
            .to_digit(10)
            .filter(|_| c.is_ascii_digit())
            .ok_or_else(err)?;
        // Tope por encima de i32: el resto de las cifras ya no cambia el recorte.
        value = (value * 10 + i64::from(d)).min(1 << 40);
    }
    let value = if neg { -value } else { value };
    Ok(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
}

/// `\clave\valor\…` → mapa en orden de llegada (clave repetida: gana el último valor).
pub fn parse_infostring(line: &str) -> IndexMap<String, String> {
    let mut parts: Vec<&str> = line.split('\\').collect();
    if parts.first() == Some(&"") {
        parts.remove(0);
    }
    parts
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[k, v]| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

/// Quita caracteres de control ASCII (\x00-\x1f, \x7f).
pub fn strip_control(text: &str) -> String {
    text.chars()
        .filter(|&c| !(c <= '\x1f' || c == '\x7f'))
        .collect()
}

/// Heurística para protocolos sin marca de bot (Quake 3, Quake II): ping 0, etiqueta
/// [BOT] o «botN» con ping 999.
pub fn is_bot(player: &Player) -> bool {
    let name = py_strip(&player.name);
    player.ping == 0 || has_bot_tag(&player.name) || (player.ping == 999 && is_cod_bot(name))
}

/// `^\s*\[bot\]` sin distinguir mayúsculas.
fn has_bot_tag(name: &str) -> bool {
    let rest = name.trim_start_matches(crate::pystr::is_py_space);
    rest.get(..5)
        .is_some_and(|t| t.eq_ignore_ascii_case("[bot]"))
}

/// `^bot\d+$` sin distinguir mayúsculas (`$` acepta un `\n` final).
fn is_cod_bot(name: &str) -> bool {
    let name = name.strip_suffix('\n').unwrap_or(name);
    name.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("bot"))
        && name.len() > 3
        && name[3..].bytes().all(|b| b.is_ascii_digit())
}

pub fn count_bots(players: &[Player]) -> i32 {
    players.iter().filter(|p| is_bot(p)).count() as i32
}

/// Cuatro bytes de IPv4 → "a.b.c.d". `b` debe tener al menos 4 bytes.
pub(crate) fn ipv4(b: &[u8]) -> String {
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}
