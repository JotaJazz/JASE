//! «Agregar servidor…» en la ventana (`window_add_server.py` y `tests/test_add_server.py`): el
//! diálogo se llena con mensajes, como el `fake_dialog` de Python, y las consultas corren al
//! ejecutar la `Task`.

mod support;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use iced::Task;
use jase::actions::{self, Action};
use jase::add_server::Msg;
use jase::app::{App, Dialog, DirectMsg, DirectView, Focus, Message};
use jase::bars::Shortcut;
use jase::scan::{ScanMsg, detect_with};
use jase::sidebar;
use jase::table;
use jase::theme::VARIANTS;
use jase_core::config::{Config, load_config};
use jase_core::games::Family;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{FakeScan, TestApp, clipboard_reads, fake_app, mk, run_task};

fn addr0() -> ServerAddress {
    ServerAddress::new("9.9.9.9", 28960)
}

fn info(address: &ServerAddress) -> ServerInfo {
    let mut info = ServerInfo::new(address.clone(), address.clone());
    info.name = "Mi servidor".into();
    info.map = "mp_x".into();
    info.num_players = 1;
    info.max_players = 8;
    info.version = "1.3".into();
    info
}

fn app(dir: &std::path::Path) -> (TestApp, FakeScan) {
    fake_app(dir, Config::default())
}

fn add(app: &mut App, m: Msg) -> Task<Message> {
    app.update(Message::AddServer(m))
}

/// Abre el diálogo como `add_server_manually` (lo que trae el portapapeles: nada).
fn open(app: &mut App) {
    let favorite = app.direct_view() == Some(DirectView::Favorites);
    let _ = add(
        app,
        Msg::Open {
            favorite,
            text: String::new(),
        },
    );
}

/// `fake_dialog`: elige el juego, escribe `text` en IP (de golpe, como `setText`), el puerto y
/// la casilla, y acepta.
fn fill_and_accept(
    app: &mut App,
    game: &'static str,
    text: &str,
    port: &str,
    favorite: Option<bool>,
) -> Task<Message> {
    open(app);
    let _ = add(app, Msg::Game(game));
    let _ = add(app, Msg::Host(text.into()));
    if !port.is_empty() {
        let _ = add(app, Msg::Port(port.into()));
    }
    if let Some(on) = favorite {
        let _ = add(app, Msg::Favorite(on));
    }
    add(app, Msg::Submit)
}

/// Corre la `Task` y entrega a la app lo que vuelve de las consultas (también lo que abren
/// sus respuestas); devuelve cada mensaje que pasó por la barra de estado.
fn drive(app: &mut App, task: Task<Message>) -> Vec<String> {
    let mut seen = Vec::new();
    for m in run_task(task) {
        if matches!(m, Message::AddServer(_) | Message::Direct(_)) {
            let next = app.update(m);
            seen.push(app.status().to_owned());
            seen.extend(drive(app, next));
        }
    }
    seen
}

fn favorites(app: &App) -> Vec<(String, Vec<String>)> {
    let mut all: Vec<_> = app
        .config()
        .favorites
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    all.sort();
    all
}

fn fav(game: &str, addresses: &[&str]) -> Vec<(String, Vec<String>)> {
    vec![(
        game.to_owned(),
        addresses.iter().map(|a| (*a).to_owned()).collect(),
    )]
}

fn selected(app: &App) -> Option<ServerAddress> {
    app.selected_server().map(|i| i.address.clone())
}

#[test]
fn add_server_shows_and_selects() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", None);
    assert_eq!(app.dialog(), None);
    assert!(app.add_form().is_none());
    assert_eq!(app.status(), "Consultando 9.9.9.9:28960…");
    let seen = drive(&mut app, task);
    assert_eq!(app.current_game().unwrap().id, "cod2");
    assert_eq!(selected(&app), Some(addr0()));
    assert!(app.config().favorites.is_empty()); // fuera de Favoritos no es favorito
    assert!(seen.contains(&"Servidor agregado: 9.9.9.9:28960".to_owned()));
}

