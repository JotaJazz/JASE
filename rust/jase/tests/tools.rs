//! Ventanas de herramientas (`iced::daemon`) y *Buscar jugador…* (`tests/test_player_search.py`
//! y los de buscar/revelar de `test_main_window.py` y `test_saved_lists.py`). Las ventanas no se
//! abren: `open_tool` registra la id y la `Task` lleva la `Action::Window` sin contestar.

mod support;

use std::path::Path;

use iced::Task;
use iced::keyboard::{Key, key::Named};
use iced::window;
use indexmap::IndexMap;
use jase::actions::{Action, Msg as ActionMsg};
use jase::app::{
    App, AutoMsg, Focus, Message, SearchMsg, Tool, key_message, tool_settings, window_message,
};
use jase::player_search::{MAX_HITS, PlayerSearch};
use jase::scan::ScanMsg;
use jase::startup::APP_NAME;
use jase::theme::VARIANTS;
use jase_core::config::Config;
use jase_core::models::{Player, QuickFilters, ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use jase_core::server_cache;
use support::{element_texts, fake_app, fake_app_task, mk, run_task, tool_ids, window_actions};

fn player(name: &str, raw: &str) -> Player {
    Player {
        raw_name: raw.to_owned(),
        ..Player::new(name, 0, 0)
    }
}

fn srv(host: &str, name: &str, players: &[&str]) -> ServerInfo {
    let mut info = mk(host, name, 40, players.len() as i32, "1.3");
    info.players = players.iter().map(|n| player(n, "")).collect();
    info
}

fn lists(entries: &[(&str, Vec<ServerInfo>)]) -> IndexMap<String, Vec<ServerInfo>> {
    entries
        .iter()
        .map(|(g, s)| ((*g).to_owned(), s.clone()))
        .collect()
}

fn search(query: &str, lists: &IndexMap<String, Vec<ServerInfo>>) -> PlayerSearch {
    let mut s = PlayerSearch::default();
    s.edit(query.to_owned());
    s.refresh(lists);
    s
}

fn players(s: &PlayerSearch) -> Vec<&str> {
    s.hits.iter().map(|h| h.player.as_str()).collect()
}

#[test]
fn tool_window_closes_itself_centered_with_half_min_size() {
    let settings = tool_settings(iced::Size::new(640.0, 420.0));
    // Iced la cierra solo: el cierre no pasa por `CloseRequested` (solo la principal).
    assert!(settings.exit_on_close_request);
    assert!(matches!(settings.position, window::Position::Centered));
    assert_eq!(settings.min_size, Some(iced::Size::new(320.0, 210.0)));
    assert_eq!(settings.size, iced::Size::new(640.0, 420.0));
}

#[test]
fn search_substring_ignores_case() {
    let l = lists(&[(
        "cod2",
        vec![srv("1.1.1.1", "a", &["Jimmy", "xXjimXx", "Bob"])],
    )]);
    let s = search("JIM", &l);
    assert_eq!(players(&s), ["Jimmy", "xXjimXx"]);
    assert_eq!(s.selected, Some(0));
    assert!(!s.stale());
}

#[test]
fn search_empty_query_has_no_hits() {
    let l = lists(&[("cod2", vec![srv("1.1.1.1", "a", &["Jimmy"])])]);
    let s = search("  ", &l);
    assert!(s.hits.is_empty());
    assert_eq!(s.selected, None);
    assert_eq!(s.games, 1);
}

#[test]
fn search_matches_plain_names() {
    let mut info = srv("1.1.1.1", "a", &[]);
    info.players = vec![player("Jane", "^1Ja^7ne")];
    let l = lists(&[("cod2", vec![info])]);
    let s = search("jane", &l);
    assert_eq!(players(&s), ["Jane"]);
    assert!(search("^1", &l).hits.is_empty());
    let _lang = support::lang::Spanish::hold();
    let texts = element_texts(s.view(VARIANTS[0].0));
    assert!(texts.iter().any(|t| t == "Jane"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.contains('^')), "{texts:?}");
}

#[test]
fn search_skips_offline_servers() {
    let mut info = srv("1.1.1.1", "a", &["Jimmy"]);
    info.online = false;
    let l = lists(&[("cod2", vec![info])]);
    assert!(search("jim", &l).hits.is_empty());
}

#[test]
fn search_sorted_by_game_server_player() {
    let l = lists(&[
        ("ut99", vec![srv("3.3.3.3", "u", &["Jim"])]),
        (
            "cod2",
            vec![
                srv("2.2.2.2", "Beta", &["jimZ", "Jima"]),
                srv("1.1.1.1", "alfa", &["Jimb"]),
            ],
        ),
    ]);
    let s = search("jim", &l);
    let games: Vec<&str> = s.hits.iter().map(|h| h.game).collect();
    assert_eq!(games, ["cod2", "cod2", "cod2", "ut99"]);
    assert_eq!(players(&s), ["Jimb", "Jima", "jimZ", "Jim"]);
    assert_eq!(s.hits[0].server, "alfa");
    assert_eq!(s.hits[0].address, ServerAddress::new("1.1.1.1", 28960));
    assert_eq!(s.hits[0].map, "mp_toujane");
}

#[test]
fn search_respects_limit() {
    let names: Vec<String> = (0..=MAX_HITS).map(|n| format!("Jim{n:04}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let l = lists(&[("cod2", vec![srv("1.1.1.1", "a", &refs)])]);
    let s = search("jim", &l);
    assert_eq!(s.hits.len(), MAX_HITS);
    assert_eq!(s.hits[0].player, "Jim0000"); // los primeros del orden
}

// ------------------------------------------------------------------ la app

fn send(app: &mut App, msg: ScanMsg) -> Task<Message> {
    let generation = app.scan.generation();
    app.update(Message::Scan { generation, msg })
}

fn server(app: &mut App, info: &ServerInfo) -> Task<Message> {
    send(app, ScanMsg::Event(ScanEvent::Server(info.clone())))
}

fn complete(app: &mut App, servers: &[ServerInfo]) {
    for info in servers {
        let _ = server(app, info);
    }
    let result = ScanResult {
        masters_ok: 1,
        ..ScanResult::default()
    };
    let _ = send(app, ScanMsg::Finished(result));
}

fn deliver(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        let _ = app.update(m);
    }
}

fn selected(app: &App) -> Option<ServerAddress> {
    app.selected_server().map(|s| s.address.clone())
}

fn jim() -> ServerInfo {
    srv("1.1.1.1", "srv jim", &["Jimmy", "Jill"])
}

fn bob() -> ServerInfo {
    srv("2.2.2.2", "srv bob", &["Bob"])
}

/// cod2 escaneado con jim y bob, y la búsqueda abierta.
fn searching(dir: &Path) -> (support::TestApp, support::FakeScan, window::Id) {
    let (mut app, fake) = fake_app(dir, Config::default());
    complete(&mut app, &[jim(), bob()]);
    let _ = app.update(Message::Search(SearchMsg::Open));
    let id = app
        .tool_window(&Tool::PlayerSearch)
        .expect("ventana de búsqueda");
    (app, fake, id)
}

fn query(app: &mut App, text: &str) -> Task<Message> {
    app.update(Message::Search(SearchMsg::Query(text.to_owned())))
}

fn hits(app: &App) -> Vec<String> {
    app.player_search()
        .map(|s| s.hits.iter().map(|h| h.player.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn reveal_server_hidden_by_filters_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        quick_filters: QuickFilters {
            hide_empty: true,
            ..QuickFilters::default()
        },
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    let empty = srv("1.1.1.1", "vacío", &[]);
    complete(
        &mut app,
        &[srv("2.2.2.2", "lleno", &["Bob"]), empty.clone()],
    );
    let _ = app.reveal_server("cod2", empty.address.clone());
    assert_eq!(
        app.status(),
        "El servidor está oculto por los filtros activos"
    );
    assert_eq!(app.pending_select(), None);
    assert_eq!(selected(&app), None);
}

#[test]
fn reveal_server_of_game_being_scanned_waits_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::SelectGame("ut99"));
    let (a, b) = (jim(), bob());
    let _ = app.reveal_server("cod2", a.address.clone());
    assert_eq!(app.current_game().map(|g| g.id), Some("cod2"));
    assert!(app.is_scanning());
    assert_eq!(app.pending_select(), Some(&a.address));
    assert_eq!(fake.calls(), ["cod2", "ut99", "cod2"]); // sin un escaneo de más
    let _ = server(&mut app, &b);
    let _ = server(&mut app, &a);
    assert_eq!(selected(&app), Some(a.address));
    assert_eq!(app.pending_select(), None);
}

#[test]
fn reveal_server_absent_after_scan_leaves_nothing_pending() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[jim()]);
    let _ = app.reveal_server("cod2", ServerAddress::new("9.9.9.9", 1));
    assert_eq!(app.pending_select(), None);
    assert_eq!(selected(&app), None);
}

