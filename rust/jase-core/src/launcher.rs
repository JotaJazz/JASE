//! Línea de comandos del juego y su lanzamiento sin shell. Traducción de
//! `launcher.py`. `launch` lee carpetas: la fase 5 lo llama con `spawn_blocking`.
use crate::games::{GameDef, LaunchStyle};
use crate::i18n::{tr, tr_fmt};
use crate::models::ServerAddress;
use crate::paths::{resolve_exe, version_dir};
use crate::pystr::{casefold, py_strip};
use std::io;
use std::process::Command;

/// Lo que se lanza: `env` es lo que se agrega al entorno heredado.
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub argv: Vec<String>,
    pub cwd: String,
    pub env: Vec<(&'static str, String)>,
}

/// Error al lanzar el juego; el mensaje está pensado para mostrarse al usuario y nunca lleva
/// la contraseña ni la URL.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Spawn(io::Error),
}

pub fn build_launch_command(
    game: &GameDef,
    exe_path: &str,
    address: &ServerAddress,
    password: Option<&str>,
    mod_name: &str,
) -> (Vec<String>, String) {
    let password = password.filter(|p| !p.is_empty());
    let addr = address.to_string();
    let mut argv = vec![exe_path.to_owned()];
    let mut push = |items: &[&str]| argv.extend(items.iter().map(|s| (*s).to_owned()));
    match game.launch_style {
        LaunchStyle::Unreal => match password {
            Some(pw) => push(&[&format!("{addr}?password={pw}")]),
            None => push(&[&addr]),
        },
        LaunchStyle::Valve => {
            push(&["-game", game.valve_gamedir]);
            if let Some(pw) = password {
                push(&["+password", pw]);
            }
            push(&["+connect", &addr]);
        }
        LaunchStyle::Zandronum => {
            // El IWAD (DOOM2.WAD...) lo informa el servidor; los PWAD deben estar ya en la
            // carpeta de Zandronum (no se descargan).
            if is_wad_name(mod_name) {
                push(&["-iwad", mod_name]);
            }
            if let Some(pw) = password {
                push(&["+cl_password", pw, "+cl_joinpassword", pw]);
            }
            push(&["-connect", &addr]);
        }
        LaunchStyle::Bf1942 => {
            push(&["+restart", "1"]);
            if is_mod_name(mod_name) && !game.is_base_mod(mod_name) {
                push(&["+game", mod_name]);
            }
            if let Some(pw) = password {
                push(&["+password", pw]);
            }
            push(&["+joinServer", &addr]);
        }
        // Steam no llega aquí (`launch` abre la URL); como en Python, cae en la rama q3.
        LaunchStyle::Q3 | LaunchStyle::Steam => {
            // Un mod de Quake II listado como juego aparte arranca ya en su carpeta.
            if !game.status_gamedir.is_empty() {
                push(&["+set", "game", game.status_gamedir]);
            }
            if let Some(pw) = password {
                push(&["+password", pw]);
            }
            push(&["+connect", &addr]);
        }
    }
    (argv, nt_dirname(exe_path))
}

/// `ntpath.dirname` (Python 3.13): entiende `\` y `/`, unidades y rutas UNC.
fn nt_dirname(p: &str) -> String {
    let b = p.as_bytes();
    let sep = |i: usize| matches!(b.get(i), Some(b'\\' | b'/'));
    let find_sep = |from: usize| (from..b.len()).find(|&i| sep(i));
    // Fin de la unidad más la raíz (`splitroot`); los separadores son ASCII: cortes válidos.
    let root_end = if sep(0) && sep(1) {
        let unc = b.len() >= 8
            && b[2] == b'?'
            && sep(3)
            && b[4..7].eq_ignore_ascii_case(b"UNC")
            && sep(7);
        match find_sep(if unc { 8 } else { 2 }).and_then(|i| find_sep(i + 1)) {
            Some(i2) => i2 + 1,
            None => return p.to_owned(),
        }
    } else if sep(0) {
        1
    } else if let Some(c) = p.chars().next()
        && p[c.len_utf8()..].starts_with(':')
    {
        // Como Python, cualquier carácter vale de letra de unidad.
        let end = c.len_utf8() + 1;
        if sep(end) { end + 1 } else { end }
    } else {
        0
    };
    let rest = &p[root_end..];
    let cut = rest.rfind(['\\', '/']).map_or(0, |i| i + 1);
    format!(
        "{}{}",
        &p[..root_end],
        rest[..cut].trim_end_matches(['\\', '/'])
    )
}

/// Un nombre de mod y nada más: lo manda el servidor y llega a «+game».
pub fn is_mod_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.chars().count() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ' ' | '.' | '-'))
}

/// Un nombre de archivo de IWAD y nada más: lo manda el servidor, y un «+comando» o una ruta
/// llegarían tal cual a la línea de comandos del juego.
pub fn is_wad_name(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    let mut chars = stem.chars();
    ["wad", "pk3", "ipk3", "pk7", "ipk7"]
        .iter()
        .any(|e| ext.eq_ignore_ascii_case(e))
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && chars
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ' ' | '.' | '(' | ')' | '-'))
}

