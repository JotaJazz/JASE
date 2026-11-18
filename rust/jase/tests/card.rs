//! Ficha del servidor elegido y tablas de jugadores y reglas (`test_server_card.py` y las de
//! `test_main_window.py` que miran la ficha): todo sobre `card::vm` y `details::*`, lo mismo
//! que dibuja la vista.

mod support;

use std::sync::Arc;

use jase::actions::{Action, Msg as A};
use jase::app::{App, Message};
use jase::card::{self, CardVm};
use jase::colored::color_runs;
use jase::details::{self, PlayerCol, PlayerSort};
use jase::scan::ScanMsg;
use jase::sidebar;
use jase::table;
use jase::theme::{VARIANTS, palette, ping_color};
use jase_core::config::Config;
use jase_core::geo::CountryDB;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{clipboard_writes, fake_app};

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
fn star_does_nothing_until_5f() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[info()]);
    let before = format!("{:?}", app.config());
    let task = app.update(Message::Actions(A::Run(Action::Favorite)));
    assert!(iced_runtime::task::into_stream(task).is_none());
    assert_eq!(format!("{:?}", app.config()), before);
    assert!(!vm(&app).favorite);
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