fn saved_dir(dir: &Path) -> std::path::PathBuf {
    dir.join("cache").join("servers")
}

/// Primera sesión: escanea cod2 y deja la lista en disco.
fn scanned(dir: &Path, servers: &[ServerInfo]) {
    let (mut app, _) = fake_app(dir, Config::default());
    complete(&mut app, servers);
    app.saved.flush();
    assert!(server_cache::read_index(&saved_dir(dir)).contains_key("cod2"));
}

#[test]
fn reveal_server_with_saved_list_scans_and_selects() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (jim(), bob());
    scanned(dir.path(), std::slice::from_ref(&a));
    let (mut app, fake, _task) = fake_app_task(dir.path(), Config::default());
    let _ = app.update(Message::SelectGame("ut99"));
    assert_eq!(fake.calls(), ["ut99"]);
    // p. ej. un aviso de amigo en un servidor nuevo: solo hay lista del disco.
    let _ = app.reveal_server("cod2", b.address.clone());
    assert_eq!(fake.calls(), ["ut99", "cod2"]);
    assert_eq!(app.pending_select(), Some(&b.address));
    complete(&mut app, &[a, b.clone()]);
    assert_eq!(selected(&app), Some(b.address));
}

#[test]
fn player_search_ignores_saved_lists() {
    let dir = tempfile::tempdir().unwrap();
    scanned(dir.path(), &[jim()]);
    let (mut app, _, task) = fake_app_task(dir.path(), Config::default());
    for m in run_task(task) {
        if matches!(m, Message::SavedList { .. }) {
            let _ = app.update(m);
        }
    }
    assert_eq!(app.servers().len(), 1); // la del disco, a la vista
    assert!(!app.searchable_lists().contains_key("cod2"));
    let _ = app.update(Message::SelectGame("ut99"));
    complete(&mut app, &[]);
    let lists = app.searchable_lists();
    assert!(!lists.contains_key("cod2"));
    assert!(lists.contains_key("ut99"));
}

