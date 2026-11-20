//! ¿Qué juego es este servidor? Lógica pura (sin red) para «Detectar juego».
//! Traducción de `identify.py`.
//!
//! Se consulta la dirección con cada familia de protocolo (`probe_plan`) y cada respuesta
//! se cruza con los `GameDef` (`identify`): en Valve y GameSpy el servidor dice su juego
//! (appid/carpeta, `gamename`); en Quake 3 varios juegos comparten protocolo y se
//! desempata con pistas del `gamename` y la versión (sacadas de las capturas de
//! `tests/fixtures`).
use crate::games::{Family, GAMES, GameDef};
use crate::models::{ServerAddress, ServerInfo};
use crate::pystr::{casefold, py_strip};
use indexmap::IndexMap;
use std::time::Duration;

/// Zandronum ignora unos 10 s todo lo que llegue de una IP tras cualquier paquete: su
/// consulta sale primero y las demás familias, un poco después (`probe_delay`).
pub const FIRST_FAMILIES: [Family; 1] = [Family::Zandronum];
pub const STAGGER: Duration = Duration::from_millis(250);

pub fn probe_delay(family: Family) -> Duration {
    if FIRST_FAMILIES.contains(&family) {
        Duration::ZERO
    } else {
        STAGGER
    }
}

/// Quake 3: textos (en minúsculas) que, en `gamename` o en la versión, delatan el juego.
pub const QUAKE3_HINTS: &[(&str, &[&str])] = &[
    ("cod2", &["call of duty 2"]),
    ("cod4", &["call of duty 4", "cod4"]),
    ("coduo", &["united offensive"]),
    ("cod1", &["call of duty"]), // el último: «call of duty 2/4» ya se reconocen antes
    ("urt", &["q3ut", "q3urt", " urt "]),
    ("smokinguns", &["smokinguns"]),
    ("oa", &["baseoa", "+oa"]),
    ("jka", &["jamp"]),
    ("jk2", &["jk2mp"]),
    ("sof2", &["sof2"]),
    ("tremulous", &["tremulous"]),
    ("rtcw", &["wolf ", "iortcw"]), // iortcw no siempre envía `protocol`
    ("et", &["et legacy", "etlegacy", "et 2."]),
    ("q3", &["baseq3"]),
];

/// Carpeta del juego → juego, para servidores que responden por otra familia (Warsow 2.1
/// contesta también A2S de Valve con carpeta «basewsw»).
pub const FOLDER_HINTS: [(&str, &str); 2] = [("basewsw", "warsow"), ("basewf", "warfork")];

/// (familia, direcciones) a consultar. En GameSpy y UT2004 el puerto de consulta suele
/// ser otro que el de juego: se prueban también los desplazamientos conocidos.
pub fn probe_plan(address: &ServerAddress) -> Vec<(Family, Vec<ServerAddress>)> {
    let mut gamespy: Vec<i32> = family_games(Family::Gamespy)
        .filter(|g| g.connect_port_fallback != 0)
        .map(|g| -g.connect_port_fallback)
        .collect();
    gamespy.sort_unstable();
    gamespy.dedup();
    Family::ALL
        .into_iter()
        .map(|family| {
            let offsets: &[i32] = match family {
                Family::Gamespy => &gamespy,
                Family::Ut2004 => &[1],
                _ => &[],
            };
            let shifted = offsets.iter().filter_map(|d| {
                u16::try_from(i32::from(address.port) + d)
                    .ok()
                    .filter(|&p| p >= 1)
            });
            let ports = std::iter::once(address.port).chain(shifted);
            let addrs = ports
                .map(|p| ServerAddress::new(address.host.clone(), p))
                .collect();
            (family, addrs)
        })
        .collect()
}

fn family_games(family: Family) -> impl Iterator<Item = &'static GameDef> {
    GAMES.iter().filter(move |g| g.family == family)
}

fn rule<'a>(info: &'a ServerInfo, key: &str) -> &'a str {
    info.rules.get(key).map_or("", String::as_str)
}

