//! `jase --screenshot` (de `test_screenshot_script.py`): las capturas de los 14 temas con los
//! servidores de ejemplo de `scripts/screenshot.py`, sin red ni la config del usuario.

use std::net::Ipv4Addr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use iced::Size;
use iced::window::Screenshot;
use jase::screenshot::{sample_servers, write_png};

fn has_config_json(dir: &Path) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|e| {
        let path = e.path();
        if path.is_dir() {
            has_config_json(&path)
        } else {
            e.file_name() == "config.json"
        }
    })
}

/// (ancho, alto, RGBA) del PNG.
fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
    let mut reader = png::Decoder::new(file).read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

#[test]
fn screenshot_servers_use_documentation_ips() {
    let servers = sample_servers();
    assert!(servers.len() >= 10);
    for s in &servers {
        for addr in [&s.address, &s.connect_address] {
            let ip: Ipv4Addr = addr.host.parse().unwrap();
            let [a, b, c, _] = ip.octets();
            assert!(
                matches!((a, b, c), (192, 0, 2) | (198, 51, 100) | (203, 0, 113)),
                "{}",
                addr.host
            );
        }
    }
}

#[test]
fn write_png_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shot.png");
    let rgba: Vec<u8> = (0..24).collect();
    write_png(&path, &Screenshot::new(rgba.clone(), Size::new(3, 2), 1.0)).unwrap();
    assert_eq!(read_png(&path), (3, 2, rgba));
}

#[test]
fn screenshot_writes_every_theme() {
    if cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
    {
        eprintln!("screenshot_writes_every_theme: sin DISPLAY ni WAYLAND_DISPLAY, se salta");
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let shots = tempfile::tempdir().unwrap();
    let logs = tempfile::tempdir().unwrap();
    let (out, err) = (logs.path().join("out.txt"), logs.path().join("err.txt"));
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_jase"))
        .arg("--screenshot")
        .arg(shots.path())
        .env("ICED_BACKEND", "tiny-skia")
        .env("HOME", home.path())
        .env("APPDATA", home.path())
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        .stdout(Stdio::from(std::fs::File::create(&out).unwrap()))
        .stderr(Stdio::from(std::fs::File::create(&err).unwrap()))
        .spawn()
        .unwrap();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            panic!("--screenshot no terminó en 180 s");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stdout = std::fs::read_to_string(&out).unwrap();
    let stderr = std::fs::read_to_string(&err).unwrap();
    assert!(status.success(), "{status}\n{stdout}\n{stderr}");
    assert!(
        stdout.contains("JASE screenshot OK 14"),
        "{stdout}\n{stderr}"
    );
    let pngs: Vec<_> = std::fs::read_dir(shots.path())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(pngs.len(), 14, "{pngs:?}");
    let mut sizes = Vec::new();
    for (id, _, _) in &jase::theme::VARIANTS {
        let (w, h, rgba) = read_png(&shots.path().join(format!("{id}.png")));
        assert!(w > 0 && h > 0, "{id}: {w}×{h}");
        assert!(
            rgba.chunks(4).any(|px| px != &rgba[..4]),
            "{id}: de un solo color"
        );
        sizes.push((w, h));
    }
    sizes.dedup();
    assert_eq!(sizes.len(), 1, "tamaños distintos: {sizes:?}");
    // En Windows el escritorio del CI (1024×768) recorta la ventana; en Linux (xvfb) cabe
    // entera: 1280×800 lógicos por el factor de escala de la pantalla.
    #[cfg(target_os = "linux")]
    {
        let (w, h) = sizes[0];
        assert!(w >= 1280 && w * 800 == h * 1280, "{w}×{h}");
    }
    assert!(!has_config_json(home.path()), "escribió config.json");
    assert!(!has_config_json(shots.path()), "escribió config.json");
}