#[test]
fn add_server_from_favorites_is_favorite() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    let _ = app.update(Message::Direct(DirectMsg::Show(DirectView::Favorites)));
    open(&mut app);
    assert!(app.add_form().unwrap().favorite());
    let _ = add(&mut app, Msg::Cancel);
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9", "28960", None);
    drive(&mut app, task);
    assert_eq!(selected(&app), Some(addr0()));
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    assert_eq!(favorites(&app), fav("cod2", &["9.9.9.9:28960"]));
}

#[test]
fn add_favorite_from_other_view_switches_to_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    let task = fill_and_accept(&mut app, "mohaa", "9.9.9.9:28960", "", Some(true));
    assert_eq!(app.current_game().unwrap().id, "mohaa");
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    drive(&mut app, task);
    assert_eq!(favorites(&app), fav("mohaa", &["9.9.9.9:28960"]));
}

#[test]
fn gamespy_game_port_resolves_to_query_port() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let query = ServerAddress::new("9.9.9.9", 7778);
    fake.answer_for(&query, Some(info(&query)));
    let task = fill_and_accept(&mut app, "ut99", "9.9.9.9:7777", "", Some(true));
    drive(&mut app, task);
    assert_eq!(favorites(&app), fav("ut99", &["9.9.9.9:7778"]));
}

#[test]
fn no_answer_only_warns_outside_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", None);
    drive(&mut app, task);
    assert_eq!(
        app.status(),
        "9.9.9.9:28960 no respondió; para guardarlo agrégalo como favorito"
    );
    assert!(app.servers().iter().all(|s| s.address != addr0()));
    assert!(app.config().favorites.is_empty());
}

#[test]
fn no_answer_favorite_is_kept_offline() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", Some(true));
    let seen = drive(&mut app, task);
    assert!(seen.contains(&"9.9.9.9:28960 no respondió; quedó en favoritos".to_owned()));
    let chosen = app.selected_server().unwrap();
    assert_eq!(chosen.address, addr0());
    assert!(!chosen.online);
    assert_eq!(favorites(&app), fav("cod2", &["9.9.9.9:28960"]));
}

#[test]
fn favorite_saved_even_if_game_changes_meanwhile() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", Some(true));
    let _ = app.update(Message::SelectGame("ut99"));
    drive(&mut app, task);
    assert_eq!(favorites(&app), fav("cod2", &["9.9.9.9:28960"]));
    let saved = load_config(&dir.path().join("config.json"));
    assert_eq!(saved.favorites["cod2"], ["9.9.9.9:28960"]);
    assert!(app.servers().is_empty()); // otro juego: sin filas
}

#[test]
fn unanswered_favorite_saved_even_if_game_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", Some(true));
    let _ = app.update(Message::SelectGame("ut99"));
    drive(&mut app, task);
    assert_eq!(favorites(&app), fav("cod2", &["9.9.9.9:28960"]));
    let saved = load_config(&dir.path().join("config.json"));
    assert_eq!(saved.favorites["cod2"], ["9.9.9.9:28960"]);
}

#[test]
fn switch_and_back_does_not_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", Some(true));
    let _ = app.update(Message::SelectGame("ut99"));
    let _ = app.update(Message::SelectGame("cod2"));
    let show = app.update(Message::Direct(DirectMsg::Show(DirectView::Favorites)));
    drive(&mut app, task);
    drive(&mut app, show);
    assert_eq!(favorites(&app), fav("cod2", &["9.9.9.9:28960"]));
    let count = app
        .servers()
        .iter()
        .filter(|s| s.address == addr0())
        .count();
    assert_eq!(count, 1);
}

#[test]
fn add_several_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    let text = "9.9.9.9:28960\n8.8.8.8:28961";
    let task = fill_and_accept(&mut app, "cod2", text, "", Some(true));
    assert_eq!(app.status(), "Consultando 2 servidores…");
    let seen = drive(&mut app, task);
    assert!(seen.contains(&"2 favoritos agregados (1 sin respuesta)".to_owned()));
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    assert_eq!(app.config().favorites["cod2"].len(), 2);
    assert_eq!(selected(&app), Some(addr0())); // la que respondió
}

