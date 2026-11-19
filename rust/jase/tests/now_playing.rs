//! *Ahora se juega* (`tests/test_now_playing.py`): orden, escaneo de los juegos listos y la
//! caché. Las ventanas no se abren; los escaneos falsos traen lo que dice `FakeScan.scripts`.

mod support;

use iced::Task;
use iced::keyboard::{Key, key::Named};
use iced::window;
use indexmap::IndexMap;
use jase::app::{App, Message, NowMsg, Tool};
use jase::now_playing::{MAX_ROWS, busy_servers, status_text};
use jase::scan::ScanMsg;
use jase::table::ROW_HEIGHT;
use jase::theme::VARIANTS;
use jase_core::config::Config;
use jase_core::models::ServerInfo;
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use jase_core::server_cache;
use support::{
    FakeScan, TestApp, element_texts, fake_app_with_key, mk, run_task, widget_ops, window_actions,
};

/// Como `srv` de Python: `people` personas y `bots` bots.
fn srv(host: &str, name: &str, people: i32, bots: i32, ping: u32) -> ServerInfo {
    let mut info = mk(host, name, ping, people + bots, "1.3");
    info.bots = bots;
    info
}

fn lists(entries: &[(&str, Vec<ServerInfo>)]) -> IndexMap<String, Vec<ServerInfo>> {
    entries
        .iter()
        .map(|(g, s)| ((*g).to_owned(), s.clone()))
        .collect()
}

#[test]
fn busy_servers_counts_people_and_sorts() {
    let mut off = srv("6.6.6.6", "caído", 9, 0, 10);
    off.online = false;
    let l = lists(&[
        (
            "cod2",
            vec![
                srv("1.1.1.1", "a", 3, 0, 40),
                srv("2.2.2.2", "b", 0, 8, 40),
                srv("3.3.3.3", "c", 0, 0, 40),
                off,
            ],
        ),
        (
            "q3",
            vec![srv("4.4.4.4", "d", 5, 0, 90), srv("5.5.5.5", "e", 5, 0, 20)],
        ),
    ]);
    let busy = busy_servers(&l, MAX_ROWS);
    let rows: Vec<(&str, &str)> = busy
        .iter()
        .map(|(g, i)| (g.as_str(), i.address.host.as_str()))
        .collect();
    assert_eq!(
        rows,
        [("q3", "5.5.5.5"), ("q3", "4.4.4.4"), ("cod2", "1.1.1.1")]
    );
    let many = lists(&[(
        "cod2",
        (0..9)
            .map(|i| srv(&format!("1.1.1.{i}"), "x", 1, 0, 40))
            .collect(),
    )]);
    assert_eq!(busy_servers(&many, 4).len(), 4);
}

#[test]
fn now_status_counts_people_and_pending() {
    let _lang = support::lang::Spanish::hold();
    let l = lists(&[
        ("cod2", vec![srv("1.1.1.1", "a", 3, 2, 40)]),
        ("q3", vec![srv("2.2.2.2", "b", 5, 0, 40)]),
    ]);
    assert_eq!(
        status_text(&l, 2),
        "8 jugadores en tus juegos · escaneando 2 juegos…"
    );
    let one = lists(&[("cod2", vec![srv("1.1.1.1", "a", 1, 0, 40)])]);
    assert_eq!(status_text(&one, 0), "1 jugador en tus juegos");
    assert_eq!(
        status_text(&one, 1),
        "1 jugador en tus juegos · escaneando 1 juego…"
    );
}

const STEAM: [&str; 5] = ["l4d", "l4d2", "css", "tf2", "gmod"];

fn paths(games: &[&str]) -> Config {
    Config {
        game_paths: games
            .iter()
            .map(|g| ((*g).to_owned(), format!("/{g}")))
            .collect(),
        ..Config::default()
    }
}

