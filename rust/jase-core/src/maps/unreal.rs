//! Mapas de Unreal Tournament (UT99). Traducción de `src/jase/maps/unreal.py`.
use crate::paths::{child, resolve_game_dir};
use crate::pystr::casefold;
use std::path::Path;

/// Ruta de Maps/<map_name>.unr (sin distinguir mayúsculas), o `None`.
/// La caché (Cache/, archivos con nombre GUID) se ignora.
pub fn find_map_file(install_dir: Option<&str>, map_name: &str) -> Option<String> {
    if map_name.is_empty() || map_name.contains(['/', '\\']) {
        return None;
    }
    let game_dir = resolve_game_dir(install_dir, "Maps")?;
    let maps_dir = child(&game_dir, "Maps")?;
    let base: String = if casefold(map_name).ends_with(".unr") {
        let n = map_name.chars().count().saturating_sub(4);
        map_name.chars().take(n).collect()
    } else {
        map_name.to_owned()
    };
    child(&maps_dir, &format!("{base}.unr")).filter(|f| Path::new(f).is_file())
}

#[cfg(feature = "map-images")]
pub use screenshot::{
    MAX_DIMENSION, MAX_TABLE_COUNT, MAX_UNR_BYTES, PACKAGE_TAG, RF_HAS_STACK, extract_screenshot,
};

/// Captura (textura Screenshot) desde el paquete `.unr`.
#[cfg(feature = "map-images")]
mod screenshot {
    use super::super::bcn;
    use crate::pystr::{casefold, latin1};
    use image::RgbImage;
    use std::collections::HashMap;
    use std::io::Read;

    pub const PACKAGE_TAG: u32 = 0x9E2A_83C1;
    pub const MAX_DIMENSION: i32 = 2048;
    pub const MAX_TABLE_COUNT: usize = 1_000_000;
    pub const RF_HAS_STACK: u32 = 0x0200_0000;
    /// Python usa `mmap` sin tope; los mapas reales miden < 30 MB.
    pub const MAX_UNR_BYTES: u64 = 128 * 1024 * 1024;

    // Tipos de propiedad (EPropertyType) que interesan.
    const PROP_BYTE: u8 = 1;
    const PROP_INT: u8 = 2;
    const PROP_BOOL: u8 = 3;
    const PROP_OBJECT: u8 = 5;
    const PROP_STRUCT: u8 = 10;

    // ETextureFormat
    const TEXF_P8: i32 = 0;
    const TEXF_DXT1: i32 = 3;
    const TEXF_RGBA8: i32 = 5;

