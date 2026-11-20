//! El exe de Windows que se empaqueta (`JASE.exe`): subsistema de ventana, icono para
//! `JASE.exe,0` del instalador, versión del crate y sin pedir el CRT de Visual C++.
#![cfg(windows)]

use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
};
use windows_sys::Win32::UI::Shell::ExtractIconExW;

const EXE: &str = env!("CARGO_BIN_EXE_jase");

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain([0]).collect()
}

fn exe_bytes() -> Vec<u8> {
    std::fs::read(EXE).unwrap()
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

#[test]
fn exe_is_windows_gui_subsystem() {
    let b = exe_bytes();
    let pe = u32_at(&b, 0x3C) as usize;
    assert_eq!(&b[pe..pe + 4], b"PE\0\0");
    let optional = pe + 24;
    let subsystem = u16::from_le_bytes([b[optional + 68], b[optional + 69]]);
    assert_eq!(subsystem, 2, "IMAGE_SUBSYSTEM_WINDOWS_GUI");
}

#[test]
fn exe_has_an_icon() {
    let path = wide(EXE);
    // SAFETY: ruta terminada en 0; con índice -1 solo cuenta los iconos.
    let n = unsafe { ExtractIconExW(path.as_ptr(), -1, null_mut(), null_mut(), 0) };
    assert!(n >= 1, "iconos: {n}");
}

#[test]
fn exe_version_matches_crate() {
    let path = wide(EXE);
    // SAFETY: ruta terminada en 0 y búfer del tamaño que pide la API.
    let info = unsafe {
        let size = GetFileVersionInfoSizeW(path.as_ptr(), null_mut());
        assert!(size > 0, "sin VS_VERSIONINFO");
        let mut data = vec![0u8; size as usize];
        assert_ne!(
            GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()),
            0
        );
        let mut ptr = null_mut();
        let mut len = 0u32;
        let root = wide("\\");
        assert_ne!(
            VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), &mut ptr, &mut len),
            0
        );
        assert!(len as usize >= size_of::<VS_FIXEDFILEINFO>());
        *ptr.cast::<VS_FIXEDFILEINFO>()
    };
    let v: Vec<u32> = env!("CARGO_PKG_VERSION")
        .split(['.', '-'])
        .take(3)
        .map(|p| p.parse().unwrap())
        .collect();
    assert_eq!(info.dwFileVersionMS, (v[0] << 16) | v[1]);
    assert_eq!(info.dwFileVersionLS >> 16, v[2]);
}

/// `ProductName` tal cual (UTF-16 en el recurso): `rc.exe` no entiende el `\'` de winresource.
#[test]
fn exe_product_name_has_plain_apostrophe() {
    let name: Vec<u8> = "\0Jimmy's All Seeing Eye\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let b = exe_bytes();
    assert!(b.windows(name.len()).any(|w| w == name));
}

#[test]
fn exe_does_not_import_vc_runtime() {
    let b = exe_bytes().to_ascii_lowercase();
    for dll in [&b"vcruntime140"[..], b"msvcp140"] {
        assert!(
            !b.windows(dll.len()).any(|w| w == dll),
            "{}",
            String::from_utf8_lossy(dll)
        );
    }
}
