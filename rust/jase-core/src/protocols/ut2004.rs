//! Protocolo de Unreal Tournament 2004: master (TCP) y consulta nativa (UDP).
//! Traducción de `src/jase/protocols/ut2004.py` (detalle del formato y fuentes en ese
//! módulo). Los hashes que se envían al master son FICTICIOS y fijos.
use super::{ProtocolError, invalid, ipv4, strip_control};
use crate::models::{Player, ServerAddress, ServerInfo};
use crate::pystr::{casefold, latin1, py_strip, utf8_or_latin1};
use indexmap::IndexMap;

/// Ficticios: 32 dígitos hex fijos, no salen de ninguna CD key real.
pub const FAKE_CDKEY_HASH: &str = "00000000000000000000000000000000";
pub const FAKE_RESPONSE_HASH: &str = "00000000000000000000000000000000";
pub const CLIENT_NAME: &str = "UT2K4CLIENT";
pub const CLIENT_VERSION: u32 = 3369;
const PLATFORM: u8 = 3; // el master resta 3 (0 = Windows)
const LANGUAGE: &str = "int";
pub const MAX_FRAME: usize = 64 * 1024;

pub const INFO_QUERY: &[u8] = b"\x80\x00\x00\x00\x00";
pub const RULES_QUERY: &[u8] = b"\x80\x00\x00\x00\x01";
pub const PLAYERS_QUERY: &[u8] = b"\x80\x00\x00\x00\x02";
const QUERY_HEADER: &[u8] = b"\x80\x00\x00\x00";
const INFO: u8 = 0;
const RULES: u8 = 1;
const PLAYERS: u8 = 2;
const ESC: u8 = 0x1b; // color: ESC + R G B

pub fn frame(body: &[u8]) -> Vec<u8> {
    [&(body.len() as u32).to_le_bytes()[..], body].concat()
}

/// Índice compacto de Unreal (bit 0x80 del primer byte = negativo).
pub fn compact_int(value: i32) -> Vec<u8> {
    let sign = if value < 0 { 0x80 } else { 0 };
    let mut v = value.unsigned_abs();
    let first = (v & 0x3F) as u8;
    v >>= 6;
    let mut out = vec![sign | first | if v != 0 { 0x40 } else { 0 }];
    while v != 0 {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        out.push(byte | if v != 0 { 0x80 } else { 0 });
    }
    out
}

/// (valor, posición siguiente). Hasta 5 bytes (34 bits), como Python.
pub fn read_compact_int(data: &[u8], pos: usize) -> Result<(i64, usize), ProtocolError> {
    let truncated = || invalid("Índice compacto truncado");
    let first = *data.get(pos).ok_or_else(truncated)?;
    let mut pos = pos + 1;
    let mut value = i64::from(first & 0x3F);
    let mut shift = 6;
    let mut more = first & 0x40 != 0;
    while more {
        if shift > 27 {
            return Err(invalid("Índice compacto demasiado largo"));
        }
        let byte = *data.get(pos).ok_or_else(truncated)?;
        pos += 1;
        value |= i64::from(byte & 0x7F) << shift;
        shift += 7;
        more = byte & 0x80 != 0;
    }
    Ok((if first & 0x80 != 0 { -value } else { value }, pos))
}

/// Bytes de un FString sin el NUL final; el bool indica UTF-16LE.
fn read_raw(data: &[u8], pos: usize) -> Result<(&[u8], bool, usize), ProtocolError> {
    let (length, pos) = read_compact_int(data, pos)?;
    let wide = length < 0;
    let size = if wide {
        length.unsigned_abs() * 2
    } else {
        length.unsigned_abs()
    };
    let end = usize::try_from(size)
        .ok()
        .and_then(|s| pos.checked_add(s))
        .filter(|&e| e <= data.len())
        .ok_or_else(|| invalid("Cadena truncada"))?;
    let mut raw = &data[pos..end];
    let terminator: &[u8] = if wide { b"\0\0" } else { b"\0" };
    if let Some(r) = raw.strip_suffix(terminator) {
        raw = r;
    }
    Ok((raw, wide, end))
}

