//! Consultas sin interfaz gráfica: `jase-cli games` y `jase-cli scan <juego>`.
//! Traducción de `src/jase/cli.py`. Con `--json` la salida sigue el contrato versionado de
//! docs/cli.md (`"schema": SCHEMA`); un cambio incompatible sube SCHEMA. Códigos de salida:
//! 0 = algún master respondió (o se agotó --timeout), 1 = ningún master respondió, 2 = uso
//! incorrecto.
use clap::{Arg, ArgAction, ArgMatches, ColorChoice, Command, value_parser};
use jase_core::browse::{BASE_MOD, mod_of};
use jase_core::config::default_config_path;
use jase_core::games::{GAMES, GameDef, game};
use jase_core::geo::{self, CountryDB};
use jase_core::i18n::{self, tr, tr_fmt};
use jase_core::masters::{self, Master, OfficialList};
use jase_core::models::ServerInfo;
use jase_core::net;
use jase_core::scanner::scan::{MasterError, ScanEvent, ScanOptions, scan};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const SCHEMA: u32 = 1;
const VERSION: &str = env!("CARGO_PKG_VERSION");
const STEAM_KEY_ENV: &str = "JASE_STEAM_KEY";
const CACHE_DIR_ENV: &str = "JASE_CACHE_DIR";

type Fetch = Box<dyn Fn(&str) -> Option<Vec<u8>>>;

/// Lo que en los tests se sustituye: descargas y mes actual (para la base de países).
pub struct Deps {
    pub masters_fetch: Fetch,
    pub country_fetch: Fetch,
    pub today: (i32, u32),
}

impl Deps {
    pub fn real() -> Self {
        let deadline = net::FETCH_DEADLINE;
        Self {
            masters_fetch: Box::new(move |url| {
                net::fetch_blocking(url, masters::MAX_BYTES, deadline, &[])
            }),
            country_fetch: Box::new(geo::default_fetch),
            today: geo::current_month(),
        }
    }
}

fn master_arg(value: &str) -> Result<Master, String> {
    let invalid = || {
        tr_fmt(
            "Master inválido: {value} (se espera host:puerto)",
            &[("value", &value)],
        )
    };
    let (host, port) = value.rsplit_once(':').ok_or_else(invalid)?;
    let port = Some(port)
        .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|p| p.parse::<u16>().ok())
        .filter(|&p| p >= 1);
    match port {
        Some(port) if masters::valid_host(host) => Ok((host.to_owned(), port)),
        _ => Err(invalid()),
    }
}

fn positive(value: &str) -> Result<f64, String> {
    match value.trim().parse::<f64>() {
        Ok(seconds) if seconds > 0.0 => Ok(seconds), // NaN tampoco pasa
        _ => Err(tr("Se espera un número de segundos mayor que 0")),
    }
}

fn flag(name: &'static str, help: String) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::SetTrue)
        .help(help)
}

fn subcommand(name: &'static str, help: String) -> Command {
    Command::new(name)
        .about(help)
        .infer_long_args(true)
        .arg(flag(
            "json",
            tr("Salida JSON (contrato versionado, ver docs/cli.md)"),
        ))
        .arg(
            Arg::new("cache-dir")
                .long("cache-dir")
                .value_name("CACHE_DIR")
                .value_parser(value_parser!(PathBuf))
                .help(tr(
                    "Carpeta de caché: lista de masters y base de países (o variable \
                          JASE_CACHE_DIR)",
                )),
        )
        .arg(flag(
            "no-update",
            tr("No descargar la lista oficial de masters (se revisa como mucho una vez al día)"),
        ))
}

