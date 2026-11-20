//! Port de tests/test_portable_update.py (sin Qt), más zips hostiles y lo que escribía Python.

use jase_core::app_update::{Asset, newest_release};
use jase_core::portable_update::{
    EXE_NAME, PortableUpdateError, SCRIPT_NAME, UNINSTALLER, extract_release, is_portable,
    updater_script, write_updater,
};
use serde_json::json;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const ZIP_URL: &str =
    "https://github.com/JotaJazz/JASE/releases/download/v99.0.0/JASE-99.0.0-win64.zip";

/// Zip *stored*, como `zipfile.ZipFile(buf, "w")`.
fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        z.start_file(*name, SimpleFileOptions::default()).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn good() -> Vec<u8> {
    make_zip(&[("JASE/JASE.exe", b"exe"), ("JASE/_internal/x.dll", b"dll")])
}

fn lines(script: &str) -> Vec<&str> {
    script.split("\r\n").collect()
}

fn sha(data: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, data)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Escribe `v` en las cabeceras de cada entrada: tamaño descomprimido en la local
/// (desplazamiento 22) y en el directorio central (24).
fn patch_sizes(data: &mut [u8], sizes: &[u32]) {
    for (sig, off) in [(b"PK\x03\x04", 22), (b"PK\x01\x02", 24)] {
        let starts: Vec<usize> = data
            .windows(4)
            .enumerate()
            .filter(|(_, w)| w == sig)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(starts.len(), sizes.len());
        for (start, size) in starts.iter().zip(sizes) {
            data[start + off..start + off + 4].copy_from_slice(&size.to_le_bytes());
        }
    }
}

#[test]
fn is_portable_cases() {
    let t = tempfile::tempdir().unwrap();
    let exe = t.path().join(EXE_NAME);
    assert!(is_portable(&exe, true, "win32"));
    assert!(!is_portable(&exe, false, "win32")); // desde el código
    assert!(!is_portable(&exe, true, "linux"));
    fs::write(t.path().join(UNINSTALLER), b"").unwrap();
    assert!(!is_portable(&exe, true, "win32")); // instalado con Inno Setup
}

#[test]
fn extract_release_ok_and_rejects_bad_zips() {
    let t = tempfile::tempdir().unwrap();
    let zip_path = t.path().join("a.zip");
    fs::write(&zip_path, good()).unwrap();
    let out = extract_release(&zip_path, &t.path().join("out")).unwrap();
    assert_eq!(out, t.path().join("out").join("JASE"));
    assert_eq!(fs::read(out.join(EXE_NAME)).unwrap(), b"exe");
    assert_eq!(
        fs::read(out.join("_internal").join("x.dll")).unwrap(),
        b"dll"
    );
    let bad: [&[(&str, &[u8])]; 4] = [
        &[("otra/JASE.exe", b"x")],
        &[("JASE/JASE.exe", b"x"), ("../evil.txt", b"x")],
        &[("JASE/JASE.exe", b"x"), ("/abs.txt", b"x")],
        &[("JASE/JASE.exe", b"x"), ("C:/win.txt", b"x")],
    ];
    for files in bad {
        fs::write(&zip_path, make_zip(files)).unwrap();
        let err = extract_release(&zip_path, &t.path().join("bad")).unwrap_err();
        assert!(matches!(err, PortableUpdateError::Invalid(_)), "{files:?}");
    }
    assert!(!t.path().join("evil.txt").exists());
    assert!(!t.path().join("bad").exists()); // se valida todo antes de escribir
}

#[test]
fn extract_release_accepts_backslash_names_and_rejects_unreadable() {
    let t = tempfile::tempdir().unwrap();
    let zip_path = t.path().join("a.zip");
    // Compress-Archive de PowerShell 5 escribe «\».
    fs::write(&zip_path, make_zip(&[("JASE\\JASE.exe", b"exe")])).unwrap();
    let out = extract_release(&zip_path, &t.path().join("out")).unwrap();
    assert_eq!(fs::read(out.join(EXE_NAME)).unwrap(), b"exe");
    fs::write(
        &zip_path,
        make_zip(&[("JASE/JASE.exe", b"x"), ("JASE\\..\\e", b"x")]),
    )
    .unwrap();
    assert!(extract_release(&zip_path, &t.path().join("bad")).is_err());
    fs::write(&zip_path, b"no es un zip").unwrap();
    let err = extract_release(&zip_path, &t.path().join("bad")).unwrap_err();
    assert!(matches!(err, PortableUpdateError::Invalid(_)));
}

