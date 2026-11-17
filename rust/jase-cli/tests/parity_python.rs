//! `jase-cli` de Rust frente a la CLI Python 1.3.0 (`.venv/bin/jase-cli`) con los mismos
//! masters y servidores falsos: misma salida JSON (salvo versión, duración y pings) y mismo
//! código de salida, en inglés y en español. Sin `.venv` se salta con un aviso.
#[path = "../../jase-core/tests/support/net.rs"]
mod net;
use jase_core::protocols::quake3;
use net::{FakeUdp, fake_udp};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");
const RUST: &str = env!("CARGO_BIN_EXE_jase-cli");
const PYTHON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.venv/bin/jase-cli");

/// Master Quake 3 que lista `ports` de 127.0.0.1 ante cualquier getservers.
async fn master(ports: Vec<u16>) -> FakeUdp {
    fake_udp(move |d| {
        if !d.starts_with(b"\xff\xff\xff\xffgetservers ") {
            return vec![];
        }
        let mut p = b"\xff\xff\xff\xffgetserversResponse".to_vec();
        for port in &ports {
            p.extend([b'\\', 127, 0, 0, 1]);
            p.extend(port.to_be_bytes());
        }
        p.extend(b"\\EOT\0\0\0");
        vec![p]
    })
    .await
}

/// Servidor Quake 3 que responde `getstatus` con `reply`.
async fn server(reply: Vec<u8>) -> FakeUdp {
    fake_udp(move |d| {
        if d == quake3::GETSTATUS {
            vec![reply.clone()]
        } else {
            vec![]
        }
    })
    .await
}

/// Corre `bin` con un entorno mínimo: ninguna variable del usuario (p. ej. JASE_STEAM_KEY)
/// cambia el resultado.
fn spawn(bin: &str, args: &[String], home: &Path, lang: &[(&str, &str)]) -> std::process::Child {
    Command::new(bin)
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("APPDATA", home)
        .envs(lang.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Quita lo que cambia entre corridas o versiones y ordena los servidores por dirección.
fn normalize(stdout: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<Value>(stdout) else {
        return stdout.to_owned();
    };
    let obj = v.as_object_mut().unwrap();
    obj.shift_remove("jase");
    obj.shift_remove("duration_s");
    if let Some(Value::Array(servers)) = obj.get_mut("servers") {
        for s in servers.iter_mut() {
            s.as_object_mut().unwrap().shift_remove("ping_ms");
        }
        servers.sort_by_key(|s| s["address"].as_str().unwrap_or("").to_owned());
    }
    // Re-serializado: compara valores y también el orden de las claves.
    serde_json::to_string_pretty(&v).unwrap()
}

#[test]
fn same_output_as_python_cli() {
    if !Path::new(PYTHON).exists() {
        eprintln!("sin .venv: se salta la paridad con la CLI Python");
        return;
    }
    let rt = tokio::runtime::Runtime::new().unwrap();
    let fixture = |name: &str| std::fs::read(format!("{FIXTURES}/{name}")).unwrap();
    let russian = "statusResponse\n\\sv_hostname\\Русский сервер\\mapname\\mp_toujane\
\\g_gametype\\tdm\\sv_maxclients\\20\\shortversion\\1.3\\fs_game\\mods/pam\n10 40 \"Иван\"\n";
    let replies = [
        fixture("cod2x_status.bin"),
        fixture("coduo_status.bin"),
        fixture("q3_status.bin"),
        [b"\xff\xff\xff\xff", russian.as_bytes()].concat(),
    ];
    let servers: Vec<FakeUdp> = replies
        .into_iter()
        .map(|r| rt.block_on(server(r)))
        .collect();
    let mute = rt.block_on(fake_udp(|_| vec![]));
    let mut ports: Vec<u16> = servers.iter().map(|s| s.port).collect();
    ports.push(mute.port);
    let m = rt.block_on(master(ports));
    let dead = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let dead_spec = format!("127.0.0.1:{}", dead.local_addr().unwrap().port());
    drop(dead);
    let spec = format!("127.0.0.1:{}", m.port);

    let home = TempDir::new().unwrap();
    let cases: &[&[&str]] = &[
        &["games", "--json"],
        &["scan", "cod2", "--json", "--master", &spec],
        &["scan", "cod2", "--json", "--master", &spec, "--country"],
        &["scan", "l4d2", "--json"],
        &["scan", "cod2", "--json", "--master", &dead_spec],
        &["scan", "nope"],
    ];
    let langs: [&[(&str, &str)]; 2] = [
        &[("LANG", "C.UTF-8")],
        &[("LANG", "C.UTF-8"), ("JASE_LANG", "es")],
    ];
    for lang in langs {
        for case in cases {
            // Cada binario con su caché: ninguno lee lo que escribió el otro.
            let caches = [TempDir::new().unwrap(), TempDir::new().unwrap()];
            let csv = "127.0.0.0,127.255.255.255,CL\n";
            let runs: Vec<_> = [PYTHON, RUST]
                .iter()
                .zip(&caches)
                .map(|(bin, cache)| {
                    std::fs::write(cache.path().join(jase_core::geo::DB_FILENAME), csv).unwrap();
                    let mut args: Vec<String> = case.iter().map(|s| s.to_string()).collect();
                    args.extend(["--cache-dir".into(), cache.path().display().to_string()]);
                    args.push("--no-update".into());
                    spawn(bin, &args, home.path(), lang)
                })
                .collect();
            // Los dos corren a la vez: los falsos responden a cualquiera.
            let [py, rs] = runs
                .into_iter()
                .map(|c| {
                    let out = c.wait_with_output().unwrap();
                    let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
                    (out.status.code(), text(out.stdout), text(out.stderr))
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let what = format!("{case:?} {lang:?}");
            assert_eq!(rs.0, py.0, "código de salida: {what}\n{}\n{}", py.2, rs.2);
            if case[1] == "nope" {
                // Los mensajes genéricos de argparse y de clap difieren (divergencia aceptada).
                assert_eq!(rs.0, Some(2), "{what}");
                assert!(!py.2.is_empty() && !rs.2.is_empty(), "{what}");
                continue;
            }
            assert_eq!(normalize(&rs.1), normalize(&py.1), "{what}");
            assert_eq!(rs.2, py.2, "stderr: {what}");
        }
    }
}
