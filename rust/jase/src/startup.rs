//! Arranque: idioma antes de la ventana, último juego y X11 en WSL (como `__main__.py`).

use jase_core::config::Config;
use jase_core::games::game;
use jase_core::i18n::{resolve_language, set_language};
use jase_core::jase_url;

use crate::single_instance::SHOW_COMMAND;

pub const APP_NAME: &str = "Jimmy's All Seeing Eye";
pub const DEFAULT_GAME: &str = "cod2";

/// En WSL el Wayland de WSLg falla con winit; X11 funciona.
pub fn force_x11(osrelease: &str, wsl_distro: Option<&str>) -> bool {
    osrelease.to_lowercase().contains("microsoft") || wsl_distro.is_some()
}

/// Idioma de visualización según el entorno: `LANGUAGE` (el primero) gana al formato regional.
pub fn locale_name_from_env(env: &impl Fn(&str) -> Option<String>) -> String {
    let language = env("LANGUAGE").and_then(|v| v.split(':').next().map(str::to_owned));
    let rest = ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter().map(env);
    std::iter::once(language)
        .chain(rest)
        .flatten()
        .find(|v| !v.is_empty() && !["C", "POSIX"].contains(&v.split('.').next().unwrap_or("")))
        .unwrap_or_default()
}

/// Como `QLocale.uiLanguages()[0]`: el idioma de la interfaz, no el formato regional.
#[cfg(windows)]
pub fn system_locale_name() -> String {
    use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    let (mut count, mut len) = (0u32, 0u32);
    let mut buf = Vec::new();
    // SAFETY: la primera llamada, con búfer nulo, da el largo; la segunda llena uno de ese largo.
    let ok = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            std::ptr::null_mut(),
            &mut len,
        ) != 0
            && {
                buf = vec![0u16; len as usize];
                GetUserPreferredUILanguages(
                    MUI_LANGUAGE_NAME,
                    &mut count,
                    buf.as_mut_ptr(),
                    &mut len,
                ) != 0
            }
    };
    // Lista de nombres separados por NUL: el primero es el preferido.
    let first = buf.split(|c| *c == 0).next().unwrap_or(&[]);
    if ok && !first.is_empty() {
        return String::from_utf16_lossy(first);
    }
    locale_name_from_env(&|k| std::env::var(k).ok())
}

#[cfg(not(windows))]
pub fn system_locale_name() -> String {
    locale_name_from_env(&|k| std::env::var(k).ok())
}

/// La versión que se compara con las releases de GitHub. *Acerca de* sigue mostrando la del
/// crate.
pub fn update_version() -> &'static str {
    let version = env!("CARGO_PKG_VERSION");
    // shortcut: mientras el crate sea 0.x cada consulta ofrecería instalar la 1.3.0 de Python
    // encima; «dev» (como `app_version()` sin metadatos) nunca avisa. Quitar al publicar 2.0.0.
    if version.starts_with("0.") {
        "dev"
    } else {
        version
    }
}

/// Último juego abierto si sigue existiendo; si no, el predeterminado.
pub fn initial_game(cfg: &Config) -> &'static str {
    cfg.ui_state
        .get("last_game")
        .and_then(|id| game(id))
        .map_or(DEFAULT_GAME, |g| g.id)
}

/// Fija el idioma antes de construir la ventana.
pub fn prepare(cfg: &Config, locale: &str) {
    // resolve_language siempre devuelve un idioma soportado.
    let _ = set_language(resolve_language(&cfg.language, locale));
}

/// Los argumentos que son UTF-8 (`env::args()` entra en pánico con uno que no lo es; un enlace
/// válido siempre es ASCII).
pub fn utf8_args(args: impl Iterator<Item = std::ffi::OsString>) -> Vec<String> {
    args.filter_map(|a| a.into_string().ok()).collect()
}

/// El primer argumento con forma de enlace `jase://` (sin validar: lo hace `jase_url::parse`).
pub fn link_from_argv(args: &[String]) -> Option<String> {
    args.iter().find(|a| jase_url::is_link(a)).cloned()
}

/// Lo que se manda a la JASE abierta: el enlace si es válido; si no, que se muestre.
pub fn running_message(link: Option<&str>) -> &str {
    link.filter(|l| jase_url::parse(l).is_some())
        .unwrap_or(SHOW_COMMAND)
}
