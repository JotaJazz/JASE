//! Port de tests/test_maps_service.py (servicio, caché y descargas) y de los tests del servicio
//! de test_maps_levelshots.py, test_games_mohaa.py y test_games_tacops.py. Solo con la feature
//! `map-images`; el `fetch` es siempre falso (sin Internet).
#![cfg(feature = "map-images")]
mod support;

use image::{ImageFormat, RgbImage};
use jase_core::games::{GameDef, game};
use jase_core::maps::service::{
    COD_LOADSCREEN_SIZE, Fetch, LocalSource, MAX_CACHED_SIZE, MapImageService, decode_image,
    extract_local, thumbnail_size,
};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use support::python::python;
use support::unreal::make_map;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// `base` más `parts`, componente por componente (sirve igual en Windows).
fn rel(base: &Path, parts: &[&str]) -> PathBuf {
    parts.iter().fold(base.to_path_buf(), |p, s| p.join(s))
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn cod2() -> &'static GameDef {
    game("cod2").unwrap()
}

fn encoded(color: [u8; 3], size: (u32, u32), fmt: ImageFormat) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    RgbImage::from_pixel(size.0, size.1, image::Rgb(color))
        .write_to(&mut buf, fmt)
        .unwrap();
    buf.into_inner()
}

fn png_bytes() -> Vec<u8> {
    encoded([255, 0, 0], (8, 8), ImageFormat::Png)
}

fn load(path: &Path) -> RgbImage {
    let data = fs::read(path).unwrap();
    assert!(data.starts_with(b"\x89PNG"), "no es PNG");
    image::load_from_memory(&data).unwrap().to_rgb8()
}

fn make_zip(path: PathBuf, entries: &[(&str, &[u8])]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        z.start_file(*name, SimpleFileOptions::default()).unwrap();
        z.write_all(data).unwrap();
    }
    fs::write(path, z.finish().unwrap().into_inner()).unwrap();
}

/// TGA sin compresión (sin firma), 24 bits BGR, origen arriba a la izquierda.
fn tga(w: u16, h: u16, bgr: [u8; 3]) -> Vec<u8> {
    let mut out = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    out.extend(w.to_le_bytes());
    out.extend(h.to_le_bytes());
    out.extend([24, 0x20]);
    out.extend(bgr.repeat(usize::from(w) * usize::from(h)));
    out
}

/// IWI v5 ARGB32 (BGRA) de un solo mip.
fn iwi_argb32(w: u16, h: u16, bgra: [u8; 4]) -> Vec<u8> {
    let pixels = bgra.repeat(usize::from(w) * usize::from(h));
    let end = 28 + pixels.len() as u32;
    let mut out = b"IWi".to_vec();
    out.extend([5, 0x01, 0]);
    for v in [w, h, 1] {
        out.extend(v.to_le_bytes());
    }
    for _ in 0..4 {
        out.extend(end.to_le_bytes());
    }
    out.extend(pixels);
    out
}

/// `Fetch` falso que guarda las URLs pedidas.
fn fake_fetch(result: Option<Vec<u8>>) -> (Fetch, Arc<Mutex<Vec<String>>>) {
    let urls = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&urls);
    let fetch: Fetch = Box::new(move |url: &str| {
        seen.lock().unwrap().push(url.to_owned());
        result.clone()
    });
    (fetch, urls)
}

type Calls = Arc<Mutex<Vec<(Option<String>, String)>>>;

/// Extractor de CoD falso con registro de llamadas; `images` = mapa → imagen.
fn local_cod(images: Vec<(&str, RgbImage)>) -> (LocalSource, Calls) {
    let calls: Calls = Arc::default();
    let seen = Arc::clone(&calls);
    let images: Vec<(String, RgbImage)> =
        images.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    let local: LocalSource = Box::new(move |_game, install_dir, map_name| {
        seen.lock()
            .unwrap()
            .push((install_dir.map(str::to_owned), map_name.to_owned()));
        images
            .iter()
            .find(|(k, _)| k == map_name)
            .map(|(_, img)| img.clone())
    });
    (local, calls)
}

fn flat(w: u32, h: u32, color: [u8; 3]) -> RgbImage {
    RgbImage::from_pixel(w, h, image::Rgb(color))
}

fn no_local() -> LocalSource {
    Box::new(|_, _, _| None)
}

fn no_fetch() -> Fetch {
    fake_fetch(None).0
}