    /// Lectura secuencial acotada; `None` = formato inválido (la `_FormatError` de Python).
    struct Reader<'a> {
        data: &'a [u8],
        pos: usize,
    }

    impl<'a> Reader<'a> {
        fn at(data: &'a [u8], pos: usize) -> Self {
            Self { data, pos }
        }

        fn take(&mut self, n: usize) -> Option<&'a [u8]> {
            let chunk = self.data.get(self.pos..self.pos.checked_add(n)?)?;
            self.pos += n;
            Some(chunk)
        }

        fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
            self.take(N)?.try_into().ok()
        }

        fn u8(&mut self) -> Option<u8> {
            Some(self.array::<1>()?[0])
        }

        fn i32(&mut self) -> Option<i32> {
            self.array().map(i32::from_le_bytes)
        }

        fn u32(&mut self) -> Option<u32> {
            self.array().map(u32::from_le_bytes)
        }

        /// FCompactIndex: bit 7 signo, bit 6 continúa, luego grupos de 7 bits.
        fn compact(&mut self) -> Option<i64> {
            let first = self.u8()?;
            let mut value = i64::from(first & 0x3F);
            let mut more = first & 0x40 != 0;
            let mut shift = 6;
            let mut count = 1;
            while more {
                if count >= 5 {
                    return None;
                }
                let b = self.u8()?;
                value |= i64::from(b & 0x7F) << shift;
                shift += 7;
                more = b & 0x80 != 0;
                count += 1;
            }
            Some(if first & 0x80 != 0 { -value } else { value })
        }
    }

    struct Export {
        class_ref: i64,
        name: String,
        flags: u32,
        size: i64,
        offset: i64,
    }

    struct Package<'a> {
        data: &'a [u8],
        version: u32,
        names: Vec<String>,
        imports: Vec<String>,
        exports: Vec<Export>,
    }

    type Props<'a> = HashMap<String, (u8, &'a [u8])>;

    impl<'a> Package<'a> {
        fn parse(data: &'a [u8]) -> Option<Self> {
            let mut r = Reader::at(data, 0);
            if r.u32()? != PACKAGE_TAG {
                return None;
            }
            let version = r.u32()? & 0xFFFF;
            r.u32()?; // flags
            let mut table = || -> Option<(usize, usize)> {
                let count = r.u32()? as usize;
                let off = r.u32()? as usize;
                (count <= MAX_TABLE_COUNT).then_some((count, off))
            };
            let (name_count, name_off) = table()?;
            let (export_count, export_off) = table()?;
            let (import_count, import_off) = table()?;
            let mut pkg = Self {
                data,
                version,
                names: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
            };

            let mut r = Reader::at(data, name_off);
            for _ in 0..name_count {
                let raw = if version >= 64 {
                    let length = r.compact()?;
                    if length > 0 {
                        r.take(usize::try_from(length).ok()?)?
                    } else {
                        &[]
                    }
                } else {
                    let end = data.get(r.pos..)?.iter().position(|&b| b == 0)?;
                    r.take(end + 1)?
                };
                r.u32()?; // flags del nombre
                let len = raw.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                pkg.names.push(latin1(&raw[..len]));
            }

            let mut r = Reader::at(data, import_off);
            for _ in 0..import_count {
                r.compact()?; // class package
                r.compact()?; // class name
                r.i32()?; // package
                let name = pkg.name(r.compact()?)?.to_owned();
                pkg.imports.push(name);
            }

            let mut r = Reader::at(data, export_off);
            for _ in 0..export_count {
                let class_ref = r.compact()?;
                r.compact()?; // super
                r.i32()?; // package
                let name = pkg.name(r.compact()?)?.to_owned();
                let flags = r.u32()?;
                let size = r.compact()?;
                let offset = if size > 0 { r.compact()? } else { 0 };
                pkg.exports.push(Export {
                    class_ref,
                    name,
                    flags,
                    size,
                    offset,
                });
            }
            Some(pkg)
        }

        fn name(&self, index: i64) -> Option<&str> {
            self.names
                .get(usize::try_from(index).ok()?)
                .map(String::as_str)
        }

        fn class_name(&self, exp: &Export) -> &str {
            let r = exp.class_ref;
            // r < 0: import -r - 1; r > 0: export r - 1 (los compactos caben en ~34 bits)
            let import = usize::try_from(-r - 1)
                .ok()
                .and_then(|i| self.imports.get(i));
            let export = usize::try_from(r - 1)
                .ok()
                .and_then(|i| self.exports.get(i));
            match (import, export) {
                (Some(name), _) => name,
                (_, Some(e)) => &e.name,
                _ if r == 0 => "Class",
                _ => "",
            }
        }

        fn export(&self, r: i64) -> Option<&Export> {
            self.exports.get(usize::try_from(r.checked_sub(1)?).ok()?)
        }

        fn open_object(&self, exp: &Export) -> Option<Reader<'a>> {
            if exp.size <= 0 || exp.offset < 0 {
                return None;
            }
            let start = usize::try_from(exp.offset).ok()?;
            let end = start.checked_add(usize::try_from(exp.size).ok()?)?;
            let mut r = Reader::at(self.data.get(start..end)?, 0);
            if exp.flags & RF_HAS_STACK != 0 {
                let node = r.compact()?;
                r.compact()?; // state node
                r.take(8)?; // probe mask
                r.take(4)?; // latent action
                if node != 0 {
                    r.compact()?; // offset
                }
            }
            Some(r)
        }

        /// Propiedades etiquetadas (solo índice de arreglo 0) hasta "None".
        fn properties(&self, r: &mut Reader<'a>) -> Option<Props<'a>> {
            let mut props = Props::new();
            loop {
                let name = self.name(r.compact()?)?;
                if name == "None" {
                    return Some(props);
                }
                let info = r.u8()?;
                let ptype = info & 0x0F;
                let size_code = (info >> 4) & 0x07;
                let is_array = info & 0x80 != 0;
                if ptype == PROP_STRUCT {
                    r.compact()?; // nombre del struct
                }
                let size = match size_code {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 12,
                    4 => 16,
                    5 => usize::from(r.u8()?),
                    6 => usize::from(u16::from_le_bytes(r.array()?)),
                    _ => r.u32()? as usize,
                };
                let mut index = 0;
                if is_array && ptype != PROP_BOOL {
                    let b = u32::from(r.u8()?);
                    index = if b & 0x80 == 0 {
                        b
                    } else if b & 0xC0 == 0x80 {
                        ((b & 0x7F) << 8) | u32::from(r.u8()?)
                    } else {
                        let [x, y, z] = r.array()?;
                        ((b & 0x3F) << 24) | u32::from_be_bytes([0, x, y, z])
                    };
                }
                let mut payload = r.take(size)?;
                if ptype == PROP_BOOL {
                    payload = if is_array { &[1] } else { &[0] };
                }
                if index == 0 {
                    props.entry(name.to_owned()).or_insert((ptype, payload));
                }
            }
        }
    }

    fn object_ref(prop: Option<&(u8, &[u8])>) -> Option<i64> {
        match prop {
            Some(&(PROP_OBJECT, payload)) => Reader::at(payload, 0).compact(),
            _ => Some(0),
        }
    }

    fn screenshot_export<'p>(pkg: &'p Package) -> Option<&'p Export> {
        for exp in &pkg.exports {
            if pkg.class_name(exp) != "LevelInfo" {
                continue;
            }
            let found = (|| {
                let props = pkg.properties(&mut pkg.open_object(exp)?)?;
                let r = object_ref(props.get("Screenshot"))?;
                if r > 0 { pkg.export(r) } else { None }
            })();
            if found.is_some() {
                return found;
            }
        }
        pkg.exports
            .iter()
            .find(|e| casefold(&e.name) == "screenshot" && pkg.class_name(e) == "Texture")
    }

    fn int_prop(props: &Props, name: &str, default: i32) -> i32 {
        match props.get(name) {
            Some(&(PROP_BYTE, [b, ..])) => i32::from(*b),
            Some(&(PROP_INT, &[a, b, c, d])) => i32::from_le_bytes([a, b, c, d]),
            _ => default,
        }
    }

    /// Paleta RGB de 256 colores, rellenada en negro.
    fn palette(pkg: &Package, r: i64) -> Option<[[u8; 3]; 256]> {
        let exp = pkg.export(r)?;
        if pkg.class_name(exp) != "Palette" {
            return None;
        }
        let mut r = pkg.open_object(exp)?;
        pkg.properties(&mut r)?;
        let count = r.compact()?;
        if !(1..=256).contains(&count) {
            return None;
        }
        let rgba = r.take(count as usize * 4)?;
        let mut pal = [[0; 3]; 256];
        for (dst, src) in pal.iter_mut().zip(rgba.as_chunks::<4>().0) {
            dst.copy_from_slice(&src[..3]);
        }
        Some(pal)
    }

    fn decode_texture(pkg: &Package, exp: &Export) -> Option<RgbImage> {
        if pkg.class_name(exp) != "Texture" {
            return None;
        }
        let mut r = pkg.open_object(exp)?;
        let props = pkg.properties(&mut r)?;
        let fmt = int_prop(&props, "Format", TEXF_P8);
        if r.compact()? < 1 {
            return None; // textura sin mips
        }
        if pkg.version >= 63 {
            r.i32()?; // skip offset del lazy array
        }
        let size = usize::try_from(r.compact()?).ok()?;
        let data = r.take(size)?;
        let (width, height) = (r.i32()?, r.i32()?);
        if !(1..=MAX_DIMENSION).contains(&width) || !(1..=MAX_DIMENSION).contains(&height) {
            return None;
        }
        let (w, h) = (width as u32, height as u32);
        let pixels = (w * h) as usize; // ≤ 2048², sin desborde
        match fmt {
            TEXF_P8 => {
                let indices = data.get(..pixels)?;
                let pal = palette(pkg, object_ref(props.get("Palette"))?)?;
                let raw = indices.iter().flat_map(|&i| pal[usize::from(i)]).collect();
                RgbImage::from_raw(w, h, raw)
            }
            TEXF_DXT1 => {
                let img = bcn::decode(1, w, h, data)?;
                Some(image::DynamicImage::ImageRgba8(img).into_rgb8())
            }
            TEXF_RGBA8 => {
                let bgra = data.get(..pixels * 4)?;
                let raw = bgra
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|p| [p[2], p[1], p[0]])
                    .collect();
                RgbImage::from_raw(w, h, raw)
            }
            _ => None, // formato de textura no soportado
        }
    }

    /// Imagen RGB de la propiedad Screenshot del LevelInfo del mapa (si no, la `Texture`
    /// llamada `screenshot`), o `None` ante cualquier problema de formato o de E/S.
    pub fn extract_screenshot(unr_path: &str) -> Option<RgbImage> {
        let file = std::fs::File::open(unr_path).ok()?;
        let meta = file.metadata().ok()?;
        if !meta.is_file() || meta.len() > MAX_UNR_BYTES {
            return None;
        }
        let mut data = Vec::new();
        // el archivo puede crecer tras el chequeo: nunca más del tope
        file.take(MAX_UNR_BYTES + 1).read_to_end(&mut data).ok()?;
        if data.len() as u64 > MAX_UNR_BYTES {
            return None;
        }
        let pkg = Package::parse(&data)?;
        decode_texture(&pkg, screenshot_export(&pkg)?)
    }
}
