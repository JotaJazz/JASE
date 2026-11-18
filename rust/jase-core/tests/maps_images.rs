//! Port de los tests de IWI (tests/test_maps_cod.py) y de capturas de tests/test_maps_unreal.py,
//! más DXT contra Pillow y archivos hostiles. Solo con la feature `map-images`.
#![cfg(feature = "map-images")]
mod support;

use image::DynamicImage;
use jase_core::maps::iwi::{IwiError, MAX_SIDE, decode_iwi};
use jase_core::maps::unreal::{MAX_TABLE_COUNT, MAX_UNR_BYTES, extract_screenshot};
use proptest::prelude::*;
use std::fs;
use std::path::Path;
use support::python::python;
use support::unreal::{
    BLUE, GREEN, PackageBuilder, RED, WHITE, compact, make_map, palette_body, texture_body,
};

fn dxt1(color0: u16, color1: u16, lut: u32) -> Vec<u8> {
    let mut b = color0.to_le_bytes().to_vec();
    b.extend(color1.to_le_bytes());
    b.extend(lut.to_le_bytes());
    b
}

/// IWI v5: mips del más chico al más grande; offsets = fin de cada mip.
fn make_iwi(fmt: u8, w: u16, h: u16, mips: &[&[u8]]) -> Vec<u8> {
    let mut ends = Vec::new();
    let mut pos = 28u32;
    for m in mips {
        pos += m.len() as u32;
        ends.push(pos);
    }
    ends.reverse(); // offsets[0] = fin del mip más grande
    let last = *ends.last().unwrap();
    ends.resize(4, last);
    let mut out = b"IWi".to_vec();
    out.extend([5, fmt, 0]);
    for v in [w, h, 1] {
        out.extend(v.to_le_bytes());
    }
    for e in ends {
        out.extend(e.to_le_bytes());
    }
    for m in mips {
        out.extend(*m);
    }
    out
}

fn rgba(img: &DynamicImage, x: u32, y: u32) -> [u8; 4] {
    img.to_rgba8().get_pixel(x, y).0
}

#[test]
fn decode_dxt1_takes_largest_mip() {
    let data = make_iwi(0x0B, 4, 4, &[&dxt1(0x07E0, 0, 0), &dxt1(0xF800, 0, 0)]);
    let img = decode_iwi(&data).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    assert_eq!(rgba(&img, 0, 0), [255, 0, 0, 255]);
    assert_eq!(rgba(&img, 3, 3), [255, 0, 0, 255]);
}

#[test]
fn decode_dxt5_single_mip() {
    let mut block = vec![255, 255, 0, 0, 0, 0, 0, 0]; // alpha0=255, índices 0
    block.extend(dxt1(0x001F, 0, 0)); // azul
    let img = decode_iwi(&make_iwi(0x0D, 4, 4, &[&block])).unwrap();
    assert_eq!(rgba(&img, 1, 2), [0, 0, 255, 255]);
}

#[test]
fn decode_argb32_bgra_order() {
    let pixels = [255, 0, 0, 255].repeat(16); // B, G, R, A → azul opaco
    let img = decode_iwi(&make_iwi(0x01, 4, 4, &[&pixels])).unwrap();
    assert!(matches!(img, DynamicImage::ImageRgba8(_)));
    assert_eq!((img.width(), img.height()), (4, 4));
    assert_eq!(rgba(&img, 2, 1), [0, 0, 255, 255]);
}

#[test]
fn decode_rgb24_bgr_order() {
    let pixels = [0, 255, 0].repeat(16); // B, G, R → verde
    let img = decode_iwi(&make_iwi(0x02, 4, 4, &[&pixels])).unwrap();
    assert!(matches!(img, DynamicImage::ImageRgb8(_)));
    assert_eq!(img.to_rgb8().get_pixel(0, 3).0, [0, 255, 0]);
}

#[test]
fn unknown_format_raises() {
    let err = decode_iwi(&make_iwi(0x7F, 4, 4, &[&[0; 64]])).unwrap_err();
    assert!(matches!(err, IwiError::Format(0x7F)));
    assert_eq!(err.to_string(), "formato de IWI desconocido: 0x7F");
}

