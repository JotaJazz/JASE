//! Lanzar el juego hacia un servidor (`connect_to` de `window_connect.py`): lo que se le pasa
//! a `jase_core::launcher::launch` (que valida mod e IWAD; la app nunca arma argv) y abrir
//! `steam://`. Lee carpetas y crea procesos: solo dentro de `app::blocking`.

use std::io;
use std::path::Path;
use std::sync::Arc;

use jase_core::browse::{BASE_MOD, mod_of};
use jase_core::config::Config;
use jase_core::games::{GameDef, LaunchStyle};
use jase_core::launcher::{LaunchCommand, launch, spawn_command, wine_wrapper};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::pystr::py_strip;

/// Lo que recibe el lanzador; sin `Debug`: lleva la contraseña.
#[derive(Clone)]
pub struct LaunchArgs {
    pub game: &'static GameDef,
    pub install_dir: Option<String>,
    /// La dirección para conectar (`connect_address`).
    pub address: ServerAddress,
    pub password: Option<String>,
    /// El mod (BF1942) o IWAD (Zandronum) tal como lo manda el servidor; "" = el juego base.
    pub mod_name: String,
    /// La versión del servidor, solo para los juegos con una copia por versión.
    pub version: String,
    pub wrapper: Option<Vec<String>>,
}

/// Lanza el juego; el error es el texto para el usuario.
pub type LaunchFn = Arc<dyn Fn(&LaunchArgs) -> Result<(), String> + Send + Sync>;

/// ¿Se puede lanzar? Con ruta, o por `steam://` (que no la usa).
pub fn has_path(cfg: &Config, game: &GameDef) -> bool {
    game.launch_style == LaunchStyle::Steam
        || cfg.game_paths.get(game.id).is_some_and(|p| !p.is_empty())
}

/// Los argumentos de `connect_to` para `info` con la config actual.
pub fn launch_args(
    game: &'static GameDef,
    info: &ServerInfo,
    cfg: &Config,
    password: Option<String>,
) -> LaunchArgs {
    let steam = game.launch_style == LaunchStyle::Steam;
    let mod_name = if matches!(
        game.launch_style,
        LaunchStyle::Bf1942 | LaunchStyle::Zandronum
    ) && !game.mod_key.is_empty()
        && mod_of(game, info) != BASE_MOD
    {
        // El valor crudo, no la clave normalizada de `mod_of`.
        info.rules
            .get(game.mod_key)
            .map_or("", |m| py_strip(m))
            .to_owned()
    } else {
        String::new()
    };
    let version = if game.version_dirs.is_empty() {
        String::new()
    } else {
        info.version.clone()
    };
    let platform = if cfg!(windows) { "win32" } else { "linux" };
    LaunchArgs {
        game,
        install_dir: cfg
            .game_paths
            .get(game.id)
            .filter(|d| !d.is_empty())
            .cloned(),
        address: info.connect_address.clone(),
        password,
        mod_name,
        version,
        wrapper: if steam {
            None
        } else {
            wine_wrapper(&cfg.wine_command, platform)
        },
    }
}

/// `launcher::launch` con el lanzador y el abridor de URL dados (los tests ponen falsos).
pub fn launch_with(
    args: &LaunchArgs,
    spawn: &mut dyn FnMut(&LaunchCommand) -> io::Result<()>,
    open_url: &mut dyn FnMut(&str) -> io::Result<()>,
) -> Result<(), String> {
    launch(
        args.game,
        args.install_dir.as_deref(),
        &args.address,
        args.password.as_deref(),
        &args.mod_name,
        &args.version,
        args.wrapper.as_deref(),
        spawn,
        open_url,
    )
    .map_err(|e| e.to_string())
}

/// El lanzador real.
pub fn launch_game(args: &LaunchArgs) -> Result<(), String> {
    launch_with(args, &mut spawn_command, &mut open_url)
}

/// Abre la URL con el programa del sistema (`xdg-open`, por argv); un hilo espera al hijo.
#[cfg(not(windows))]
pub fn open_url(url: &str) -> io::Result<()> {
    let mut child = std::process::Command::new("xdg-open").arg(url).spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Lanza un proceso suelto (`QProcess::startDetached`); un hilo espera al hijo.
#[cfg(not(windows))]
pub fn spawn_detached(program: &Path, args: &[String]) -> io::Result<()> {
    let mut child = std::process::Command::new(program).args(args).spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Lanza un proceso suelto sin abrir una consola (`QProcess::startDetached` desde una app sin
/// consola): el instalador o el `cmd.exe` del script del portátil.
#[cfg(windows)]
pub fn spawn_detached(program: &Path, args: &[String]) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000; // `Win32_System_Threading`, sin sumar la feature
    std::process::Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

/// Abre la URL con su programa registrado (`os.startfile`).
#[cfg(windows)]
pub fn open_url(url: &str) -> io::Result<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    // El manejador del protocolo lo elige el registro y puede ser una extensión COM.
    let _com = com_guard();
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (verb, file) = (wide("open"), wide(url));
    // SAFETY: las dos cadenas UTF-16 terminan en 0 y viven hasta después de la llamada; los
    // punteros nulos (ventana, parámetros, carpeta) son válidos para `ShellExecuteW`.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // Más de 32 = bien (`HINSTANCE` de compatibilidad, no un puntero); si no, es el código
    // `SE_ERR_*` (`GetLastError` no es fiable aquí).
    let code = result as usize;
    if code > 32 {
        Ok(())
    } else {
        Err(io::Error::other(format!("ShellExecuteW: {code}")))
    }
}

/// COM iniciado en este hilo (STA, como pide el shell) mientras vive; los hilos del pool
/// bloqueante no lo traen. Un `CoInitializeEx` correcto (S_OK o S_FALSE) se empareja con
/// `CoUninitialize`; si el hilo ya era MTA (RPC_E_CHANGED_MODE) u otro fallo, no se toca.
#[cfg(windows)]
pub(crate) fn com_guard() -> impl Drop {
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };

    struct Com(bool);
    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: empareja el `CoInitializeEx` correcto de este mismo hilo.
                unsafe { CoUninitialize() };
            }
        }
    }
    // SAFETY: el reservado va nulo; las banderas son valores válidos de `COINIT`.
    let hr = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    Com(hr >= 0)
}
