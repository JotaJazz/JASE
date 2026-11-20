//! Protocolo de lanzadores de Zandronum (Doom multijugador): master y consulta. Sin red.
//! Traducción de `protocols/zandronum.py`.
//!
//! Todo paquete va con el Huffman de `huffman` (o sin comprimir, con 0xFF delante).
//! Enteros en little-endian; cadenas terminadas en \0.
use super::{ProtocolError, huffman, invalid, ipv4, strip_control};
use crate::models::{Player, ServerAddress, ServerInfo};
use crate::pystr::latin1;
use indexmap::IndexMap;

pub const MASTER_CHALLENGE: i32 = 5660028;
pub const MASTER_PROTOCOL: u16 = 2;
pub const SERVER_CHALLENGE: i32 = 199;
pub const SERVER_ACCEPTED: i32 = 5660023;
/// Se consultó hace muy poco.
pub const SERVER_IGNORING: i32 = 5660024;
/// Espera antes de reintentar un servidor que ignora la consulta (sv_queryignoretime).
pub const IGNORE_WAIT_S: f64 = 10.5;

const MSC_BEGINSERVERLISTPART: i32 = 6;
const MSC_SERVERBLOCK: u8 = 8;
const MSC_ENDSERVERLISTPART: u8 = 7;
const MSC_ENDSERVERLIST: u8 = 2;
/// El master pide unos 10 s entre consultas.
pub const MASTER_IGNORING: i32 = 4;

fn master_error(code: i32) -> Option<&'static str> {
    match code {
        3 => Some("IP bloqueada en el master"),
        4 => Some("consulta ignorada (demasiado seguida)"),
        5 => Some("versión de protocolo antigua"),
        _ => None,
    }
}

// Campos pedidos al servidor (SQF_*), en el orden en que llegan.
pub const SQF_NAME: u32 = 0x1;
pub const SQF_MAPNAME: u32 = 0x8;
pub const SQF_MAXCLIENTS: u32 = 0x10;
pub const SQF_MAXPLAYERS: u32 = 0x20;
pub const SQF_PWADS: u32 = 0x40;
pub const SQF_GAMETYPE: u32 = 0x80;
pub const SQF_GAMENAME: u32 = 0x100;
pub const SQF_IWAD: u32 = 0x200;
pub const SQF_FORCEPASSWORD: u32 = 0x400;
pub const SQF_FORCEJOINPASSWORD: u32 = 0x800;
pub const SQF_NUMPLAYERS: u32 = 0x80000;
pub const SQF_PLAYERDATA: u32 = 0x100000;
pub const QUERY_FLAGS: u32 = SQF_NAME
    | SQF_MAPNAME
    | SQF_MAXCLIENTS
    | SQF_MAXPLAYERS
    | SQF_PWADS
    | SQF_GAMETYPE
    | SQF_GAMENAME
    | SQF_IWAD
    | SQF_FORCEPASSWORD
    | SQF_FORCEJOINPASSWORD
    | SQF_NUMPLAYERS
    | SQF_PLAYERDATA;

/// Modos de juego (GAMEMODE_e).
pub const GAMEMODES: [&str; 16] = [
    "cooperative",
    "survival",
    "invasion",
    "deathmatch",
    "teamplay",
    "duel",
    "terminator",
    "lastmanstanding",
    "teamlms",
    "possession",
    "teampossession",
    "teamgame",
    "ctf",
    "oneflagctf",
    "skulltag",
    "domination",
];
/// Modos con equipos (el jugador trae un byte más).
const TEAM_MODES: [i32; 8] = [4, 8, 10, 11, 12, 13, 14, 15];

pub fn master_request() -> Vec<u8> {
    huffman::encode_raw(
        &[
            &MASTER_CHALLENGE.to_le_bytes()[..],
            &MASTER_PROTOCOL.to_le_bytes(),
        ]
        .concat(),
    )
}

