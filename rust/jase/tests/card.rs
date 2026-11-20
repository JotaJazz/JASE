//! Ficha del servidor elegido y tablas de jugadores y reglas (`test_server_card.py` y las de
//! `test_main_window.py` que miran la ficha): todo sobre `card::vm` y `details::*`, lo mismo
//! que dibuja la vista.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use iced::widget::image::Handle;
use iced::{Size, Task};
use jase::actions::{Action, Msg as A};
use jase::app::{App, Message};
use jase::card::{self, CardVm};
use jase::colored::color_runs;
use jase::details::{self, PlayerCol, PlayerSort};
use jase::scan::{self, ScanMsg};
use jase::sidebar;
use jase::table;
use jase::theme::{VARIANTS, palette, ping_color};
use jase_core::config::Config;
use jase_core::geo::CountryDB;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{clipboard_writes, fake_app, run_task};

/// El `info()` de `test_server_card.py`.
fn info() -> ServerInfo {
    let mut i = ServerInfo::new(
        ServerAddress::new("1.2.3.4", 7778),
        ServerAddress::new("1.2.3.4", 7777),
    );
    i.name = "Jimmy TDM".into();
    i.map = "mp_toujane".into();
    i.gametype = "tdm".into();
    i.num_players = 3;
    i.max_players = 20;
    i.version = "1.3".into();
    i.ping_ms = 42;
    i
}

fn complete(app: &mut App, servers: &[ServerInfo]) {
    let generation = app.scan.generation();
    for s in servers {
        let _ = app.update(Message::Scan {
            generation,
            msg: ScanMsg::Event(ScanEvent::Server(s.clone())),
        });
    }
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Finished(ScanResult {
            masters_ok: 1,
            servers_listed: servers.len(),
            servers_responded: servers.len(),
            errors: vec![],
        }),
    });
}

fn select(app: &mut App, address: &ServerAddress) {
    let _ = app.update(Message::Table(table::Msg::RowClicked(address.clone())));
}

/// App con `servers` y el primero elegido.
fn app_with(
    dir: &std::path::Path,
    cfg: Config,
    servers: &[ServerInfo],
) -> (support::TestApp, support::FakeScan) {
    let (mut app, fake) = fake_app(dir, cfg);
    complete(&mut app, servers);
    select(&mut app, &servers[0].address);
    (app, fake)
}

fn vm(app: &App) -> CardVm {
    card::vm(app).expect("hay servidor elegido")
}

/// Base de países falsa con `ip → code`.
fn load_countries(app: &mut App, dir: &std::path::Path, rows: &str) {
    let csv = dir.join("fake.csv");
    std::fs::write(&csv, rows).unwrap();
    let db = CountryDB::from_csv(&csv).unwrap();
    let request = app.country_request();
    let _ = app.update(Message::CountryDb {
        request,
        result: Ok(Arc::new(db)),
    });
}

#[test]
fn empty_state() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fake_app(dir.path(), Config::default());
    assert!(card::vm(&app).is_none());
    assert_eq!(card::empty_text(), "Selecciona un servidor");
}

#[test]
fn shows_server_details() {
    let dir = tempfile::tempdir().unwrap();
    let mut locked = info();
    locked.address = ServerAddress::new("5.6.7.8", 7778);
    locked.password = true;
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info(), locked.clone()]);
    let v = vm(&app);
    assert_eq!(v.name_tip, "Jimmy TDM");
    assert_eq!(v.address, "1.2.3.4:7777");
    assert_eq!(v.map, "mp_toujane");
    assert_eq!(v.mode, "tdm");
    assert_eq!(v.players, "3 / 20");
    assert_eq!(v.ping, "42 ms");
    assert!(v.ping_bold);
    assert_eq!(v.version, "1.3");
    assert!(!v.password);
    assert_eq!(v.placeholder, "mp_toujane");
    select(&mut app, &locked.address);
    assert!(vm(&app).password);
}

#[test]
fn ping_is_colored_by_theme() {
    let dir = tempfile::tempdir().unwrap();
    let mut slow = info();
    slow.ping_ms = 200;
    let (mut app, _) = app_with(dir.path(), Config::default(), &[slow]);
    for id in ["dark", "light"] {
        app.theme_id = id;
        assert_eq!(vm(&app).ping_color, ping_color(200, palette(id)), "{id}");
    }
}

