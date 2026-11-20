//! Protocolo Valve (GoldSrc/Source): Master Server Query y consultas A2S.
//! Traducción de `protocols/valve.py` (detalle del formato en ese módulo).
use super::{ProtocolError, invalid, ipv4, strip_control};
use crate::models::{Player, ServerAddress, ServerInfo};
use crate::pystr::{casefold, utf8_or_latin1};
use std::collections::BTreeMap;

pub const OOB: &[u8] = b"\xff\xff\xff\xff";
pub const SPLIT: &[u8] = b"\xfe\xff\xff\xff";
pub const MASTER_HEADER: &[u8] = b"\xff\xff\xff\xff\x66\x0a";
pub const A2S_INFO: &[u8] = b"\xff\xff\xff\xffTSource Engine Query\x00";
pub const REGION_ALL: u8 = 0xFF;
/// Counter-Strike 1.6.
pub const DEFAULT_FILTER: &str = "\\appid\\10";

pub const S2C_CHALLENGE: u8 = 0x41;
pub const INFO_SOURCE: u8 = 0x49;
pub const INFO_GOLDSRC: u8 = 0x6D;
pub const PLAYER_RESPONSE: u8 = 0x44;

/// Tope de partes por respuesta (y de fragmentos retenidos por formato).
pub const MAX_SPLIT_PACKETS: usize = 16;

const SEED_START: &str = "0.0.0.0:0";

fn is_end(a: &ServerAddress) -> bool {
    a.host == "0.0.0.0" && a.port == 0
}

pub fn build_master_query(seed: Option<&ServerAddress>, filter: &str) -> Vec<u8> {
    let start = seed.map_or_else(|| SEED_START.to_owned(), ToString::to_string);
    let mut out = vec![0x31, REGION_ALL];
    out.extend(start.bytes());
    out.push(0);
    out.extend(
        filter
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')),
    );
    out.push(0);
    out
}

/// (servidores, semilla de la página siguiente); semilla None = fin de la lista.
pub fn parse_master_response(
    data: &[u8],
) -> Result<(Vec<ServerAddress>, Option<ServerAddress>), ProtocolError> {
    let body = data
        .strip_prefix(MASTER_HEADER)
        .ok_or_else(|| invalid("Respuesta del master Valve inválida"))?;
    let mut servers: Vec<ServerAddress> = body
        .as_chunks::<6>()
        .0
        .iter()
        .map(|e| ServerAddress::new(ipv4(e), u16::from_be_bytes([e[4], e[5]])))
        .collect();
    match servers.last() {
        None => Ok((servers, None)),
        Some(last) if is_end(last) => {
            servers.retain(|s| !is_end(s));
            Ok((servers, None))
        }
        Some(last) => {
            let seed = last.clone();
            Ok((servers, Some(seed)))
        }
    }
}

pub fn build_info_query(challenge: Option<&[u8]>) -> Vec<u8> {
    [A2S_INFO, challenge.unwrap_or_default()].concat()
}

pub fn build_player_query(challenge: Option<&[u8]>) -> Vec<u8> {
    [OOB, b"U", challenge.unwrap_or(OOB)].concat()
}

