//! Decodificador DXT1/3/5 (BC1/2/3) con el mismo resultado que el `bcn` de Pillow
//! (`BcnDecode.c`): 565 por replicación de bits e interpolación entera.
use image::RgbaImage;

fn rgb565(v: u16) -> [u16; 3] {
    let r = (v & 0xF800) >> 8;
    let g = (v & 0x07E0) >> 3;
    let b = (v & 0x001F) << 3;
    [r | (r >> 5), g | (g >> 6), b | (b >> 5)]
}

/// Los 16 colores de un bloque BC1; BC2/BC3 (`separate_alpha`) siempre usan 4 colores.
fn color_block(src: &[u8; 8], separate_alpha: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([src[0], src[1]]);
    let c1 = u16::from_le_bytes([src[2], src[3]]);
    let lut = u32::from_le_bytes([src[4], src[5], src[6], src[7]]);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |wa: u16, wb: u16, d: u16| -> [u8; 4] {
        let ch = |i: usize| ((wa * a[i] + wb * b[i]) / d) as u8;
        [ch(0), ch(1), ch(2), 255]
    };
    let p = if c0 > c1 || separate_alpha {
        [mix(1, 0, 1), mix(0, 1, 1), mix(2, 1, 3), mix(1, 2, 3)]
    } else {
        [mix(1, 0, 1), mix(0, 1, 1), mix(1, 1, 2), [0, 0, 0, 0]]
    };
    std::array::from_fn(|n| p[((lut >> (2 * n)) & 3) as usize])
}

/// Alfa interpolado de BC3 (8 bytes) para los 16 píxeles.
fn bc3_alpha(src: &[u8; 8]) -> [u8; 16] {
    let (a0, a1) = (u16::from(src[0]), u16::from(src[1]));
    let mut a = [a0, a1, 0, 0, 0, 0, 0, 0];
    if a0 > a1 {
        for i in 2..8u16 {
            a[usize::from(i)] = ((8 - i) * a0 + (i - 1) * a1) / 7;
        }
    } else {
        for i in 2..6u16 {
            a[usize::from(i)] = ((6 - i) * a0 + (i - 1) * a1) / 5;
        }
        a[7] = 255;
    }
    let lut = u64::from_le_bytes([src[2], src[3], src[4], src[5], src[6], src[7], 0, 0]);
    std::array::from_fn(|n| a[((lut >> (3 * n)) & 7) as usize] as u8)
}

/// Imagen RGBA de `data` en BC`n` (`n` = 1, 2, 3), o `None` si faltan bytes o `n` no vale.
/// Los bloques del borde se recortan si `width`/`height` no son múltiplos de 4.
pub(crate) fn decode(n: u8, width: u32, height: u32, data: &[u8]) -> Option<RgbaImage> {
    let block = match n {
        1 => 8,
        2 | 3 => 16,
        _ => return None,
    };
    let (bw, bh) = (width.div_ceil(4) as usize, height.div_ceil(4) as usize);
    if data.len() < bw.checked_mul(bh)?.checked_mul(block)? {
        return None;
    }
    let mut img = RgbaImage::new(width, height);
    for (i, src) in data.chunks_exact(block).take(bw * bh).enumerate() {
        let (alpha, color) = src.split_at(block - 8);
        let mut px = color_block(color.try_into().ok()?, n != 1);
        match n {
            2 => {
                for (k, p) in px.iter_mut().enumerate() {
                    let v = (alpha[k / 2] >> ((k % 2) * 4)) & 0xF;
                    p[3] = (v << 4) | v;
                }
            }
            3 => {
                let a = bc3_alpha(alpha.try_into().ok()?);
                for (p, v) in px.iter_mut().zip(a) {
                    p[3] = v;
                }
            }
            _ => {}
        }
        let (x0, y0) = ((i % bw) as u32 * 4, (i / bw) as u32 * 4);
        for (k, p) in px.into_iter().enumerate() {
            let (x, y) = (x0 + k as u32 % 4, y0 + k as u32 / 4);
            if x < width && y < height {
                img.put_pixel(x, y, image::Rgba(p));
            }
        }
    }
    Some(img)
}