#[test]
fn name_uses_color_codes() {
    let dir = tempfile::tempdir().unwrap();
    let mut i = info();
    i.name = "<b>Jimmy</b>".into();
    i.raw_name = "^1<b>Jimmy</b>".into();
    let (app, _) = app_with(dir.path(), Config::default(), &[i]);
    let red = palette(app.theme_id).q3[1];
    assert!(red.is_some());
    assert_eq!(vm(&app).name, [("<b>Jimmy</b>".to_owned(), red)]);
    assert_eq!(vm(&app).name_tip, "<b>Jimmy</b>");
}

#[test]
fn country_hidden_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info()]);
    assert_eq!(vm(&app).country, None);
    load_countries(&mut app, dir.path(), "1.2.3.4,1.2.3.4,AR\n");
    assert_eq!(
        vm(&app).country,
        Some((Some("ar".to_owned()), "Argentina".to_owned()))
    );
}

#[test]
fn country_without_flag_shows_name_only() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info()]);
    load_countries(&mut app, dir.path(), "1.2.3.4,1.2.3.4,QQ\n");
    assert_eq!(vm(&app).country, Some((None, "QQ".to_owned())));
}

#[test]
fn card_updates_when_db_arrives_after_selection() {
    let dir = tempfile::tempdir().unwrap();
    let mut other = info();
    other.address = ServerAddress::new("2.2.2.2", 7778);
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info(), other]);
    assert_eq!(vm(&app).country, None);
    load_countries(
        &mut app,
        dir.path(),
        "1.2.3.4,1.2.3.4,CL\n2.2.2.2,2.2.2.2,CL\n",
    );
    assert_eq!(vm(&app).country.map(|c| c.1), Some("Chile".to_owned()));
}

#[test]
fn card_shows_selected_server() {
    let dir = tempfile::tempdir().unwrap();
    let mut alpha = info();
    alpha.name = "Alpha".into();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, std::slice::from_ref(&alpha));
    assert!(card::vm(&app).is_none());
    select(&mut app, &alpha.address);
    assert_eq!(vm(&app).name, [("Alpha".to_owned(), None)]);
    // Clic derecho en la zona vacía suelta la selección: vuelve el estado vacío.
    let _ = app.update(Message::Table(table::Msg::RightClicked(None)));
    assert!(card::vm(&app).is_none());
}

#[test]
fn card_copy_reports_in_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info()]);
    // El botón *Copiar* manda la misma acción que el menú.
    let task = app.update(Message::Actions(A::Run(Action::Copy)));
    assert_eq!(app.status(), "Dirección copiada: 1.2.3.4:7777");
    assert_eq!(clipboard_writes(task), ["1.2.3.4:7777"]);
}

#[test]
fn cod2x_label_in_version_column_and_card() {
    let dir = tempfile::tempdir().unwrap();
    let mut i = info();
    i.version = "1.4".into();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, std::slice::from_ref(&i));
    let _ = app.update(Message::Sidebar(sidebar::Msg::Version("1.4".into())));
    select(&mut app, &i.address);
    assert_eq!(vm(&app).version, "1.4 (CoD2x)");
}

#[test]
fn refresh_selected_updates_row_and_card() {
    let dir = tempfile::tempdir().unwrap();
    let mut old = info();
    old.name = "Viejo".into();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[old.clone()]);
    let _ = app.update(Message::RefreshSelected);
    let mut new = old.clone();
    new.name = "Nuevo".into();
    new.ping_ms = 7;
    let request = app.scan.query_request();
    let _ = app.update(Message::Query {
        request,
        game: "cod2",
        result: Ok(Some(new)),
    });
    let v = vm(&app);
    assert_eq!(v.name, [("Nuevo".to_owned(), None)]);
    assert_eq!(v.ping, "7 ms");
}