#[test]
fn extract_release_rejects_lying_sizes() {
    let t = tempfile::tempdir().unwrap();
    let zip_path = t.path().join("a.zip");
    let mut data = make_zip(&[("JASE/JASE.exe", b"exe-de-verdad-mas-largo")]);
    patch_sizes(&mut data, &[3]);
    fs::write(&zip_path, data).unwrap();
    let dest = t.path().join("out");
    assert!(extract_release(&zip_path, &dest).is_err());
    let exe = dest.join("JASE").join(EXE_NAME);
    assert!(fs::read(&exe).map_or(true, |d| d.len() <= 4));
}

#[test]
fn extract_release_too_big() {
    let t = tempfile::tempdir().unwrap();
    let zip_path = t.path().join("a.zip");
    let mut data = make_zip(&[("JASE/JASE.exe", b"a"), ("JASE/b", b"b")]);
    let half = 512 * 1024 * 1024;
    patch_sizes(&mut data, &[half, half + 1]);
    fs::write(&zip_path, data).unwrap();
    let dest = t.path().join("out");
    let err = extract_release(&zip_path, &dest).unwrap_err();
    assert!(matches!(err, PortableUpdateError::Invalid(_)));
    assert!(!dest.exists());
}

#[cfg(unix)]
#[test]
fn extract_release_never_creates_symlinks() {
    let t = tempfile::tempdir().unwrap();
    let zip_path = t.path().join("a.zip");
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    z.start_file("JASE/JASE.exe", SimpleFileOptions::default())
        .unwrap();
    z.write_all(b"exe").unwrap();
    z.add_symlink("JASE/link", "/etc/passwd", SimpleFileOptions::default())
        .unwrap();
    fs::write(&zip_path, z.finish().unwrap().into_inner()).unwrap();
    let dest = t.path().join("out");
    extract_release(&zip_path, &dest).unwrap();
    let link = dest.join("JASE").join("link");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_file());
    assert_eq!(fs::read(&link).unwrap(), b"/etc/passwd");
}

/// El zip que escribe `zipfile.ZipFile(…, 'w', ZIP_DEFLATED)` de Python con `JASE/JASE.exe`
/// (`b'exe' * 1000`), la carpeta `JASE/_internal/` y `JASE/_internal/x.dll`
/// (`'dll José' * 500`), en hexadecimal.
const ZIPFILE_RELEASE: &str = concat!(
    "504b030414000000080096bd735d35b84d4617000000b80b00000d0000004a4153452f4a4153452e657865ed",
    "c2311100000c0321b5efa1f26b23031c5da9aaaadb1f504b030414000000080096bd735d0000000002000000",
    "000000000f0000004a4153452f5f696e7465726e616c2f0300504b030414000000080096bd735d69f1531029",
    "00000094110000140000004a4153452f5f696e7465726e616c2f782e646c6cedc6310d00300800302be8990e",
    "4e921d53850e8ce162577b35abe2dc379d2222222222222222f22b0b504b0102140314000000080096bd735d",
    "35b84d4617000000b80b00000d00000000000000000000008001000000004a4153452f4a4153452e65786550",
    "4b0102140314000000080096bd735d0000000002000000000000000f0000000000000000001000fd41420000",
    "004a4153452f5f696e7465726e616c2f504b0102140314000000080096bd735d69f153102900000094110000",
    "1400000000000000000000008001710000004a4153452f5f696e7465726e616c2f782e646c6c504b05060000",
    "000003000300ba000000cc0000000000",
);

#[test]
fn release_zip_from_zipfile_extracts() {
    let t = tempfile::tempdir().unwrap();
    let zip_path = t.path().join("py.zip");
    let bytes: Vec<u8> = (0..ZIPFILE_RELEASE.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&ZIPFILE_RELEASE[i..i + 2], 16).unwrap())
        .collect();
    fs::write(&zip_path, bytes).unwrap();
    let out = extract_release(&zip_path, &t.path().join("out")).unwrap();
    assert_eq!(fs::read(out.join(EXE_NAME)).unwrap(), b"exe".repeat(1000));
    assert_eq!(
        fs::read(out.join("_internal").join("x.dll")).unwrap(),
        "dll José".repeat(500).into_bytes()
    );
}