/// Los 4 bytes del S2C_CHALLENGE, o None si `data` no es un challenge.
pub fn challenge_of(data: &[u8]) -> Option<[u8; 4]> {
    if data.len() >= 9 && data.starts_with(OOB) && data[4] == S2C_CHALLENGE {
        data[5..9].try_into().ok()
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Total {
    Unset,
    Set(u8),
    Discarded,
}

#[derive(Debug)]
struct Format {
    total: Total,
    parts: BTreeMap<u8, Vec<u8>>,
}

impl Format {
    fn new() -> Self {
        Self {
            total: Total::Unset,
            parts: BTreeMap::new(),
        }
    }
}

/// Fragmento interpretado: (total, índice, datos); `Err(motivo)` solo se usa si
/// tampoco encaja en el otro formato.
type Parsed<'a> = Option<Result<(u8, u8, &'a [u8]), String>>;

/// Reensambla respuestas multipaquete (formatos GoldSrc y Source), desordenadas.
///
/// La cabecera no dice qué formato es, así que cada fragmento se interpreta de las dos
/// formas. Solo se retienen fragmentos de un id de petición: un id nuevo descarta los
/// anteriores. Memoria y trabajo por paquete acotados.
#[derive(Debug)]
pub struct PacketAssembler {
    request_id: Option<i32>,
    formats: [Format; 2], // goldsrc, source (en ese orden, como el dict de Python)
}

impl Default for PacketAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl PacketAssembler {
    pub fn new() -> Self {
        Self {
            request_id: None,
            formats: [Format::new(), Format::new()],
        }
    }

    /// Fragmentos retenidos (memoria acotada; lo usan los tests).
    pub fn held_parts(&self) -> usize {
        self.formats.iter().map(|f| f.parts.len()).sum()
    }

    /// El mensaje completo (empieza por FF FF FF FF) o None si faltan partes.
    pub fn feed(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>, ProtocolError> {
        if data.starts_with(OOB) {
            return Ok(Some(data.to_vec()));
        }
        if !data.starts_with(SPLIT) || data.len() < 9 {
            return Err(invalid("Paquete Valve inválido"));
        }
        let request_id = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if self.request_id != Some(request_id) {
            self.request_id = Some(request_id);
            self.formats = [Format::new(), Format::new()];
        }
        let candidates = [goldsrc(data), source(data, request_id)];
        let mut accepted = false;
        let mut error = "Fragmento Valve inválido".to_owned();
        for (fmt, parsed) in self.formats.iter_mut().zip(candidates) {
            let (total, index, payload) = match parsed {
                Some(Err(reason)) => {
                    error = reason;
                    continue;
                }
                None => continue,
                Some(Ok(p)) => p,
            };
            match fmt.total {
                Total::Discarded => continue,
                Total::Unset => fmt.total = Total::Set(total),
                Total::Set(t) if t != total => {
                    fmt.total = Total::Discarded;
                    fmt.parts.clear();
                    continue;
                }
                Total::Set(_) => {}
            }
            fmt.parts.insert(index, payload.to_vec());
            accepted = true;
            if fmt.parts.len() == usize::from(total) {
                return Ok(Some(fmt.parts.values().flatten().copied().collect()));
            }
        }
        if !accepted {
            return Err(invalid(error));
        }
        Ok(None)
    }
}

fn goldsrc(data: &[u8]) -> Parsed<'_> {
    let (total, index) = (data[8] & 0x0F, data[8] >> 4);
    let payload = &data[9..];
    if total == 0 || index >= total || (index == 0 && !payload.starts_with(OOB)) {
        return None;
    }
    Some(Ok((total, index, payload)))
}

fn source(data: &[u8], request_id: i32) -> Parsed<'_> {
    if data.len() < 12 {
        return None;
    }
    let (total, index) = (data[8], data[9]);
    if total == 0 || index >= total {
        return None;
    }
    if usize::from(total) > MAX_SPLIT_PACKETS {
        return Some(Err(format!(
            "Respuesta Valve en {total} partes (máx. {MAX_SPLIT_PACKETS})"
        )));
    }
    if index == 0 && request_id < 0 {
        return Some(Err("Respuesta comprimida (bzip2) no soportada".to_owned()));
    }
    let payload = &data[12..];
    if index == 0 && !payload.starts_with(OOB) {
        return None;
    }
    Some(Ok((total, index, payload)))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ProtocolError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        let end = end.ok_or_else(|| invalid("Respuesta Valve truncada"))?;
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn byte(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i32(&mut self) -> Result<i32, ProtocolError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn string(&mut self) -> Result<String, ProtocolError> {
        let rest = self.data.get(self.pos..).unwrap_or_default();
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| invalid("Cadena sin terminar en respuesta Valve"))?;
        let s = utf8_or_latin1(&rest[..len]);
        self.pos += len + 1;
        Ok(s)
    }
}