/// App con la clave de Steam ya leída y lo que trae cada escaneo (`masters_ok` para todos).
fn now_app(
    dir: &std::path::Path,
    cfg: Config,
    key: &str,
    scripts: &[(&'static str, Vec<ServerInfo>)],
    masters_ok: usize,
) -> (TestApp, FakeScan) {
    let (mut app, fake, _) = fake_app_with_key(dir, cfg, key);
    let _ = app.update(Message::SteamKey(key.to_owned()));
    script(&fake, scripts, masters_ok);
    (app, fake)
}

/// Lo que traen los escaneos de esos juegos (los de Steam no nombrados, nada).
fn script(fake: &FakeScan, scripts: &[(&'static str, Vec<ServerInfo>)], masters_ok: usize) {
    let mut s = fake.scripts.lock().unwrap();
    for g in STEAM {
        s.insert(g, (Vec::new(), masters_ok));
    }
    for (g, servers) in scripts {
        s.insert(g, (servers.clone(), masters_ok));
    }
}

/// Termina el escaneo principal de cod2 con esos servidores.
fn complete(app: &mut App, servers: &[ServerInfo]) {
    let generation = app.scan.generation();
    for info in servers {
        let msg = ScanMsg::Event(ScanEvent::Server(info.clone()));
        let _ = app.update(Message::Scan { generation, msg });
    }
    let msg = ScanMsg::Finished(ScanResult {
        masters_ok: 1,
        ..ScanResult::default()
    });
    let _ = app.update(Message::Scan { generation, msg });
}

/// Corre la `Task` y entrega sus mensajes de *Ahora se juega*, con los escaneos que estos
/// arrancan, hasta que no quede nada.
fn drive(app: &mut App, task: Task<Message>) {
    let mut pending = vec![task];
    while let Some(t) = pending.pop() {
        for m in now_messages(t) {
            pending.push(app.update(m));
        }
    }
}

fn now_messages(task: Task<Message>) -> Vec<Message> {
    run_task(task)
        .into_iter()
        .filter(|m| matches!(m, Message::Now(_)))
        .collect()
}

fn open(app: &mut App) -> Task<Message> {
    app.update(Message::Now(NowMsg::Open))
}

fn now_id(app: &App) -> window::Id {
    app.tool_window(&Tool::NowPlaying)
        .expect("ventana de Ahora se juega")
}

fn names(app: &App) -> Vec<String> {
    app.now_rows().into_iter().map(|r| r.name).collect()
}

/// Llamadas al escáner sin la del arranque (cod2).
fn scans(fake: &FakeScan) -> Vec<&'static str> {
    fake.calls().into_iter().skip(1).collect()
}

fn saved_index(dir: &std::path::Path) -> Vec<String> {
    let index = server_cache::read_index(&dir.join("cache").join("servers"));
    index.keys().cloned().collect()
}

fn selected_name(app: &App) -> Option<String> {
    app.selected_server().map(|s| s.name.clone())
}

#[test]
fn scans_ready_games_reuses_session_and_caches() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = now_app(
        dir.path(),
        paths(&["cod2", "cod1"]),
        "k",
        &[
            ("cod1", vec![srv("1.1.1.1", "Uno", 2, 0, 40)]),
            ("l4d2", vec![srv("2.2.2.2", "Seis", 6, 0, 40)]),
        ],
        1,
    );
    complete(&mut app, &[srv("3.3.3.3", "Ya cargado", 1, 0, 40)]);
    let ready: Vec<&str> = app.now_playing_games().iter().map(|g| g.id).collect();
    assert_eq!(ready, ["cod2", "cod1", "l4d", "l4d2", "css", "tf2", "gmod"]);
    let task = open(&mut app);
    // cod2 tiene lista de la sesión; de los otros seis, tres a la vez.
    assert_eq!(scans(&fake), ["cod1", "l4d", "l4d2"]);
    assert!(app.now_busy());
    assert!(app.now_status().ends_with("escaneando 6 juegos…"));
    let mut msgs = now_messages(task).into_iter();
    // El primero que termina deja lugar al cuarto.
    loop {
        let m = msgs.next().expect("termina un escaneo");
        let done = matches!(
            &m,
            Message::Now(NowMsg::Scan {
                msg: ScanMsg::Finished(_),
                ..
            })
        );
        let next = app.update(m);
        if done {
            assert_eq!(scans(&fake).len(), 4);
            drive(&mut app, next);
            break;
        }
    }
    for m in msgs {
        let next = app.update(m);
        drive(&mut app, next);
    }
    assert!(!app.now_busy());
    let mut scanned = scans(&fake);
    scanned.sort_unstable();
    let mut expected = vec!["cod1", "l4d", "l4d2", "css", "tf2", "gmod"];
    expected.sort_unstable();
    assert_eq!(scanned, expected);
    assert_eq!(names(&app), ["Seis", "Uno", "Ya cargado"]);
    assert_eq!(app.now_status(), "9 jugadores en tus juegos");
    // La lista nueva sirve a la ventana y queda para la próxima sesión.
    let session = app.searchable_lists();
    assert_eq!(session["cod1"][0].name, "Uno");
    assert_eq!(session["l4d2"][0].name, "Seis");
    app.saved.flush();
    assert!(saved_index(dir.path()).contains(&"cod1".to_owned()));
    let _ = app.update(Message::Now(NowMsg::Choose(0)));
    assert_eq!(app.current_game().map(|g| g.id), Some("l4d2"));
    assert_eq!(selected_name(&app).as_deref(), Some("Seis"));
}

#[test]
fn refresh_forces_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = now_app(dir.path(), paths(&["cod2", "cod1"]), "", &[], 1);
    complete(&mut app, &[]);
    let task = open(&mut app);
    drive(&mut app, task);
    assert_eq!(scans(&fake), ["cod1"]);
    let task = app.update(Message::Now(NowMsg::Refresh));
    drive(&mut app, task);
    assert_eq!(scans(&fake), ["cod1", "cod2", "cod1"]);
}

#[test]
fn no_ready_games_explains() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = now_app(dir.path(), paths(&[]), "", &[], 1);
    let ready: Vec<&str> = app.now_playing_games().iter().map(|g| g.id).collect();
    assert!(ready.is_empty(), "{ready:?}"); // sin rutas ni clave de Steam
    let _ = open(&mut app);
    assert!(app.now_status().starts_with("No hay juegos listos"));
    assert!(scans(&fake).is_empty());
    assert!(!app.now_busy());
}

