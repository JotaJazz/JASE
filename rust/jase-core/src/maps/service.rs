//! Nombres seguros para la caché y URL de GameTracker. Traducción de
//! `src/jase/maps/service.py`.
use crate::pystr::casefold;

pub const GAMETRACKER_URL: &str =
    "https://image.gametracker.com/images/maps/160x120/{gt_id}/{map}.jpg";

const WINDOWS_RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Nombre de archivo (sin extensión) válido en Windows para `map_name`, o `None`.
pub fn safe_map_filename(map_name: &str) -> Option<String> {
    let kept: String = map_name
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'..='\x1f'
            )
        })
        .collect();
    let mut name = casefold(kept.trim_matches([' ', '.']));
    if name.is_empty() {
        return None;
    }
    if WINDOWS_RESERVED.contains(&name.as_str()) {
        name.push('_');
    }
    Some(name)
}

/// GameTracker guarda los mapas en minúsculas y con `_` en todo carácter que no sea
/// alfanumérico (MOHAA «obj/obj_team2», UT99 «DM-Deck16][»).
pub fn gametracker_map_name(map_name: &str) -> String {
    casefold(map_name)
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn gametracker_url(gt_id: &str, map_name: &str) -> String {
    GAMETRACKER_URL
        .replace("{gt_id}", gt_id)
        .replace("{map}", &gametracker_map_name(map_name))
}

#[cfg(feature = "map-images")]
pub use images::{
    COD_LOADSCREEN_SIZE, Fetch, LocalSource, MAX_CACHED_SIZE, MAX_DECODE_ALLOC, MAX_IMAGE_SIDE,
    MapImageService, decode_image, default_fetch, extract_local, store, thumbnail_size,
};

/// Imagen de un mapa: caché en disco → archivos del juego → GameTracker (opcional).
#[cfg(feature = "map-images")]
mod images {
    use super::{gametracker_url, safe_map_filename};
    use crate::fsutil::atomic_write;
    use crate::games::{Family, GameDef, LaunchStyle};
    use crate::maps::cod::find_loadscreen;
    use crate::maps::iwi::decode_iwi;
    use crate::maps::levelshots::find_levelshot;
    use crate::maps::unreal::{extract_screenshot, find_map_file};
    use crate::net::{FETCH_DEADLINE, MAX_DOWNLOAD_BYTES, fetch_blocking};
    use crate::paths::version_dir;
    use image::imageops::{self, FilterType};
    use image::{ImageFormat, ImageReader, Limits, RgbImage};
    use std::collections::HashSet;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// Tamaño máximo guardado en caché (la ficha lo muestra a 160×120; margen para HiDPI).
    pub const MAX_CACHED_SIZE: (u32, u32) = (480, 360);
    /// CoD estira la pantalla de carga (cuadrada) a toda la pantalla: se guarda en 4:3.
    pub const COD_LOADSCREEN_SIZE: (u32, u32) = (480, 360);
    /// Topes al decodificar datos de afuera (una bomba de descompresión no reserva más).
    pub const MAX_IMAGE_SIDE: u32 = 8192;
    pub const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

    pub type Fetch = Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;
    pub type LocalSource =
        Box<dyn Fn(&GameDef, Option<&str>, &str) -> Option<RgbImage> + Send + Sync>;

    pub fn default_fetch(url: &str) -> Option<Vec<u8>> {
        fetch_blocking(url, MAX_DOWNLOAD_BYTES, FETCH_DEADLINE, &[])
    }

    /// JPEG o PNG por la firma; si no, TGA (no tiene firma). RGB sin alfa, como
    /// `convert("RGB")` de Pillow.
    pub fn decode_image(data: &[u8]) -> Option<RgbImage> {
        let mut reader = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .ok()?;
        if !matches!(reader.format(), Some(ImageFormat::Jpeg | ImageFormat::Png)) {
            reader.set_format(ImageFormat::Tga);
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_IMAGE_SIDE);
        limits.max_image_height = Some(MAX_IMAGE_SIDE);
        limits.max_alloc = Some(MAX_DECODE_ALLOC);
        reader.limits(limits);
        Some(reader.decode().ok()?.to_rgb8())
    }

    /// Tamaño de `Image.thumbnail(max)` de Pillow: solo achica y conserva el aspecto.
    pub fn thumbnail_size(width: u32, height: u32, max: (u32, u32)) -> (u32, u32) {
        let (x, y) = max;
        if x >= width && y >= height {
            return (width, height);
        }
        // round_aspect: floor o ceil, el de menor error (empate: floor), al menos 1
        let round_aspect = |n: f64, key: &dyn Fn(f64) -> f64| {
            let (lo, hi) = (n.floor(), n.ceil());
            let best = if key(hi) < key(lo) { hi } else { lo };
            (best as u32).max(1)
        };
        let aspect = f64::from(width) / f64::from(height);
        let (fx, fy) = (f64::from(x), f64::from(y));
        if fx / fy >= aspect {
            (round_aspect(fy * aspect, &|n| (aspect - n / fy).abs()), y)
        } else {
            let key = |n: f64| {
                if n == 0.0 {
                    0.0
                } else {
                    (aspect - fx / n).abs()
                }
            };
            (x, round_aspect(fx / aspect, &key))
        }
    }

    /// Imagen del mapa desde los archivos del juego, o `None`.
    pub fn extract_local(
        game: &GameDef,
        install_dir: Option<&str>,
        map_name: &str,
    ) -> Option<RgbImage> {
        let install_dir = install_dir.filter(|d| !d.is_empty())?;
        if !game.levelshot_dir.is_empty() {
            let data = find_levelshot(Some(install_dir), game.levelshot_dir, map_name)?;
            return decode_image(&data);
        }
        if game.cod_loadscreens {
            let data = find_loadscreen(Some(install_dir), map_name)?;
            let img = decode_iwi(&data).ok()?.to_rgb8();
            let (w, h) = COD_LOADSCREEN_SIZE;
            return Some(imageops::resize(&img, w, h, FilterType::Lanczos3));
        }
        if game.family == Family::Gamespy && game.launch_style == LaunchStyle::Unreal {
            let copies: Vec<String> = game
                .version_dirs
                .iter()
                .filter_map(|(_, folder)| version_dir(Some(install_dir), folder))
                .collect();
            let folders = if copies.is_empty() {
                vec![install_dir.to_owned()]
            } else {
                copies
            };
            let unr = folders
                .iter()
                .find_map(|folder| find_map_file(Some(folder), map_name))?;
            return extract_screenshot(&unr);
        }
        None
    }

    fn encode_png(img: &RgbImage) -> Option<Vec<u8>> {
        if img.width() == 0 || img.height() == 0 {
            return None;
        }
        let (w, h) = thumbnail_size(img.width(), img.height(), MAX_CACHED_SIZE);
        let small;
        let img = if (w, h) == img.dimensions() {
            img
        } else {
            small = imageops::resize(img, w, h, FilterType::Lanczos3);
            &small
        };
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).ok()?;
        Some(buf.into_inner())
    }

    /// Guarda como PNG (miniatura) de forma atómica; `None` si no se puede escribir.
    pub fn store(img: &RgbImage, path: &Path) -> Option<PathBuf> {
        let written = encode_png(img).is_some_and(|png| atomic_write(path, &png).is_ok());
        // Windows: el rename falla si el destino está abierto (otro hilo ya lo guardó y
        // se está leyendo); ese archivo sirve igual.
        (written || path.is_file()).then(|| path.to_path_buf())
    }

    /// Ruta a un PNG con la imagen del mapa, o `None`.
    ///
    /// `get` es bloqueante (lee archivos del juego y puede descargar): llamarlo desde un hilo
    /// de trabajo; es seguro desde varios a la vez. Los «no encontrado» se recuerdan solo en
    /// memoria durante la sesión, para que un mapa instalado después aparezca al reiniciar.
    pub struct MapImageService {
        cache_dir: PathBuf,
        fetch: Fetch,
        local: LocalSource,
        local_misses: Mutex<HashSet<(String, String, String)>>,
        download_misses: Mutex<HashSet<(String, String)>>,
    }

    impl MapImageService {
        pub fn new(cache_dir: PathBuf) -> Self {
            Self::with_sources(cache_dir, Box::new(default_fetch), Box::new(extract_local))
        }

        pub fn with_sources(cache_dir: PathBuf, fetch: Fetch, local: LocalSource) -> Self {
            Self {
                cache_dir,
                fetch,
                local,
                local_misses: Mutex::default(),
                download_misses: Mutex::default(),
            }
        }

        pub fn cache_path(&self, game: &GameDef, map_name: &str) -> Option<PathBuf> {
            let name = safe_map_filename(map_name)?;
            Some(self.cache_dir.join(game.id).join(format!("{name}.png")))
        }

        pub fn get(
            &self,
            game: &GameDef,
            install_dir: Option<&str>,
            map_name: &str,
            allow_download: bool,
        ) -> Option<PathBuf> {
            let path = self.cache_path(game, map_name)?;
            if path.is_file() {
                return Some(path);
            }
            let key = safe_map_filename(map_name)?; // no vacío: cache_path ya lo validó
            let local_key = (
                game.id.to_owned(),
                install_dir.unwrap_or("").to_owned(),
                key.clone(),
            );
            // los candados nunca se tienen durante la E/S
            if !self.local_misses.lock().unwrap().contains(&local_key) {
                if let Some(img) = (self.local)(game, install_dir, map_name) {
                    return store(&img, &path);
                }
                self.local_misses.lock().unwrap().insert(local_key);
            }
            if !allow_download || game.gametracker_id.is_empty() {
                return None;
            }
            let download_key = (game.id.to_owned(), key);
            if self.download_misses.lock().unwrap().contains(&download_key) {
                return None;
            }
            let url = gametracker_url(game.gametracker_id, map_name);
            if let Some(img) = (self.fetch)(&url).and_then(|data| decode_image(&data)) {
                return store(&img, &path);
            }
            self.download_misses.lock().unwrap().insert(download_key);
            None
        }
    }
}
