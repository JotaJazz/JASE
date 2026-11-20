//! Actualizar el zip portátil (Windows). Traducción de `portable_update.py`.
//!
//! El exe en uso no se puede sobrescribir: se extrae el zip ya verificado en la caché y un
//! script .cmd espera a que JASE se cierre, reemplaza la carpeta (apartando el `_internal`
//! viejo hasta que la copia sale bien) y vuelve a abrir JASE. Todo bloqueante.
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub const EXE_NAME: &str = "JASE.exe";
pub const ZIP_TOP: &str = "JASE"; // Compress-Archive -Path dist\JASE: todo va dentro de «JASE/»
pub const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;
pub const UNINSTALLER: &str = "unins000.exe"; // lo deja Inno Setup: si está, es una instalación
pub const SCRIPT_NAME: &str = "jase-update.cmd";

/// El zip no sirve o la carpeta no se puede actualizar así.
#[derive(Debug, thiserror::Error)]
pub enum PortableUpdateError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn invalid(msg: impl Into<String>) -> PortableUpdateError {
    PortableUpdateError::Invalid(msg.into())
}

/// JASE corre desde un zip descomprimido (no del instalador ni desde el código).
pub fn is_portable(exe: &Path, frozen: bool, platform: &str) -> bool {
    frozen
        && platform == "win32"
        && !exe
            .parent()
            .unwrap_or(Path::new(""))
            .join(UNINSTALLER)
            .exists()
}

/// Extrae el zip en `dest` y devuelve la carpeta con JASE.exe. Rechaza (antes de escribir
/// nada) rutas que salgan de `dest`, zips demasiado grandes y los que no traen JASE/JASE.exe.
pub fn extract_release(zip_path: &Path, dest: &Path) -> Result<PathBuf, PortableUpdateError> {
    let mut archive = ZipArchive::new(BufReader::new(File::open(zip_path)?))
        .map_err(|e| invalid(format!("zip ilegible: {e}")))?;
    // (partes de la ruta como PurePosixPath, es carpeta, tamaño declarado)
    let mut members = Vec::with_capacity(archive.len());
    let mut total: u64 = 0;
    let mut has_exe = false;
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| invalid(format!("zip ilegible: {e}")))?;
        let name = entry
            .name()
            .map_err(|e| invalid(format!("zip ilegible: {e}")))?
            .replace('\\', "/");
        if name.starts_with('/') || name.contains(':') {
            return Err(invalid(format!("ruta no válida en el zip: {name}")));
        }
        let parts: Vec<String> = name
            .split('/')
            .filter(|p| !p.is_empty() && *p != ".")
            .map(str::to_owned)
            .collect();
        if parts.iter().any(|p| p == "..") {
            return Err(invalid(format!("ruta no válida en el zip: {name}")));
        }
        has_exe |= parts == [ZIP_TOP, EXE_NAME];
        total = total.saturating_add(entry.size());
        members.push((parts, name.ends_with('/'), entry.size()));
    }
    if !has_exe {
        return Err(invalid("el zip no trae JASE/JASE.exe"));
    }
    if total > MAX_UNPACKED_BYTES {
        return Err(invalid("zip demasiado grande"));
    }
    // Bucle propio, no `ZipArchive::extract`: nunca crea enlaces simbólicos.
    for (i, (parts, is_dir, size)) in members.into_iter().enumerate() {
        let path = parts.iter().fold(dest.to_path_buf(), |p, s| p.join(s));
        if is_dir {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let entry = archive
            .by_index(i)
            .map_err(|e| invalid(format!("zip ilegible: {e}")))?;
        let copied = io::copy(
            &mut entry.take(size.saturating_add(1)),
            &mut File::create(&path)?,
        )?;
        if copied != size {
            return Err(invalid(format!(
                "tamaño falso en el zip: {}",
                parts.join("/")
            )));
        }
    }
    Ok(dest.join(ZIP_TOP))
}

/// Script de cmd que reemplaza `install_dir` con `source` cuando el proceso `pid` termina y
/// vuelve a abrir JASE. Los caracteres que cmd interpreta (% " ^ & | < >) no pueden aparecer
/// en las rutas.
pub fn updater_script(
    pid: u32,
    source: &Path,
    install_dir: &Path,
) -> Result<String, PortableUpdateError> {
    let text = |path: &Path| match path.to_str() {
        Some(s) if !s.contains(['%', '"', '^', '&', '|', '<', '>']) => Ok(s.to_owned()),
        _ => Err(invalid(format!(
            "carpeta con caracteres no admitidos: {}",
            path.display()
        ))),
    };
    let (src, dst) = (text(source)?, text(install_dir)?);
    Ok([
        "@echo off".to_owned(),
        "chcp 65001 >nul".to_owned(), // el script va en UTF-8 (rutas con tildes)
        "setlocal".to_owned(),
        ":wait".to_owned(),
        format!(r#"tasklist /FI "PID eq {pid}" | find "{pid}" >nul"#),
        "if not errorlevel 1 (".to_owned(),
        "  timeout /t 1 /nobreak >nul".to_owned(),
        "  goto wait".to_owned(),
        ")".to_owned(),
        // El _internal viejo se aparta (no se borra) hasta que la copia salga bien.
        format!(r#"if exist "{dst}\_internal.old" rmdir /s /q "{dst}\_internal.old""#),
        format!(r#"if exist "{dst}\_internal" ren "{dst}\_internal" _internal.old"#),
        // _internal primero: si falla (disco lleno), el exe viejo sigue con su _internal.
        format!(
            r#"robocopy "{src}\_internal" "{dst}\_internal" /E /R:5 /W:1 /NFL /NDL /NJH /NJS /NP >nul"#
        ),
        "if errorlevel 8 goto failed".to_owned(),
        // shortcut: si falla la copia del exe puede quedar a medias; apartar también el exe si ocurre en la práctica
        format!(
            r#"robocopy "{src}" "{dst}" /E /XD _internal /R:5 /W:1 /NFL /NDL /NJH /NJS /NP >nul"#
        ),
        "if errorlevel 8 goto failed".to_owned(),
        format!(r#"if exist "{dst}\_internal.old" rmdir /s /q "{dst}\_internal.old""#),
        format!(r#"start "" "{dst}\{EXE_NAME}""#),
        "goto end".to_owned(),
        ":failed".to_owned(), // fuera de bloques ( ): una ruta con paréntesis no rompe el script
        format!(r#"if exist "{dst}\_internal" rmdir /s /q "{dst}\_internal""#),
        format!(r#"if exist "{dst}\_internal.old" ren "{dst}\_internal.old" _internal"#),
        "echo No se pudo copiar la version nueva de JASE. / The new JASE could not be copied."
            .to_owned(),
        "pause".to_owned(),
        ":end".to_owned(),
        r#"(goto) 2>nul & del "%~f0""#.to_owned(),
        String::new(),
    ]
    .join("\r\n"))
}

/// Escribe el script en `cache_dir` como bytes UTF-8 tal cual (con sus `\r\n`).
pub fn write_updater(
    cache_dir: &Path,
    pid: u32,
    source: &Path,
    install_dir: &Path,
) -> Result<PathBuf, PortableUpdateError> {
    let script = cache_dir.join(SCRIPT_NAME);
    let text = updater_script(pid, source, install_dir)?;
    fs::create_dir_all(cache_dir)?;
    fs::write(&script, text)?;
    Ok(script)
}