#[test]
fn incomplete_scan_is_not_cached_and_close_stops_queue() {
    let dir = tempfile::tempdir().unwrap();
    let nine = vec![srv("9.9.9.9", "Nueve", 3, 0, 40)];
    let (mut app, fake) = now_app(
        dir.path(),
        paths(&["cod1", "q3", "et", "urt"]),
        "",
        &[
            ("cod1", nine.clone()),
            ("q3", nine.clone()),
            ("et", nine.clone()),
            ("urt", nine),
        ],
        0,
    );
    let task = open(&mut app);
    let id = now_id(&app);
    assert_eq!(scans(&fake), ["cod1", "q3", "et"]);
    // Lo que el escaneo ya mandó y aún no llegó cuando se cierra la ventana.
    let late = now_messages(task);
    assert!(!late.is_empty());
    let _ = app.update(Message::WindowClosed(id));
    assert!(!app.now_busy()); // sin escaneos en curso ni cola
    for m in late {
        let next = app.update(m);
        assert!(now_messages(next).is_empty());
    }
    assert_eq!(scans(&fake).len(), 3); // el cuarto nunca arrancó
    assert!(app.now_rows().is_empty());
    // Abortados: si el runtime los corre igual, no traen nada.
    let task = open(&mut app);
    let id = now_id(&app);
    let _ = app.update(Message::WindowClosed(id));
    assert!(now_messages(task).is_empty());
    // Sin un master que respondiera la lista no se da por completa.
    let task = open(&mut app);
    drive(&mut app, task);
    assert_eq!(names(&app), ["Nueve", "Nueve", "Nueve", "Nueve"]);
    assert!(!app.searchable_lists().contains_key("cod1"));
}

