//! Búsqueda de juegos instalados (registro, Steam y carpetas). Traducción de
//! `src/jase/detect.py`. Bloqueante (lee carpetas): la UI la llama fuera de su hilo.
use crate::config::Config;
use crate::games::{Family, GAMES, GameDef, game};
use crate::paths::{
    basename, dirname, normcase, normpath, resolve_exe, resolve_game_dir, win_parts,
};
use crate::pystr::{casefold, is_py_space};
use indexmap::{IndexMap, IndexSet};
use std::collections::HashSet;
use std::fs;
use std::path::{MAIN_SEPARATOR, Path};

/// De mejor a peor: el orden decide qué hallazgo queda al repetirse una carpeta.
pub const SOURCES: [&str; 4] = ["registro", "steam", "carpeta típica", "carpeta elegida"];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Finding {
    pub game_id: &'static str,
    pub path: String,
    pub source: &'static str,
}

/// Exe del juego en `install_dir`, o `None` si la carpeta no es de este juego: un hl.exe
/// solo es CS 1.6 si además existe la carpeta del mod (igual quake2.exe y D-Day).
pub fn resolve_game_exe(game: &GameDef, install_dir: Option<&str>) -> Option<String> {
    let exe = resolve_exe(
        install_dir,
        game.exe_relpath,
        game.exe_candidates,
        game.preferred_exes,
    )?;
    let mod_dir = if game.family == Family::Valve {
        game.valve_gamedir
    } else {
        game.status_gamedir
    };
    if !mod_dir.is_empty() && resolve_game_dir(install_dir, mod_dir).is_none() {
        return None;
    }
    Some(exe)
}

pub fn is_configured(game: &GameDef, cfg: &Config) -> bool {
    resolve_game_exe(game, cfg.game_paths.get(game.id).map(String::as_str)).is_some()
}

/// Pares (subclave bajo `HKLM\SOFTWARE`, valor); se leen en las vistas de 32 y 64 bits.
pub const REGISTRY_KEYS: &[(&str, &[(&str, &str)])] = &[
    ("cod2", &[("Activision\\Call of Duty 2", "InstallPath")]),
    ("cod1", &[("Activision\\Call of Duty", "InstallPath")]),
    (
        "coduo",
        &[("Activision\\Call of Duty United Offensive", "InstallPath")],
    ),
    ("cod4", &[("Activision\\Call of Duty 4", "InstallPath")]),
    ("q3", &[("Id\\Quake III Arena", "INSTALLPATH")]),
    (
        "rtcw",
        &[("Activision\\Return to Castle Wolfenstein", "InstallPath")],
    ),
    // Primero el instalador de ET: Legacy (Program Files\ETLegacy).
    (
        "et",
        &[
            ("ETLegacy", "InstallPath"),
            ("Activision\\Wolfenstein - Enemy Territory", "InstallPath"),
        ],
    ),
    (
        "jka",
        &[(
            "LucasArts\\Star Wars Jedi Knight Jedi Academy\\1.0",
            "Install Path",
        )],
    ),
    (
        "sof2",
        &[(
            "Activision\\Soldier of Fortune II - Double Helix",
            "InstallPath",
        )],
    ),
    ("cs16", &[("Valve\\Half-Life", "InstallPath")]),
    (
        "ut99",
        &[(
            "Unreal Technology\\Installed Apps\\UnrealTournament",
            "Folder",
        )],
    ),
    (
        "ut2004",
        &[("Unreal Technology\\Installed Apps\\UT2004", "Folder")],
    ),
    ("mohaa", &[("EA GAMES\\MOHAA", "InstallPath")]),
    ("mohaas", &[("EA GAMES\\MOHAA", "InstallPath")]),
    ("mohaab", &[("EA GAMES\\MOHAA", "InstallPath")]),
    ("bf1942", &[("EA GAMES\\Battlefield 1942", "GAMEDIR")]),
];