#[test]
fn offline_server_card_shows_no_response() {
    let dir = tempfile::tempdir().unwrap();
    let mut off = info();
    off.online = false;
    off.version = String::new();
    // Un servidor sin respuesta solo se ve si es favorito.
    let mut cfg = Config::default();
    cfg.favorites
        .insert("cod2".into(), vec!["1.2.3.4:7778".into()]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    complete(&mut app, std::slice::from_ref(&off));
    // Sin versión no pasa el filtro de versión por defecto.
    let all = jase_core::games::ALL_VERSIONS.to_owned();
    let _ = app.update(Message::Sidebar(sidebar::Msg::Version(all)));
    select(&mut app, &off.address);
    let v = vm(&app);
    assert_eq!((v.map.as_str(), v.mode.as_str()), ("—", "—"));
    assert_eq!(v.players, "—");
    assert_eq!(v.ping, "Sin respuesta");
    assert_eq!(v.ping_color, palette(app.theme_id).muted);
    assert!(!v.ping_bold);
    assert_eq!(v.version, "—");
    assert!(v.favorite);
}

#[test]
fn favorite_star_and_note_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let mut other = info();
    other.address = ServerAddress::new("2.2.2.2", 7778);
    let mut cfg = Config::default();
    cfg.favorites
        .insert("cod2".into(), vec!["1.2.3.4:7778".into()]);
    let notes = [
        ("1.2.3.4:7778".to_owned(), "clan <b>".to_owned()),
        ("2.2.2.2:7778".to_owned(), "no es favorito".to_owned()),
    ];
    cfg.favorite_notes
        .insert("cod2".into(), notes.into_iter().collect());
    let (mut app, _) = app_with(dir.path(), cfg, &[info(), other.clone()]);
    let v = vm(&app);
    assert!(v.favorite);
    assert_eq!(v.note, "clan <b>");
    // La nota solo se muestra en un favorito.
    select(&mut app, &other.address);
    let v = vm(&app);
    assert!(!v.favorite);
    assert_eq!(v.note, "");
}

#[test]
fn rules_panel_strips_color_codes() {
    let dir = tempfile::tempdir().unwrap();
    let mut i = info();
    i.rules.insert("sv_hostname".into(), "^1Jimmy^7 TDM".into());
    i.rules.insert("_Admin".into(), "^7Tit^5a^4N".into());
    let (app, _) = app_with(dir.path(), Config::default(), &[i]);
    let server = app.selected_server().unwrap();
    assert_eq!(
        details::rules_rows(server),
        [
            ("_Admin".to_owned(), "TitaN".to_owned()),
            ("sv_hostname".to_owned(), "Jimmy TDM".to_owned()),
        ]
    );
    assert_eq!(server.rules["_Admin"], "^7Tit^5a^4N"); // el dato crudo no se toca
}

#[test]
fn server_and_player_names_are_colored() {
    let dir = tempfile::tempdir().unwrap();
    let mut i = info();
    let mut p = Player::new("Sgt.Jimmy", 5, 40);
    p.raw_name = "Sgt.^2Jimmy".into();
    i.players.push(p);
    let (app, _) = app_with(dir.path(), Config::default(), &[i]);
    let rows = details::players_rows(app.selected_server().unwrap(), PlayerSort::default());
    assert_eq!(rows[0].name, "Sgt.Jimmy");
    let runs = color_runs(&rows[0].raw_name, palette(app.theme_id));
    let texts: Vec<&str> = runs.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(texts, ["Sgt.", "Jimmy"]);
}

#[test]
fn players_sort_matches_qt() {
    let mut i = info();
    for (name, score, ping) in [
        ("a", 5, 30),
        ("B", 1, 90),
        ("c", 5, 10),
        ("a", 2, 30),
        ("é", 3, 50),
    ] {
        i.players.push(Player::new(name, score, ping));
    }
    let key = |rows: Vec<&Player>| -> Vec<(String, i32)> {
        rows.iter().map(|p| (p.name.clone(), p.score)).collect()
    };
    let s = |n: &str, sc: i32| (n.to_owned(), sc);
    let initial = PlayerSort::default();
    assert_eq!(initial.col, PlayerCol::Name);
    assert!(!initial.ascending);
    // Descendente por unidades UTF-16 (mayúsculas antes que minúsculas); empates estables.
    assert_eq!(
        key(details::players_rows(&i, initial)),
        [s("é", 3), s("c", 5), s("a", 5), s("a", 2), s("B", 1)]
    );
    let score = details::click(initial, PlayerCol::Score);
    assert_eq!(
        score,
        PlayerSort {
            col: PlayerCol::Score,
            ascending: true
        }
    );
    assert_eq!(
        key(details::players_rows(&i, score)),
        [s("B", 1), s("a", 2), s("é", 3), s("a", 5), s("c", 5)]
    );
    let score_desc = details::click(score, PlayerCol::Score);
    assert!(!score_desc.ascending);
    assert_eq!(
        key(details::players_rows(&i, score_desc)),
        [s("a", 5), s("c", 5), s("é", 3), s("a", 2), s("B", 1)]
    );
    let ping = details::click(score_desc, PlayerCol::Ping);
    assert_eq!(
        ping,
        PlayerSort {
            col: PlayerCol::Ping,
            ascending: true
        }
    );
    let pings: Vec<i32> = details::players_rows(&i, ping)
        .iter()
        .map(|p| p.ping)
        .collect();
    assert_eq!(pings, [10, 30, 30, 50, 90]);
    let name = details::click(ping, PlayerCol::Name);
    assert!(name.ascending);
    assert_eq!(
        key(details::players_rows(&i, name)),
        [s("B", 1), s("a", 5), s("a", 2), s("c", 5), s("é", 3)]
    );
}

