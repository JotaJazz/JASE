//! `jase-cli` contra masters y servidores falsos locales. Traducción de tests/test_cli.py
//! (sin los dos de dependencias de la GUI ni el de cp1252, que no aplican a Rust).
#[path = "../../jase-core/tests/support/net.rs"]
mod net;
use jase_cli::{Deps, SCHEMA, run};
use jase_core::games::{GAMES, game};
use jase_core::protocols::{quake3, valve};
use net::{FakeUdp, fake_udp};
use serde_json::Value;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

// El idioma de i18n es global: las corridas no se solapan.
static LOCK: Mutex<()> = Mutex::new(());

fn deps() -> Deps {
    Deps {
        masters_fetch: Box::new(|_| panic!("sin red en los tests")),
        country_fetch: Box::new(|_| panic!("sin red en los tests")),
        today: (2026, 11),
    }
}

fn run_with(argv: &[&str], env: &[(&str, &str)], deps: &Deps) -> (i32, String, String) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
    let env: Vec<(String, String)> = env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let lookup = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&argv, &lookup, &mut out, &mut err, deps);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn run_cli(argv: &[&str]) -> (i32, String, String) {
    run_with(argv, &[], &deps())
}

/// `scan GAME --cache-dir DIR --no-update` más `extra`.
fn scan_args<'a>(game: &'a str, dir: &'a TempDir, extra: &[&'a str]) -> Vec<&'a str> {
    let mut v = vec!["scan", game, "--cache-dir", dir.path().to_str().unwrap()];
    v.push("--no-update");
    v.extend_from_slice(extra);
    v
}

fn status(name: &str, fs_game: &str) -> Vec<u8> {
    let text = format!(
        "statusResponse\n\\sv_hostname\\{name}\\mapname\\mp_toujane\\g_gametype\\tdm\
\\sv_maxclients\\20\\shortversion\\1.3\\fs_game\\{fs_game}\n10 40 \"Jimmy\"\n0 0 \"bot\"\n"
    );
    [b"\xff\xff\xff\xff", text.as_bytes()].concat()
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

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

fn dead_udp_port() -> u16 {
    let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

fn json(out: &str) -> Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

#[test]
fn games_json_lists_every_game() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_str().unwrap();
    let (code, out, _) = run_cli(&["games", "--json", "--cache-dir", path, "--no-update"]);
    let data = json(&out);
    assert_eq!(code, 0);
    assert_eq!((data["schema"].as_u64(), SCHEMA), (Some(1), 1));
    let games = data["games"].as_array().unwrap();
    let ids: Vec<&str> = games.iter().map(|g| g["id"].as_str().unwrap()).collect();
    let expected: Vec<&str> = GAMES.iter().map(|g| g.id).collect();
    assert_eq!(ids, expected);
    let by_id = |id: &str| games.iter().find(|g| g["id"] == id).unwrap();
    assert_eq!(by_id("l4d2")["needs_steam_key"], true);
    assert_eq!(by_id("l4d2")["family"], "valve");
    let cod2 = by_id("cod2");
    assert_eq!(cod2["needs_steam_key"], false);
    assert_eq!(cod2["name"], game("cod2").unwrap().name);
    let masters = cod2["masters"].as_array().unwrap();
    assert!(!masters.is_empty());
    for m in masters {
        let keys: Vec<&String> = m.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["host", "port"]);
    }
}

#[test]
fn scan_json_contract() {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let srv = rt.block_on(server(status("Jimmy TDM", "mods/pam")));
    let m = rt.block_on(master(vec![srv.port]));
    let spec = format!("127.0.0.1:{}", m.port);
    let (code, out, _) = run_cli(&scan_args("cod2", &dir, &["--json", "--master", &spec]));
    let data = json(&out);
    assert_eq!(code, 0);
    assert_eq!(data["schema"], 1);
    let name = game("cod2").unwrap().name;
    assert_eq!(
        data["game"],
        serde_json::json!({"id": "cod2", "name": name})
    );
    assert_eq!(data["timed_out"], false);
    assert_eq!(data["errors"], serde_json::json!([]));
    assert_eq!(
        data["masters"],
        serde_json::json!([{"host": "127.0.0.1", "port": m.port, "ok": true}])
    );
    assert_eq!(data["masters_ok"], 1);
    assert_eq!(
        (&data["servers_listed"], &data["servers_responded"]),
        (&1.into(), &1.into())
    );
    assert!(data["duration_s"].is_f64());
    let s = &data["servers"][0];
    let address = format!("127.0.0.1:{}", srv.port);
    let expected = serde_json::json!({
        "address": address, "connect": address, "connect_port_deduced": false,
        "players": 2, "max_players": 20, "bots": 1, "humans": 1,
        "mod": "pam", "map": "mp_toujane", "version": "1.3",
        "country": null, "password": false,
    });
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(&s[key], value, "{key}");
    }
    assert!(s["ping_ms"].is_u64());
    assert_eq!(
        s["player_list"],
        serde_json::json!([{"name": "Jimmy", "score": 10, "ping": 40},
                           {"name": "bot", "score": 0, "ping": 0}])
    );
    assert_eq!(s["rules"]["g_gametype"], "tdm");
}