pub const STEAM_DIRS: &[(&str, &[&str])] = &[
    ("cod1", &["Call of Duty"]),
    ("coduo", &["Call of Duty"]),
    ("cod2", &["Call of Duty 2"]),
    ("cod4", &["Call of Duty 4"]),
    ("q3", &["Quake 3 Arena"]),
    ("q2", &["Quake 2", "Quake II"]),
    ("dday", &["Quake 2", "Quake II"]),
    ("rtcw", &["Return to Castle Wolfenstein"]),
    (
        "jka",
        &["Jedi Academy", "Star Wars Jedi Knight Jedi Academy"],
    ),
    ("jk2", &["Jedi Outcast"]),
    ("cs16", &["Half-Life"]),
    ("l4d", &["Left 4 Dead"]),
    ("l4d2", &["Left 4 Dead 2"]),
    ("css", &["Counter-Strike Source"]),
    ("tf2", &["Team Fortress 2"]),
    ("gmod", &["GarrysMod"]),
    ("hldm", &["Half-Life"]),
    ("dod", &["Half-Life"]),
    ("tfc", &["Half-Life"]),
    ("ut99", &["Unreal Tournament"]),
    ("ut2004", &["Unreal Tournament 2004"]),
];

/// Valor de texto no vacío de `HKLM\SOFTWARE\<subkey>`, en la vista de 32 bits y luego en
/// la de 64. Fuera de Windows, `None`.
pub fn read_hklm(subkey: &str, value: &str) -> Option<String> {
    #[cfg(windows)]
    {
        use crate::winreg::{
            HKEY_LOCAL_MACHINE, RRF_SUBKEY_WOW6432KEY, RRF_SUBKEY_WOW6464KEY, read_string,
        };
        let subkey = format!("SOFTWARE\\{subkey}");
        [RRF_SUBKEY_WOW6432KEY, RRF_SUBKEY_WOW6464KEY]
            .into_iter()
            .find_map(|view| read_string(HKEY_LOCAL_MACHINE, &subkey, value, view))
    }
    #[cfg(not(windows))]
    {
        let _ = (subkey, value);
        None
    }
}

fn read_hkcu(subkey: &str, value: &str) -> Option<String> {
    #[cfg(windows)]
    {
        crate::winreg::read_string(crate::winreg::HKEY_CURRENT_USER, subkey, value, 0)
    }
    #[cfg(not(windows))]
    {
        let _ = (subkey, value);
        None
    }
}

pub fn registry_findings(read_value: &dyn Fn(&str, &str) -> Option<String>) -> Vec<Finding> {
    REGISTRY_KEYS
        .iter()
        .filter_map(|&(game_id, keys)| {
            let path = keys
                .iter()
                .find_map(|&(sk, v)| read_value(sk, v).filter(|p| !p.is_empty()))?;
            Some(Finding {
                game_id,
                path,
                source: "registro",
            })
        })
        .collect()
}

fn join(base: &str, name: &str) -> String {
    Path::new(base)
        .join(name)
        .to_str()
        .unwrap_or_default()
        .to_owned()
}

/// Las capturas de `"path"\s+"(.+?)"` (sin `\n` dentro), como `re.findall`.
fn vdf_paths(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("\"path\"") {
        let after = &rest[at + 6..];
        let value = after.trim_start_matches(is_py_space);
        let capture = (value.len() < after.len())
            .then(|| value.strip_prefix('"'))
            .flatten()
            .and_then(|v| {
                // `.+?`: al menos un carácter (puede ser `"`), luego hasta el próximo `"`.
                let first = v.chars().next().filter(|&c| c != '\n')?.len_utf8();
                let end = first + v[first..].find(['"', '\n'])?;
                (v[end..].starts_with('"')).then(|| &v[..end])
            });
        match capture {
            Some(c) => {
                found.push(c);
                // Sigue después del `"` que cierra la captura.
                let consumed = after.len() - value.len() + 1 + c.len() + 1;
                rest = &after[consumed..];
            }
            None => rest = &rest[at + 1..],
        }
    }
    found
}