#[test]
fn player_sort_message_updates_app() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info()]);
    assert_eq!(app.player_sort, PlayerSort::default());
    let _ = app.update(Message::PlayerSort(PlayerCol::Ping));
    assert_eq!(
        app.player_sort,
        PlayerSort {
            col: PlayerCol::Ping,
            ascending: true
        }
    );
}

const HOSTILE: &str = "<img src=\"file:///x.png\"><h1>VIP</h1>\nsegunda ^1línea";

#[test]
fn remote_text_is_plain_in_card_and_details() {
    let dir = tempfile::tempdir().unwrap();
    let long = "x".repeat(300);
    let mut i = info();
    i.name = HOSTILE.into();
    i.raw_name = String::new();
    i.map = HOSTILE.into();
    i.gametype = long.clone();
    i.rules.insert(HOSTILE.into(), HOSTILE.into());
    i.players.push(Player::new(HOSTILE, 1, 2));
    i.players.push(Player::new("", 0, 0));
    let mut empty = info();
    empty.address = ServerAddress::new("9.9.9.9", 7778);
    empty.name = String::new();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[i, empty.clone()]);
    let v = vm(&app);
    // Tal cual: ni HTML ni recortes en los datos.
    assert_eq!(v.name, [(HOSTILE.to_owned(), None)]);
    assert_eq!(v.name_tip, HOSTILE);
    assert_eq!(v.map, HOSTILE);
    assert_eq!(v.mode, long);
    assert_eq!(v.placeholder, HOSTILE);
    let server = app.selected_server().unwrap().clone();
    let rules = details::rules_rows(&server);
    assert_eq!(rules[0].0, HOSTILE);
    // `strip_colors` también quita los caracteres de control (el salto de línea).
    assert_eq!(
        rules[0].1,
        "<img src=\"file:///x.png\"><h1>VIP</h1>segunda línea"
    );
    assert_eq!(
        details::players_rows(&server, PlayerSort::default()).len(),
        2
    );
    for id in ["dark", "classic", "xp", "goldsrc", "terminal_green"] {
        app.theme_id = id;
        let _ = card::view(&app);
        let _ = details::view(&app);
        let _ = app.view();
    }
    select(&mut app, &empty.address);
    assert_eq!(vm(&app).name, [(String::new(), None)]);
    let _ = card::view(&app);
}

#[test]
fn card_follows_hidden_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info()]);
    assert!(card::vm(&app).is_some());
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text("zzz".into())));
    assert!(card::vm(&app).is_none());
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text(String::new())));
    select(&mut app, &info().address);
    assert!(card::vm(&app).is_some());
    let _ = app.update(Message::Refresh);
    assert!(card::vm(&app).is_none());
}

#[test]
fn card_view_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let mut off = info();
    off.address = ServerAddress::new("2.2.2.2", 7778);
    off.online = false;
    let mut locked = info();
    locked.address = ServerAddress::new("3.3.3.3", 7778);
    locked.password = true;
    locked.players.push(Player::new("p", 1, 2));
    locked.rules.insert("k".into(), "v".into());
    let mut cfg = Config::default();
    cfg.favorites
        .insert("cod2".into(), vec!["2.2.2.2:7778".into()]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    complete(&mut app, &[info(), off.clone(), locked.clone()]);
    select(&mut app, &off.address);
    assert_eq!(
        card::vm(&app).map(|v| v.ping),
        Some("Sin respuesta".to_owned())
    );
    let _ = app.update(Message::Table(table::Msg::RightClicked(None)));
    let _ = app.update(Message::Actions(A::Close));
    for (id, _, _) in VARIANTS.iter() {
        app.theme_id = id;
        assert!(card::vm(&app).is_none());
        let _ = card::view(&app);
        let _ = details::view(&app);
        for a in [&info().address, &off.address, &locked.address] {
            select(&mut app, a);
            let _ = card::view(&app);
            let _ = details::view(&app);
            let _ = app.view();
        }
        let _ = app.update(Message::Table(table::Msg::RightClicked(None)));
        let _ = app.update(Message::Actions(A::Close));
    }
    assert_eq!(card::height(&app), card::CARD_HEIGHT);
    assert_eq!(card::CARD_HEIGHT, card::MAP_IMAGE_SIZE.height + 18.0);
}