#[test]
fn malformed_raises_iwi_error() {
    let red = dxt1(0xF800, 0, 0);
    let full = make_iwi(0x0B, 4, 4, &[&red]);
    let mut junk = b"XYZ".to_vec();
    junk.extend([0; 40]);
    let mut v4 = full.clone();
    v4[3] = 4;
    let cases: [(&[u8], &str); 7] = [
        (b"", "IWI demasiado corto"),
        (b"IWi", "IWI demasiado corto"),
        (&junk, "no es un archivo IWI"),
        (&full[..full.len() - 3], "IWI truncado"),
        (&make_iwi(0x0B, 0, 4, &[&red]), "dimensiones inválidas: 0x4"),
        (
            &make_iwi(0x0B, 60000, 60000, &[&red]),
            "dimensiones inválidas: 60000x60000",
        ),
        (&v4, "versión de IWI no soportada: 4"),
    ];
    for (data, text) in cases {
        assert_eq!(decode_iwi(data).unwrap_err().to_string(), text);
    }
    assert!(decode_iwi(&make_iwi(0x0B, MAX_SIDE + 1, 4, &[&red])).is_err());
}

#[test]
fn iwi_la88_and_a8() {
    let img = decode_iwi(&make_iwi(0x03, 2, 2, &[&[10, 20].repeat(4)])).unwrap();
    let DynamicImage::ImageLumaA8(la) = img else {
        panic!("no es LA")
    };
    assert_eq!(la.get_pixel(1, 1).0, [10, 20]);
    let img = decode_iwi(&make_iwi(0x04, 2, 2, &[&[1, 2, 3, 4]])).unwrap();
    let DynamicImage::ImageLuma8(l) = img else {
        panic!("no es L")
    };
    assert_eq!(l.into_raw(), [1, 2, 3, 4]);
}

#[test]
fn iwi_end0_out_of_range_uses_file_end() {
    // end0 más allá del archivo: el mip principal son los últimos bytes
    let mut data = make_iwi(0x04, 2, 2, &[&[9, 9, 9, 9], &[1, 2, 3, 4]]);
    data[12..16].copy_from_slice(&1000u32.to_le_bytes());
    let DynamicImage::ImageLuma8(l) = decode_iwi(&data).unwrap() else {
        panic!("no es L")
    };
    assert_eq!(l.into_raw(), [1, 2, 3, 4]);
    // end0 dentro del archivo pero antes del mip: también el final
    data[12..16].copy_from_slice(&30u32.to_le_bytes());
    let DynamicImage::ImageLuma8(l) = decode_iwi(&data).unwrap() else {
        panic!("no es L")
    };
    assert_eq!(l.into_raw(), [1, 2, 3, 4]);
    // end0 válido apunta al mip chico, que va primero
    data[12..16].copy_from_slice(&32u32.to_le_bytes());
    let DynamicImage::ImageLuma8(l) = decode_iwi(&data).unwrap() else {
        panic!("no es L")
    };
    assert_eq!(l.into_raw(), [9, 9, 9, 9]);
}