/// `shlex.split` (POSIX, sin comentarios): comillas sin cerrar o `\` final → `None`.
pub fn shlex_split(s: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut token = String::new();
    let mut in_token = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\r' | '\n' => {
                if in_token {
                    out.push(std::mem::take(&mut token));
                    in_token = false;
                }
            }
            '\\' => {
                token.push(chars.next()?);
                in_token = true;
            }
            '\'' => {
                in_token = true;
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => token.push(c),
                    }
                }
            }
            '"' => {
                in_token = true;
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => {
                            // Entre comillas dobles `\` solo escapa `"` y `\`.
                            let next = chars.next()?;
                            if next != '"' && next != '\\' {
                                token.push('\\');
                            }
                            token.push(next);
                        }
                        c => token.push(c),
                    }
                }
            }
            c => {
                token.push(c);
                in_token = true;
            }
        }
    }
    if in_token {
        out.push(token);
    }
    Some(out)
}

/// Fuera de Windows, el comando (partido como en la shell) que abre los .exe. `platform` como
/// `sys.platform`: `"win32"` o vacío → `None`.
pub fn wine_wrapper(command: &str, platform: &str) -> Option<Vec<String>> {
    if platform.is_empty() || platform == "win32" || py_strip(command).is_empty() {
        return None;
    }
    shlex_split(command)
}

pub fn steam_url(address: &ServerAddress, password: Option<&str>) -> String {
    match password.filter(|p| !p.is_empty()) {
        Some(pw) => format!("steam://connect/{address}/{pw}"),
        None => format!("steam://connect/{address}"),
    }
}

/// Appid para un exe de una biblioteca de Steam (como lo abre Steam): sin él, el DRM de Steam
/// de JKA/JK2 corta con «Application load error 5:0000065434».
fn steam_env(game: &GameDef, exe_path: &str) -> Vec<(&'static str, String)> {
    if game.steam_appid == 0
        || !exe_path
            .split(['\\', '/'])
            .any(|p| casefold(p) == "steamapps")
    {
        return Vec::new();
    }
    let appid = game.steam_appid.to_string();
    vec![("SteamAppId", appid.clone()), ("SteamGameId", appid)]
}

/// Lanza de verdad, sin shell; un hilo espera al hijo para que no quede zombi.
pub fn spawn_command(cmd: &LaunchCommand) -> io::Result<()> {
    let Some((exe, args)) = cmd.argv.split_first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "argv vacío"));
    };
    let mut child = Command::new(exe)
        .args(args)
        .current_dir(&cmd.cwd)
        .envs(cmd.env.iter().map(|(k, v)| (k, v)))
        .spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// `wrapper`: fuera de Windows, el programa que abre el .exe (p. ej. `["wine"]`).
/// `version`: la del servidor, para elegir la copia del juego (`GameDef::version_dirs`).
#[allow(clippy::too_many_arguments)]
pub fn launch(
    game: &GameDef,
    install_dir: Option<&str>,
    address: &ServerAddress,
    password: Option<&str>,
    mod_name: &str,
    version: &str,
    wrapper: Option<&[String]>,
    spawn: &mut dyn FnMut(&LaunchCommand) -> io::Result<()>,
    open_url: &mut dyn FnMut(&str) -> io::Result<()>,
) -> Result<(), LaunchError> {
    if game.launch_style == LaunchStyle::Steam {
        // Steam resuelve el juego instalado; no hace falta ruta ni exe. El error va sin la URL:
        // puede llevar la contraseña.
        return open_url(&steam_url(address, password))
            .map_err(|_| LaunchError::Message(tr("No se pudo abrir Steam. ¿Está instalado?")));
    }
    let Some(install_dir) = install_dir.filter(|d| !d.is_empty()) else {
        return Err(LaunchError::Message(tr_fmt(
            "No hay ruta configurada para {name}. Configúrala en Ajustes.",
            &[("name", &game.name)],
        )));
    };
    let folder = game
        .version_dirs
        .iter()
        .find(|(v, _)| *v == version)
        .map_or("", |(_, f)| f);
    let base = version_dir(Some(install_dir), folder).unwrap_or_else(|| install_dir.to_owned());
    let Some(exe_path) = resolve_exe(
        Some(&base),
        game.exe_relpath,
        game.exe_candidates,
        game.preferred_exes,
    ) else {
        return Err(LaunchError::Message(tr_fmt(
            "No se encontró {exe} en {folder}. Revisa la ruta en Ajustes.",
            &[("exe", &game.exe_relpath), ("folder", &install_dir)],
        )));
    };
    let (mut argv, cwd) = build_launch_command(game, &exe_path, address, password, mod_name);
    let wrapper = wrapper.filter(|w| !w.is_empty());
    if let Some(w) = wrapper
        && exe_path.to_lowercase().ends_with(".exe")
    {
        argv.splice(0..0, w.iter().cloned());
    }
    let cmd = LaunchCommand {
        argv,
        cwd,
        env: steam_env(game, &exe_path),
    };
    spawn(&cmd).map_err(|e| match wrapper.and_then(|w| w.first()) {
        Some(program) if e.kind() == io::ErrorKind::NotFound => LaunchError::Message(tr_fmt(
            "No se encontró «{command}» para abrir el juego. Instala Wine o cambia el comando en \
             Ajustes → General.",
            &[("command", program)],
        )),
        _ => LaunchError::Spawn(e),
    })
}