fn utf16le(raw: &[u8]) -> Vec<u16> {
    raw.as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect()
}

pub fn read_fstring(data: &[u8], pos: usize) -> Result<(String, usize), ProtocolError> {
    let (raw, wide, pos) = read_raw(data, pos)?;
    let text = if wide {
        String::from_utf16_lossy(&utf16le(raw))
    } else {
        latin1(raw)
    };
    Ok((text, pos))
}

/// Quita ESC + las 3 unidades siguientes (si quedan menos de 3, el ESC queda).
fn strip_colour<T: Copy + PartialEq + From<u8>>(units: &[T]) -> Vec<T> {
    let esc = T::from(ESC);
    let mut out = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        if units[i] == esc && i + 3 < units.len() {
            i += 4;
        } else {
            out.push(units[i]);
            i += 1;
        }
    }
    out
}

/// FString para mostrar: sin códigos de color ni caracteres de control.
fn read_text(data: &[u8], pos: usize) -> Result<(String, usize), ProtocolError> {
    let (raw, wide, pos) = read_raw(data, pos)?;
    let text = if wide {
        // Python quita el color sobre caracteres ya decodificados (no unidades UTF-16).
        let chars: Vec<char> = String::from_utf16_lossy(&utf16le(raw)).chars().collect();
        strip_colour(&chars).into_iter().collect()
    } else {
        // hay servidores con nombres en UTF-8 dentro de cadenas de 8 bits
        utf8_or_latin1(&strip_colour(raw))
    };
    Ok((py_strip(&strip_control(&text)).to_owned(), pos))
}

pub fn fstring(text: &str) -> Vec<u8> {
    let mut raw: Vec<u8> = text
        .chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect();
    raw.push(0);
    [compact_int(raw.len() as i32), raw].concat()
}

fn int32(data: &[u8], pos: usize) -> Result<(i32, usize), ProtocolError> {
    let b = data
        .get(pos..pos.saturating_add(4))
        .filter(|b| b.len() == 4);
    let b = b.ok_or_else(|| invalid("Entero truncado"))?;
    Ok((i32::from_le_bytes([b[0], b[1], b[2], b[3]]), pos + 4))
}

// ------------------------------------------------------------------ master
pub fn build_challenge_response() -> Vec<u8> {
    let body = [
        fstring(FAKE_CDKEY_HASH),
        fstring(FAKE_RESPONSE_HASH),
        fstring(CLIENT_NAME),
        CLIENT_VERSION.to_le_bytes().to_vec(),
        vec![PLATFORM],
        fstring(LANGUAGE),
        vec![0; 13],
    ]
    .concat();
    frame(&body)
}

/// Tras APPROVED el cliente manda un mensaje que el master no comprueba (ficticio, fijo).
pub fn verify_request() -> Vec<u8> {
    frame(&fstring(FAKE_RESPONSE_HASH))
}

/// Petición de lista: tipo 0 (lista de servidores) y 0 filtros.
pub fn list_request() -> Vec<u8> {
    frame(b"\x00\x00")
}

/// Exige que el mensaje empiece con el FString `expected` (APPROVED, VERIFIED).
pub fn check_reply(body: &[u8], expected: &str) -> Result<(), ProtocolError> {
    let (got, _) = read_fstring(body, 0)?;
    if got != expected {
        let shown: String = got.chars().take(32).collect();
        return Err(invalid(format!(
            "El master respondió {shown:?} en vez de {expected}"
        )));
    }
    Ok(())
}

pub fn parse_list_header(body: &[u8]) -> Result<u32, ProtocolError> {
    Ok(int32(body, 0)?.0 as u32)
}