#[test]
fn stale_now_scan_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = now_app(
        dir.path(),
        paths(&["cod1"]),
        "",
        &[("cod1", vec![srv("1.1.1.1", "Viejo", 2, 0, 40)])],
        1,
    );
    let old = now_messages(open(&mut app));
    let refresh = app.update(Message::Now(NowMsg::Refresh));
    script(
        &fake,
        &[("cod1", vec![srv("2.2.2.2", "Nuevo", 2, 0, 40)])],
        1,
    );
    for m in old {
        let next = app.update(m);
        assert!(now_messages(next).is_empty());
    }
    assert!(app.now_rows().is_empty());
    assert!(app.now_busy());
    drive(&mut app, refresh);
    assert_eq!(names(&app), ["Viejo"]); // el escaneo de *Actualizar* (pedido antes del cambio)
    let task = app.update(Message::Now(NowMsg::Refresh));
    drive(&mut app, task);
    assert_eq!(names(&app), ["Nuevo"]);
}

#[test]
fn now_keeps_selected_row() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = now_app(
        dir.path(),
        paths(&["cod4", "q3"]),
        "",
        &[
            ("cod4", vec![srv("1.1.1.1", "Cinco", 5, 0, 40)]),
            ("q3", vec![srv("2.2.2.2", "Nueve", 9, 0, 40)]),
        ],
        1,
    );
    let msgs = now_messages(open(&mut app));
    let id = now_id(&app);
    let (cod4, rest): (Vec<_>, Vec<_>) = msgs
        .into_iter()
        .partition(|m| matches!(m, Message::Now(NowMsg::Scan { game: "cod4", .. })));
    for m in cod4 {
        let _ = app.update(m);
    }
    assert_eq!(names(&app), ["Cinco"]);
    let _ = app.update(Message::Now(NowMsg::Select(0)));
    for m in rest {
        let _ = app.update(m);
    }
    assert_eq!(names(&app), ["Nueve", "Cinco"]);
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::Enter)));
    assert_eq!(app.current_game().map(|g| g.id), Some("cod4"));
    assert_eq!(selected_name(&app).as_deref(), Some("Cinco"));
}

#[test]
fn now_choose_reveals_server() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = now_app(
        dir.path(),
        paths(&["cod4", "q3"]),
        "",
        &[
            ("cod4", vec![srv("1.1.1.1", "Uno", 1, 0, 40)]),
            ("q3", vec![srv("2.2.2.2", "Dos", 2, 0, 40)]),
        ],
        1,
    );
    let task = open(&mut app);
    let id = now_id(&app);
    drive(&mut app, task);
    assert_eq!(names(&app), ["Dos", "Uno"]);
    let task = app.update(Message::Now(NowMsg::Choose(1)));
    assert!(window_actions(task).is_empty()); // la ventana sigue abierta
    assert_eq!(app.current_game().map(|g| g.id), Some("cod4"));
    assert_eq!(selected_name(&app).as_deref(), Some("Uno"));
    // ↑ y Enter desde la tabla de la ventana.
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::ArrowUp)));
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::Enter)));
    assert_eq!(app.current_game().map(|g| g.id), Some("q3"));
    assert_eq!(selected_name(&app).as_deref(), Some("Dos"));
    assert_eq!(app.tool_window(&Tool::NowPlaying), Some(id));
}

#[test]
fn now_down_without_selection_picks_first() {
    // Como la tabla de Qt: ↓ sin fila actual elige la primera.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = now_app(
        dir.path(),
        paths(&["cod4"]),
        "",
        &[("cod4", vec![srv("1.1.1.1", "Uno", 1, 0, 40)])],
        1,
    );
    let task = open(&mut app);
    let id = now_id(&app);
    drive(&mut app, task);
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::ArrowDown)));
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::Enter)));
    assert_eq!(selected_name(&app).as_deref(), Some("Uno"));
}