#[test]
fn add_several_outside_favorites_only_answering() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    let other = ServerAddress::new("8.8.8.8", 28961);
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960\n8.8.8.8:28961", "", None);
    drive(&mut app, task);
    assert!(app.servers().iter().any(|s| s.address == addr0()));
    assert!(app.servers().iter().all(|s| s.address != other));
    assert_eq!(app.status(), "1 servidor respondió de 2");
}

#[test]
fn existing_favorite_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr0(), Some(info(&addr0())));
    app.config_mut()
        .favorites
        .insert("cod2".into(), vec!["9.9.9.9:28960".into()]);
    let task = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", Some(true));
    let seen = drive(&mut app, task);
    assert!(seen.contains(&"9.9.9.9:28960 ya estaba en favoritos".to_owned()));
    assert_eq!(favorites(&app), fav("cod2", &["9.9.9.9:28960"]));
}

#[test]
fn pending_favorite_saved_when_second_add_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let first = fill_and_accept(&mut app, "cod2", "9.9.9.9:28960", "", Some(true));
    let old = app.add_request();
    let second = fill_and_accept(&mut app, "ut99", "8.8.8.8:7778", "", Some(true));
    assert_ne!(app.add_request(), old);
    // Lo de la primera llega tarde y se descarta, pero su favorito ya quedó.
    drive(&mut app, first);
    drive(&mut app, second);
    let cfg = app.config();
    assert_eq!(cfg.favorites["cod2"], ["9.9.9.9:28960"]);
    assert_eq!(cfg.favorites["ut99"].len(), 1);
    assert!(cfg.favorites["ut99"][0].contains("8.8.8.8"));
}

#[test]
fn dialog_prefilled_from_clipboard() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let task = app.add_server_manually(None);
    assert_eq!(clipboard_reads(task), 1);
    assert!(app.dialog().is_none()); // abre al llegar el portapapeles
    let link = "jase://connect/ut99/9.9.9.9:7777";
    let _ = add(
        &mut app,
        Msg::Open {
            favorite: false,
            text: link.into(),
        },
    );
    assert_eq!(app.dialog(), Some(&Dialog::AddServer));
    assert_eq!(app.focus(), Focus::Dialog);
    let form = app.add_form().unwrap();
    assert_eq!(form.game(), "ut99");
    assert_eq!(form.addresses(), [ServerAddress::new("9.9.9.9", 7777)]);
    let _ = add(&mut app, Msg::Cancel);
    let _ = add(
        &mut app,
        Msg::Open {
            favorite: false,
            text: "nada útil aquí".into(),
        },
    );
    assert!(app.add_form().unwrap().addresses().is_empty());
}

#[test]
fn ctrl_v_in_table_opens_dialog() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    let row = ServerAddress::new("1.1.1.1", 28960);
    let _ = app.update(Message::Table(table::Msg::RowClicked(row)));
    assert_eq!(app.focus(), Focus::Table);
    let task = app.update(Message::Shortcut(Shortcut::Paste));
    assert_eq!(clipboard_reads(task), 1);
    let _ = add(&mut app, Msg::Pasted(Some("nada".into())));
    assert_eq!(
        app.status(),
        "El portapapeles no tiene ninguna dirección IP:puerto"
    );
    assert!(app.dialog().is_none());
    let _ = add(&mut app, Msg::Pasted(None));
    assert!(app.dialog().is_none());
    let _ = add(&mut app, Msg::Pasted(Some("9.9.9.9:28960".into())));
    assert_eq!(app.dialog(), Some(&Dialog::AddServer));
    assert_eq!(app.add_form().unwrap().addresses(), [addr0()]);
    let _ = add(&mut app, Msg::Cancel);
    assert_eq!(app.focus(), Focus::Table);
    // Con el foco en el filtro, Ctrl+V pega en el campo.
    let _ = app.update(Message::FilterFocused(true));
    let task = app.update(Message::Shortcut(Shortcut::Paste));
    assert_eq!(clipboard_reads(task), 0);
}