#[test]
fn json_key_order_matches_contract() {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let srv = rt.block_on(server(status("x", "mods/pam")));
    let m = rt.block_on(master(vec![srv.port]));
    let spec = format!("127.0.0.1:{}", m.port);
    let (_, out, _) = run_cli(&scan_args("cod2", &dir, &["--json", "--master", &spec]));
    let data = json(&out);
    let keys = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    assert_eq!(
        keys(&data),
        [
            "schema",
            "jase",
            "game",
            "duration_s",
            "timed_out",
            "masters",
            "masters_ok",
            "errors",
            "servers_listed",
            "servers_responded",
            "servers"
        ]
    );
    assert_eq!(keys(&data["masters"][0]), ["host", "port", "ok"]);
    assert_eq!(
        keys(&data["servers"][0]),
        [
            "address",
            "connect",
            "connect_port_deduced",
            "name",
            "map",
            "gametype",
            "mod",
            "players",
            "max_players",
            "bots",
            "humans",
            "password",
            "version",
            "ping_ms",
            "country",
            "player_list",
            "rules"
        ]
    );
    assert_eq!(
        keys(&data["servers"][0]["player_list"][0]),
        ["name", "score", "ping"]
    );
    // Las reglas, en el orden en que las envió el servidor.
    let rules = keys(&data["servers"][0]["rules"]);
    assert_eq!(rules[..3], ["sv_hostname", "mapname", "g_gametype"]);
    // Sangría de 1 espacio y salto de línea final, como json.dump(indent=1).
    assert!(out.starts_with("{\n \"schema\": 1,\n \"jase\": "), "{out}");
    assert!(out.ends_with("}\n"));
}

#[test]
fn scan_json_keeps_unicode() {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    // Quake 3 decodifica en latin-1; A2S_INFO (Valve) admite UTF-8.
    let fixture = |name: &str| std::fs::read(format!("{FIXTURES}/{name}")).unwrap();
    let old: &[u8] = b"ROMANIA.FULLBOOST.RO V.I.P FREE";
    let raw = fixture("cs16_info.bin");
    let at = raw.windows(old.len()).position(|w| w == old).unwrap();
    let info = [
        &raw[..at],
        "Русский сервер".as_bytes(),
        &raw[at + old.len()..],
    ]
    .concat();
    let (player, c) = (fixture("cs16_player.bin"), b"\x01\x02\x03\x04");
    let challenge = [valve::OOB, b"A", c].concat();
    let srv = rt.block_on(fake_udp(move |d| {
        if d == valve::A2S_INFO {
            vec![info.clone()]
        } else if d == valve::build_player_query(None) {
            vec![challenge.clone()]
        } else if d == valve::build_player_query(Some(c)) {
            vec![player.clone()]
        } else {
            vec![]
        }
    }));
    let mut page = valve::MASTER_HEADER.to_vec();
    page.extend([127, 0, 0, 1]);
    page.extend(srv.port.to_be_bytes());
    page.extend([0u8; 6]);
    let m = rt.block_on(fake_udp(move |_| vec![page.clone()]));
    let spec = format!("127.0.0.1:{}", m.port);
    let (_, out, _) = run_cli(&scan_args("cs16", &dir, &["--json", "--master", &spec]));
    assert!(out.contains("\"name\": \"Русский сервер\""), "{out}");
}

