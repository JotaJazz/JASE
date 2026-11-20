//! Protocolo Quake3 (CoD, CoD:UO, CoD2, CoD4, Quake III, RtCW, ET): master y getstatus.
//! Traducción de `protocols/quake3.py`.
use super::{
    ProtocolError, bounded_int, count_bots, invalid, ipv4, parse_infostring, strip_control,
};
use crate::models::{Player, ServerAddress, ServerInfo};
use crate::pystr::{casefold, latin1, py_split, py_strip};

pub const OOB: &[u8] = b"\xff\xff\xff\xff";
pub const GETSTATUS: &[u8] = b"\xff\xff\xff\xffgetstatus";

const MASTER_HEADER: &[u8] = b"getserversResponse";
const STATUS_HEADER: &[u8] = b"\xff\xff\xff\xffstatusResponse";
const ENTRY_SIZE: usize = 7; // "\" + 4 IP + 2 puerto big-endian

/// Versión legible a partir del número de protocolo (SoF2, `version_key="protocol"`).
const PROTOCOL_VERSIONS: [(&str, &str); 2] = [("2002", "1.00"), ("2004", "1.03")];

/// Alias de versión: (prefijo de la versión informada, clave del selector, etiqueta).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionAlias {
    pub prefix: String,
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct StatusOptions<'a> {
    pub version_key: &'a str,
    pub version_aliases: &'a [VersionAlias],
    pub exclude_mods: &'a [String],
}

impl Default for StatusOptions<'_> {
    fn default() -> Self {
        Self {
            version_key: "shortversion",
            version_aliases: &[],
            exclude_mods: &[],
        }
    }
}

/// Consulta al master; con `gamename` (dpmaster, que mezcla juegos) va antes del protocolo.
pub fn build_getservers(protocol: u32, gamename: &str) -> Vec<u8> {
    let prefix = if gamename.is_empty() {
        String::new()
    } else {
        format!("{gamename} ")
    };
    let mut out = OOB.to_vec();
    out.extend(format!("getservers {prefix}{protocol} full empty").bytes());
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// (servidores, es_el_último_paquete). Paquetes intermedios terminan en "\EOT" y el
/// último en "\EOF"; también se acepta "\EOT\0\0\0" (Quake3 clásico) como final.
pub fn parse_getservers_response(data: &[u8]) -> Result<(Vec<ServerAddress>, bool), ProtocolError> {
    let start =
        find(data, MASTER_HEADER).ok_or_else(|| invalid("Respuesta del master inválida"))?;
    let body = &data[start + MASTER_HEADER.len()..];
    let Some(mut pos) = body.iter().position(|&b| b == b'\\') else {
        return Ok((Vec::new(), false));
    };
    let mut servers = Vec::new();
    while body.len() - pos >= 4 {
        if body[pos] != b'\\' {
            break;
        }
        let tail = &body[pos + 1..pos + 4];
        let rest = &body[pos + 4..];
        let padded = rest.starts_with(b"\0\0\0");
        if (tail == b"EOT" || tail == b"EOF") && (rest.is_empty() || padded) {
            return Ok((servers, tail == b"EOF" || padded));
        }
        if body.len() - pos < ENTRY_SIZE {
            break;
        }
        let port = u16::from_be_bytes([body[pos + 5], body[pos + 6]]);
        servers.push(ServerAddress::new(ipv4(&body[pos + 1..pos + 5]), port));
        pos += ENTRY_SIZE;
    }
    Ok((servers, false))
}

/// Versión corta y filtrable a partir del cvar `version` (Quake III, RtCW, ET): las
/// palabras hasta la primera que empieza por número (o "v" + número), cortada en el
/// primer "+", "_", "/" o "-".
pub fn short_engine_version(version: &str) -> String {
    let words = py_split(version);
    for (i, word) in words.iter().enumerate() {
        let mut chars = word.chars();
        let first = chars.next();
        let numeric = match first {
            Some(c) if c.is_ascii_digit() => true,
            Some('v') => chars.next().is_some_and(|c| c.is_ascii_digit()),
            _ => false,
        };
        if numeric {
            let cut = word.split(['+', '_', '/', '-']).next().unwrap_or_default();
            let mut out: Vec<&str> = words[..i].to_vec();
            out.push(cut);
            return out.join(" ");
        }
    }
    words.join(" ")
}

/// Quita códigos de color ^X (también «^^N» y «^^NN») y caracteres de control.
pub fn strip_colors(text: &str) -> String {
    let chars: Vec<char> = strip_control(text).chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '^' {
            let next = chars.get(i + 1).copied();
            let digit2 = chars.get(i + 2).copied().filter(char::is_ascii_digit);
            if next == Some('^')
                && let Some(d) = digit2
            {
                i += if chars.get(i + 3) == Some(&d) { 4 } else { 3 };
                continue;
            }
            if next.is_some_and(|c| c.is_ascii_alphanumeric()) {
                i += 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
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
    let raw = &line[first + 1..last];
    let mut p = Player::new(strip_colors(raw), bounded_int(score)?, bounded_int(ping)?);
    p.raw_name = strip_control(raw);
    Ok(p)
}

pub fn parse_status_response(
    data: &[u8],
    address: &ServerAddress,
    ping_ms: u32,
    opts: &StatusOptions,
) -> Result<ServerInfo, ProtocolError> {
    if !data.starts_with(STATUS_HEADER) {
        return Err(invalid("Falta statusResponse"));
    }
    let text = latin1(data);
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() < 2 {
        return Err(invalid("statusResponse sin infostring"));
    }
    let rules = parse_infostring(lines[1]);
    let get = |k: &str| rules.get(k).map(String::as_str).unwrap_or("");
    let max_players = bounded_int(rules.get("sv_maxclients").map_or("0", String::as_str))
        .map_err(|_| invalid("sv_maxclients inválido"))?;
    let gamename = casefold(get("gamename"));
    if opts.exclude_mods.iter().any(|m| casefold(m) == gamename) {
        return Err(invalid(format!("gamename excluido: {}", get("gamename"))));
    }
    let players = lines[2..]
        .iter()
        .filter(|l| !py_strip(l).is_empty())
        .map(|l| parse_player(l))
        .collect::<Result<Vec<_>, _>>()?;
    let mut version = get(opts.version_key).to_owned();
    if opts.version_key == "version" {
        version = short_engine_version(&version);
    } else if opts.version_key == "protocol"
        && let Some((_, v)) = PROTOCOL_VERSIONS.iter().find(|(p, _)| *p == version)
    {
        version = (*v).to_owned();
    }
    if let Some(a) = opts
        .version_aliases
        .iter()
        .find(|a| version == a.prefix || version.starts_with(&format!("{}.", a.prefix)))
    {
        version = a.key.clone();
    }
    // DarkPlaces (Xonotic) manda `hostname` en vez de `sv_hostname`.
    let hostname = match get("sv_hostname") {
        "" => get("hostname"),
        h => h,
    }
    .to_owned();
    let mut info = ServerInfo::new(address.clone(), address.clone());
    info.name = strip_colors(&hostname);
    info.map = get("mapname").to_owned();
    info.gametype = get("g_gametype").to_owned();
    info.num_players = players.len() as i32;
    info.max_players = max_players;
    info.password = get("pswrd") == "1" || get("g_needpass") == "1";
    info.version = version;
    info.ping_ms = ping_ms;
    info.bots = count_bots(&players);
    info.players = players;
    info.raw_name = strip_control(&hostname);
    info.rules = rules;
    Ok(info)
}
