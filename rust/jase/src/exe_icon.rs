//! Icono del exe de cada juego para el árbol (`_platform_exe_icon` y `ExeIconLoader`): solo
//! Windows lo tiene; fuera, `None` y el árbol dibuja la insignia. Toca el disco: va en un hilo.

use iced::widget::image::Handle;
use jase_core::games::GameDef;
use jase_core::paths::resolve_exe;

/// El exe que mostraría Qt (`paths.resolve_exe`) y su icono chico.
pub fn game_exe_icon(game: &'static GameDef, dir: &str) -> Option<Handle> {
    let exe = resolve_exe(
        Some(dir),
        game.exe_relpath,
        game.exe_candidates,
        game.preferred_exes,
    )?;
    platform_icon(&exe)
}

/// Píxeles BGRA de `GetDIBits` a RGBA. Si ningún píxel trae alfa (iconos viejos), la máscara
/// AND de 1 bit por píxel (filas de 4 bytes, como la da `GetDIBits`) dice qué es transparente;
/// sin máscara, opaco.
pub fn bgra_to_rgba(bgra: &[u8], mask: Option<&[u8]>, width: usize) -> Vec<u8> {
    let (pixels, _) = bgra.as_chunks::<4>();
    let use_mask = pixels.iter().all(|p| p[3] == 0);
    let stride = width.div_ceil(32) * 4;
    let masked = |i: usize| {
        let (Some(m), true) = (mask, width > 0) else {
            return false;
        };
        let (y, x) = (i / width, i % width);
        m.get(y * stride + x / 8)
            .is_some_and(|b| b >> (7 - x % 8) & 1 == 1)
    };
    pixels
        .iter()
        .enumerate()
        .flat_map(|(i, &[b, g, r, a])| {
            let alpha = match (use_mask, masked(i)) {
                (false, _) => a,
                (true, false) => 255,
                (true, true) => 0,
            };
            [r, g, b, alpha]
        })
        .collect()
}

#[cfg(not(windows))]
pub fn platform_icon(_path: &str) -> Option<Handle> {
    None
}

#[cfg(windows)]
pub fn platform_icon(path: &str) -> Option<Handle> {
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HBITMAP, HDC, RGBQUAD,
    };
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows_sys::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON, SHGetFileInfoW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

    // `CoInitializeEx` correcto (S_OK o S_FALSE) se empareja con `CoUninitialize`; si el hilo
    // ya era MTA (RPC_E_CHANGED_MODE) u otro fallo, se sigue sin desinicializar.
    struct Com(bool);
    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: empareja el `CoInitializeEx` correcto de este mismo hilo.
                unsafe { CoUninitialize() };
            }
        }
    }
    // Cada handle se libera al soltarse, también en los caminos de error.
    struct Icon(HICON);
    impl Drop for Icon {
        fn drop(&mut self) {
            // SAFETY: el icono es nuestro (lo dio `SHGetFileInfoW`) y no se usa después.
            unsafe { DestroyIcon(self.0) };
        }
    }
    struct Bitmap(HBITMAP);
    impl Drop for Bitmap {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: el bitmap es nuestro (lo creó `GetIconInfo`) y no se usa después.
                unsafe { DeleteObject(self.0) };
            }
        }
    }
    struct Dc(HDC);
    impl Drop for Dc {
        fn drop(&mut self) {
            // SAFETY: el DC es nuestro (de `CreateCompatibleDC`) y no se usa después.
            unsafe { DeleteDC(self.0) };
        }
    }
    // `BITMAPINFO` con lugar para la paleta de 2 colores que `GetDIBits` escribe con 1 bit.
    #[repr(C)]
    struct Info {
        header: BITMAPINFOHEADER,
        colors: [RGBQUAD; 2],
    }

    /// Las filas de arriba abajo con `bits` bits por píxel; `None` si falla.
    fn dib_bits(dc: &Dc, bm: &Bitmap, w: i32, h: i32, bits: u16) -> Option<Vec<u8>> {
        let stride = (w as usize * bits as usize).div_ceil(32) * 4;
        let mut buf = vec![0u8; stride * h as usize];
        let mut info = Info {
            header: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // negativa: de arriba abajo
                biPlanes: 1,
                biBitCount: bits,
                biCompression: BI_RGB,
                ..Default::default()
            },
            colors: [RGBQUAD::default(); 2],
        };
        // SAFETY: `buf` tiene `h` filas de `stride` bytes para ese formato e `Info` tiene lugar
        // para la paleta; los handles están vivos mientras duran `dc` y `bm`.
        let lines = unsafe {
            GetDIBits(
                dc.0,
                bm.0,
                0,
                h as u32,
                buf.as_mut_ptr().cast(),
                (&raw mut info).cast::<BITMAPINFO>(),
                DIB_RGB_COLORS,
            )
        };
        (lines == h).then_some(buf)
    }

    // SHGetFileInfoW pide COM iniciado; va primero para soltarse último.
    // SAFETY: el reservado va nulo; las banderas son valores válidos de `COINIT`.
    let hr = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    let _com = Com(hr >= 0);
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `SHFILEINFOW` es una estructura C de enteros y un handle: los ceros valen.
    let mut sfi: SHFILEINFOW = unsafe { std::mem::zeroed() };
    // SAFETY: `wide` termina en 0 y vive durante la llamada; `sfi` es local y de ese tamaño.
    let ok = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut sfi,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    };
    if ok == 0 || sfi.hIcon.is_null() {
        return None;
    }
    let icon = Icon(sfi.hIcon);
    let mut ii = ICONINFO::default();
    // SAFETY: `icon` es un icono válido y `ii` una salida local.
    if unsafe { GetIconInfo(icon.0, &mut ii) } == 0 {
        return None;
    }
    let (color, mask) = (Bitmap(ii.hbmColor), Bitmap(ii.hbmMask));
    // shortcut: los iconos monocromos (sin bitmap de color) quedan con la insignia.
    if color.0.is_null() {
        return None;
    }
    let mut bm = BITMAP::default();
    // SAFETY: `color` es un bitmap válido y `bm` una salida local de ese tamaño.
    let got = unsafe { GetObjectW(color.0, size_of::<BITMAP>() as i32, (&raw mut bm).cast()) };
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if got == 0 || !(1..=256).contains(&w) || !(1..=256).contains(&h) {
        return None;
    }
    // SAFETY: sin argumentos: un DC de memoria compatible con la pantalla.
    let raw = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if raw.is_null() {
        return None; // sin DC no hay nada que borrar: el guardia no debe envolver null
    }
    let dc = Dc(raw);
    let bgra = dib_bits(&dc, &color, w, h, 32)?;
    let and_mask = (!mask.0.is_null())
        .then(|| dib_bits(&dc, &mask, w, h, 1))
        .flatten();
    let rgba = bgra_to_rgba(&bgra, and_mask.as_deref(), w as usize);
    Some(Handle::from_rgba(w as u32, h as u32, rgba))
}