fn build_command() -> Command {
    let scan = subcommand(
        "scan",
        tr("Consulta los masters y los servidores de un juego"),
    )
    .arg(
        Arg::new("game")
            .required(true)
            .help(tr("id del juego (ver «jase-cli games»)")),
    )
    .arg(
        Arg::new("steam-key")
            .long("steam-key")
            .value_name("STEAM_KEY")
            .help(tr(
                "Clave de la API web de Steam (o variable JASE_STEAM_KEY)",
            )),
    )
    .arg(flag(
        "include-empty",
        tr("Incluir servidores vacíos de Steam"),
    ))
    .arg(
        Arg::new("timeout")
            .long("timeout")
            .value_name("TIMEOUT")
            .value_parser(positive)
            .allow_negative_numbers(true) // como argparse: «-1» es un valor, no una opción
            .help(tr(
                "Tope de tiempo total en segundos (por defecto, sin tope)",
            )),
    )
    .arg(flag(
        "country",
        tr("Agregar el país de cada servidor (descarga la base de DB-IP la primera vez)"),
    ))
    .arg(
        Arg::new("master")
            .long("master")
            .value_parser(master_arg)
            .action(ArgAction::Append)
            .value_name("HOST:PORT")
            .help(tr("Master a consultar en vez de los oficiales (repetible)")),
    );
    Command::new("jase-cli")
        .infer_long_args(true)
        .no_binary_name(true)
        .bin_name("jase-cli")
        .version(VERSION)
        .about(tr(
            "Consulta servidores de juegos clásicos sin interfaz gráfica.",
        ))
        .color(ColorChoice::Never)
        .subcommand_required(true)
        .disable_help_subcommand(true)
        .subcommand(subcommand("games", tr("Lista los juegos soportados")))
        .subcommand(scan)
}

fn official(cache_dir: &Path, update: bool, fetch: &Fetch) -> OfficialList {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    if update && masters::should_check(&cache_dir.join(masters::META_FILENAME), now) {
        // Si no se puede crear, check_official falla y lo anota: queda la lista cargada.
        let _ = std::fs::create_dir_all(cache_dir);
        masters::check_official(cache_dir, now, |url| fetch(url));
    }
    masters::load_official(cache_dir)
}

fn masters_json(list: &[Master]) -> Vec<Value> {
    list.iter()
        .map(|(h, p)| json!({"host": h, "port": p}))
        .collect()
}

fn games_json(official: &OfficialList) -> Value {
    let games: Vec<Value> = GAMES
        .iter()
        .map(|g| {
            json!({
                "id": g.id, "name": g.name, "short_name": g.short_name(),
                "family": g.family.as_str(), "needs_steam_key": g.steam_webapi,
                "uses_steam_api": g.uses_steam_api(),
                "masters": masters_json(&masters::official_for(g.id, official)),
            })
        })
        .collect();
    json!({"schema": SCHEMA, "jase": VERSION, "games": games})
}

/// Los mensajes de la clave de Steam de la app hablan de Ajustes; aquí, de la CLI.
fn error_message(error: &MasterError) -> String {
    match error.kind {
        "missing_key" => {
            tr("Falta la clave de API de Steam; usa --steam-key o la variable JASE_STEAM_KEY")
        }
        "invalid_key" => tr("La clave de API de Steam no es válida"),
        _ => error.message.clone(),
    }
}

fn server_json(game: &GameDef, info: &ServerInfo, country: Option<&str>) -> Value {
    let m = mod_of(game, info);
    let players: Vec<Value> = info
        .players
        .iter()
        .map(|p| json!({"name": p.name, "score": p.score, "ping": p.ping}))
        .collect();
    json!({
        "address": info.address.to_string(),
        "connect": info.connect_address.to_string(),
        "connect_port_deduced": info.connect_port_deduced,
        "name": info.name,
        "map": info.map,
        "gametype": info.gametype,
        "mod": (m != BASE_MOD).then_some(m),
        "players": info.num_players,
        "max_players": info.max_players,
        "bots": info.bots,
        "humans": info.humans(),
        "password": info.password,
        "version": info.version,
        "ping_ms": info.ping_ms,
        "country": country,
        "player_list": players,
        "rules": info.rules.iter()
            .map(|(k, v)| (k.clone(), Value::from(v.as_str())))
            .collect::<serde_json::Map<_, _>>(),
    })
}

/// Como `json.dump(ensure_ascii=False, indent=1)` más el salto de línea final.
fn write_json(out: &mut dyn Write, value: &Value) -> io::Result<()> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        serde_json::ser::PrettyFormatter::with_indent(b" "),
    );
    if value.serialize(&mut ser).is_ok() {
        buf.push(b'\n');
        return out.write_all(&buf);
    }
    Ok(())
}