#[test]
fn ctrl_n_opens_from_any_focus() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    assert_eq!(
        clipboard_reads(app.update(Message::Shortcut(Shortcut::AddServer))),
        1
    );
    let _ = app.update(Message::FilterFocused(true));
    assert_eq!(app.focus(), Focus::Filter);
    assert_eq!(
        clipboard_reads(app.update(Message::Shortcut(Shortcut::AddServer))),
        1
    );
    // El menú «Servidor» y el atajo son la misma acción.
    let menu = app.update(Message::Actions(actions::Msg::Run(Action::AddServer)));
    assert_eq!(clipboard_reads(menu), 1);
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::About)));
    assert_eq!(
        clipboard_reads(app.update(Message::Shortcut(Shortcut::AddServer))),
        0
    );
    let _ = app.update(Message::CloseDialog);
    open(&mut app);
    assert_eq!(
        clipboard_reads(app.update(Message::Shortcut(Shortcut::AddServer))),
        0
    );
    assert_eq!(
        clipboard_reads(app.update(Message::Shortcut(Shortcut::Paste))),
        0
    );
}

#[test]
fn late_open_with_dialog_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::About)));
    let _ = add(
        &mut app,
        Msg::Open {
            favorite: false,
            text: "9.9.9.9:28960".into(),
        },
    );
    assert_eq!(app.dialog(), Some(&Dialog::About));
    assert!(app.add_form().is_none());
}

#[test]
fn enter_submits_and_escape_cancels() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    open(&mut app);
    // Enter sin dirección: nada.
    let _ = add(&mut app, Msg::HostSubmit);
    assert_eq!(app.dialog(), Some(&Dialog::AddServer));
    // Tecla a tecla y Enter: se separa y agrega.
    let mut typed = String::new();
    for c in "9.9.9.9:28960".chars() {
        typed.push(c);
        let _ = add(&mut app, Msg::Host(typed.clone()));
    }
    assert_eq!(app.add_form().unwrap().host(), "9.9.9.9:28960");
    let task = add(&mut app, Msg::HostSubmit);
    assert!(app.dialog().is_none());
    run_task(task);
    assert_eq!(fake.queries(), [("cod2", addr0())]);
    // Esc cierra sin consultar.
    open(&mut app);
    let _ = add(&mut app, Msg::Host("8.8.8.8:28960".into()));
    let task = app.update(Message::Escape);
    assert!(app.dialog().is_none() && app.add_form().is_none());
    run_task(task);
    assert_eq!(fake.queries().len(), 1);
}

#[test]
fn add_and_detect_run_in_the_task() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    // 100 direcciones de ut99: 200 candidatas (juego y consulta).
    let list: Vec<String> = (0..100).map(|i| format!("10.0.0.{i}:7777")).collect();
    open(&mut app);
    let _ = add(&mut app, Msg::Game("ut99"));
    let _ = add(&mut app, Msg::Host(list.join("\n")));
    assert!(app.add_form().unwrap().is_multi());
    let task = add(&mut app, Msg::Submit);
    assert!(fake.queries().is_empty());
    drive(&mut app, task);
    assert_eq!(fake.queries().len(), 200);
    let max = fake.query_max.load(Ordering::SeqCst);
    assert!((1..=64).contains(&max), "{max}");
    // *Detectar*: tampoco en `update`.
    open(&mut app);
    let _ = add(&mut app, Msg::Host("9.9.9.9:28960".into()));
    let task = add(&mut app, Msg::Detect);
    assert!(fake.detect_calls.lock().unwrap().is_empty());
    run_task(task);
    assert_eq!(*fake.detect_calls.lock().unwrap(), [addr0()]);
}

#[test]
fn detect_picks_game_in_the_dialog() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let query = ServerAddress::new("9.9.9.9", 7778);
    *fake.detect.lock().unwrap() = vec![("ut99", query)];
    open(&mut app);
    let _ = add(&mut app, Msg::Host("9.9.9.9:7777".into()));
    let task = add(&mut app, Msg::Detect);
    let form = app.add_form().unwrap();
    assert_eq!(
        form.detect_label(),
        Some("Detectando el juego de 9.9.9.9:7777…")
    );
    drive(&mut app, task);
    let form = app.add_form().unwrap();
    assert_eq!((form.game(), form.port()), ("ut99", "7778"));
    assert_eq!(form.detect_label(), Some("Detectado: Unreal Tournament"));
    assert!(form.can_detect());
}

