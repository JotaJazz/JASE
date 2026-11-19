//! Actualizar JASE desde la app (`update_dialog.py`): textos de los errores de la descarga,
//! la descarga verificada (y la extracción del zip portátil) bloqueante, y cómo se
//! reemplaza este ejecutable. Sin `App`: la ventana lo corre en un hilo.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use jase_core::app_update::{Asset, DownloadCode, download_installer};
use jase_core::i18n::tr;
use jase_core::portable_update::{EXE_NAME, extract_release, is_portable};

use crate::scan::StreamFn;

/// El zip portátil no se pudo extraer o no trae `JASE/JASE.exe`.
pub const BAD_ZIP: &str = "bad_zip";

/// Texto traducido para un código de `DownloadCode::as_str` o [`BAD_ZIP`].
pub fn download_error_text(code: &str) -> String {
    // Cada texto con su literal: el test de los catálogos los busca así.
    match code {
        c if c == DownloadCode::Size.as_str() => tr("el archivo descargado está incompleto"),
        c if c == DownloadCode::TooMuch.as_str() => tr("el archivo es más grande de lo anunciado"),
        c if c == DownloadCode::Hash.as_str() => tr("la suma SHA-256 no coincide"),
        c if c == DownloadCode::Insecure.as_str() => {
            tr("la descarga no usa una conexión segura (https)")
        }
        c if c == DownloadCode::Cancelled.as_str() => tr("descarga cancelada"),
        BAD_ZIP => tr("el zip descargado no es una versión portátil válida de JASE"),
        _ => tr("error de red o de disco"),
    }
}

/// Borra las descargas anteriores (pueden ser cientos de MB: por eso aquí, en el hilo), baja
/// y verifica `asset` en `dest` y, con `extract_to`, extrae el zip y da la carpeta nueva. El
/// error es un código para [`download_error_text`]; cancelada en cualquier punto, es un error.
pub fn download_update(
    asset: &Asset,
    dest: &Path,
    extract_to: Option<&Path>,
    stream: &StreamFn,
    progress: &mut dyn FnMut(u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<PathBuf, String> {
    // Cada descarga baja en su propia carpeta: un hilo cancelado que aún limpia su `.part` no
    // toca el de la descarga que lo reemplazó (mismo nombre).
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let _ = fs::remove_dir_all(dest);
    let work = dest.join(format!(
        ".download-{}",
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = download_installer(asset, &work, |url| stream(url), progress, cancelled)
        .map_err(|e| e.code.as_str().to_owned())
        .and_then(|part| {
            if cancelled() {
                return Err(DownloadCode::Cancelled.as_str().to_owned());
            }
            let path = dest.join(&asset.name);
            fs::rename(&part, &path).map_err(|_| DownloadCode::Network.as_str().to_owned())?;
            Ok(path)
        });
    let _ = fs::remove_dir_all(&work);
    let path = result?;
    match extract_to {
        Some(to) => extract_release(&path, to).map_err(|_| BAD_ZIP.to_owned()),
        None => Ok(path),
    }
}

/// Cómo se actualiza este ejecutable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfUpdate {
    /// Fuera de Windows: solo se abre la página de la release.
    Unsupported,
    /// Instalado con Inno Setup, o sin empaquetar en Windows (como Python sin `sys.frozen`):
    /// se lanza el instalador nuevo.
    Installer,
    /// Zip descomprimido en esta carpeta: un script la reemplaza al cerrar.
    Portable(PathBuf),
}

impl SelfUpdate {
    /// Solo cuenta como empaquetado el `JASE.exe` (el `jase.exe` de `cargo build` no, como
    /// Python sin `sys.frozen`).
    pub fn detect() -> SelfUpdate {
        if !cfg!(windows) {
            return SelfUpdate::Unsupported;
        }
        let Ok(exe) = std::env::current_exe() else {
            return SelfUpdate::Unsupported;
        };
        let frozen = exe.file_name().is_some_and(|n| n == EXE_NAME);
        match exe.parent() {
            Some(dir) if is_portable(&exe, frozen, "win32") => SelfUpdate::Portable(dir.into()),
            _ => SelfUpdate::Installer,
        }
    }
}