/// La carpeta de Steam y las bibliotecas de su `libraryfolders.vdf`, sin repetir.
pub fn steam_libraries(steam_root: &str) -> Vec<String> {
    let vdf = Path::new(steam_root)
        .join("steamapps")
        .join("libraryfolders.vdf");
    let text = fs::read(vdf)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let libs = std::iter::once(steam_root.to_owned()).chain(
        vdf_paths(&text)
            .into_iter()
            .map(|p| p.replace("\\\\", "\\")),
    );
    let mut seen = HashSet::new();
    libs.filter(|lib| seen.insert(normcase(&normpath(lib))))
        .collect()
}

pub fn steam_findings(steam_roots: &[String]) -> Vec<Finding> {
    let mut found = Vec::new();
    for root in steam_roots {
        for lib in steam_libraries(root) {
            let common = join(&join(&lib, "steamapps"), "common");
            for &(game_id, names) in STEAM_DIRS {
                for name in names {
                    let path = join(&common, name);
                    if Path::new(&path).is_dir() {
                        found.push(Finding {
                            game_id,
                            path,
                            source: "steam",
                        });
                    }
                }
            }
        }
    }
    found
}

pub fn typical_roots(drives: &[String]) -> Vec<String> {
    let publishers = [
        "Activision",
        "EA Games",
        "id Software",
        "GT Interactive",
        "LucasArts",
        "Infogrames",
    ];
    let mut roots = Vec::new();
    for drive in drives {
        let pf = [
            join(drive, "Program Files (x86)"),
            join(drive, "Program Files"),
        ];
        roots.extend(pf.iter().cloned());
        roots.extend(["Juegos", "Games", "GOG Games", "TacticalOps"].map(|n| join(drive, n)));
        roots.extend(pf.iter().flat_map(|base| publishers.map(|p| join(base, p))));
    }
    roots
}

/// (nombre del exe en minúsculas, carpetas que lo preceden, id de juego).
fn exe_markers() -> Vec<(String, Vec<String>, &'static str)> {
    let mut markers = Vec::new();
    for g in GAMES.iter() {
        for relpath in std::iter::once(g.exe_relpath).chain(g.exe_candidates.iter().copied()) {
            if let [prefix @ .., exe] = win_parts(relpath).as_slice() {
                markers.push((
                    casefold(exe),
                    prefix.iter().map(|p| casefold(p)).collect(),
                    g.id,
                ));
            }
        }
    }
    markers
}

/// Sube por `prefix` (p. ej. `["system"]`) desde la carpeta del exe.
fn install_dir(directory: &str, prefix: &[String]) -> Option<String> {
    let mut current = directory.to_owned();
    for name in prefix.iter().rev() {
        if casefold(basename(&current)) != *name {
            return None;
        }
        current = dirname(&current);
    }
    Some(current)
}

/// La carpeta común si `install` es una de las copias por versión del juego.
fn above_version_copy(game_id: &str, install: String) -> String {
    let folders = game(game_id).map_or(&[][..], |g| g.version_dirs);
    let name = casefold(basename(&install));
    if folders.iter().any(|&(_, folder)| casefold(folder) == name) {
        dirname(&install)
    } else {
        install
    }
}

/// Recorre `root` hasta `max_depth` niveles sin seguir enlaces; las carpetas que no se
/// pueden leer se saltan.
pub fn scan_folder(
    root: &str,
    source: &'static str,
    max_depth: u32,
    cancelled: &dyn Fn() -> bool,
) -> Vec<Finding> {
    let markers = exe_markers();
    let mut found = IndexSet::new();
    let mut stack = vec![(root.to_owned(), 0)];
    while let Some((path, depth)) = stack.pop() {
        if cancelled() {
            break;
        }
        // Como el `scandir` de Python: un error a mitad de la lectura salta la carpeta entera.
        let Ok(entries) = fs::read_dir(&path).and_then(|it| it.collect::<Result<Vec<_>, _>>())
        else {
            continue;
        };
        let mut names = HashSet::new();
        let mut subdirs = Vec::new();
        for entry in entries {
            let (Ok(kind), Some(name)) = (
                entry.file_type(),
                entry.file_name().to_str().map(str::to_owned),
            ) else {
                continue;
            };
            if kind.is_dir() {
                subdirs.extend(entry.path().to_str().map(str::to_owned));
            } else {
                names.insert(casefold(&name));
            }
        }
        for (exe, prefix, game_id) in &markers {
            if names.contains(exe)
                && let Some(install) = install_dir(&path, prefix).filter(|i| !i.is_empty())
            {
                found.insert(Finding {
                    game_id,
                    path: above_version_copy(game_id, install),
                    source,
                });
            }
        }
        if depth < max_depth {
            subdirs.sort_unstable_by(|a, b| b.cmp(a));
            stack.extend(subdirs.into_iter().map(|d| (d, depth + 1)));
        }
    }
    drop_nested(found.into_iter().collect())
}

