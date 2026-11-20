//! Protocolo GameSpy (UT99, MOHAA, BF1942, Tactical Ops): master y consulta \status\.
//! Traducción de `protocols/gamespy.py`.
use super::quake3::VersionAlias;
use super::{ProtocolError, bounded_int, invalid, strip_control};
use crate::models::{Player, ServerAddress, ServerInfo};
use crate::pystr::{casefold, latin1};
use indexmap::IndexMap;
use std::collections::BTreeMap;

pub const UT_GAMENAME: &str = "ut";
pub const UT_GAMEKEY: &str = "Z5Nfb0";
pub const STATUS_QUERY: &[u8] = b"\\status\\";

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn parse_kv(data: &[u8]) -> Vec<(String, String)> {
    let text = latin1(data);
    let mut parts: Vec<&str> = text.split('\\').collect();
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

pub fn parse_secure(data: &[u8]) -> Result<String, ProtocolError> {
    parse_kv(data)
        .into_iter()
        .find(|(k, _)| k == "secure")
        .map(|(_, v)| v)
        .ok_or_else(|| invalid("Falta el campo secure en la respuesta del master"))
}

/// Respuesta al desafío `secure` del master (algoritmo gsmsalg, RC4 + base64 propio).
pub fn gsmsalg(secure: &str, key: &str) -> String {
    debug_assert!(!key.is_empty());
    let k = key.as_bytes();
    let mut src = secure.as_bytes().to_vec();
    let mut table: Vec<u8> = (0..=255).collect();
    let mut a: u8 = 0;
    for i in 0..256 {
        a = a.wrapping_add(table[i]).wrapping_add(k[i % k.len()]);
        table.swap(usize::from(a), i);
    }
    let (mut a, mut b) = (0u8, 0u8);
    for byte in &mut src {
        a = a.wrapping_add(*byte).wrapping_add(1);
        let x = table[usize::from(a)];
        b = b.wrapping_add(x);
        let y = table[usize::from(b)];
        table[usize::from(b)] = x;
        table[usize::from(a)] = y;
        *byte ^= table[usize::from(x.wrapping_add(y))];
    }
    let mut out = String::new();
    for c in src.as_chunks::<3>().0 {
        let t = (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]);
        for s in [18, 12, 6, 0] {
            out.push(char::from(ALPHABET[((t >> s) & 63) as usize]));
        }
    }
    out
}