// --- Imagen del mapa ---

/// PNG de 4 × 3 de un solo color.
fn solid_png(path: &Path, rgb: [u8; 3]) -> PathBuf {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 4, 3);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().unwrap();
    w.write_image_data(&rgb.repeat(12)).unwrap();
    w.finish().unwrap();
    path.to_path_buf()
}

/// Color del primer píxel de la imagen de la ficha.
fn shown(app: &App) -> Option<[u8; 4]> {
    match card::map_image(app).image.as_ref()? {
        Handle::Rgba { pixels, .. } => Some([pixels[0], pixels[1], pixels[2], pixels[3]]),
        other => panic!("se esperaba RGBA: {other:?}"),
    }
}

/// Corre la `Task` y le pasa sus mensajes a la app.
fn run_into(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        let _ = app.update(m);
    }
}

fn select_task(app: &mut App, address: &ServerAddress) -> Task<Message> {
    app.update(Message::Table(table::Msg::RowClicked(address.clone())))
}

/// Corre la `Task` en otro hilo (como el runtime de Iced): el test sigue mientras espera.
fn spawn_task(task: Task<Message>) -> std::thread::JoinHandle<Vec<Message>> {
    std::thread::spawn(move || run_task(task))
}

fn at(host: &str, map: &str) -> ServerInfo {
    let mut i = info();
    i.address = ServerAddress::new(host, 7778);
    i.map = map.into();
    i
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

#[test]
fn card_loads_map_image_off_ui_thread() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.game_paths.insert("cod2".into(), "D:/CoD2".into());
    cfg.download_map_images = true;
    let (mut app, fake) = fake_app(dir.path(), cfg);
    let red = solid_png(&dir.path().join("a.png"), [255, 0, 0]);
    fake.maps
        .paths
        .lock()
        .unwrap()
        .insert("mp_toujane".into(), red);
    complete(&mut app, &[info()]);
    let task = select_task(&mut app, &info().address);
    run_into(&mut app, task);
    assert_eq!(
        fake.maps.calls(),
        [(
            "cod2",
            Some("D:/CoD2".to_owned()),
            "mp_toujane".to_owned(),
            true
        )]
    );
    assert_ne!(
        fake.maps.threads.lock().unwrap()[0],
        std::thread::current().id()
    );
    assert_eq!(shown(&app), Some(RED));
}

#[test]
fn map_image_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[info()]);
    let task = select_task(&mut app, &info().address);
    assert!(fake.maps.calls().is_empty());
    run_into(&mut app, task);
    assert_eq!(fake.maps.calls().len(), 1);
}

#[test]
fn stale_map_image_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (at("1.1.1.1", "mp_slow"), at("2.2.2.2", "mp_fast"));
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let red = solid_png(&dir.path().join("a.png"), [255, 0, 0]);
    let green = solid_png(&dir.path().join("b.png"), [0, 255, 0]);
    {
        let mut paths = fake.maps.paths.lock().unwrap();
        paths.insert("mp_slow".into(), red.clone());
        paths.insert("mp_fast".into(), green);
    }
    fake.maps.block("mp_slow");
    complete(&mut app, &[a.clone(), b.clone()]);
    let slow = spawn_task(select_task(&mut app, &a.address));
    fake.maps.wait_calls(1); // A ya corre: no se puede abortar
    let old = card::map_image(&app).request;
    let task = select_task(&mut app, &b.address);
    run_into(&mut app, task);
    assert_eq!(shown(&app), Some(GREEN));
    fake.maps.open();
    for m in slow.join().unwrap() {
        let _ = app.update(m);
    }
    // Lo que A hubiera entregado, con su pedido viejo, no cambia nada.
    let _ = app.update(Message::MapImage {
        request: old,
        image: card::decode_map_png(&red),
    });
    assert_eq!(shown(&app), Some(GREEN));
    assert_eq!(app.selected_server().unwrap().address.host, "2.2.2.2");
}