#[test]
fn detect_runs_once_per_click() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    open(&mut app);
    let _ = add(&mut app, Msg::Host("9.9.9.9:28960".into()));
    let first = add(&mut app, Msg::Detect);
    let request = app.detect_request();
    let again = add(&mut app, Msg::Detect);
    assert_eq!(app.detect_request(), request);
    run_task(again);
    assert!(fake.detect_calls.lock().unwrap().is_empty());
    run_task(first);
    assert_eq!(fake.detect_calls.lock().unwrap().len(), 1);
}

#[test]
fn detect_after_close_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    open(&mut app);
    let _ = add(&mut app, Msg::Host("9.9.9.9:28960".into()));
    let _ = add(&mut app, Msg::Detect);
    let request = app.detect_request();
    let _ = add(&mut app, Msg::Cancel);
    assert_ne!(app.detect_request(), request);
    let _ = add(
        &mut app,
        Msg::Detected {
            request,
            address: addr0(),
            games: vec![("ut99", addr0())],
        },
    );
    assert!(app.dialog().is_none() && app.add_form().is_none());
    assert_eq!(app.current_game().unwrap().id, "cod2");
}

#[test]
fn window_detect_game() {
    let mut cod2 = info(&addr0());
    cod2.rules
        .insert("gamename".into(), "Call of Duty 2".into());
    cod2.rules.insert("protocol".into(), "118".into());
    let asked: Arc<Mutex<Vec<Family>>> = Arc::default();
    let seen = asked.clone();
    let detect = detect_with(addr0(), move |family, addresses| {
        seen.lock().unwrap().push(family);
        let answer = if family == Family::Quake3 && addresses.contains(&addr0()) {
            vec![cod2.clone()]
        } else {
            Vec::new()
        };
        async move { answer }.boxed()
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    assert_eq!(rt.block_on(detect), [("cod2", addr0())]);
    let mut families = asked.lock().unwrap().clone();
    families.sort_by_key(|f| Family::ALL.iter().position(|a| a == f));
    assert_eq!(families, Family::ALL);
}

#[test]
fn select_address_scrolls_and_skips_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let servers: Vec<ServerInfo> = (0..100)
        .map(|i| mk(&format!("1.1.1.{i}"), &format!("srv {i}"), 40, 3, "1.3"))
        .collect();
    complete(&mut app, &servers);
    app.table.viewport_h = 200.0;
    let last = ServerAddress::new("1.1.1.99", 28960);
    assert!(table::select_address(&mut app.table, &last).is_some());
    assert_eq!(app.table.selected, Some(last));
    assert!(app.table.offset_y > 0.0);
    let missing = ServerAddress::new("2.2.2.2", 1);
    assert!(table::select_address(&mut app.table, &missing).is_none());
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text("srv 5".into())));
    let hidden = ServerAddress::new("1.1.1.7", 28960);
    assert!(table::select_address(&mut app.table, &hidden).is_none());
}

#[test]
fn add_dialog_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path());
    let build = |app: &mut App| {
        for (id, _, _) in VARIANTS.iter() {
            app.theme_id = id;
            let _ = app.view();
        }
    };
    open(&mut app);
    build(&mut app);
    let _ = add(&mut app, Msg::Host("9.9.9.9:28960".into()));
    let _ = add(&mut app, Msg::Detect);
    build(&mut app); // con la etiqueta de detección
    let _ = add(&mut app, Msg::ToggleMulti);
    assert!(app.add_form().unwrap().is_multi());
    build(&mut app);
}

fn complete(app: &mut App, servers: &[ServerInfo]) {
    let generation = app.scan.generation();
    for s in servers {
        let msg = ScanMsg::Event(ScanEvent::Server(s.clone()));
        let _ = app.update(Message::Scan { generation, msg });
    }
    let result = ScanResult {
        masters_ok: 1,
        servers_listed: servers.len(),
        servers_responded: servers.len(),
        errors: vec![],
    };
    let msg = ScanMsg::Finished(result);
    let _ = app.update(Message::Scan { generation, msg });
}