/// Bloques DXT pseudoaleatorios (xorshift con semilla fija) en IWI de 8×8 y 7×5 (bloques
/// recortados), decodificados aquí y por `decode_iwi` de Python (Pillow, `bcn`).
#[test]
fn bcn_matches_pillow() {
    let Some(mut py) = python() else { return };
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut iwis: Vec<Vec<u8>> = Vec::new();
    for (fmt, block) in [(0x0Bu8, 8usize), (0x0C, 16), (0x0D, 16)] {
        let mut payload = Vec::new();
        for i in 0..200 {
            let mut b: Vec<u8> = (0..block).map(|_| next() as u8).collect();
            let at = block - 8;
            let c0 = u16::from_le_bytes([b[at], b[at + 1]]);
            let c1 = u16::from_le_bytes([b[at + 2], b[at + 3]]);
            // la mitad con color0 <= color1 (BC1 de 3 colores), la otra con color0 > color1
            let (lo, hi) = (c0.min(c1), c0.max(c1).max(1));
            let (c0, c1) = if i % 2 == 0 {
                (lo.min(hi - 1), hi)
            } else {
                (hi, lo.min(hi - 1))
            };
            let (c0, c1) = if i % 10 == 0 { (c0, c0) } else { (c0, c1) };
            b[at..at + 2].copy_from_slice(&c0.to_le_bytes());
            b[at + 2..at + 4].copy_from_slice(&c1.to_le_bytes());
            payload.extend(b);
        }
        for chunk in payload.chunks(block * 4) {
            iwis.push(make_iwi(fmt, 8, 8, &[chunk]));
            iwis.push(make_iwi(fmt, 7, 5, &[chunk]));
        }
    }
    let hex: Vec<String> = iwis
        .iter()
        .map(|d| d.iter().map(|b| format!("{b:02x}")).collect())
        .collect();
    let out = py
        .args([
            "-c",
            "import json,sys\n\
             from jase.maps.iwi import decode_iwi\n\
             print(json.dumps([decode_iwi(bytes.fromhex(h)).convert('RGBA').tobytes().hex()\n\
                               for h in json.loads(sys.stdin.read())]))",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    out.stdin
        .as_ref()
        .unwrap()
        .write_all(serde_json::to_string(&hex).unwrap().as_bytes())
        .unwrap();
    let out = out.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let expected: Vec<String> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(expected.len(), iwis.len());
    for (i, (data, want)) in iwis.iter().zip(expected).enumerate() {
        let got: String = decode_iwi(data)
            .unwrap()
            .to_rgba8()
            .into_raw()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(got, want, "IWI {i} (formato 0x{:02X})", data[4]);
    }
}

fn write(dir: &Path, name: &str, data: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, data).unwrap();
    path.to_str().unwrap().to_owned()
}

fn assert_rgb_2x2(img: Option<image::RgbImage>) {
    let img = img.expect("sin captura");
    assert_eq!(img.dimensions(), (2, 2));
    assert_eq!(img.get_pixel(0, 0).0, RED);
    assert_eq!(img.get_pixel(1, 0).0, GREEN);
    assert_eq!(img.get_pixel(0, 1).0, BLUE);
    assert_eq!(img.get_pixel(1, 1).0, WHITE);
}

#[test]
fn extracts_p8_screenshot_via_levelinfo() {
    let t = tempfile::tempdir().unwrap();
    let path = write(t.path(), "DM-Test.unr", &make_map(true, "Shot"));
    assert_rgb_2x2(extract_screenshot(&path));
}

#[test]
fn falls_back_to_texture_named_screenshot() {
    let t = tempfile::tempdir().unwrap();
    let path = write(t.path(), "DM-Test.unr", &make_map(false, "Shot"));
    assert_rgb_2x2(extract_screenshot(&path));
}

#[test]
fn dxt1_texture() {
    let mut b = PackageBuilder::new();
    let body = texture_body(&mut b, 4, 4, &dxt1(0xF800, 0, 0), None, Some(3));
    b.add_export("Texture", "Screenshot", body, 0);
    let t = tempfile::tempdir().unwrap();
    let img = extract_screenshot(&write(t.path(), "DM-Dxt.unr", &b.build())).unwrap();
    assert_eq!(img.dimensions(), (4, 4));
    assert_eq!(img.get_pixel(3, 3).0, RED);
}

#[test]
fn rgba8_texture_is_bgra() {
    let mut b = PackageBuilder::new();
    let body = texture_body(&mut b, 1, 1, &[255, 0, 0, 255], None, Some(5));
    b.add_export("Texture", "Screenshot", body, 0);
    let t = tempfile::tempdir().unwrap();
    let img = extract_screenshot(&write(t.path(), "DM-Rgba.unr", &b.build())).unwrap();
    assert_eq!(img.get_pixel(0, 0).0, BLUE);
}

#[test]
fn truncated_file_returns_none() {
    let data = make_map(true, "Shot");
    let t = tempfile::tempdir().unwrap();
    for cut in [0, 3, 20, 64, data.len() / 2, data.len() - 1] {
        let path = write(t.path(), &format!("cut{cut}.unr"), &data[..cut]);
        assert!(extract_screenshot(&path).is_none(), "{cut}");
    }
}

#[test]
fn garbage_and_missing_return_none() {
    let t = tempfile::tempdir().unwrap();
    let junk = write(t.path(), "junk.unr", &b"\x00\x01garbage".repeat(100));
    assert!(extract_screenshot(&junk).is_none());
    let missing = t.path().join("nope.unr");
    assert!(extract_screenshot(missing.to_str().unwrap()).is_none());
    assert!(extract_screenshot(t.path().to_str().unwrap()).is_none());
}

#[test]
fn absurd_dimensions_return_none() {
    let mut b = PackageBuilder::new();
    let body = palette_body(&mut b, &[RED]);
    let pal = b.add_export("Palette", "Palette0", body, 0);
    let body = texture_body(&mut b, 100_000, 100_000, &[0; 4], Some(pal), None);
    b.add_export("Texture", "Screenshot", body, 0);
    let t = tempfile::tempdir().unwrap();
    assert!(extract_screenshot(&write(t.path(), "DM-Huge.unr", &b.build())).is_none());
}

#[test]
fn no_screenshot_returns_none() {
    let mut b = PackageBuilder::new();
    let body = palette_body(&mut b, &[RED]);
    b.add_export("Palette", "Palette0", body, 0);
    let t = tempfile::tempdir().unwrap();
    assert!(extract_screenshot(&write(t.path(), "DM-None.unr", &b.build())).is_none());
}

#[test]
fn unreal_huge_table_counts_are_rejected() {
    let t = tempfile::tempdir().unwrap();
    let mut data = make_map(true, "Shot");
    // name_count está en el byte 12
    data[12..16].copy_from_slice(&(MAX_TABLE_COUNT as u32 + 1).to_le_bytes());
    assert!(extract_screenshot(&write(t.path(), "big.unr", &data)).is_none());
    // en el tope pero en un archivo de 200 bytes: falla al leer, sin reservar nada
    let mut small = data[..64].to_vec();
    small.resize(200, 0);
    small[12..16].copy_from_slice(&(MAX_TABLE_COUNT as u32).to_le_bytes());
    let start = std::time::Instant::now();
    assert!(extract_screenshot(&write(t.path(), "small.unr", &small)).is_none());
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
}

#[test]
fn unreal_file_too_big() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("huge.unr");
    let f = fs::File::create(&path).unwrap();
    f.set_len(MAX_UNR_BYTES + 1).unwrap(); // disperso: no ocupa disco
    drop(f);
    let start = std::time::Instant::now();
    assert!(extract_screenshot(path.to_str().unwrap()).is_none());
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
}