#[test]
fn local_extraction_is_cached_as_png() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let (local, calls) = local_cod(vec![("mp_toujane", flat(16, 16, [0, 255, 0]))]);
    let svc = MapImageService::with_sources(cache.clone(), no_fetch(), local);
    let path = svc.get(cod2(), Some("D:/CoD2"), "mp_toujane", false);
    let expected = rel(&cache, &["cod2", "mp_toujane.png"]);
    assert_eq!(path.as_deref(), Some(expected.as_path()));
    load(&expected);
    assert_eq!(calls.lock().unwrap().len(), 1);
    // Caché positiva en disco: ni otra instancia vuelve a extraer.
    let (local2, calls2) = local_cod(vec![]);
    let svc2 = MapImageService::with_sources(cache.clone(), no_fetch(), local2);
    assert_eq!(
        svc2.get(cod2(), Some("D:/CoD2"), "mp_toujane", false),
        Some(expected.clone())
    );
    assert_eq!(
        svc.get(cod2(), Some("D:/CoD2"), "mp_toujane", false),
        Some(expected)
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(calls2.lock().unwrap().is_empty());
}

#[test]
fn cod_loadscreen_is_stored_as_4_3() {
    // Las pantallas de carga son cuadradas pero el juego las estira a la pantalla (4:3).
    let t = tempfile::tempdir().unwrap();
    let inst = t.path().join("cod2");
    make_zip(
        rel(&inst, &["main", "iw_00.iwd"]),
        &[(
            "images/loadscreen_mp_x.iwi",
            &iwi_argb32(512, 512, [3, 2, 1, 255]),
        )],
    );
    let svc =
        MapImageService::with_sources(t.path().join("cache"), no_fetch(), Box::new(extract_local));
    let path = svc.get(cod2(), Some(s(&inst)), "mp_x", false).unwrap();
    let (w, h) = load(&path).dimensions();
    assert!(w * 3 == h * 4 && w <= 640);
}

#[test]
fn local_has_priority_over_download() {
    let t = tempfile::tempdir().unwrap();
    let (local, _) = local_cod(vec![("mp_toujane", flat(4, 4, [0, 0, 255]))]);
    let (fetch, urls) = fake_fetch(Some(png_bytes()));
    let svc = MapImageService::with_sources(t.path().into(), fetch, local);
    let path = svc.get(cod2(), Some("d"), "mp_toujane", true).unwrap();
    assert!(urls.lock().unwrap().is_empty());
    assert_eq!(load(&path).get_pixel(0, 0).0, [0, 0, 255]);
}

#[test]
fn download_only_when_allowed() {
    let t = tempfile::tempdir().unwrap();
    let (local, _) = local_cod(vec![]);
    let jpeg = encoded([10, 20, 30], (8, 8), ImageFormat::Jpeg);
    let (fetch, urls) = fake_fetch(Some(jpeg));
    let svc = MapImageService::with_sources(t.path().into(), fetch, local);
    assert_eq!(svc.get(cod2(), Some("d"), "mp_custom", false), None);
    assert!(urls.lock().unwrap().is_empty());
    let path = svc.get(cod2(), Some("d"), "mp_custom", true);
    assert_eq!(
        *urls.lock().unwrap(),
        ["https://image.gametracker.com/images/maps/160x120/cod2/mp_custom.jpg"]
    );
    let expected = rel(t.path(), &["cod2", "mp_custom.png"]);
    assert_eq!(path.as_deref(), Some(expected.as_path()));
    load(&expected);
}

#[test]
fn no_download_without_gametracker_id() {
    let t = tempfile::tempdir().unwrap();
    let (fetch, urls) = fake_fetch(Some(png_bytes()));
    let svc = MapImageService::with_sources(t.path().into(), fetch, no_local());
    let jka = game("jka").unwrap();
    assert_eq!(svc.get(jka, Some("d"), "ffa_bespin", true), None);
    assert!(urls.lock().unwrap().is_empty());
}

