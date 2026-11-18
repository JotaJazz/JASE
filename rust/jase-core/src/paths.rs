//! Rutas de los juegos sin distinguir mayúsculas. Traducción de `src/jase/paths.py`.
//! Las rutas son texto, como en Python; las entradas con nombre no UTF-8 se saltan.
use crate::pystr::casefold;
use std::fs;
use std::path::{Component, MAIN_SEPARATOR_STR, Path};

/// `os.path.normpath`: léxico, sin tocar el disco.
pub fn normpath(path: &str) -> String {
    let mut head = String::new();
    let mut parts: Vec<&str> = Vec::new();
    let mut rooted = false;
    for comp in Path::new(path).components() {
        match comp {
            Component::Prefix(p) => head.push_str(p.as_os_str().to_str().unwrap_or_default()),
            Component::RootDir => rooted = true,
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(&last) if last != ".." => {
                    parts.pop();
                }
                _ if rooted => {}
                _ => parts.push(".."),
            },
            Component::Normal(n) => parts.push(n.to_str().unwrap_or_default()),
        }
    }
    if rooted {
        // POSIX conserva exactamente dos barras iniciales (`//a`), no tres.
        let two = cfg!(not(windows)) && path.starts_with("//") && !path.starts_with("///");
        head.push_str(if two { "//" } else { MAIN_SEPARATOR_STR });
    }
    let out = head + &parts.join(MAIN_SEPARATOR_STR);
    if out.is_empty() {
        return ".".to_owned();
    }
    if cfg!(windows) {
        out.replace('/', "\\")
    } else {
        out
    }
}

/// `os.path.normcase`: en Windows minúsculas y `\`; en el resto, igual.
pub fn normcase(path: &str) -> String {
    if cfg!(windows) {
        path.to_lowercase().replace('/', "\\")
    } else {
        path.to_owned()
    }
}

fn to_string(p: &Path) -> Option<String> {
    p.to_str().map(str::to_owned)
}

/// `os.path.dirname` de una ruta sin barra final (la raíz es su propio padre).
pub(crate) fn dirname(path: &str) -> String {
    Path::new(path).parent().map_or_else(
        || path.to_owned(),
        |p| p.to_str().unwrap_or_default().to_owned(),
    )
}

/// `os.path.basename` de una ruta sin barra final.
pub(crate) fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
}

/// Partes de una ruta relativa de la tabla, como `PureWindowsPath(...).parts`.
pub(crate) fn win_parts(relpath: &str) -> Vec<&str> {
    relpath
        .split(['\\', '/'])
        .filter(|p| !p.is_empty() && *p != ".")
        .collect()
}

/// Entrada de `base` cuyo nombre coincide con `name` ignorando mayúsculas.
pub(crate) fn child(base: &str, name: &str) -> Option<String> {
    let wanted = casefold(name);
    fs::read_dir(base).ok()?.flatten().find_map(|e| {
        let n = e.file_name();
        (casefold(n.to_str()?) == wanted)
            .then(|| to_string(&e.path()))
            .flatten()
    })
}

fn find_file(base: &str, parts: &[&str]) -> Option<String> {
    let mut current = base.to_owned();
    for (i, part) in parts.iter().enumerate() {
        let found = child(&current, part)?;
        if i + 1 < parts.len() && !Path::new(&found).is_dir() {
            return None;
        }
        current = found;
    }
    Path::new(&current).is_file().then_some(current)
}

/// Subcarpetas de `dir` (siguiendo enlaces) en orden de texto.
fn subdirs(dir: &str) -> Vec<String> {
    let mut subs: Vec<String> = fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| to_string(&e.path()))
                .collect()
        })
        .unwrap_or_default();
    subs.sort();
    subs
}

/// Padre de `dir` si es distinto de `dir` (como el `parent != normpath` de Python).
fn parent_of(dir: &str) -> Option<String> {
    let base = normpath(dir);
    let parent = to_string(Path::new(&base).parent()?)?;
    (!parent.is_empty() && parent != base).then_some(parent)
}

/// Ruta del ejecutable del juego. Orden: `install_dir/exe`, `install_dir/<subcarpeta>/exe`,
/// `padre/exe` y, si `install_dir` ya es la carpeta del exe (p. ej. «System»), el nombre suelto.
/// Si `exe_relpath` no aparece se prueba cada uno de `candidates`; resuelto, gana el primero
/// de `preferred` que esté en esa MISMA carpeta.
pub fn resolve_exe(
    install_dir: Option<&str>,
    exe_relpath: &str,
    candidates: &[&str],
    preferred: &[&str],
) -> Option<String> {
    let found = std::iter::once(exe_relpath)
        .chain(candidates.iter().copied())
        .find_map(|relpath| resolve_one(install_dir, relpath))?;
    let folder = dirname(&found);
    preferred
        .iter()
        .find_map(|name| find_file(&folder, &[name]))
        .or(Some(found))
}

fn resolve_one(install_dir: Option<&str>, exe_relpath: &str) -> Option<String> {
    let install_dir = install_dir.filter(|d| !d.is_empty())?;
    let parts = win_parts(exe_relpath);
    if parts.is_empty() || !Path::new(install_dir).is_dir() {
        return None;
    }
    if let Some(found) = find_file(install_dir, &parts) {
        return Some(found);
    }
    if let Some(found) = subdirs(install_dir)
        .iter()
        .find_map(|sub| find_file(sub, &parts))
    {
        return Some(found);
    }
    if let Some(found) = parent_of(install_dir).and_then(|p| find_file(&p, &parts)) {
        return Some(found);
    }
    let last_dir = casefold(basename(&normpath(install_dir)));
    match parts.as_slice() {
        [.., folder, name] if last_dir == casefold(folder) => find_file(install_dir, &[name]),
        _ => None,
    }
}

/// Instalación `folder` de un juego con una copia por versión: subcarpeta de `install_dir`
/// o carpeta hermana (la ruta configurada puede ser ya una de las copias).
pub fn version_dir(install_dir: Option<&str>, folder: &str) -> Option<String> {
    let base = normpath(install_dir.filter(|d| !d.is_empty())?);
    [base.clone(), dirname(&base)]
        .iter()
        .find_map(|parent| child(parent, folder).filter(|f| Path::new(f).is_dir()))
}

/// Carpeta del juego que contiene la subcarpeta `marker` (p. ej. «main»): `install_dir`,
/// sus subcarpetas directas (en orden) o su padre, sin distinguir mayúsculas.
pub fn resolve_game_dir(install_dir: Option<&str>, marker: &str) -> Option<String> {
    let install_dir = install_dir.filter(|d| !d.is_empty())?;
    if marker.is_empty() || !Path::new(install_dir).is_dir() {
        return None;
    }
    let has_marker = |base: &str| child(base, marker).is_some_and(|f| Path::new(&f).is_dir());
    if has_marker(install_dir) {
        return Some(install_dir.to_owned());
    }
    if let Some(sub) = subdirs(install_dir).into_iter().find(|s| has_marker(s)) {
        return Some(sub);
    }
    parent_of(install_dir).filter(|p| has_marker(p))
}