#[test]
fn scan_base_mod_is_null() {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let srv = rt.block_on(server(status("x", "")));
    let m = rt.block_on(master(vec![srv.port]));
    let spec = format!("127.0.0.1:{}", m.port);
    let (_, out, _) = run_cli(&scan_args("cod2", &dir, &["--json", "--master", &spec]));
    assert_eq!(json(&out)["servers"][0]["mod"], Value::Null);
}

#[test]
fn scan_reports_master_errors_and_exit_code() {
    let dir = TempDir::new().unwrap();
    let spec = format!("127.0.0.1:{}", dead_udp_port());
    let (code, out, _) = run_cli(&scan_args("cod2", &dir, &["--json", "--master", &spec]));
    let data = json(&out);
    assert_eq!(code, 1);
    let e = &data["errors"][0];
    assert_eq!(e["master"], spec.as_str());
    // Puerto cerrado: el ICMP da error de red; sin él vence el plazo.
    assert!(e["kind"] == "network" || e["kind"] == "timeout", "{e}");
    assert_eq!(e["message"], "Master 127.0.0.1 is not responding");
    assert_eq!(data["masters_ok"], 0);
    assert_eq!(data["masters"][0]["ok"], false);
}

fn first_error(out: &str) -> Value {
    json(out)["errors"][0].clone()
}

#[test]
fn steam_key_from_flag_or_env() {
    let dir = TempDir::new().unwrap();
    let base = scan_args("l4d2", &dir, &["--json", "--master", "127.0.0.1:1"]);
    let (_, out, _) = run_cli(&base);
    assert_eq!(first_error(&out)["kind"], "missing_key");
    let (_, out, err) = run_with(&base, &[("JASE_STEAM_KEY", "ENVKEY")], &deps());
    assert_eq!(first_error(&out)["kind"], "unsupported_host");
    assert!(!out.contains("ENVKEY") && !err.contains("ENVKEY"));
    let mut flag = base.clone();
    flag.extend(["--steam-key", "FLAGKEY", "--include-empty"]);
    let (_, out, err) = run_cli(&flag);
    assert_eq!(first_error(&out)["kind"], "unsupported_host");
    assert!(!out.contains("FLAGKEY") && !err.contains("FLAGKEY"));
}

#[test]
fn missing_steam_key_message_points_to_cli_options() {
    let dir = TempDir::new().unwrap();
    let (_, out, _) = run_with(
        &scan_args("l4d2", &dir, &["--json"]),
        &[("JASE_LANG", "es")],
        &deps(),
    );
    let e = first_error(&out);
    assert_eq!(e["kind"], "missing_key");
    let message = e["message"].as_str().unwrap();
    assert!(
        message.contains("JASE_STEAM_KEY") && !message.contains("Ajustes"),
        "{message}"
    );
}

#[test]
fn master_override() {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let m = rt.block_on(master(vec![]));
    let spec = format!("127.0.0.1:{}", m.port);
    let (_, out, _) = run_cli(&scan_args(
        "cod2",
        &dir,
        &["--json", "--master", &spec, "--master", "localhost:1"],
    ));
    let hosts: Vec<(Value, Value)> = json(&out)["masters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["host"].clone(), m["port"].clone()))
        .collect();
    assert_eq!(
        hosts,
        [
            ("127.0.0.1".into(), m.port.into()),
            ("localhost".into(), 1.into())
        ]
    );
    assert!(
        m.received()
            .iter()
            .any(|d| d.starts_with(b"\xff\xff\xff\xffgetservers "))
    );
}

#[test]
fn bad_master_is_usage_error() {
    let dir = TempDir::new().unwrap();
    for bad in ["nohost", "h:0", "h:99999", ":20710"] {
        let (code, out, err) = run_cli(&scan_args("cod2", &dir, &["--master", bad]));
        assert_eq!((code, out.as_str()), (2, ""), "{bad}");
        assert!(err.starts_with("jase-cli: "), "{bad}: {err}");
        let message = format!("Invalid master: {bad} (expected host:port)");
        assert!(err.contains(&message), "{bad}: {err}");
    }
}

#[test]
fn bad_timeout_is_usage_error() {
    let dir = TempDir::new().unwrap();
    for bad in ["0", "-1", "abc"] {
        let (code, out, err) = run_cli(&scan_args("cod2", &dir, &["--timeout", bad]));
        assert_eq!((code, out.as_str()), (2, ""), "{bad}");
        assert!(err.starts_with("jase-cli: "), "{bad}: {err}");
        assert!(
            err.contains("Expected a number of seconds greater than 0"),
            "{bad}: {err}"
        );
    }
}

#[test]
fn unknown_option_is_usage_error() {
    let dir = TempDir::new().unwrap();
    let (code, out, err) = run_cli(&scan_args("cod2", &dir, &["--bogus"]));
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(
        err.starts_with("jase-cli: ") && err.contains("--bogus"),
        "{err}"
    );
    assert!(!err.contains("error: "), "{err}");
}

#[test]
fn unknown_game_is_usage_error() {
    let dir = TempDir::new().unwrap();
    let (code, out, err) = run_cli(&scan_args("nope", &dir, &["--json"]));
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.contains("nope"), "{err}");
}