#[test]
fn card_placeholder_when_no_image() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[info()]);
    let task = select_task(&mut app, &info().address);
    run_into(&mut app, task);
    assert_eq!(fake.maps.done.lock().unwrap().as_slice(), ["mp_toujane"]);
    assert_eq!(shown(&app), None);
    assert_eq!(vm(&app).placeholder, "mp_toujane");
}

/// Respuesta de «Refrescar este servidor» con `info`.
fn refreshed(app: &mut App, info: ServerInfo) -> Task<Message> {
    let _ = app.update(Message::RefreshSelected);
    let request = app.scan.query_request();
    app.update(Message::Query {
        request,
        game: "cod2",
        result: Ok(Some(info)),
    })
}

#[test]
fn card_image_requested_once_per_server() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[info()]);
    let task = select_task(&mut app, &info().address);
    run_into(&mut app, task);
    app.theme_id = "light";
    let mut more = info();
    more.num_players = 5;
    let task = refreshed(&mut app, more);
    run_into(&mut app, task);
    assert_eq!(fake.maps.calls().len(), 1);
    let task = refreshed(&mut app, at("1.2.3.4", "mp_other"));
    run_into(&mut app, task);
    assert_eq!(fake.maps.maps_called(), ["mp_toujane", "mp_other"]);
}

#[test]
fn new_server_resets_image_but_same_server_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let red = solid_png(&dir.path().join("a.png"), [255, 0, 0]);
    fake.maps
        .paths
        .lock()
        .unwrap()
        .insert("mp_toujane".into(), red);
    complete(&mut app, &[info()]);
    let task = select_task(&mut app, &info().address);
    run_into(&mut app, task);
    let mut more = info();
    more.num_players = 5;
    let _ = refreshed(&mut app, more);
    assert_eq!(shown(&app), Some(RED));
    let _ = refreshed(&mut app, at("1.2.3.4", "mp_other"));
    assert_eq!(shown(&app), None);
    assert_eq!(vm(&app).placeholder, "mp_other");
}

#[test]
fn placeholder_shows_map_name() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = app_with(
        dir.path(),
        Config::default(),
        &[at("1.2.3.4", "mp_carentan")],
    );
    assert_eq!(shown(&app), None);
    assert_eq!(vm(&app).placeholder, "mp_carentan");
    let _ = card::view(&app);
}

#[test]
fn map_image_scaled_keeping_aspect() {
    assert_eq!(card::fit_size(1024, 1024), Size::new(120.0, 120.0));
    assert_eq!(card::fit_size(480, 360), Size::new(160.0, 120.0));
    assert_eq!(card::fit_size(80, 40), Size::new(160.0, 80.0));
}

#[test]
fn default_map_service_uses_cache_next_to_config() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        scan::map_cache_dir(&dir.path().join("config.json")),
        dir.path().join("cache").join("maps")
    );
}

#[test]
fn close_mid_map_task_is_prompt_and_harmless() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    fake.maps.block("mp_toujane");
    complete(&mut app, &[info()]);
    let pending = spawn_task(select_task(&mut app, &info().address));
    fake.maps.wait_calls(1);
    let _ = app.update(Message::Close { maximized: false });
    // Salió con el servicio todavía en la barrera: no lo esperó.
    assert_eq!(fake.exits.lock().unwrap().as_slice(), [0]);
    fake.maps.open();
    for m in pending.join().unwrap() {
        let _ = app.update(m);
    }
}

#[test]
fn only_two_map_requests_run_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let servers: Vec<ServerInfo> = (0..5)
        .map(|n| at(&format!("1.1.1.{n}"), &format!("m{n}")))
        .collect();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    for s in &servers {
        fake.maps.block(&s.map);
    }
    complete(&mut app, &servers);
    let mut threads = Vec::new();
    for (n, s) in servers.iter().enumerate() {
        threads.push(spawn_task(select_task(&mut app, &s.address)));
        if n < 2 {
            fake.maps.wait_calls(n + 1); // los dos primeros toman los dos permisos
        } else {
            // Los que ya corren retienen su permiso aunque se suelte su `Task`.
            assert!(!fake.maps.calls_within(3, Duration::from_millis(300)));
        }
    }
    // Los tres que esperan permiso no llaman; el de m2 y m3 se soltó al cambiar de servidor.
    assert_eq!(fake.maps.calls().len(), 2);
    fake.maps.open();
    fake.maps.wait_calls(3);
    let mut msgs = Vec::new();
    for t in threads {
        msgs.extend(t.join().unwrap());
    }
    assert_eq!(fake.maps.maps_called(), ["m0", "m1", "m4"]);
    assert!(fake.maps.max_running.load(Ordering::SeqCst) <= 2);
    assert_eq!(msgs.len(), 1, "solo responde el pedido vigente");
}