/// Respuesta A2S_INFO (0x49 o 0x6D) → ServerInfo. La dirección de conexión es la
/// consultada salvo que el EDF (0x80) anuncie otro puerto de juego.
///
/// `expected_gamedir`: si no está vacío y el servidor informa otra carpeta, Err.
pub fn parse_info(
    data: &[u8],
    address: &ServerAddress,
    ping_ms: u32,
    expected_gamedir: &str,
) -> Result<ServerInfo, ProtocolError> {
    if data.len() < 6 || !data.starts_with(OOB) {
        return Err(invalid("Respuesta A2S_INFO inválida"));
    }
    let kind = data[4];
    let mut r = Reader { data, pos: 5 };
    let mut rules = indexmap::IndexMap::new();
    let (name, map, folder, game, players, max_players, bots, visibility, version);
    if kind == INFO_SOURCE {
        rules.insert("protocol".to_owned(), r.byte()?.to_string());
        (name, map, folder, game) = (r.string()?, r.string()?, r.string()?, r.string()?);
        let appid = r.u16()?;
        (players, max_players, bots) = (r.byte()?, r.byte()?, r.byte()?);
        r.byte()?; // tipo de servidor (d/l/p)
        r.byte()?; // entorno (l/w/m/o)
        visibility = r.byte()?;
        r.byte()?; // VAC
        version = r.string()?;
        rules.insert("appid".to_owned(), appid.to_string());
    } else if kind == INFO_GOLDSRC {
        r.string()?; // dirección "ip:puerto" anunciada
        (name, map, folder, game) = (r.string()?, r.string()?, r.string()?, r.string()?);
        (players, max_players) = (r.byte()?, r.byte()?);
        rules.insert("protocol".to_owned(), r.byte()?.to_string());
        r.byte()?; // tipo de servidor
        r.byte()?; // entorno
        visibility = r.byte()?;
        if r.byte()? == 1 {
            // mod de Half-Life: enlace, descarga, versión, tamaño…
            r.string()?;
            r.string()?;
            r.byte()?;
            r.take(8)?;
            r.byte()?;
            r.byte()?;
        }
        r.byte()?; // VAC
        bots = r.byte()?;
        version = String::new();
    } else {
        return Err(invalid(format!(
            "Tipo de respuesta A2S_INFO desconocido: {kind:#x}"
        )));
    }
    let mut connect = address.clone();
    if kind == INFO_SOURCE && r.pos < data.len() {
        let edf = r.byte()?;
        if edf & 0x80 != 0 && r.pos + 2 <= data.len() {
            let game_port = r.u16()?; // puerto de juego (el primer campo EDF)
            if game_port != 0 {
                connect = ServerAddress::new(address.host.clone(), game_port);
            }
        }
    }
    if !expected_gamedir.is_empty() && casefold(&folder) != casefold(expected_gamedir) {
        return Err(invalid(format!("Servidor de otro juego: {folder:?}")));
    }
    rules.insert("gamedir".to_owned(), folder.clone());
    rules.insert("game".to_owned(), game.clone());
    rules.insert("bots".to_owned(), bots.to_string());
    let mut info = ServerInfo::new(address.clone(), connect);
    info.name = strip_control(&name);
    info.map = map;
    info.gametype = if game.is_empty() { folder } else { game };
    info.num_players = i32::from(players);
    info.bots = i32::from(bots.min(players));
    info.max_players = i32::from(max_players);
    info.password = visibility == 1;
    info.version = version;
    info.ping_ms = ping_ms;
    info.rules = rules;
    Ok(info)
}

/// Respuesta A2S_PLAYER (0x44). El protocolo no da ping por jugador (queda en 0).
pub fn parse_players(data: &[u8]) -> Result<Vec<Player>, ProtocolError> {
    if data.len() < 6 || !data.starts_with(OOB) || data[4] != PLAYER_RESPONSE {
        return Err(invalid("Respuesta A2S_PLAYER inválida"));
    }
    let mut r = Reader { data, pos: 5 };
    let count = r.byte()?;
    (0..count)
        .map(|_| {
            r.byte()?; // índice
            let name = r.string()?;
            let score = r.i32()?;
            r.take(4)?; // duración (f32)
            Ok(Player::new(strip_control(&name), score, 0))
        })
        .collect()
}