/// Dirección de consulta (IP + puerto de consulta) de un registro de la lista.
pub fn parse_list_entry(body: &[u8]) -> Result<ServerAddress, ProtocolError> {
    if body.len() < 8 {
        return Err(invalid("Registro del master truncado"));
    }
    let query_port = u16::from_le_bytes([body[6], body[7]]);
    if query_port == 0 {
        return Err(invalid("Registro del master sin puerto de consulta"));
    }
    Ok(ServerAddress::new(ipv4(body), query_port))
}

// ------------------------------------------------------------------ consulta
fn check_header(data: &[u8], kind: u8) -> Result<usize, ProtocolError> {
    if data.len() < 5 || !data.starts_with(QUERY_HEADER) || data[4] != kind {
        return Err(invalid(format!("Cabecera de respuesta {kind} inválida")));
    }
    Ok(5)
}

pub fn parse_info(
    data: &[u8],
    address: &ServerAddress,
    ping_ms: u32,
) -> Result<ServerInfo, ProtocolError> {
    let pos = check_header(data, INFO)?;
    let (_, pos) = int32(data, pos)?; // id del servidor
    let (_, pos) = read_fstring(data, pos)?; // IP (suele ir vacía)
    let (game_port, pos) = int32(data, pos)?;
    let (_, pos) = int32(data, pos)?; // puerto de consulta
    let (name, pos) = read_text(data, pos)?;
    let (map, pos) = read_text(data, pos)?;
    let (gametype, pos) = read_text(data, pos)?;
    let (num_players, pos) = int32(data, pos)?;
    let (max_players, _) = int32(data, pos)?;
    let port = match u16::try_from(game_port) {
        Ok(p) if p > 0 => p,
        _ => address.port.saturating_sub(1), // puerto 0: 0, nunca negativo
    };
    let mut info = ServerInfo::new(
        address.clone(),
        ServerAddress::new(address.host.clone(), port),
    );
    info.name = name;
    info.map = map;
    info.gametype = gametype;
    info.num_players = num_players.max(0);
    info.max_players = max_players.max(0);
    info.ping_ms = ping_ms; // contraseña y versión van en las reglas; ver apply_details
    Ok(info)
}

/// Pares clave/valor añadidos a `rules` (para juntar varios datagramas); las claves
/// repetidas (Mutator) se unen con ", ".
pub fn parse_rules(data: &[u8], rules: &mut IndexMap<String, String>) -> Result<(), ProtocolError> {
    let mut pos = check_header(data, RULES)?;
    while pos < data.len() {
        let (key, p) = read_text(data, pos)?;
        let (value, p) = read_text(data, p)?;
        pos = p;
        match rules.get_mut(&key) {
            Some(v) => *v = format!("{v}, {value}"),
            None => {
                rules.insert(key, value);
            }
        }
    }
    Ok(())
}

/// Jugadores de un datagrama; se omiten las filas de id 0 (puntuación de equipos).
pub fn parse_players(data: &[u8]) -> Result<Vec<Player>, ProtocolError> {
    let mut pos = check_header(data, PLAYERS)?;
    let mut players = Vec::new();
    while pos < data.len() {
        let (player_id, p) = int32(data, pos)?;
        let (name, p) = read_text(data, p)?;
        let (ping, p) = int32(data, p)?;
        let (score, p) = int32(data, p)?;
        let (_, p) = int32(data, p)?; // id de estadísticas + bits de equipo
        pos = p;
        if player_id != 0 {
            players.push(Player::new(name, score, ping));
        }
    }
    Ok(players)
}

/// Completa `info` con reglas (contraseña, versión) y jugadores.
pub fn apply_details(info: &mut ServerInfo, rules: IndexMap<String, String>, players: Vec<Player>) {
    info.password = casefold(rules.get("GamePassword").map_or("", String::as_str)) == "true";
    info.version = rules.get("ServerVersion").cloned().unwrap_or_default();
    info.rules = rules;
    info.players = players;
}