/// JPEG de 8 × 8 gris (Pillow, `quality=50, optimize=True`).
const JPEG_HEX: &str = "ffd8ffe000104a46494600010100000100010000ffdb004300100b0c0e0c0a100e0d0e1211101318281a181616183123251d283a333d3c3933383740485c4e404457453738506d51575f626768673e4d71797064785c656763ffc0000b080008000801011100ffc40014000100000000000000000000000000000004ffc4001e100001030403000000000000000000000001020304000511122171b1ffda0008010100003f003a62404b69312d325d5e790eead8c7636f2bffd9";

#[test]
fn corrupt_cached_image_shows_placeholder() {
    let dir = tempfile::tempdir().unwrap();
    let good = std::fs::read(solid_png(&dir.path().join("good.png"), [1, 2, 3])).unwrap();
    let jpeg: Vec<u8> = (0..JPEG_HEX.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&JPEG_HEX[i..i + 2], 16).unwrap())
        .collect();
    // Un JPEG de verdad: lo rechaza por no ser PNG, no por estar roto.
    assert!(jase_core::maps::service::decode_image(&jpeg).is_some());
    let cases: [(&str, &[u8]); 4] = [
        ("empty.png", b""),
        ("junk.png", b"no soy una imagen"),
        ("cut.png", &good[..good.len() / 2]),
        ("photo.png", &jpeg),
    ];
    for (name, bytes) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        assert!(card::decode_map_png(&path).is_none(), "{name}");
    }
    assert!(card::decode_map_png(&dir.path().join("missing.png")).is_none());
    assert!(card::decode_map_png(&dir.path().join("good.png")).is_some());
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    fake.maps
        .paths
        .lock()
        .unwrap()
        .insert("mp_toujane".into(), dir.path().join("cut.png"));
    complete(&mut app, &[info()]);
    let task = select_task(&mut app, &info().address);
    run_into(&mut app, task);
    assert_eq!(fake.maps.calls().len(), 1);
    assert_eq!(shown(&app), None);
    assert_eq!(vm(&app).placeholder, "mp_toujane");
    let _ = card::view(&app);
}

#[test]
fn offline_or_mapless_server_requests_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut off = info();
    off.online = false;
    let mapless = at("2.2.2.2", "");
    let mut cfg = Config::default();
    cfg.favorites
        .insert("cod2".into(), vec!["1.2.3.4:7778".into()]);
    let (mut app, fake) = fake_app(dir.path(), cfg);
    complete(&mut app, &[off.clone(), mapless.clone()]);
    for a in [&off.address, &mapless.address] {
        let task = select_task(&mut app, a);
        run_into(&mut app, task);
        assert!(card::vm(&app).is_some());
    }
    assert!(fake.maps.calls().is_empty());
}

#[test]
fn hidden_selection_cancels_map_request() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let red = solid_png(&dir.path().join("a.png"), [255, 0, 0]);
    fake.maps
        .paths
        .lock()
        .unwrap()
        .insert("mp_toujane".into(), red.clone());
    fake.maps.block("mp_toujane");
    complete(&mut app, &[info()]);
    let pending = spawn_task(select_task(&mut app, &info().address));
    fake.maps.wait_calls(1);
    let old = card::map_image(&app).request;
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text("zzz".into())));
    assert_eq!(card::map_image(&app).key, None);
    fake.maps.open();
    for m in pending.join().unwrap() {
        let _ = app.update(m);
    }
    let _ = app.update(Message::MapImage {
        request: old,
        image: card::decode_map_png(&red),
    });
    assert_eq!(shown(&app), None);
}

#[test]
fn lower_pane_keeps_room_for_table_header_and_a_row() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    for id in ["dark", "classic", "xp"] {
        app.theme_id = id;
        assert_eq!(
            card::lower_min_height(&app),
            card::height(&app) + 6.0 + table::header_height(id) + table::ROW_HEIGHT,
            "{id}"
        );
    }
}
