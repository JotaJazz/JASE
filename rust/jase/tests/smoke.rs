//! `jase --smoke` (de `test_smoke.py`): arranca la ventana con 5.000 servidores falsos y sale
//! sin esperar a una tarea bloqueante colgada, sin red ni config del usuario.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

#[test]
fn smoke_exits_ok() {
    if cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
    {
        eprintln!("smoke_exits_ok: sin DISPLAY ni WAYLAND_DISPLAY, se salta (usa xvfb-run)");
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let logs = tempfile::tempdir().unwrap();
    let (out, err) = (logs.path().join("out.txt"), logs.path().join("err.txt"));
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_jase"))
        .arg("--smoke")
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
        if start.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            panic!("el smoke no terminó en 45 s (¿el cierre espera a la tarea colgada?)");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stdout = std::fs::read_to_string(&out).unwrap();
    let stderr = std::fs::read_to_string(&err).unwrap();
    assert!(status.success(), "{status}\n{stdout}\n{stderr}");
    assert!(
        stdout.contains(&format!("JASE smoke OK {}", env!("CARGO_PKG_VERSION"))),
        "{stdout}\n{stderr}"
    );
    assert!(
        !has_config_json(home.path()),
        "el smoke escribió config.json"
    );
}