/// Un master que lista dos servidores: uno contesta y el otro nunca. El escaneo espera
/// el plazo de consulta (1,5 s) por el mudo; `--timeout` lo corta antes.
fn timeout_run(timeout_flag: &str) -> (i32, Value) {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let srv = rt.block_on(server(status("x", "mods/pam")));
    let mute = rt.block_on(fake_udp(|_| vec![]));
    let m = rt.block_on(master(vec![srv.port, mute.port]));
    let spec = format!("127.0.0.1:{}", m.port);
    let extra = ["--json", "--master", &spec, timeout_flag, "0.3"];
    let (code, out, _) = run_cli(&scan_args("cod2", &dir, &extra));
    (code, json(&out))
}

#[test]
fn timeout_returns_partial_results() {
    let (code, data) = timeout_run("--timeout");
    assert_eq!((code, &data["timed_out"]), (0, &true.into()));
    assert_eq!(data["servers"].as_array().unwrap().len(), 1);
    assert_eq!(data["servers_responded"], 1);
    for key in ["servers_listed", "masters_ok"] {
        assert_eq!(data[key], Value::Null, "{key}");
    }
    assert_eq!(data["masters"][0]["ok"], Value::Null);
    assert!(data["duration_s"].as_f64().unwrap() < 1.0);
}

#[test]
fn abbreviated_long_option_works() {
    let (code, data) = timeout_run("--time");
    assert_eq!((code, &data["timed_out"]), (0, &true.into()));
}

/// CSV de DB-IP con 127.0.0.0/8 → CL.
fn country_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    let csv = "127.0.0.0,127.255.255.255,CL\n";
    std::fs::write(dir.path().join(jase_core::geo::DB_FILENAME), csv).unwrap();
    dir
}

#[test]
fn country_flag_uses_cached_db() {
    let dir = country_dir();
    let rt = rt();
    let srv = rt.block_on(server(status("x", "mods/pam")));
    let m = rt.block_on(master(vec![srv.port]));
    let spec = format!("127.0.0.1:{}", m.port);
    let extra = ["--json", "--country", "--master", &spec];
    let (code, out, _) = run_cli(&scan_args("cod2", &dir, &extra));
    assert_eq!(code, 0);
    assert_eq!(json(&out)["servers"][0]["country"], "cl");
}

#[test]
fn country_db_failure_is_a_warning_not_an_error() {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let srv = rt.block_on(server(status("x", "mods/pam")));
    let m = rt.block_on(master(vec![srv.port]));
    let spec = format!("127.0.0.1:{}", m.port);
    let deps = Deps {
        country_fetch: Box::new(|_| None),
        ..deps()
    };
    let extra = ["--json", "--country", "--master", &spec];
    let (code, out, err) = run_with(
        &scan_args("cod2", &dir, &extra),
        &[("JASE_LANG", "es")],
        &deps,
    );
    assert_eq!(code, 0);
    assert_eq!(json(&out)["servers"][0]["country"], Value::Null);
    assert_eq!(err, "jase-cli: No se pudo descargar la base de países\n");
}

#[test]
fn text_output_is_a_table() {
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let srv = rt.block_on(server(status("Jimmy TDM", "mods/pam")));
    let m = rt.block_on(master(vec![srv.port]));
    let spec = format!("127.0.0.1:{}", m.port);
    let (code, out, err) = run_cli(&scan_args("cod2", &dir, &["--master", &spec]));
    assert_eq!(code, 0);
    let address = format!("127.0.0.1:{}", srv.port);
    assert!(out.starts_with(&format!("{address:<22}    2/20 ")), "{out}");
    assert!(
        out.ends_with(&format!(" ms  {:<20}  Jimmy TDM\n", "mp_toujane")),
        "{out}"
    );
    assert_eq!(err, "1 of 1 servers responded\n");
}