#[test]
fn now_choose_waits_for_scanning_game() {
    let dir = tempfile::tempdir().unwrap();
    let x = srv("2.2.2.2", "X", 3, 0, 40);
    let (mut app, _) = now_app(
        dir.path(),
        paths(&["cod2"]),
        "",
        &[("cod2", vec![x.clone()])],
        1,
    );
    let _ = open(&mut app);
    assert!(app.now_rows().is_empty()); // la tabla vacía de cod2 cuenta como de la sesión
    let task = app.update(Message::Now(NowMsg::Refresh));
    drive(&mut app, task);
    assert!(app.is_scanning()); // el escaneo principal de cod2 aún no lo trajo
    let _ = app.update(Message::Now(NowMsg::Choose(0)));
    assert_eq!(app.pending_select(), Some(&x.address));
}

#[test]
fn now_current_game_not_cached() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = now_app(
        dir.path(),
        paths(&["cod2"]),
        "",
        &[("cod2", vec![srv("2.2.2.2", "De la ventana", 4, 0, 40)])],
        1,
    );
    complete(&mut app, &[srv("1.1.1.1", "De la tabla", 1, 0, 40)]);
    let _ = open(&mut app);
    let task = app.update(Message::Now(NowMsg::Refresh));
    drive(&mut app, task);
    assert_eq!(names(&app), ["De la ventana"]);
    let table: Vec<&str> = app.servers().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(table, ["De la tabla"]);
    let _ = app.update(Message::SelectGame("ut99"));
    assert_eq!(app.searchable_lists()["cod2"][0].name, "De la tabla");
}

#[test]
fn now_reopen_does_not_restart_running() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = now_app(dir.path(), paths(&["cod1"]), "", &[], 1);
    let first = window_actions(open(&mut app));
    let id = now_id(&app);
    assert_eq!(first, [("open", Some(id))]);
    assert_eq!(scans(&fake), ["cod1"]);
    let second = window_actions(open(&mut app));
    assert!(second.contains(&("focus", Some(id))), "{second:?}");
    assert!(!second.iter().any(|(a, _)| *a == "open"), "{second:?}");
    assert_eq!(scans(&fake), ["cod1"]);
}

#[test]
fn now_playing_action_and_shortcut_open_it() {
    use iced::keyboard::Modifiers;
    use jase::actions::{Action, Msg as ActionMsg, enabled};
    use jase::bars::{Shortcut, shortcut_for};
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = now_app(dir.path(), paths(&["cod1"]), "", &[], 1);
    assert!(enabled(&app, Action::NowPlaying));
    let ctrl_j = shortcut_for(&Key::Character("j".into()), Modifiers::COMMAND);
    assert_eq!(ctrl_j, Some(Shortcut::NowPlaying));
    let _ = app.update(Message::Shortcut(Shortcut::NowPlaying));
    let id = now_id(&app);
    let _ = app.update(Message::WindowClosed(id));
    let _ = app.update(Message::Actions(ActionMsg::Run(Action::NowPlaying)));
    assert!(app.tool_window(&Tool::NowPlaying).is_some());
}

#[test]
fn now_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = now_app(
        dir.path(),
        paths(&["cod1"]),
        "",
        &[("cod1", vec![srv("1.1.1.1", "Uno <b>&", 1, 0, 40)])],
        1,
    );
    let task = open(&mut app);
    let id = now_id(&app);
    drive(&mut app, task);
    for (theme, _, _) in VARIANTS.iter() {
        app.theme_id = theme;
        let texts = element_texts(app.view_window(id));
        for t in ["Uno <b>&", "Call of Duty", "1/20", "Actualizar", "Cerrar"] {
            assert!(texts.iter().any(|x| x == t), "{theme} {t}: {texts:?}");
        }
        assert!(texts.iter().any(|t| t.starts_with("Servidores con gente")));
        assert!(texts.iter().any(|t| t == "1 jugador en tus juegos"));
    }
}

