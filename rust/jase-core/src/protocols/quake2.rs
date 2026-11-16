//! Protocolo Quake II: master (`query`) y `status` por UDP.
//! Traducción de `src/jase/protocols/quake2.py`.
use super::{
    ProtocolError, bounded_int, count_bots, invalid, ipv4, parse_infostring, strip_control,
};
use crate::models::{Player, ServerAddress, ServerInfo};
use crate::pystr::{casefold, is_py_digit, latin1, py_split, py_strip};

pub const MASTER_QUERY: &[u8] = b"query\n\0";
pub const STATUS_QUERY: &[u8] = b"\xff\xff\xff\xffstatus\n";

const MASTER_HEADER: &[u8] = b"\xff\xff\xff\xffservers ";
const STATUS_HEADER: &[u8] = b"\xff\xff\xff\xffprint\n";
const ENTRY_SIZE: usize = 6; // 4 IP + 2 puerto big-endian

#[derive(Clone, Debug)]
pub struct Quake2Options<'a> {
    pub version_key: &'a str,
    pub expected_gamedir: &'a str,
    pub exclude_mods: &'a [String],
}

impl Default for Quake2Options<'_> {
    fn default() -> Self {
        Self {
            version_key: "version",
            expected_gamedir: "",
            exclude_mods: &[],
        }
    }
}

pub fn parse_master_response(data: &[u8]) -> Result<Vec<ServerAddress>, ProtocolError> {
    let body = data
        .strip_prefix(MASTER_HEADER)
        .ok_or_else(|| invalid("Respuesta del master inválida"))?;
    Ok(body
        .chunks_exact(ENTRY_SIZE)
        .filter_map(|e| {
            let port = u16::from_be_bytes([e[4], e[5]]);
            (port != 0).then(|| ServerAddress::new(ipv4(e), port))
        })
        .collect())
}

/// «3.20 x86_64 Linux» → «3.20»; «q2proded r1~277d2ef Mar 24 2026 …» → «q2proded r1~277d2ef».
pub fn short_version(version: &str) -> String {
    let words = py_split(version);
    match words.first() {
        None => String::new(),
        Some(w) if w.chars().next().is_some_and(is_py_digit) => (*w).to_owned(),
        Some(_) => words[..words.len().min(2)].join(" "),
    }
}

fn parse_player(line: &str) -> Result<Player, ProtocolError> {
    let err = || invalid(format!("Línea de jugador inválida: {line:?}"));
    let (Some(first), Some(last)) = (line.find('"'), line.rfind('"')) else {
        return Err(err());
    };
    if last <= first {
        return Err(err());
    }
    let [score, ping] = py_split(&line[..first])[..] else {
        return Err(err());
    };
    let name = strip_control(&line[first + 1..last]);
    let mut p = Player::new(name.clone(), bounded_int(score)?, bounded_int(ping)?);
    p.raw_name = name;
    Ok(p)
}

/// `expected_gamedir` descarta los servidores de otra carpeta de juego y `exclude_mods`
/// los de esas carpetas (comparadas con casefold).
pub fn parse_status(
    data: &[u8],
    address: &ServerAddress,
    ping_ms: u32,
    opts: &Quake2Options,
) -> Result<ServerInfo, ProtocolError> {
    let body = data
        .strip_prefix(STATUS_HEADER)
        .ok_or_else(|| invalid("Falta la cabecera print"))?;
    let text = latin1(body);
    let lines: Vec<&str> = text.split('\n').collect();
    let rules = parse_infostring(lines[0]);
    let get = |k: &str| rules.get(k).map(String::as_str).unwrap_or("");
    let gamedir = casefold(match get("gamedir") {
        "" => get("game"),
        g => g,
    });
    if !opts.expected_gamedir.is_empty() && gamedir != casefold(opts.expected_gamedir) {
        return Err(invalid(format!("gamedir distinta: {gamedir}")));
    }
    if !gamedir.is_empty() && opts.exclude_mods.iter().any(|m| casefold(m) == gamedir) {
        return Err(invalid(format!("gamedir excluida: {gamedir}")));
    }
    let bad = |_| invalid("maxclients/needpass inválido");
    let max_players =
        bounded_int(rules.get("maxclients").map_or("0", String::as_str)).map_err(bad)?;
    let password = match get("needpass") {
        "" => false,
        n => bounded_int(n).map_err(bad)? != 0,
    };
    let players = lines[1..]
        .iter()
        .filter(|l| !py_strip(l).is_empty())
        .map(|l| parse_player(l))
        .collect::<Result<Vec<_>, _>>()?;
    let version = if opts.version_key == "version" {
        short_version(get("version"))
    } else {
        get(opts.version_key).to_owned()
    };
    let name = strip_control(get("hostname"));
    let mut info = ServerInfo::new(address.clone(), address.clone());
    info.name = name.clone();
    info.map = get("mapname").to_owned();
    info.gametype = get("gamename").to_owned();
    info.num_players = players.len() as i32;
    info.max_players = max_players;
    info.password = password;
    info.version = version;
    info.ping_ms = ping_ms;
    info.bots = count_bots(&players);
    info.players = players;
    info.raw_name = name;
    info.rules = rules;
    Ok(info)
}