fn latin1_bytes(text: &str) -> Vec<u8> {
    // Los textos son constantes o vienen del master en latin-1.
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

pub fn build_validate(secure: &str, gamename: &str, gamekey: &str) -> Vec<u8> {
    let response = gsmsalg(secure, gamekey);
    latin1_bytes(&format!(
        "\\gamename\\{gamename}\\enctype\\0\\validate\\{response}\\final\\"
    ))
}

pub fn build_list_request(gamename: &str) -> Vec<u8> {
    latin1_bytes(&format!("\\list\\\\gamename\\{gamename}\\final\\"))
}

pub fn parse_master_list(data: &[u8]) -> Vec<ServerAddress> {
    parse_kv(data)
        .into_iter()
        .filter(|(k, _)| k == "ip")
        .filter_map(|(_, value)| {
            let (host, port) = value.rsplit_once(':')?;
            if host.is_empty() || port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let port: u16 = port.parse().ok().filter(|&p| p >= 1)?;
            Some(ServerAddress::new(host, port))
        })
        .collect()
}

/// `$` de una regex de Python acepta un `\n` final.
fn strip_final_newline(s: &str) -> &str {
    s.strip_suffix('\n').unwrap_or(s)
}

/// `^(.+)_(\d+)$` → (prefijo, dígitos del índice).
fn indexed_field(key: &str) -> Option<(&str, &str)> {
    let k = strip_final_newline(key);
    if k.contains('\n') {
        return None;
    }
    let (prefix, digits) = k.rsplit_once('_')?;
    if prefix.is_empty() || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((prefix, digits))
}

/// `^\d+\.(\d+)$` → número de paquete (≥ 1).
fn queryid_number(value: &str) -> Option<u32> {
    let (a, b) = strip_final_newline(value).split_once('.')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    if !digits(a) || !digits(b) {
        return None;
    }
    b.parse::<u32>().ok().filter(|&n| n >= 1)
}

/// Reensambla las respuestas \status\ fragmentadas, posiblemente desordenadas.
#[derive(Debug, Default)]
pub struct StatusAssembler {
    packets: BTreeMap<u32, Vec<(String, String)>>,
    total: Option<u32>,
}

impl StatusAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, data: &[u8]) -> Result<bool, ProtocolError> {
        let pairs = parse_kv(data);
        let mut number = None;
        let mut last = false;
        for (k, v) in &pairs {
            match k.as_str() {
                "queryid" => {
                    if let Some(n) = queryid_number(v) {
                        number = Some(n);
                    }
                }
                "final" => last = true,
                _ => {}
            }
        }
        let number = number.ok_or_else(|| invalid("Paquete de estado sin queryid válido"))?;
        self.packets.insert(number, pairs);
        if last {
            self.total = Some(number);
        }
        Ok(self.is_complete())
    }

    pub fn is_complete(&self) -> bool {
        self.total
            .is_some_and(|t| (1..=t).all(|n| self.packets.contains_key(&n)))
    }

    pub fn fields(&self) -> IndexMap<String, String> {
        let mut out = IndexMap::new();
        for pairs in self.packets.values() {
            for (k, v) in pairs {
                if k != "queryid" && k != "final" {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct GamespyOptions<'a> {
    /// Desplazamiento sobre el puerto de consulta si falta `hostport` (UT99: −1).
    pub connect_port_fallback: i32,
    /// Si no está vacío y el servidor envía otro `gamename`, Err (otro juego).
    pub expected_gamename: &'a str,
    pub version_key: &'a str,
    pub version_aliases: &'a [VersionAlias],
}

impl Default for GamespyOptions<'_> {
    fn default() -> Self {
        Self {
            connect_port_fallback: -1,
            expected_gamename: "",
            version_key: "gamever",
            version_aliases: &[],
        }
    }
}

fn to_int(fields: &IndexMap<String, String>, key: &str) -> Result<i32, ProtocolError> {
    match fields.get(key) {
        None => Ok(0),
        Some(v) => {
            bounded_int(v).map_err(|_| invalid(format!("Valor numérico inválido en {key}: {v:?}")))
        }
    }
}

pub fn parse_status(
    fields: &IndexMap<String, String>,
    address: &ServerAddress,
    ping_ms: u32,
    opts: &GamespyOptions,
) -> Result<ServerInfo, ProtocolError> {
    let get = |k: &str| fields.get(k).map(String::as_str).unwrap_or("");
    let gamename = get("gamename");
    if !opts.expected_gamename.is_empty()
        && !gamename.is_empty()
        && casefold(gamename) != casefold(opts.expected_gamename)
    {
        return Err(invalid(format!(
            "Servidor de otro juego: gamename {gamename:?}"
        )));
    }
    let num_players = to_int(fields, "numplayers")?;
    let max_players = to_int(fields, "maxplayers")?;
    let fallback = i64::from(address.port) + i64::from(opts.connect_port_fallback);
    let hostport = match fields.get("hostport") {
        Some(_) => i64::from(to_int(fields, "hostport")?),
        None => fallback,
    };
    let in_range = (1..=65535).contains(&hostport);
    let deduced = !fields.contains_key("hostport") || !in_range;
    // Fuera de rango tras el fallback (puerto de consulta 0): 0, nunca negativo.
    let port = if in_range { hostport } else { fallback }.clamp(0, 65535) as u16;

    let mut indices = Vec::new();
    for key in fields.keys() {
        if let Some(("player", digits)) = indexed_field(key) {
            // Python acepta cualquier cifra; un índice que no cabe en u64 descarta el servidor.
            let n: u64 = digits
                .parse()
                .map_err(|_| invalid(format!("Índice demasiado grande: {key:?}")))?;
            indices.push(n);
        }
    }
    indices.sort_unstable();
    let players = indices
        .iter()
        .map(|i| {
            let name = fields
                .get(&format!("player_{i}"))
                .ok_or_else(|| invalid(format!("Falta player_{i}")))?;
            Ok(Player::new(
                strip_control(name),
                to_int(fields, &format!("frags_{i}"))?,
                to_int(fields, &format!("ping_{i}"))?,
            ))
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    let rules = fields
        .iter()
        .filter(|(k, _)| *k != "queryid" && *k != "final" && indexed_field(k).is_none())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut version = match get(opts.version_key) {
        "" => get("gamever"),
        v => v,
    }
    .to_owned();
    if let Some(a) = opts.version_aliases.iter().find(|a| a.prefix == version) {
        version = a.key.clone();
    }
    let mut info = ServerInfo::new(
        address.clone(),
        ServerAddress::new(address.host.clone(), port),
    );
    info.connect_port_deduced = deduced;
    info.name = strip_control(get("hostname"));
    info.map = get("mapname").to_owned();
    info.gametype = get("gametype").to_owned();
    info.num_players = num_players;
    info.max_players = max_players;
    info.password = matches!(get("password").to_lowercase().as_str(), "true" | "1");
    info.version = version;
    info.ping_ms = ping_ms;
    info.players = players;
    info.rules = rules;
    Ok(info)
}