#[test]
fn download_failure_is_remembered_in_memory() {
    // None = 404, error de red o el OSError de Python (el fetch real devuelve None)
    for result in [None, Some(b"<html>not an image</html>".to_vec())] {
        let t = tempfile::tempdir().unwrap();
        let (local, calls) = local_cod(vec![]);
        let (fetch, urls) = fake_fetch(result.clone());
        let svc = MapImageService::with_sources(t.path().into(), fetch, local);
        assert_eq!(svc.get(cod2(), Some("d"), "mp_missing", true), None);
        assert_eq!(svc.get(cod2(), Some("d"), "mp_missing", true), None);
        assert_eq!(urls.lock().unwrap().len(), 1);
        assert_eq!(calls.lock().unwrap().len(), 1); // la extracción local fallida también
        // Nada se guarda en disco: otra sesión vuelve a intentarlo.
        assert!(!rel(t.path(), &["cod2", "mp_missing.png"]).exists());
        let (fetch2, urls2) = fake_fetch(result);
        let svc2 = MapImageService::with_sources(t.path().into(), fetch2, no_local());
        assert_eq!(svc2.get(cod2(), Some("d"), "mp_missing", true), None);
        assert_eq!(urls2.lock().unwrap().len(), 1);
    }
}

#[test]
fn local_miss_then_enabling_download_tries_download() {
    let t = tempfile::tempdir().unwrap();
    let (fetch, urls) = fake_fetch(Some(png_bytes()));
    let svc = MapImageService::with_sources(t.path().into(), fetch, local_cod(vec![]).0);
    assert_eq!(svc.get(cod2(), Some("d"), "mp_late", false), None);
    assert!(svc.get(cod2(), Some("d"), "mp_late", true).is_some());
    assert_eq!(urls.lock().unwrap().len(), 1);
}

#[test]
fn changed_install_dir_retries_local() {
    let t = tempfile::tempdir().unwrap();
    let (local, calls) = local_cod(vec![]);
    let svc = MapImageService::with_sources(t.path().into(), no_fetch(), local);
    svc.get(cod2(), Some("a"), "mp_x", false);
    svc.get(cod2(), Some("b"), "mp_x", false);
    let dirs: Vec<_> = calls.lock().unwrap().iter().map(|c| c.0.clone()).collect();
    assert_eq!(dirs, [Some("a".to_owned()), Some("b".to_owned())]);
}

#[test]
fn corrupt_local_image_gives_none() {
    let t = tempfile::tempdir().unwrap();
    let inst = t.path().join("cod2");
    make_zip(
        rel(&inst, &["main", "iw_00.iwd"]),
        &[("images/loadscreen_mp_x.iwi", b"garbage")],
    );
    let svc =
        MapImageService::with_sources(t.path().join("cache"), no_fetch(), Box::new(extract_local));
    assert_eq!(svc.get(cod2(), Some(s(&inst)), "mp_x", false), None); // IwiError real
}

#[test]
fn real_cod_loadscreen_end_to_end() {
    let t = tempfile::tempdir().unwrap();
    let inst = t.path().join("cod2");
    make_zip(
        rel(&inst, &["main", "iw_00.iwd"]),
        &[(
            "images/loadscreen_mp_x.iwi",
            &iwi_argb32(4, 4, [30, 20, 10, 255]),
        )],
    );
    let svc = MapImageService::new(t.path().join("cache"));
    let path = svc.get(cod2(), Some(s(&inst)), "mp_x", false).unwrap();
    let img = load(&path);
    assert_eq!(img.dimensions(), COD_LOADSCREEN_SIZE);
    assert!(img.pixels().all(|p| p.0 == [10, 20, 30]));
}

#[test]
fn ut99_uses_unreal_screenshot() {
    let t = tempfile::tempdir().unwrap();
    let inst = t.path().join("ut");
    fs::create_dir_all(inst.join("Maps")).unwrap();
    fs::write(
        rel(&inst, &["Maps", "DM-Deck16][.unr"]),
        make_map(true, "Shot"),
    )
    .unwrap();
    let cache = t.path().join("cache");
    let svc = MapImageService::with_sources(cache.clone(), no_fetch(), Box::new(extract_local));
    let path = svc
        .get(game("ut99").unwrap(), Some(s(&inst)), "DM-Deck16][", false)
        .unwrap();
    assert_eq!(path.parent(), Some(cache.join("ut99").as_path()));
    assert_eq!(load(&path).dimensions(), (2, 2));
}

#[test]
fn unsafe_map_name_gives_valid_filename() {
    let t = tempfile::tempdir().unwrap();
    let local: LocalSource = Box::new(|_, _, _| Some(flat(2, 2, [0, 0, 0])));
    let svc = MapImageService::with_sources(t.path().into(), no_fetch(), local);
    let path = svc
        .get(
            game("ut99").unwrap(),
            Some("d"),
            "DM-a][b:c/d\\e*?\"<>|",
            false,
        )
        .unwrap();
    assert!(path.exists());
    assert_eq!(path.parent(), Some(t.path().join("ut99").as_path()));
    assert_eq!(path.file_name().unwrap(), "dm-a][bcde.png");
}