/// Quita el hallazgo de un juego que cuelga de otro hallazgo del mismo juego (jamp.exe en
/// GameData\ cuenta como el candidato suelto y como GameData\jamp.exe).
fn drop_nested(found: Vec<Finding>) -> Vec<Finding> {
    let norm = |p: &str| normcase(&normpath(p));
    let nested = |inner: &Finding| {
        let path = norm(&inner.path);
        found.iter().any(|o| {
            o.game_id == inner.game_id
                && path.starts_with(&format!("{}{MAIN_SEPARATOR}", norm(&o.path)))
        })
    };
    found.iter().filter(|f| !nested(f)).cloned().collect()
}

pub fn default_steam_roots() -> Vec<String> {
    let mut roots: Vec<String> = read_hkcu("Software\\Valve\\Steam", "SteamPath")
        .map(|v| normpath(&v))
        .into_iter()
        .collect();
    let default = "C:\\Program Files (x86)\\Steam";
    if Path::new(default).is_dir() && !roots.iter().any(|r| normcase(r) == normcase(default)) {
        roots.push(default.to_owned());
    }
    roots
}

pub fn default_drives() -> Vec<String> {
    if cfg!(windows) {
        ('A'..='Z')
            .map(|l| format!("{l}:\\"))
            .filter(|d| Path::new(d).is_dir())
            .collect()
    } else {
        Vec::new()
    }
}

fn rank(source: &str) -> usize {
    SOURCES
        .iter()
        .position(|&s| s == source)
        .unwrap_or(SOURCES.len())
}

/// Hallazgos por juego, de la mejor fuente a la peor; cada carpeta una sola vez por juego.
pub fn find_games(
    read_value: &dyn Fn(&str, &str) -> Option<String>,
    steam_roots: &[String],
    drives: &[String],
    extra_root: Option<&str>,
    cancelled: &dyn Fn() -> bool,
) -> IndexMap<String, Vec<Finding>> {
    let mut raw = registry_findings(read_value);
    raw.extend(steam_findings(steam_roots));
    for root in typical_roots(drives) {
        if cancelled() {
            break;
        }
        raw.extend(scan_folder(&root, "carpeta típica", 2, cancelled));
    }
    if let Some(extra) = extra_root.filter(|r| !r.is_empty())
        && !cancelled()
    {
        raw.extend(scan_folder(extra, "carpeta elegida", 6, cancelled));
    }

    let mut best: IndexMap<(&str, String), Finding> = IndexMap::new();
    for f in raw {
        let Some(g) = game(f.game_id) else { continue };
        if resolve_game_exe(g, Some(&f.path)).is_none() {
            continue;
        }
        let key = (f.game_id, normcase(&normpath(&f.path)));
        if best
            .get(&key)
            .is_none_or(|old| rank(f.source) < rank(old.source))
        {
            best.insert(key, f);
        }
    }
    let mut result: IndexMap<String, Vec<Finding>> = IndexMap::new();
    for f in best.into_values() {
        result.entry(f.game_id.to_owned()).or_default().push(f);
    }
    for items in result.values_mut() {
        items.sort_by_key(|f| rank(f.source));
    }
    result
}