#[test]
fn enter_before_debounce_uses_fresh_results() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, _) = searching(dir.path());
    let t = query(&mut app, "jim");
    deliver(&mut app, t);
    assert_eq!(hits(&app), ["Jimmy"]);
    let _ = query(&mut app, "bo"); // Enter antes de que venza el plazo
    let task = app.update(Message::Search(SearchMsg::Submit));
    deliver(&mut app, task);
    assert_eq!(hits(&app), ["Bob"]);
    assert_eq!(selected(&app), Some(bob().address));
}

#[test]
fn find_player_opens_one_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let run = |app: &mut App| app.update(Message::Actions(ActionMsg::Run(Action::FindPlayer)));
    let first = window_actions(run(&mut app));
    let ids = tool_ids(&app);
    assert_eq!(app.tool_windows(), [(ids[0], Tool::PlayerSearch)]);
    assert_eq!(first, [("open", Some(ids[0]))]);
    let second = window_actions(run(&mut app));
    assert_eq!(tool_ids(&app), ids);
    assert!(!second.iter().any(|(a, _)| *a == "open"), "{second:?}");
    assert!(second.contains(&("focus", Some(ids[0]))), "{second:?}");
}

#[test]
fn debounced_search_ignores_older_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, _) = searching(dir.path());
    let old = query(&mut app, "bob");
    let new = query(&mut app, "jim");
    deliver(&mut app, old);
    assert!(hits(&app).is_empty());
    assert!(app.player_search().unwrap().stale());
    deliver(&mut app, new);
    assert_eq!(hits(&app), ["Jimmy"]);
}