#[test]
fn updater_script_waits_replaces_and_restarts() {
    let script = updater_script(
        4321,
        Path::new(r"C:\cache\JASE"),
        Path::new(r"D:\Juegos\José\JASE"),
    )
    .unwrap();
    let lines = lines(&script);
    assert!(lines.contains(&r#"tasklist /FI "PID eq 4321" | find "4321" >nul"#));
    assert!(lines.contains(
        &r#"if exist "D:\Juegos\José\JASE\_internal" rmdir /s /q "D:\Juegos\José\JASE\_internal""#
    ));
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with(r#"robocopy "C:\cache\JASE" "D:\Juegos\José\JASE" /E"#))
    );
    assert!(lines.contains(&r#"start "" "D:\Juegos\José\JASE\JASE.exe""#));
    assert!(lines.contains(&"chcp 65001 >nul"));
    for bad in [
        "C:/100% JASE",
        "C:/a\"b",
        "C:/a^b",
        "C:/a&b",
        "C:/a|b",
        "C:/a<b",
        "C:/a>b",
    ] {
        let err = updater_script(1, Path::new("C:/a"), Path::new(bad)).unwrap_err();
        assert!(matches!(err, PortableUpdateError::Invalid(_)), "{bad}");
        assert!(updater_script(1, Path::new(bad), Path::new("C:/b")).is_err());
    }
    let t = tempfile::tempdir().unwrap();
    let written = write_updater(t.path(), 1, Path::new("C:/a"), Path::new("C:/b")).unwrap();
    assert_eq!(written.file_name().unwrap(), SCRIPT_NAME);
    assert!(!fs::read_to_string(&written).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn updater_script_rejects_non_utf8_paths() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let bad = Path::new(OsStr::from_bytes(b"/a\xff"));
    let err = updater_script(1, bad, Path::new("/b")).unwrap_err();
    assert!(matches!(err, PortableUpdateError::Invalid(_)));
}

/// Lo que escribía `updater_script(4321, …)` de Python 1.3.0, con saltos `\r\n`.
const SCRIPT_4321: &str = r#"@echo off
chcp 65001 >nul
setlocal
:wait
tasklist /FI "PID eq 4321" | find "4321" >nul
if not errorlevel 1 (
  timeout /t 1 /nobreak >nul
  goto wait
)
if exist "D:\Juegos\José\JASE\_internal.old" rmdir /s /q "D:\Juegos\José\JASE\_internal.old"
if exist "D:\Juegos\José\JASE\_internal" ren "D:\Juegos\José\JASE\_internal" _internal.old
robocopy "C:\cache\JASE\_internal" "D:\Juegos\José\JASE\_internal" /E /R:5 /W:1 /NFL /NDL /NJH /NJS /NP >nul
if errorlevel 8 goto failed
robocopy "C:\cache\JASE" "D:\Juegos\José\JASE" /E /XD _internal /R:5 /W:1 /NFL /NDL /NJH /NJS /NP >nul
if errorlevel 8 goto failed
if exist "D:\Juegos\José\JASE\_internal.old" rmdir /s /q "D:\Juegos\José\JASE\_internal.old"
start "" "D:\Juegos\José\JASE\JASE.exe"
goto end
:failed
if exist "D:\Juegos\José\JASE\_internal" rmdir /s /q "D:\Juegos\José\JASE\_internal"
if exist "D:\Juegos\José\JASE\_internal.old" ren "D:\Juegos\José\JASE\_internal.old" _internal
echo No se pudo copiar la version nueva de JASE. / The new JASE could not be copied.
pause
:end
(goto) 2>nul & del "%~f0""#;

#[test]
fn updater_script_known_answers() {
    let pairs = [
        (4321, r"C:\cache\JASE", r"D:\Juegos\José\JASE"),
        (7, r"C:\c\JASE", r"C:\Program Files (x86)\JASE"),
        (1, r"C:\n", r"C:\d"),
    ];
    for (pid, src, dst) in pairs {
        // Las otras dos salidas de Python son la misma plantilla con su pid y sus rutas.
        let want: String = SCRIPT_4321
            .replace(r"D:\Juegos\José\JASE", dst)
            .replace(r"C:\cache\JASE", src)
            .replace("4321", &pid.to_string())
            .lines()
            .map(|l| format!("{l}\r\n"))
            .collect();
        let ours = updater_script(pid, Path::new(src), Path::new(dst)).unwrap();
        assert_eq!(ours, want, "{pid}");
    }
}

#[test]
fn release_lists_portable_asset() {
    let body = good();
    let release = json!({
        "tag_name": "v99.0.0", "draft": false, "prerelease": false,
        "html_url": "https://github.com/JotaJazz/JASE/releases/tag/v99.0.0",
        "assets": [{"name": "JASE-99.0.0-win64.zip", "size": body.len(),
                    "digest": format!("sha256:{}", sha(&body)),
                    "browser_download_url": ZIP_URL}],
    });
    let r = newest_release(&serde_json::to_vec(&[release]).unwrap()).unwrap();
    assert_eq!(
        r.portable,
        Some(Asset {
            name: "JASE-99.0.0-win64.zip".into(),
            url: ZIP_URL.into(),
            size: body.len() as u64,
            sha256: sha(&body),
        })
    );
    assert_eq!(r.installer, None);
}

#[test]
fn script_restores_old_internal_when_copy_fails() {
    let script = updater_script(
        7,
        Path::new(r"C:\c\JASE"),
        Path::new(r"C:\Program Files (x86)\JASE"),
    )
    .unwrap();
    let lines = lines(&script);
    let pos = |l: &str| lines.iter().position(|x| *x == l).unwrap();
    let rename = pos(
        r#"if exist "C:\Program Files (x86)\JASE\_internal" ren "C:\Program Files (x86)\JASE\_internal" _internal.old"#,
    );
    let copy = lines
        .iter()
        .position(|l| l.starts_with("robocopy"))
        .unwrap();
    let failed = pos(":failed");
    assert!(
        rename < copy
            && copy < pos("if errorlevel 8 goto failed")
            && pos("if errorlevel 8 goto failed") < failed
    );
    assert!(lines[failed..].contains(
        &r#"if exist "C:\Program Files (x86)\JASE\_internal.old" ren "C:\Program Files (x86)\JASE\_internal.old" _internal"#
    ));
    // Ninguna línea con ruta queda dentro de un bloque ( ): «(x86)» lo cerraría.
    let mut depth = 0i32;
    for line in &lines {
        assert!(!(line.contains("JASE") && depth != 0), "{line}");
        depth += i32::from(line.trim_end().ends_with('(')) - i32::from(line.trim() == ")");
    }
}

#[test]
fn updater_file_keeps_exact_line_endings() {
    let t = tempfile::tempdir().unwrap();
    let script = write_updater(t.path(), 1, Path::new("C:/a"), Path::new("C:/b")).unwrap();
    let data = fs::read(script).unwrap();
    assert!(!data.windows(3).any(|w| w == b"\r\r\n"));
    assert_eq!(
        data,
        updater_script(1, Path::new("C:/a"), Path::new("C:/b"))
            .unwrap()
            .into_bytes()
    );
}

#[test]
fn internal_is_copied_before_the_exe() {
    let script = updater_script(7, Path::new(r"C:\n"), Path::new(r"C:\d")).unwrap();
    let lines = lines(&script);
    let copies: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].starts_with("robocopy"))
        .collect();
    assert_eq!(copies.len(), 2);
    assert!(lines[copies[0]].starts_with(r#"robocopy "C:\n\_internal" "C:\d\_internal""#));
    assert!(lines[copies[1]].contains("/XD _internal"));
    assert_eq!(lines[copies[0] + 1], "if errorlevel 8 goto failed");
    assert_eq!(lines[copies[1] + 1], "if errorlevel 8 goto failed");
}

/// Instalación vieja y script en carpetas separadas; ejecuta el script de verdad.
#[cfg(windows)]
fn run_script(tmp: &Path, with_source: bool) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::process::{Command, Stdio};
    let root = tmp.join("Program Files (x86)").join("JASÉ");
    let (install, new) = (root.join("install"), root.join("new"));
    fs::create_dir_all(install.join("_internal")).unwrap();
    fs::write(install.join(EXE_NAME), b"viejo").unwrap();
    fs::write(install.join("_internal").join("old.txt"), "old").unwrap();
    if with_source {
        fs::create_dir_all(new.join("_internal")).unwrap();
        // whoami termina solo y no abre diálogos
        fs::copy(r"C:\Windows\System32\whoami.exe", new.join(EXE_NAME)).unwrap();
        fs::write(new.join("_internal").join("new.txt"), "new").unwrap();
    }
    let mut finished = Command::new("cmd").args(["/c", "exit"]).spawn().unwrap();
    finished.wait().unwrap(); // PID de un proceso ya terminado
    let script = write_updater(&root.join("cache"), finished.id(), &new, &install).unwrap();
    Command::new("cmd.exe")
        .arg("/c")
        .arg(&script)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (install, script)
}

#[cfg(windows)]
#[test]
fn script_replaces_install() {
    let t = tempfile::tempdir().unwrap();
    let (install, script) = run_script(t.path(), true);
    assert!(install.join("_internal").join("new.txt").exists());
    assert!(!install.join("_internal.old").exists());
    assert_ne!(fs::read(install.join(EXE_NAME)).unwrap(), b"viejo");
    assert!(!script.exists());
}

#[cfg(windows)]
#[test]
fn script_restores_when_source_is_missing() {
    let t = tempfile::tempdir().unwrap();
    let (install, script) = run_script(t.path(), false); // robocopy devuelve 16
    assert!(!script.exists());
    assert!(install.join("_internal").join("old.txt").exists());
    assert_eq!(fs::read(install.join(EXE_NAME)).unwrap(), b"viejo");
}