#[test]
fn too_long_map_name_gives_none() {
    // safe_map_filename no tiene tope de largo: más de 255 bytes no se puede escribir
    let t = tempfile::tempdir().unwrap();
    let local: LocalSource = Box::new(|_, _, _| Some(flat(2, 2, [0, 0, 0])));
    let svc = MapImageService::with_sources(t.path().into(), no_fetch(), local);
    let name = "a".repeat(300);
    assert_eq!(
        svc.get(game("ut99").unwrap(), Some("d"), &name, false),
        None
    );
}

#[test]
fn empty_map_name_gives_none() {
    let t = tempfile::tempdir().unwrap();
    let (fetch, urls) = fake_fetch(Some(png_bytes()));
    let svc = MapImageService::with_sources(t.path().into(), fetch, no_local());
    assert_eq!(svc.get(cod2(), Some("d"), "", true), None);
    assert!(urls.lock().unwrap().is_empty());
    assert_eq!(svc.cache_path(cod2(), ""), None);
}

#[test]
fn unwritable_cache_gives_none() {
    let t = tempfile::tempdir().unwrap();
    let blocker = t.path().join("file");
    fs::write(&blocker, "no soy carpeta").unwrap();
    let (local, _) = local_cod(vec![("mp_x", flat(4, 4, [0, 0, 0]))]);
    let svc = MapImageService::with_sources(blocker, no_fetch(), local);
    assert_eq!(svc.get(cod2(), Some("d"), "mp_x", false), None);
}