#[test]
fn choose_keeps_search_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, id) = searching(dir.path());
    let t = query(&mut app, "bob");
    deliver(&mut app, t);
    let task = app.update(Message::Search(SearchMsg::Choose(0)));
    assert!(window_actions(task).is_empty()); // no cierra ni trae la principal
    assert_eq!(selected(&app), Some(bob().address));
    assert!(app.player_search().is_some());
    assert_eq!(tool_ids(&app), [id]);
}

#[test]
fn window_closed_drops_search() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, id) = searching(dir.path());
    let _ = app.update(Message::WindowClosed(id));
    assert!(app.player_search().is_none());
    assert!(app.tool_windows().is_empty());
    assert_eq!(app.title(id), APP_NAME); // cerrándose: nada propio
}

#[test]
fn tool_escape_closes_only_that_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, tool) = searching(dir.path());
    let main = app.main_window();
    let _ = app.update(Message::Actions(ActionMsg::Run(Action::About)));
    assert!(app.dialog().is_some());
    let m = window_message(main, tool, Message::Escape);
    assert!(
        matches!(m, Some(Message::CloseTool(id)) if id == tool),
        "{m:?}"
    );
    let task = app.update(m.unwrap());
    assert_eq!(window_actions(task), [("close", Some(tool))]);
    assert!(app.dialog().is_some()); // el diálogo de la principal sigue
    let m = window_message(main, main, Message::Escape);
    assert!(matches!(m, Some(Message::Escape)), "{m:?}");
}

#[test]
fn tool_keys_never_reach_main() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, tool) = searching(dir.path());
    let main = app.main_window();
    let down = Key::Named(Named::ArrowDown);
    let m = key_message(main, Focus::Table, tool, down.clone());
    assert!(
        matches!(&m, Some(Message::ToolKey(id, k)) if *id == tool && *k == down),
        "{m:?}"
    );
    let m = key_message(main, Focus::Table, main, down.clone());
    assert!(matches!(m, Some(Message::Table(_))), "{m:?}");
    for msg in [
        Message::MousePressed,
        Message::FilterFocused(true),
        Message::WindowResized(iced::Size::new(10.0, 10.0)),
        Message::WindowOpened(tool, None),
    ] {
        assert!(window_message(main, tool, msg).is_none());
    }
    assert!(window_message(main, main, Message::MousePressed).is_some());
    // ↓ en la búsqueda mueve su fila; la tabla de la principal no se entera.
    let t = query(&mut app, "j");
    deliver(&mut app, t);
    let before = selected(&app);
    let _ = app.update(Message::ToolKey(tool, down));
    assert_eq!(app.player_search().unwrap().selected, Some(1));
    let _ = app.update(Message::ToolKey(tool, Key::Named(Named::ArrowDown)));
    assert_eq!(app.player_search().unwrap().selected, Some(1)); // la última
    let _ = app.update(Message::ToolKey(tool, Key::Named(Named::ArrowUp)));
    assert_eq!(app.player_search().unwrap().selected, Some(0));
    assert_eq!(selected(&app), before);
}