#[test]
fn text_games_list() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_str().unwrap();
    let (code, out, _) = run_cli(&["games", "--cache-dir", path, "--no-update"]);
    assert_eq!(code, 0);
    assert_eq!(out.lines().count(), GAMES.len());
    assert!(
        out.contains("l4d2        Left 4 Dead 2  [Steam key]\n"),
        "{out}"
    );
}

#[test]
fn language_from_environment() {
    let dir = TempDir::new().unwrap();
    let args = scan_args("nope", &dir, &[]);
    let (code, _, err) = run_with(&args, &[("JASE_LANG", "es")], &deps());
    assert_eq!(code, 2);
    assert!(err.contains("Juego desconocido"), "{err}");
    let (_, _, err) = run_with(&args, &[("LANG", "C.UTF-8")], &deps());
    assert!(err.contains("Unknown game"), "{err}");
}

#[test]
fn help_and_version_exit_0() {
    let (code, out, _) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("scan"), "{out}");
    let (code, out, _) = run_cli(&["--version"]);
    assert_eq!(
        (code, out.trim()),
        (0, concat!("jase-cli ", env!("CARGO_PKG_VERSION")))
    );
    let (code, out, _) = run_cli(&["scan", "--help"]);
    assert!(code == 0 && out.contains("--steam-key"), "{out}");
    let (code, _, err) = run_cli(&[]);
    assert!(code == 2 && !err.is_empty());
}

#[test]
fn cli_does_not_depend_on_gui_crates() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let Ok(out) = std::process::Command::new("cargo")
        .args(["tree", "-p", "jase-cli", "-e", "normal", "--prefix", "none"])
        .args(["--manifest-path", manifest])
        .output()
    else {
        return; // sin cargo en el PATH
    };
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8(out.stdout).unwrap();
    for line in tree.lines() {
        let name = line.split(' ').next().unwrap_or("");
        assert!(!["iced", "keyring", "image"].contains(&name), "{line}");
    }
}

/// Escritor que anota en un registro común qué flujo escribe y en qué orden.
struct Logged(&'static str, Arc<Mutex<Vec<String>>>);
impl Write for Logged {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.1.lock().unwrap().push(format!("{}:{text}", self.0));
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn text_scan_orders_errors_table_summary() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let rt = rt();
    let srv = rt.block_on(server(status("Jimmy TDM", "mods/pam")));
    let good = rt.block_on(master(vec![srv.port]));
    let specs = [
        format!("127.0.0.1:{}", dead_udp_port()),
        format!("127.0.0.1:{}", good.port),
    ];
    let log = Arc::new(Mutex::new(Vec::new()));
    let (mut out, mut err) = (Logged("out", log.clone()), Logged("err", log.clone()));
    let args = scan_args(
        "cod2",
        &dir,
        &["--master", &specs[0], "--master", &specs[1]],
    );
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    run(&argv, &|_| None, &mut out, &mut err, &deps());
    let log = log.lock().unwrap();
    let pos = |p: &str| log.iter().position(|l| l.starts_with(p)).unwrap();
    assert!(pos("err:127.0.0.1:") < pos("out:"), "{log:?}");
    assert!(pos("out:") < pos("err:1 of 1"), "{log:?}");
}

#[test]
fn abbreviated_root_option_works() {
    let (code, out, _) = run_cli(&["--vers"]);
    assert_eq!(
        (code, out.trim()),
        (0, concat!("jase-cli ", env!("CARGO_PKG_VERSION")))
    );
}

struct Failing(io::ErrorKind);
impl Write for Failing {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(self.0.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn stdout_write_error_is_exit_1_but_broken_pipe_is_silent() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_str().unwrap();
    let code = |kind, extra: &[&str]| {
        let argv: Vec<String> = ["games", "--cache-dir", path, "--no-update"]
            .iter()
            .chain(extra)
            .map(|s| s.to_string())
            .collect();
        run(
            &argv,
            &|_| None,
            &mut Failing(kind),
            &mut Vec::new(),
            &deps(),
        )
    };
    for extra in [&[][..], &["--json"][..]] {
        assert_eq!(code(io::ErrorKind::Other, extra), 1);
        assert_eq!(code(io::ErrorKind::BrokenPipe, extra), 0);
    }
}