#[test]
fn unreal_bad_palette_is_none() {
    let t = tempfile::tempdir().unwrap();
    for count in [0, 257] {
        let mut b = PackageBuilder::new();
        let mut body = b.none();
        body.extend(compact(count));
        body.extend(vec![255; 257 * 4]);
        let pal = b.add_export("Palette", "Palette0", body, 0);
        let body = texture_body(&mut b, 1, 1, &[0], Some(pal), None);
        b.add_export("Texture", "Screenshot", body, 0);
        let path = write(t.path(), &format!("pal{count}.unr"), &b.build());
        assert!(extract_screenshot(&path).is_none(), "{count}");
    }
}

#[test]
fn unreal_short_palette_pads_black() {
    let mut b = PackageBuilder::new();
    let mut body = b.none();
    body.extend(compact(1));
    body.extend([0, 255, 0, 0]);
    let pal = b.add_export("Palette", "Palette0", body, 0);
    let body = texture_body(&mut b, 2, 1, &[0, 200], Some(pal), None);
    b.add_export("Texture", "Screenshot", body, 0);
    let t = tempfile::tempdir().unwrap();
    let img = extract_screenshot(&write(t.path(), "DM-Pal.unr", &b.build())).unwrap();
    assert_eq!(img.into_raw(), [0, 255, 0, 0, 0, 0]);
}

