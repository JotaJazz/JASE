//! Pantallas de carga de mapas de CoD1/CoD2 desde los .iwd/.pk3 del juego.
//! Traducción de `src/jase/maps/cod.py`. Las entradas se leen a memoria, nunca al disco.
use crate::paths::{child, resolve_game_dir};
use crate::pystr::casefold;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

pub const ARCHIVE_EXTS: [&str; 2] = [".iwd", ".pk3"];
pub const MAX_ENTRY_SIZE: u64 = 64 * 1024 * 1024;

/// Archivos .iwd/.pk3 de `folder` en orden de carga (alfabético). Sin seguir enlaces.
pub fn archives_in(folder: &str) -> Vec<String> {
    let Ok(it) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<(String, String)> = it
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let key = casefold(e.file_name().to_str()?);
            let path = e.path().to_str()?.to_owned();
            ARCHIVE_EXTS
                .iter()
                .any(|x| key.ends_with(x))
                .then_some((key, path))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}

/// main/ primero y luego cada mods/<mod>/: los posteriores pisan a los anteriores.
fn search_order(game_dir: &str) -> Vec<String> {
    let mut order = Vec::new();
    if let Some(main) = child(game_dir, "main").filter(|m| Path::new(m).is_dir()) {
        order.extend(archives_in(&main));
    }
    let Some(mods) = child(game_dir, "mods").filter(|m| Path::new(m).is_dir()) else {
        return order;
    };
    let mut mod_dirs: Vec<(String, String)> = fs::read_dir(&mods)
        .map(|it| {
            it.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter_map(|e| {
                    Some((
                        casefold(e.file_name().to_str()?),
                        e.path().to_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    mod_dirs.sort();
    for (_, dir) in mod_dirs {
        order.extend(archives_in(&dir));
    }
    order
}

/// Primera entrada (no carpeta) cuyo nombre en `casefold` está en `wanted` (ya en
/// `casefold`), en el orden de `wanted`. Las que declaran más de `max_entry` se saltan.
/// Cualquier error del zip (dañado, cifrado, tamaño falso) da `None`.
fn read_matching(archive: &str, wanted: &[String], max_entry: u64) -> Option<Vec<u8>> {
    let mut z = ZipArchive::new(BufReader::new(File::open(archive).ok()?)).ok()?;
    // nombre en casefold -> (índice, tamaño declarado); con duplicados gana el primero.
    let mut found: Vec<(String, usize, u64)> = Vec::new();
    for i in 0..z.len() {
        let Ok(entry) = z.by_index_raw(i) else {
            return None;
        };
        let Ok(name) = entry.name() else { continue };
        if name.ends_with('/') {
            continue;
        }
        let key = casefold(&name);
        if !found.iter().any(|(k, ..)| *k == key) {
            found.push((key, i, entry.size()));
        }
    }
    let (index, size) = wanted.iter().find_map(|w| {
        found
            .iter()
            .find(|(k, ..)| k == w)
            .filter(|(_, _, size)| *size <= max_entry)
            .map(|&(_, i, size)| (i, size))
    })?;
    let entry = z.by_index(index).ok()?;
    let mut data = Vec::new();
    // El tamaño declarado puede mentir: nunca más de `size + 1` bytes.
    entry
        .take(size.saturating_add(1))
        .read_to_end(&mut data)
        .ok()?;
    (data.len() as u64 == size).then_some(data)
}

/// La entrada `wanted` (ya en `casefold`), o `None` si falta o supera `MAX_ENTRY_SIZE`.
pub fn read_entry(archive: &str, wanted: &str) -> Option<Vec<u8>> {
    read_matching(archive, &[wanted.to_owned()], MAX_ENTRY_SIZE)
}

/// Abre el zip una vez y devuelve la primera entrada de `wanted` (en ese orden, sin
/// distinguir mayúsculas) que exista y no supere `MAX_ENTRY_SIZE`.
pub fn read_first(archive: &str, wanted: &[String]) -> Option<Vec<u8>> {
    read_first_limited(archive, wanted, MAX_ENTRY_SIZE)
}

#[doc(hidden)]
pub fn read_first_limited(archive: &str, wanted: &[String], max_entry: u64) -> Option<Vec<u8>> {
    let wanted: Vec<String> = wanted.iter().map(|w| casefold(w)).collect();
    read_matching(archive, &wanted, max_entry)
}

/// Bytes de images/loadscreen_<map>.iwi, o `None`. Si varios archivos lo contienen gana el
/// último en orden de carga (semántica de Quake 3); los que no son zip válidos se ignoran.
pub fn find_loadscreen(install_dir: Option<&str>, map_name: &str) -> Option<Vec<u8>> {
    if map_name.is_empty() || map_name.contains(['/', '\\']) {
        return None;
    }
    let game_dir = resolve_game_dir(install_dir, "main")?;
    let wanted = casefold(&format!("images/loadscreen_{map_name}.iwi"));
    search_order(&game_dir)
        .iter()
        .rev()
        .find_map(|archive| read_entry(archive, &wanted))
}
