//! Decodificador de texturas IWI (versión 5: CoD1/CoD2). Traducción de `src/jase/maps/iwi.py`.
//!
//! Cabecera (28 bytes, little-endian): `"IWi"`, versión (5), formato, uso, ancho u16, alto u16,
//! profundidad u16 y el fin de cada mip (u32 × 4; el primero, del mip más grande). Los mips van
//! del más chico al más grande: el principal ocupa los últimos bytes antes del primer fin.
use super::bcn;
use image::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

const HEADER: usize = 28;
pub const MAX_SIDE: u16 = 8192;

pub const FMT_ARGB32: u8 = 0x01;
pub const FMT_RGB24: u8 = 0x02;
pub const FMT_LA88: u8 = 0x03;
pub const FMT_A8: u8 = 0x04;
pub const FMT_DXT1: u8 = 0x0B;
pub const FMT_DXT3: u8 = 0x0C;
pub const FMT_DXT5: u8 = 0x0D;

/// El archivo IWI está dañado o usa un formato no soportado.
#[derive(Debug, thiserror::Error)]
pub enum IwiError {
    #[error("IWI demasiado corto")]
    Short,
    #[error("no es un archivo IWI")]
    NotIwi,
    #[error("versión de IWI no soportada: {0}")]
    Version(u8),
    #[error("dimensiones inválidas: {0}x{1}")]
    Dimensions(u16, u16),
    #[error("formato de IWI desconocido: 0x{0:02X}")]
    Format(u8),
    #[error("IWI truncado")]
    Truncated,
}

/// Imagen del mip más grande del IWI.
pub fn decode_iwi(data: &[u8]) -> Result<DynamicImage, IwiError> {
    let head: &[u8; HEADER] = data
        .get(..HEADER)
        .and_then(|h| h.try_into().ok())
        .ok_or(IwiError::Short)?;
    if &head[..3] != b"IWi" {
        return Err(IwiError::NotIwi);
    }
    if head[3] != 5 {
        return Err(IwiError::Version(head[3]));
    }
    let fmt = head[4];
    let width = u16::from_le_bytes([head[6], head[7]]);
    let height = u16::from_le_bytes([head[8], head[9]]);
    let end0 = u32::from_le_bytes([head[12], head[13], head[14], head[15]]) as usize;
    if !(1..=MAX_SIDE).contains(&width) || !(1..=MAX_SIDE).contains(&height) {
        return Err(IwiError::Dimensions(width, height));
    }
    let (w, h) = (usize::from(width), usize::from(height));
    // con lados ≤ 8192 los productos caben de sobra en usize
    let size = match fmt {
        FMT_DXT1 => w.div_ceil(4) * h.div_ceil(4) * 8,
        FMT_DXT3 | FMT_DXT5 => w.div_ceil(4) * h.div_ceil(4) * 16,
        FMT_ARGB32 => w * h * 4,
        FMT_RGB24 => w * h * 3,
        FMT_LA88 => w * h * 2,
        FMT_A8 => w * h,
        _ => return Err(IwiError::Format(fmt)),
    };
    let end = if (HEADER + size..=data.len()).contains(&end0) {
        end0
    } else {
        data.len()
    };
    let start = end
        .checked_sub(size)
        .filter(|&s| s >= HEADER)
        .ok_or(IwiError::Truncated)?;
    let payload = &data[start..end];
    let (w, h) = (u32::from(width), u32::from(height));
    let img = match fmt {
        FMT_DXT1 => bcn::decode(1, w, h, payload).map(DynamicImage::ImageRgba8),
        FMT_DXT3 => bcn::decode(2, w, h, payload).map(DynamicImage::ImageRgba8),
        FMT_DXT5 => bcn::decode(3, w, h, payload).map(DynamicImage::ImageRgba8),
        FMT_ARGB32 => {
            RgbaImage::from_raw_bgra(w, h, payload.to_vec()).map(DynamicImage::ImageRgba8)
        }
        FMT_RGB24 => RgbImage::from_raw_bgr(w, h, payload.to_vec()).map(DynamicImage::ImageRgb8),
        FMT_LA88 => GrayAlphaImage::from_raw(w, h, payload.to_vec()).map(DynamicImage::ImageLumaA8),
        _ => GrayImage::from_raw(w, h, payload.to_vec()).map(DynamicImage::ImageLuma8),
    };
    // no pasa: el tamaño del payload ya es el exacto
    img.ok_or(IwiError::Truncated)
}