/// Un nombre de 1 MiB al que apuntan 100 000 exports y 100 000 propiedades: con copias por
/// entrada serían ~200 GB; con índices se resuelve al instante.
#[test]
fn unreal_shared_huge_name_is_cheap() {
    const BIG: i64 = 4;
    const N: usize = 100_000;
    let mut names = Vec::new();
    for n in ["None", "Core", "Class", "LevelInfo"] {
        names.extend(compact(n.len() as i64 + 1));
        names.extend(n.bytes());
        names.extend([0; 5]);
    }
    names.extend(compact(1 << 20));
    names.extend(vec![b'x'; 1 << 20]);
    names.extend([0; 4]);
    let mut import = compact(1); // Core
    import.extend(compact(2)); // Class
    import.extend(0i32.to_le_bytes());
    import.extend(compact(3)); // LevelInfo
    // cuerpo del LevelInfo: N propiedades byte con el nombre enorme, luego "None"
    let mut body = Vec::new();
    for _ in 0..N {
        body.extend(compact(BIG));
        body.extend([0x01, 7]);
    }
    body.extend(compact(0));
    let name_off = 64usize;
    let import_off = name_off + names.len();
    let body_off = import_off + import.len();
    let export_off = body_off + body.len();
    let mut table = Vec::new();
    for i in 0..N {
        // solo el primero es un LevelInfo (import); el resto, clase 0 ("Class")
        table.extend(compact(if i == 0 { -1 } else { 0 }));
        table.extend(compact(0));
        table.extend(0i32.to_le_bytes());
        table.extend(compact(BIG));
        table.extend(0u32.to_le_bytes());
        table.extend(compact(body.len() as i64));
        table.extend(compact(body_off as i64));
    }
    let mut data = 0x9E2A_83C1u32.to_le_bytes().to_vec();
    data.extend(68u16.to_le_bytes());
    data.extend(0u16.to_le_bytes());
    for v in [0, 5, name_off, N, export_off, 1, import_off] {
        data.extend((v as u32).to_le_bytes());
    }
    data.resize(64, 0);
    data.extend(names);
    data.extend(import);
    data.extend(body);
    data.extend(table);
    let t = tempfile::tempdir().unwrap();
    let path = write(t.path(), "DM-Big.unr", &data);
    let start = std::time::Instant::now();
    // el LevelInfo no tiene Screenshot y ningún export se llama "screenshot"
    assert!(extract_screenshot(&path).is_none());
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}

proptest! {
    #[test]
    fn iwi_never_panics(
        data in proptest::collection::vec(any::<u8>(), 0..600),
        header in any::<bool>(),
    ) {
        let mut data = data;
        if header && data.len() >= 28 {
            data[..4].copy_from_slice(b"IWi\x05");
            data[4] = [0x01, 0x02, 0x03, 0x04, 0x0B, 0x0C, 0x0D][data[4] as usize % 7];
            // lados chicos (1–16, y a veces no múltiplos de 4) para llegar a decodificar
            data[6] %= 16;
            data[6] += 1;
            data[7] = 0;
            data[8] %= 16;
            data[8] += 1;
            data[9] = 0;
        }
        let _ = decode_iwi(&data);
    }

    #[test]
    fn unreal_never_panics(
        junk in proptest::collection::vec(any::<u8>(), 0..400),
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..=8),
    ) {
        let t = tempfile::tempdir().unwrap();
        let _ = extract_screenshot(&write(t.path(), "junk.unr", &junk));
        let mut map = make_map(true, "Shot");
        for (at, value) in edits {
            let n = map.len();
            map[at % n] = value;
        }
        let _ = extract_screenshot(&write(t.path(), "map.unr", &map));
    }
}