fn quake3(info: &ServerInfo) -> Vec<&'static str> {
    let rules: IndexMap<String, &str> = info
        .rules
        .iter()
        .map(|(k, v)| (casefold(k), v.as_str()))
        .collect();
    let get = |k: &str| rules.get(k).copied().unwrap_or("");
    let gamename = casefold(get("gamename"));
    let text = format!(" {gamename} {} ", casefold(get("version")));
    let protocol = py_strip(get("protocol")).parse::<i64>().ok();
    let by_master: Vec<_> = family_games(Family::Quake3)
        .filter(|g| !g.master_gamename.is_empty() && casefold(g.master_gamename) == gamename)
        .map(|g| g.id)
        .collect();
    if !by_master.is_empty() {
        return by_master; // Xonotic, Warsow, Warfork: el gamename es el del master
    }
    let mut hinted: Vec<&'static str> = QUAKE3_HINTS
        .iter()
        .filter(|(_, hints)| hints.iter().any(|h| text.contains(h)))
        .map(|(id, _)| *id)
        .collect();
    if gamename != "call of duty" {
        hinted.retain(|&id| id != "cod1"); // «call of duty» sale también en CoD2/CoD4
    }
    let by_protocol: Vec<_> = family_games(Family::Quake3)
        .filter(|g| protocol.is_some_and(|p| g.protocols.iter().any(|&q| i64::from(q) == p)))
        .filter(|g| {
            !g.status_exclude_mods
                .iter()
                .any(|m| casefold(m) == gamename)
        })
        .map(|g| g.id)
        .collect();
    let both: Vec<_> = hinted
        .iter()
        .copied()
        .filter(|id| by_protocol.contains(id))
        .collect();
    if !both.is_empty() {
        both
    } else if !hinted.is_empty() {
        hinted.truncate(1);
        hinted
    } else {
        by_protocol
    }
}

fn gamespy(info: &ServerInfo) -> Vec<&'static str> {
    let gamename = casefold(rule(info, "gamename"));
    if gamename.is_empty() {
        return Vec::new();
    }
    family_games(Family::Gamespy)
        .filter(|g| {
            (casefold(g.status_gamename) == gamename || casefold(g.gamespy_gamename) == gamename)
                && g.lists_server(&info.rules)
        })
        .map(|g| g.id)
        .collect()
}

fn valve(info: &ServerInfo) -> Vec<&'static str> {
    let appid = rule(info, "appid");
    let by_appid: Vec<_> = family_games(Family::Valve)
        .filter(|g| !appid.is_empty() && g.valve_appid.to_string() == appid)
        .map(|g| g.id)
        .collect();
    if !by_appid.is_empty() {
        return by_appid;
    }
    let gamedir = casefold(rule(info, "gamedir"));
    let by_gamedir: Vec<_> = family_games(Family::Valve)
        .filter(|g| !gamedir.is_empty() && casefold(g.valve_gamedir) == gamedir)
        .map(|g| g.id)
        .collect();
    if !by_gamedir.is_empty() {
        return by_gamedir;
    }
    FOLDER_HINTS
        .iter()
        .filter(|(dir, _)| *dir == gamedir)
        .map(|(_, id)| *id)
        .collect()
}

fn quake2(info: &ServerInfo) -> Vec<&'static str> {
    let folder = match rule(info, "gamedir") {
        "" => rule(info, "game"),
        dir => dir,
    };
    let folder = casefold(folder);
    let special: Vec<_> = family_games(Family::Quake2)
        .filter(|g| !g.status_gamedir.is_empty() && casefold(g.status_gamedir) == folder)
        .map(|g| g.id)
        .collect();
    if !special.is_empty() {
        return special;
    }
    family_games(Family::Quake2)
        .filter(|g| g.status_gamedir.is_empty())
        .map(|g| g.id)
        .collect()
}

/// Ids de juego que encajan con la respuesta, del más probable al menos.
pub fn identify(family: Family, info: &ServerInfo) -> Vec<&'static str> {
    match family {
        Family::Quake3 => quake3(info),
        Family::Gamespy => gamespy(info),
        Family::Valve => valve(info),
        Family::Quake2 => quake2(info),
        // Un solo juego por familia.
        Family::Ut2004 | Family::Zandronum => family_games(family).map(|g| g.id).collect(),
    }
}

/// (juego, dirección de consulta) de todas las respuestas, sin repetir juego.
pub fn detected_games(responses: &[(Family, ServerInfo)]) -> Vec<(&'static str, ServerAddress)> {
    let mut result: IndexMap<&'static str, ServerAddress> = IndexMap::new();
    for (family, info) in responses {
        for id in identify(*family, info) {
            result.entry(id).or_insert_with(|| info.address.clone());
        }
    }
    result.into_iter().collect()
}