#[test]
fn tool_close_request_is_not_app_close() {
    let dir = tempfile::tempdir().unwrap();
    let (app, fake, tool) = searching(dir.path());
    let main = app.main_window();
    assert!(window_message(main, tool, Message::CloseRequested).is_none());
    assert!(fake.exits.lock().unwrap().is_empty());
}

#[test]
fn main_close_exits_with_tools_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _) = searching(dir.path());
    let main = app.main_window();
    let m = window_message(main, main, Message::CloseRequested);
    assert!(matches!(m, Some(Message::CloseRequested)), "{m:?}");
    let task = app.update(m.unwrap());
    assert_eq!(window_actions(task), [("maximized", Some(main))]);
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
}

#[test]
fn main_window_tasks_never_use_latest() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        auto_refresh_minutes: 1,
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg.clone());
    let before = app.main_window();
    let open = app.open_main(jase::window_settings(&cfg));
    let main = app.main_window();
    assert_ne!(main, before);
    let _ = app.update(Message::Search(SearchMsg::Open)); // una herramienta, la última abierta
    assert_eq!(window_actions(open.discard()), [("open", Some(main))]);
    assert_eq!(
        window_actions(jase::window_state::close(main)),
        [("maximized", Some(main))]
    );
    let tick = app.update(Message::AutoRefresh(AutoMsg::Tick));
    assert_eq!(window_actions(tick), [("minimized", Some(main))]);
    let resized = app.update(Message::WindowResized(iced::Size::new(900.0, 600.0)));
    assert_eq!(window_actions(resized), [("maximized", Some(main))]);
    let show = app.update(Message::Link(jase::app::LinkMsg::Received(
        jase::single_instance::SHOW_COMMAND.to_owned(),
    )));
    let show = window_actions(show);
    assert!(
        !show.is_empty() && show.iter().all(|(_, id)| *id == Some(main)),
        "{show:?}"
    );
}

#[test]
fn tool_titles() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, search) = searching(dir.path());
    assert_eq!(app.title(app.main_window()), APP_NAME);
    assert_eq!(app.title(search), "Buscar jugador…");
    let (now, _) = app.open_tool(Tool::NowPlaying);
    assert_eq!(app.title(now), "Ahora se juega");
    let address = ServerAddress::new("1.1.1.1", 28960);
    let (rcon, _) = app.open_tool(Tool::Rcon(("cod2", address)));
    assert_eq!(app.title(rcon), "Consola RCON: 1.1.1.1:28960");
    assert_eq!(tool_ids(&app), [search, now, rcon]);
}

#[test]
fn search_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, id) = searching(dir.path());
    let t = query(&mut app, "j");
    deliver(&mut app, t);
    for (theme, _, _) in VARIANTS.iter() {
        app.theme_id = theme;
        let texts = element_texts(app.view_window(id));
        assert!(texts.iter().any(|t| t == "Jimmy"), "{theme}: {texts:?}");
        assert!(
            texts
                .iter()
                .any(|t| t == "Busca en el juego cargado en esta sesión")
        );
    }
}

#[test]
fn tool_focus_takes_main_caret() {
    // `operation::focus` en una ventana quita el caret de todas: la principal no queda
    // creyendo que el filtro lo tiene.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::Shortcut(jase::bars::Shortcut::Find));
    assert_eq!(app.focus(), Focus::Filter);
    let _ = app.update(Message::Search(SearchMsg::Open));
    assert_eq!(app.focus(), Focus::Sidebar);
    // Con un diálogo, `Dialog` no habla de un caret: sigue igual.
    let _ = app.update(Message::Actions(ActionMsg::Run(Action::About)));
    let _ = app.update(Message::Search(SearchMsg::Open));
    assert_eq!(app.focus(), Focus::Dialog);
}

#[test]
fn now_playing_keeps_main_caret() {
    // *Ahora se juega* no enfoca ningún campo: el del filtro sigue con el caret.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::Shortcut(jase::bars::Shortcut::Find));
    let _ = app.update(Message::Now(jase::app::NowMsg::Open));
    assert_eq!(app.focus(), Focus::Filter);
}