#[cfg(unix)]
#[test]
fn store_returns_existing_file_when_replace_fails() {
    use jase_core::maps::service::store;
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("cod2");
    fs::create_dir(&dir).unwrap();
    let path = dir.join("mp_x.png");
    fs::write(&path, png_bytes()).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(dir.join("probe"), b"").is_ok() {
        eprintln!("corriendo como root: se salta");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let img = flat(4, 4, [0, 0, 0]);
    assert_eq!(store(&img, &path), Some(path.clone()));
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["mp_x.png"]); // sin temporales
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_file(&path).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    assert_eq!(store(&img, &path), None);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn service_uses_levelshots_for_oa() {
    let t = tempfile::tempdir().unwrap();
    let inst = t.path().join("oa");
    let jpeg = encoded([255, 0, 0], (8, 8), ImageFormat::Jpeg);
    make_zip(
        rel(&inst, &["baseoa", "pak0.pk3"]),
        &[("levelshots/oa_dm1.jpg", &jpeg)],
    );
    let svc = MapImageService::new(t.path().join("cache"));
    let path = svc
        .get(game("oa").unwrap(), Some(s(&inst)), "oa_dm1", false)
        .unwrap();
    assert_eq!(path.extension().unwrap(), "png");
    load(&path);
}

#[test]
fn tga_levelshot_decodes() {
    let t = tempfile::tempdir().unwrap();
    let inst = t.path().join("oa");
    make_zip(
        rel(&inst, &["baseoa", "pak0.pk3"]),
        &[("levelshots/oa_dm2.tga", &tga(3, 2, [30, 20, 10]))],
    );
    let img = extract_local(game("oa").unwrap(), Some(s(&inst)), "oa_dm2").unwrap();
    assert_eq!(img.dimensions(), (3, 2));
    assert!(img.pixels().all(|p| p.0 == [10, 20, 30]));
}

#[test]
fn mohaa_never_uses_unreal_or_cod_extractor() {
    let t = tempfile::tempdir().unwrap();
    // un .unr válido: daría imagen si MOHAA usara el extractor de Unreal
    fs::create_dir_all(t.path().join("Maps")).unwrap();
    fs::write(rel(t.path(), &["Maps", "mohdm6.unr"]), make_map(true, "S")).unwrap();
    let (fetch, urls) = fake_fetch(None);
    let svc = MapImageService::with_sources(t.path().join("cache"), fetch, Box::new(extract_local));
    let mohaa = game("mohaa").unwrap();
    assert_eq!(extract_local(mohaa, Some(s(t.path())), "mohdm6"), None);
    assert_eq!(svc.get(mohaa, Some(s(t.path())), "dm/mohdm6", true), None);
    assert_eq!(
        *urls.lock().unwrap(),
        ["https://image.gametracker.com/images/maps/160x120/mohaa/dm_mohdm6.jpg"]
    );
}

#[test]
fn local_map_image_looks_in_both_installs() {
    let t = tempfile::tempdir().unwrap();
    for (folder, name, data) in [
        ("TO340", "TO-Other.unr", b"basura".to_vec()),
        ("TO350", "TO-Scope.unr", make_map(true, "Shot")),
    ] {
        fs::create_dir_all(rel(t.path(), &[folder, "Maps"])).unwrap();
        fs::write(rel(t.path(), &[folder, "Maps", name]), data).unwrap();
    }
    let img = extract_local(game("tacops").unwrap(), Some(s(t.path())), "TO-Scope");
    assert_eq!(img.map(|i| i.dimensions()), Some((2, 2)));
}

const SIZES: [(u32, u32); 15] = [
    (1, 1),
    (8, 8),
    (480, 360),
    (481, 361),
    (1024, 1024),
    (1, 5000),
    (5000, 1),
    (640, 480),
    (800, 600),
    (1920, 1080),
    (333, 777),
    (1000, 3),
    (479, 1000),
    (512, 360),
    (7, 9999),
];

#[test]
fn thumbnail_size_matches_pillow() {
    let Some(mut cmd) = python() else { return };
    let code = format!(
        "from PIL import Image\nfor w, h in {SIZES:?}:\n    im = Image.new('L', (w, h))\n    \
         im.thumbnail({MAX_CACHED_SIZE:?})\n    print(*im.size)\n"
    );
    let out = cmd.args(["-c", &code]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let want = String::from_utf8(out.stdout).unwrap();
    let got: String = SIZES
        .iter()
        .map(|&(w, h)| {
            let (a, b) = thumbnail_size(w, h, MAX_CACHED_SIZE);
            format!("{a} {b}\n")
        })
        .collect();
    assert_eq!(got, want);
}

/// Formatos reconocidos que no son JPEG ni PNG no se decodifican como TGA.
#[test]
fn bmp_is_none() {
    let mut data = b"BM".to_vec();
    data.extend(58u32.to_le_bytes()); // tamaño del archivo
    data.extend([0; 4]);
    data.extend(54u32.to_le_bytes()); // offset de los píxeles
    data.extend(40u32.to_le_bytes()); // BITMAPINFOHEADER
    data.extend(1i32.to_le_bytes());
    data.extend(1i32.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(24u16.to_le_bytes());
    data.extend([0; 24]);
    data.extend([0, 0, 255, 0]); // un píxel rojo y relleno a 4 bytes
    assert_eq!(decode_image(&data), None);
    // firma de AVIF que también es un TGA válido de 1×1: tampoco se decodifica como TGA
    let mut data = vec![0, 0, 2, 0];
    data.extend(b"ftypavif");
    data.extend([1, 0, 1, 0, 24, 0, 0, 0, 255]);
    assert_eq!(decode_image(&data), None);
}

#[test]
fn huge_image_is_rejected() {
    let mut data = tga(1, 1, [0, 0, 0]);
    data[12..16].copy_from_slice(&[0xFF; 4]); // 65535×65535
    let start = Instant::now();
    assert_eq!(decode_image(&data), None);
    let t = tempfile::tempdir().unwrap();
    let svc = MapImageService::with_sources(t.path().into(), fake_fetch(Some(data)).0, no_local());
    assert_eq!(svc.get(cod2(), Some("d"), "mp_huge", true), None);
    assert!(start.elapsed().as_secs_f64() < 0.5);
}

#[test]
fn service_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MapImageService>();
}

#[test]
fn concurrent_gets_store_one_file() {
    let t = tempfile::tempdir().unwrap();
    let local: LocalSource = Box::new(|_, _, _| Some(flat(64, 64, [5, 6, 7])));
    let svc = MapImageService::with_sources(t.path().into(), no_fetch(), local);
    let paths: Vec<_> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..8)
            .map(|_| sc.spawn(|| svc.get(cod2(), Some("d"), "mp_same", false)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let expected = rel(t.path(), &["cod2", "mp_same.png"]);
    assert!(
        paths
            .iter()
            .all(|p| p.as_deref() == Some(expected.as_path()))
    );
    let names: Vec<_> = fs::read_dir(t.path().join("cod2"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["mp_same.png"]);
}