#[test]
fn now_choose_with_dialog_open_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = now_app(
        dir.path(),
        paths(&["cod4", "q3"]),
        "",
        &[
            ("cod4", vec![srv("1.1.1.1", "Uno", 1, 0, 40)]),
            ("q3", vec![srv("2.2.2.2", "Dos", 2, 0, 40)]),
        ],
        1,
    );
    let task = open(&mut app);
    drive(&mut app, task);
    let game = app.current_game().map(|g| g.id);
    let _ = app.update(Message::AddServer(jase::add_server::Msg::Open {
        favorite: false,
        text: String::new(),
    }));
    assert!(app.dialog().is_some());
    let _ = app.update(Message::Now(NowMsg::Choose(1)));
    assert_eq!(app.current_game().map(|g| g.id), game);
    assert_eq!(selected_name(&app), None);
}

// ------------------------------------------------------- desplazamiento de la tabla

const VIEW_H: f32 = 10.0 * ROW_HEIGHT;

/// cod4 con `n` servidores de más a menos gente y la tabla de 10 filas.
fn now_many(
    dir: &std::path::Path,
    n: i32,
    extra: &[(&'static str, Vec<ServerInfo>)],
) -> (TestApp, Vec<Message>, window::Id) {
    let servers = (0..n)
        .map(|i| {
            srv(
                &format!("1.1.{}.{}", i / 200, i % 200),
                &format!("s{i:03}"),
                400 - i,
                0,
                40,
            )
        })
        .collect();
    let mut scripts = vec![("cod4", servers)];
    scripts.extend(extra.iter().cloned());
    let games: Vec<&str> = scripts.iter().map(|(g, _)| *g).collect();
    let (mut app, _) = now_app(dir, paths(&games), "", &scripts, 1);
    let msgs = now_messages(open(&mut app));
    let id = now_id(&app);
    let (cod4, rest): (Vec<_>, Vec<_>) = msgs
        .into_iter()
        .partition(|m| matches!(m, Message::Now(NowMsg::Scan { game: "cod4", .. })));
    for m in cod4 {
        let _ = app.update(m);
    }
    let _ = app.update(Message::Now(NowMsg::Scrolled(0.0, VIEW_H)));
    (app, rest, id)
}

#[test]
fn now_arrow_scrolls_only_when_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, id) = now_many(dir.path(), 300, &[]);
    assert_eq!(app.now_rows().len(), 300);
    let down = Key::Named(Named::ArrowDown);
    for _ in 0..10 {
        let task = app.update(Message::ToolKey(id, down.clone()));
        assert_eq!(widget_ops(task), 0);
    }
    assert_eq!(app.now_offset_y(), 0.0); // filas 0 a 9
    let task = app.update(Message::ToolKey(id, down));
    assert_eq!(app.now_offset_y(), 11.0 * ROW_HEIGHT - VIEW_H);
    assert_eq!(widget_ops(task), 1); // el `scroll_to`
}

#[test]
fn now_rebuild_keeps_offset() {
    let dir = tempfile::tempdir().unwrap();
    let q3 = vec![srv("9.9.9.9", "último", 1, 0, 40)];
    let (mut app, rest, id) = now_many(dir.path(), 30, &[("q3", q3)]);
    let _ = app.update(Message::Now(NowMsg::Select(12)));
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::ArrowDown)));
    let offset = 14.0 * ROW_HEIGHT - VIEW_H;
    assert_eq!(app.now_offset_y(), offset);
    for m in rest {
        let _ = app.update(m);
    }
    assert_eq!(app.now_rows().len(), 31);
    assert_eq!(app.now_offset_y(), offset);
}