/// Un error de escritura en stdout da código 1, salvo la tubería cerrada (`| head`).
fn exit_code(code: i32, written: io::Result<()>) -> i32 {
    match written {
        Err(e) if e.kind() != io::ErrorKind::BrokenPipe => 1,
        _ => code,
    }
}

fn load_countries(cache_dir: &Path, deps: &Deps, stderr: &mut dyn Write) -> Option<CountryDB> {
    let path = cache_dir.join(geo::DB_FILENAME);
    match geo::load_country_db(&path, false, deps.today, &*deps.country_fetch) {
        Ok(db) => Some(db),
        Err(error) => {
            let _ = writeln!(stderr, "jase-cli: {error}");
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_scan(
    args: &ArgMatches,
    game: &GameDef,
    official: &OfficialList,
    cache_dir: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    deps: &Deps,
) -> i32 {
    let master_list: Vec<Master> = match args.get_many::<Master>("master") {
        Some(list) => list.cloned().collect(),
        None => masters::official_for(game.id, official),
    };
    let db = if args.get_flag("country") {
        load_countries(cache_dir, deps, stderr)
    } else {
        None
    };
    let opts = ScanOptions {
        steam_key: args
            .get_one::<String>("steam-key")
            .cloned()
            .or_else(|| env(STEAM_KEY_ENV))
            .unwrap_or_default(),
        steam_include_empty: args.get_flag("include-empty"),
        ..ScanOptions::default()
    };
    // Un tope infinito no convierte a Duration: equivale a no poner tope.
    let limit = args
        .get_one::<f64>("timeout")
        .and_then(|&t| Duration::try_from_secs_f64(t).ok());
    let mut servers: Vec<ServerInfo> = Vec::new();
    let started = Instant::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime de tokio");
    let result = {
        let mut on_event = |event| {
            if let ScanEvent::Server(info) = event {
                servers.push(info);
            }
        };
        let scanning = scan(game, &master_list, &opts, &mut on_event);
        runtime.block_on(async {
            match limit {
                Some(limit) => tokio::time::timeout(limit, scanning).await.ok(),
                None => Some(scanning.await),
            }
        })
    };
    // Consultas DNS bloqueantes pendientes no retienen la salida tras el tope.
    runtime.shutdown_background();
    let timed_out = result.is_none();
    let errors = result.as_ref().map_or(&[][..], |r| &r.errors[..]);
    let failed: HashSet<&str> = errors.iter().map(|e| e.master.as_str()).collect();
    let countries: Vec<Option<&str>> = servers
        .iter()
        .map(|s| db.as_ref().and_then(|db| db.lookup(&s.address.host)))
        .collect();
    let data = json!({
        "schema": SCHEMA,
        "jase": VERSION,
        "game": {"id": game.id, "name": game.name},
        "duration_s": (started.elapsed().as_secs_f64() * 1000.0).round() / 1000.0,
        "timed_out": timed_out,
        "masters": master_list.iter().map(|(h, p)| json!({
            "host": h, "port": p,
            "ok": (!timed_out).then(|| !failed.contains(format!("{h}:{p}").as_str())),
        })).collect::<Vec<_>>(),
        "masters_ok": result.as_ref().map(|r| r.masters_ok),
        "errors": errors.iter().map(|e| json!({
            "master": e.master, "kind": e.kind, "message": error_message(e),
        })).collect::<Vec<_>>(),
        "servers_listed": result.as_ref().map(|r| r.servers_listed),
        "servers_responded": servers.len(),
        "servers": servers.iter().zip(&countries)
            .map(|(info, country)| server_json(game, info, *country))
            .collect::<Vec<_>>(),
    });
    let written = if args.get_flag("json") {
        write_json(stdout, &data)
    } else {
        print_table(&data, &servers, &countries, stdout, stderr)
    };
    exit_code(i32::from(result.is_some_and(|r| !r.ok())), written)
}

fn print_table(
    data: &Value,
    servers: &[ServerInfo],
    countries: &[Option<&str>],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<()> {
    let mut err = String::new();
    for e in data["errors"].as_array().into_iter().flatten() {
        let text = |k: &str| e[k].as_str().unwrap_or("").to_owned();
        err += &format!("{}: {}\n", text("master"), text("message"));
    }
    if data["timed_out"] == true {
        err += &tr("Se alcanzó el tope de tiempo; resultados parciales");
        err.push('\n');
    }
    // Como `_print_table`: errores y aviso antes de la tabla; el resumen, después.
    let _ = stderr.write_all(err.as_bytes());
    err.clear();
    let mut rows: Vec<(&ServerInfo, Option<&str>)> =
        servers.iter().zip(countries.iter().copied()).collect();
    rows.sort_by_key(|(s, _)| (-i64::from(s.num_players), s.ping_ms));
    let show_country = rows.iter().any(|(_, c)| c.is_some());
    let mut out = String::new();
    for (s, country) in rows {
        let country = if show_country {
            format!("{:2}  ", country.unwrap_or("--"))
        } else {
            String::new()
        };
        let map: String = s.map.chars().take(20).collect();
        out += &format!(
            "{:<22}  {country}{:>3}/{:<3}  {:>4} ms  {map:<20}  {}\n",
            s.connect_address.to_string(),
            s.num_players,
            s.max_players,
            s.ping_ms,
            s.name
        );
    }
    if let Some(listed) = data["servers_listed"].as_u64() {
        let responded = servers.len();
        err += &tr_fmt(
            "{responded} de {listed} servidores respondieron",
            &[("responded", &responded), ("listed", &listed)],
        );
        err.push('\n');
    }
    let written = stdout.write_all(out.as_bytes());
    let _ = stderr.write_all(err.as_bytes());
    written
}

/// `argv` sin el nombre del programa. Devuelve el código de salida.
pub fn run(
    argv: &[String],
    env: &dyn Fn(&str) -> Option<String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    deps: &Deps,
) -> i32 {
    let _ = i18n::set_language(i18n::environment_language(&env));
    let matches = match build_command().try_get_matches_from(argv) {
        Ok(m) => m,
        Err(e) => {
            // --help y --version salen con 0 por stdout; los errores de uso, con 2.
            let code = e.exit_code();
            let text = e.render().to_string();
            let _ = if code == 0 {
                stdout.write_all(text.as_bytes())
            } else {
                // Como cli.py: «jase-cli: <mensaje>»; sin el uso ni la sugerencia de clap.
                let first = text.split("\n\n").next().unwrap_or("");
                let message = first.strip_prefix("error: ").unwrap_or(first);
                writeln!(stderr, "jase-cli: {}", message.trim_end())
            };
            return code;
        }
    };
    let Some((command, args)) = matches.subcommand() else {
        return 2; // subcommand_required: no ocurre
    };
    let mut scan_game = None;
    if command == "scan" {
        let id = args.get_one::<String>("game").map_or("", String::as_str);
        let Some(g) = game(id) else {
            let message = tr_fmt("Juego desconocido: {id}", &[("id", &id)]);
            let _ = writeln!(stderr, "jase-cli: {message}");
            return 2;
        };
        scan_game = Some(g);
    }
    let cache_dir = args
        .get_one::<PathBuf>("cache-dir")
        .cloned()
        .or_else(|| {
            env(CACHE_DIR_ENV)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| {
            let config = default_config_path();
            config.parent().unwrap_or(Path::new("")).join("cache")
        });
    let official = official(&cache_dir, !args.get_flag("no-update"), &deps.masters_fetch);
    let Some(game) = scan_game else {
        let written = if args.get_flag("json") {
            write_json(stdout, &games_json(&official))
        } else {
            let mut out = String::new();
            for g in GAMES.iter() {
                let key = if g.steam_webapi {
                    "  [Steam key]"
                } else if g.uses_steam_api() {
                    "  [Steam key optional]"
                } else {
                    ""
                };
                out += &format!("{:<10}  {}{key}\n", g.id, g.name);
            }
            stdout.write_all(out.as_bytes())
        };
        return exit_code(0, written);
    };
    run_scan(args, game, &official, &cache_dir, env, stdout, stderr, deps)
}
