//! Lectura de valores de texto del registro de Windows (el único `unsafe` de la detección).
use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW,
};

pub(crate) use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_SUBKEY_WOW6432KEY, RRF_SUBKEY_WOW6464KEY,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Valor `REG_SZ`/`REG_EXPAND_SZ` (sin expandir) no vacío, o `None`. `view` = `RRF_SUBKEY_*` o 0.
pub(crate) fn read_string(root: HKEY, subkey: &str, value: &str, view: u32) -> Option<String> {
    let (subkey, value) = (wide(subkey), wide(value));
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND | view;
    let mut buf: Vec<u16> = vec![0; 260];
    loop {
        let mut bytes = u32::try_from(buf.len() * 2).ok()?;
        // SAFETY: las cadenas terminan en NUL y `bytes` es el tamaño real de `buf` en bytes.
        let status = unsafe {
            RegGetValueW(
                root,
                subkey.as_ptr(),
                value.as_ptr(),
                flags,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        match status {
            ERROR_SUCCESS => {
                buf.truncate(bytes as usize / 2);
                while buf.last() == Some(&0) {
                    buf.pop();
                }
                return String::from_utf16(&buf).ok().filter(|s| !s.is_empty());
            }
            // Puede crecer entre dos lecturas: se vuelve a pedir con el tamaño nuevo.
            ERROR_MORE_DATA => buf.resize((bytes as usize).div_ceil(2).max(buf.len() + 1), 0),
            _ => return None,
        }
    }
}