pub fn build_query(time_ms: u64) -> Vec<u8> {
    let time = (time_ms & 0x7FFF_FFFF) as i32;
    huffman::encode_raw(
        &[
            SERVER_CHALLENGE.to_le_bytes(),
            QUERY_FLAGS.to_le_bytes(),
            time.to_le_bytes(),
        ]
        .concat(),
    )
}

/// (direcciones del paquete, ¿era el último?).
pub fn parse_master_packet(data: &[u8]) -> Result<(Vec<ServerAddress>, bool), ProtocolError> {
    let body = huffman::decode(data);
    if body.len() < 4 {
        return Err(invalid("Respuesta del master demasiado corta"));
    }
    let code = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    if code == MASTER_IGNORING {
        return Err(ProtocolError::MasterBusy(
            master_error(code).unwrap_or_default().to_owned(),
        ));
    }
    if let Some(msg) = master_error(code) {
        return Err(invalid(msg));
    }
    if code != MSC_BEGINSERVERLISTPART || body.len() < 6 {
        return Err(invalid(format!("Respuesta del master inesperada: {code}")));
    }
    let truncated = || invalid("Lista del master truncada");
    let mut pos = 5; // código + número de paquete
    let mut found = Vec::new();
    loop {
        let command = *body.get(pos).ok_or_else(truncated)?;
        pos += 1;
        match command {
            MSC_ENDSERVERLISTPART => return Ok((found, false)),
            MSC_ENDSERVERLIST => return Ok((found, true)),
            MSC_SERVERBLOCK => {}
            _ => {
                return Err(invalid(format!(
                    "Bloque desconocido en el master: {command}"
                )));
            }
        }
        loop {
            let count = *body.get(pos).ok_or_else(truncated)?;
            pos += 1;
            if count == 0 {
                break;
            }
            let host = ipv4(body.get(pos..pos + 4).ok_or_else(truncated)?);
            pos += 4;
            for _ in 0..count {
                let p = body.get(pos..pos + 2).ok_or_else(truncated)?;
                let port = u16::from_le_bytes([p[0], p[1]]);
                pos += 2;
                if port != 0 {
                    found.push(ServerAddress::new(host.clone(), port));
                }
            }
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ProtocolError> {
        let out = self
            .data
            .get(self.pos..self.pos + n)
            .ok_or_else(|| invalid("Respuesta truncada"))?;
        self.pos += n;
        Ok(out)
    }
    fn byte(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16, ProtocolError> {
        let b = self.take(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Result<i32, ProtocolError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(self.i32()? as u32)
    }
    fn string(&mut self) -> Result<String, ProtocolError> {
        let rest = self.data.get(self.pos..).unwrap_or_default();
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| invalid("Cadena sin terminar"))?;
        let s = latin1(&rest[..len]);
        self.pos += len + 1;
        Ok(s)
    }
}

/// Quita los códigos de color de Doom (\x1c + letra o \x1c[nombre]).
pub fn strip_colors(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\x1c' {
            if chars.get(i + 1) == Some(&'[') {
                i = chars[i + 2..]
                    .iter()
                    .position(|&c| c == ']')
                    .map_or(chars.len(), |e| i + 2 + e + 1);
            } else {
                i += 2;
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    strip_control(&out)
}

fn read_players(
    r: &mut Reader,
    count: u8,
    teams: bool,
) -> Result<(Vec<Player>, i32), ProtocolError> {
    let (mut players, mut bots) = (Vec::new(), 0);
    for _ in 0..count {
        let name = strip_colors(&r.string()?);
        let (score, ping) = (r.i16()?, r.i16()?);
        let _spectator = r.byte()?;
        let bot = r.byte()?;
        if teams {
            r.byte()?;
        }
        r.byte()?; // minutos en el servidor
        players.push(Player::new(name, i32::from(score), i32::from(ping)));
        bots += i32::from(bot != 0);
    }
    Ok((players, bots))
}

pub fn parse_response(
    data: &[u8],
    address: &ServerAddress,
    ping_ms: u32,
) -> Result<ServerInfo, ProtocolError> {
    let body = huffman::decode(data);
    let mut r = Reader {
        data: &body,
        pos: 0,
    };
    let code = r.i32()?;
    if code == SERVER_IGNORING {
        return Err(ProtocolError::ServerIgnoring(
            "consulta ignorada: demasiado seguida".to_owned(),
        ));
    }
    if code != SERVER_ACCEPTED {
        return Err(invalid(format!(
            "El servidor no respondió la consulta ({code})"
        )));
    }
    r.i32()?; // nuestro tiempo, devuelto
    let version = r.string()?;
    let flags = r.u32()?;
    if flags & !QUERY_FLAGS != 0 {
        return Err(invalid(format!(
            "Campos no pedidos en la respuesta: {flags:#x}"
        )));
    }
    let mut rules = IndexMap::new();
    rules.insert("version".to_owned(), version.clone());
    let name = if flags & SQF_NAME != 0 {
        strip_colors(&r.string()?)
    } else {
        String::new()
    };
    let map = if flags & SQF_MAPNAME != 0 {
        r.string()?
    } else {
        String::new()
    };
    let max_clients = if flags & SQF_MAXCLIENTS != 0 {
        r.byte()?
    } else {
        0
    };
    let max_players = if flags & SQF_MAXPLAYERS != 0 {
        r.byte()?
    } else {
        max_clients
    };
    if flags & SQF_PWADS != 0 {
        let n = r.byte()?;
        let pwads = (0..n).map(|_| r.string()).collect::<Result<Vec<_>, _>>()?;
        rules.insert("pwads".to_owned(), pwads.join(" "));
    }
    let mut mode: i32 = -1;
    if flags & SQF_GAMETYPE != 0 {
        let (m, instagib, buckshot) = (r.byte()?, r.byte()?, r.byte()?);
        mode = i32::from(m);
        rules.insert("instagib".to_owned(), instagib.to_string());
        rules.insert("buckshot".to_owned(), buckshot.to_string());
    }
    if flags & SQF_GAMENAME != 0 {
        rules.insert("game".to_owned(), r.string()?);
    }
    if flags & SQF_IWAD != 0 {
        rules.insert("iwad".to_owned(), r.string()?);
    }
    let password = flags & SQF_FORCEPASSWORD != 0 && r.byte()? != 0;
    if flags & SQF_FORCEJOINPASSWORD != 0 {
        rules.insert("forcejoinpassword".to_owned(), r.byte()?.to_string());
    }
    let count = if flags & SQF_NUMPLAYERS != 0 {
        r.byte()?
    } else {
        0
    };
    let (mut players, mut bots) = (Vec::new(), 0);
    if flags & SQF_PLAYERDATA != 0 && count != 0 {
        let start = r.pos;
        let teams = TEAM_MODES.contains(&mode);
        let first = read_players(&mut r, count, teams);
        match first {
            Ok(found) if r.pos == body.len() => (players, bots) = found,
            _ => {
                // modo desconocido: se prueba con/sin byte de equipo
                r.pos = start;
                (players, bots) = read_players(&mut r, count, !teams)?;
                if r.pos != body.len() {
                    return Err(invalid("Datos de jugadores inconsistentes"));
                }
            }
        }
    }
    let gametype = usize::try_from(mode)
        .ok()
        .and_then(|m| GAMEMODES.get(m))
        .copied()
        .unwrap_or("");
    // Versión filtrable: «3.2.1-r230709 on Linux…» → «3.2.1».
    let short = version
        .split(' ')
        .next()
        .unwrap_or("")
        .split('-')
        .next()
        .unwrap_or("");
    let mut info = ServerInfo::new(address.clone(), address.clone());
    info.name = name.clone();
    info.map = map;
    info.gametype = gametype.to_owned();
    info.num_players = i32::from(count);
    info.max_players = i32::from(max_players);
    info.password = password;
    info.version = short.to_owned();
    info.ping_ms = ping_ms;
    info.players = players;
    info.rules = rules;
    info.raw_name = name;
    info.bots = bots;
    Ok(info)
}
