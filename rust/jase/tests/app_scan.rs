//! Escaneo cableado en la app (`test_main_window.py` y `ScanWorker` de Python): los mensajes
//! que traería la `Task` se meten a mano con la generación vigente o una vieja.

mod support;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures::StreamExt;
use futures::executor::block_on;
use futures::task::noop_waker_ref;
use iced::event::Status;
use iced::keyboard::{self, Key, key::Named};
use iced::theme::Mode;
use iced::{Event, Task, mouse, window};
use jase::actions::{Menu, entries};
use jase::app::{App, Dialog, Focus, Message, event_message};
use jase::menu::Entry;
use jase::presets::{self, PresetItem};
use jase::scan::{ScanMsg, scan_stream, scan_stream_with};
use jase::sidebar::{self, ping_options};
use jase::table;
use jase::theme::palette;
use jase::tree::{self, Item, Kind, Node, TreeRow};
use jase_core::config::{Config, load_config};
use jase_core::filter_presets::{FilterPreset, MAX_PRESETS};
use jase_core::games::{GAMES, GameDef, LaunchStyle, game};
use jase_core::geo::CountryDB;
use jase_core::masters::{
    CustomMasters, effective_masters, load_official, master_key, save_custom,
};
use jase_core::models::{QuickFilters, ServerInfo};
use jase_core::scanner::scan::{MasterError, ScanEvent, ScanOptions, ScanResult};
use support::lang::with_language;
use support::{fake_app, fake_app_task, mk};

fn send(app: &mut App, generation: u64, msg: ScanMsg) {
    let _ = app.update(Message::Scan { generation, msg });
}

fn event(app: &mut App, generation: u64, e: ScanEvent) {
    send(app, generation, ScanMsg::Event(e));
}

/// Como `FakeScan` de Python: un servidor «muerto» de más que nunca responde.
fn complete(app: &mut App, generation: u64, servers: &[ServerInfo]) {
    let total = servers.len() + 1;
    event(app, generation, ScanEvent::Status("Escaneando".into()));
    event(app, generation, ScanEvent::Progress { done: 0, total });
    for (i, info) in servers.iter().enumerate() {
        event(app, generation, ScanEvent::Server(info.clone()));
        event(app, generation, ScanEvent::Progress { done: i + 1, total });
    }
    let result = ScanResult {
        masters_ok: 1,
        servers_listed: total,
        servers_responded: servers.len(),
        errors: vec![],
    };
    send(app, generation, ScanMsg::Finished(result));
}

fn complete_now(app: &mut App, servers: &[ServerInfo]) {
    let generation = app.scan.generation();
    complete(app, generation, servers);
}

fn no_master_now(app: &mut App) {
    let generation = app.scan.generation();
    event(
        app,
        generation,
        ScanEvent::Status("Ningún master respondió".into()),
    );
    send(app, generation, ScanMsg::Finished(ScanResult::default()));
}

fn select(app: &mut App, id: &'static str) {
    let _ = app.update(Message::SelectGame(id));
}

fn hosts(app: &App) -> Vec<String> {
    let mut h: Vec<String> = app
        .servers()
        .iter()
        .map(|s| s.address.host.clone())
        .collect();
    h.sort();
    h
}

fn cod2() -> Vec<ServerInfo> {
    vec![
        mk("1.1.1.1", "a", 40, 3, "1.3"),
        mk("2.2.2.2", "b", 50, 3, "1.3"),
    ]
}

fn ut99() -> Vec<ServerInfo> {
    vec![mk("9.9.9.9", "u", 60, 3, "451")]
}

#[test]
fn scan_populates_table_and_sidebar_count() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    assert_eq!(fake.calls(), ["cod2"]);
    assert!(app.is_scanning());
    complete_now(&mut app, &cod2());
    assert_eq!(app.servers().len(), 2);
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
    assert!(app.game_label("cod1").contains('—'));
    assert!(app.game_label("ut99").contains('—'));
    assert_eq!(app.count_text(), "2 servidores");
}

#[test]
fn switching_game_discards_stale_results() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let old = app.scan.generation();
    select(&mut app, "ut99");
    assert_ne!(app.scan.generation(), old);
    complete(&mut app, old, &cod2()); // llega tarde
    assert!(app.servers().is_empty());
    complete_now(&mut app, &ut99());
    assert_eq!(hosts(&app), ["9.9.9.9"]);
    let n = fake.calls().len();
    select(&mut app, "ut99");
    assert_eq!(fake.calls().len(), n);
}

#[test]
fn switching_back_restores_saved_list_without_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2()[..1]);
    select(&mut app, "ut99");
    complete_now(&mut app, &ut99());
    select(&mut app, "cod2");
    assert_eq!(fake.calls(), ["cod2", "ut99"]);
    assert!(!app.is_scanning());
    assert_eq!(hosts(&app), ["1.1.1.1"]);
    assert_eq!(app.status(), "Call of Duty 2: lista guardada de la sesión");
    assert_eq!(app.game_label("ut99"), "Unreal Tournament · 1");
}

#[test]
fn refresh_rescans_current_game() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2()[..1]);
    let _ = app.update(Message::Refresh);
    assert!(app.is_scanning());
    assert!(app.servers().is_empty());
    complete_now(&mut app, &cod2()[..1]);
    assert!(!app.is_scanning());
    assert_eq!(app.servers().len(), 1);
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
}

#[test]
fn signals_from_cancelled_worker_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let old = app.scan.generation();
    select(&mut app, "ut99");
    event(&mut app, old, ScanEvent::Status("estado obsoleto".into()));
    event(
        &mut app,
        old,
        ScanEvent::Server(mk("6.6.6.6", "x", 1, 1, "1.3")),
    );
    complete_now(&mut app, &ut99());
    assert_eq!(hosts(&app), ["9.9.9.9"]);
    assert_ne!(app.status(), "estado obsoleto");
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · —");
}

#[test]
fn last_complete_result_survives_refresh_and_switch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
    let _ = app.update(Message::Refresh); // queda colgado
    assert!(app.is_scanning());
    assert!(!app.game_label("cod2").contains('—'));
    let stuck = app.scan.generation();
    select(&mut app, "ut99"); // cambio a mitad del refresco
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
    complete(&mut app, stuck, &cod2()[..1]); // el refresco cancelado no pisa la caché
    complete_now(&mut app, &ut99());
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
    select(&mut app, "cod2");
    assert_eq!(hosts(&app), ["1.1.1.1", "2.2.2.2"]);
    assert_eq!(fake.calls(), ["cod2", "cod2", "ut99"]);
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
}

#[test]
fn errored_scan_is_not_cached() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let generation = app.scan.generation();
    event(
        &mut app,
        generation,
        ScanEvent::Server(mk("1.1.1.1", "a", 1, 1, "1.3")),
    );
    let result = ScanResult {
        errors: vec![MasterError {
            master: "cod2master.activision.com:20710".into(),
            kind: "timeout",
            message: "Master cod2master.activision.com no responde".into(),
        }],
        ..ScanResult::default()
    };
    send(&mut app, generation, ScanMsg::Finished(result));
    assert!(!app.is_scanning());
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · —");
    select(&mut app, "ut99");
    complete_now(&mut app, &ut99());
    select(&mut app, "cod2");
    assert_eq!(fake.calls(), ["cod2", "ut99", "cod2"]);
}

#[test]
fn no_master_scan_is_not_cached() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    no_master_now(&mut app);
    assert!(!app.is_scanning());
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · —");
    assert_eq!(app.status(), "Ningún master respondió");
    select(&mut app, "ut99");
    complete_now(&mut app, &ut99());
    select(&mut app, "cod2"); // sin lista guardada: se vuelve a escanear
    assert_eq!(fake.calls(), ["cod2", "ut99", "cod2"]);
}

#[test]
fn no_master_refresh_keeps_previous_list() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    let _ = app.update(Message::Refresh); // corte de red durante el refresco
    no_master_now(&mut app);
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
    select(&mut app, "ut99");
    complete_now(&mut app, &ut99());
    select(&mut app, "cod2");
    assert_eq!(hosts(&app), ["1.1.1.1", "2.2.2.2"]);
    assert_eq!(fake.calls(), ["cod2", "cod2", "ut99"]);
}

#[test]
fn successful_scan_shows_final_status() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    assert_eq!(app.status(), "2 de 3 servidores respondieron");
}

#[test]
fn scan_progress_bar_visible_during_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    assert_eq!(app.scan.progress(), None);
    let generation = app.scan.generation();
    event(
        &mut app,
        generation,
        ScanEvent::Progress { done: 0, total: 3 },
    );
    event(
        &mut app,
        generation,
        ScanEvent::Progress { done: 2, total: 3 },
    );
    assert_eq!(app.scan.progress(), Some((2, 3)));
    assert_eq!(app.progress_text(), "Consultando 2 / 3");
    send(
        &mut app,
        generation,
        ScanMsg::Finished(ScanResult::default()),
    );
    assert_eq!(app.scan.progress(), None);
}

#[test]
fn scan_progress_bar_hidden_when_scan_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let generation = app.scan.generation();
    event(
        &mut app,
        generation,
        ScanEvent::Progress { done: 0, total: 2 },
    );
    let _ = app.update(Message::Stop);
    assert!(!app.is_scanning());
    assert_eq!(app.scan.progress(), None);
    event(
        &mut app,
        generation,
        ScanEvent::Progress { done: 1, total: 2 },
    ); // tardío
    assert_eq!(app.scan.progress(), None);
}

#[test]
fn status_messages_are_shown() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let generation = app.scan.generation();
    event(&mut app, generation, ScanEvent::Status("hola".into()));
    assert_eq!(app.status(), "hola");
    let _ = app.update(Message::Stop);
    event(&mut app, generation, ScanEvent::Status("viejo".into()));
    assert_eq!(app.status(), "hola");
}

#[test]
fn stopped_scan_keeps_rows_in_label_but_not_in_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let generation = app.scan.generation();
    event(
        &mut app,
        generation,
        ScanEvent::Server(mk("1.1.1.1", "a", 1, 1, "1.3")),
    );
    let _ = app.update(Message::Stop);
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 1");
    select(&mut app, "ut99");
    select(&mut app, "cod2");
    assert_eq!(fake.calls(), ["cod2", "ut99", "cod2"]);
}

#[test]
fn selection_is_cleared_when_rows_go_away() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    app.table.selected = Some(cod2()[0].address.clone());
    select(&mut app, "ut99");
    assert_eq!(app.table.selected, None);
}

#[test]
fn no_masters_or_steam_key_does_not_scan() {
    let dir = tempfile::tempdir().unwrap();
    let official = load_official(&dir.path().join("cache"));
    let mut custom = CustomMasters::default();
    let keys = effective_masters("cod2", &official, &custom)
        .iter()
        .map(master_key)
        .collect();
    custom.disabled.insert("cod2".into(), keys);
    save_custom(&custom, &dir.path().join("masters.custom.json")).unwrap();

    let (mut app, fake) = fake_app(dir.path(), Config::default());
    assert!(fake.calls().is_empty());
    assert!(!app.is_scanning());
    assert_eq!(
        app.status(),
        "Call of Duty 2: no hay masters activos; revísalos en Ajustes → Masters"
    );
    assert!(game("l4d").unwrap().steam_webapi);
    select(&mut app, "l4d");
    assert!(fake.calls().is_empty());
    assert_eq!(
        app.status(),
        "Left 4 Dead: falta la clave de API de Steam; ponla en Ajustes → General"
    );
}

#[test]
fn close_saves_and_exits() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    select(&mut app, "ut99");
    let _ = app.update(Message::CloseRequested);
    assert!(!app.is_scanning());
    let cfg = load_config(&dir.path().join("config.json"));
    assert_eq!(
        cfg.ui_state.get("last_game").map(String::as_str),
        Some("ut99")
    );
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
}

#[test]
fn switching_game_or_refresh_scrolls_to_top() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    // `units` cuenta las tareas: el escaneo y el `scroll_to` del `scrollable`.
    app.table.offset_y = 500.0;
    assert_eq!(app.update(Message::SelectGame("ut99")).units(), 2);
    assert_eq!(app.table.offset_y, 0.0);
    complete_now(&mut app, &ut99());
    app.table.offset_y = 500.0;
    assert_eq!(app.update(Message::SelectGame("cod2")).units(), 1); // lista guardada
    assert_eq!(app.table.offset_y, 0.0);
    app.table.offset_y = 500.0;
    assert_eq!(app.update(Message::Refresh).units(), 2);
    assert_eq!(app.table.offset_y, 0.0);
}

// ------------------------------------------------------------ scan_stream (ScanWorker)

#[test]
fn worker_marks_no_master_as_failed() {
    let msgs: Vec<ScanMsg> =
        block_on(scan_stream_with(|_emit| async { ScanResult::default() }).collect());
    assert_eq!(msgs, [ScanMsg::Finished(ScanResult::default())]);
    let ScanMsg::Finished(r) = &msgs[0] else {
        unreachable!()
    };
    assert!(!r.ok());
}

#[test]
fn worker_emits_progress() {
    let stream = scan_stream_with(|mut emit| async move {
        for done in 0..=2 {
            emit(ScanEvent::Progress { done, total: 2 });
        }
        ScanResult {
            masters_ok: 1,
            ..ScanResult::default()
        }
    });
    let msgs: Vec<ScanMsg> = block_on(stream.collect());
    let progress: Vec<ScanMsg> = (0..=2)
        .map(|done| ScanMsg::Event(ScanEvent::Progress { done, total: 2 }))
        .collect();
    assert_eq!(msgs[..3], progress[..]);
    assert!(matches!(msgs.last(), Some(ScanMsg::Finished(r)) if r.ok()));
    assert_eq!(msgs.len(), 4);
}

struct Guard(Arc<AtomicBool>);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn worker_cancel_stops_scan() {
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Guard(dropped.clone());
    let mut stream = scan_stream_with(move |mut emit| async move {
        let _guard = guard;
        emit(ScanEvent::Status("consultando".into()));
        futures::future::pending::<()>().await;
        ScanResult::default()
    });
    let mut cx = Context::from_waker(noop_waker_ref());
    for _ in 0..3 {
        if let Poll::Ready(None) = stream.poll_next_unpin(&mut cx) {
            panic!("el escaneo colgado no termina solo");
        }
    }
    assert!(!dropped.load(Ordering::SeqCst));
    drop(stream);
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn scan_stream_is_send() {
    fn assert_send<T: Send>(_: T) {}
    assert_send(scan_stream(
        game("cod2").unwrap(),
        vec![],
        ScanOptions::default(),
    ));
}

#[test]
fn panicking_scan_reports_error_and_keeps_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    let _ = app.update(Message::Refresh);
    let msgs: Vec<ScanMsg> = block_on(scan_stream_with(|_emit| async { panic!("boom") }).collect());
    let generation = app.scan.generation();
    for msg in msgs {
        send(&mut app, generation, msg);
    }
    assert!(!app.is_scanning());
    assert_eq!(app.status(), "Error en el escaneo: boom");
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
}

#[test]
fn system_theme_change_applies_live() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        theme: "system".into(),
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    let _ = app.update(Message::SystemMode(Mode::Light));
    assert_eq!(app.theme_id, "light");
    let _ = app.update(Message::SystemMode(Mode::Dark));
    assert_eq!(app.theme_id, "dark");
    let cfg = Config {
        theme: "classic".into(),
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    let _ = app.update(Message::SystemMode(Mode::Light));
    assert_eq!(app.theme_id, "classic");
}

// ------------------------------------------------ árbol de mods, mapas y países

fn mkm(host: &str, map: &str, fs_game: &str) -> ServerInfo {
    let mut info = mk(host, host, 40, 3, "1.3");
    info.map = map.to_owned();
    if !fs_game.is_empty() {
        info.rules.insert("fs_game".into(), fs_game.into());
    }
    info
}

fn tree_servers() -> Vec<ServerInfo> {
    vec![
        mkm("1.1.1.1", "mp_a", "mods/pam"),
        mkm("2.2.2.2", "mp_a", ""),
        mkm("3.3.3.3", "mp_b", "mods/pam"),
    ]
}

/// Hoja `value` de la carpeta `kind` del juego (abre el juego y sus carpetas para verla).
fn leaf(app: &mut App, id: &'static str, kind: Kind, value: &str) -> Option<TreeRow> {
    app.tree.set_expanded(id, Node::Game, true);
    app.tree.set_expanded(id, Node::Folder(kind), true);
    app.tree
        .rows()
        .into_iter()
        .find(|r| r.game == id && r.item == Item::Leaf(kind, value.to_owned()))
}

fn leaf_text(app: &mut App, id: &'static str, kind: Kind, value: &str) -> String {
    leaf(app, id, kind, value)
        .map(|r| r.text)
        .unwrap_or_default()
}

fn toggle(app: &mut App, id: &'static str, kind: Kind, value: &str) {
    let _ = app.update(Message::Tree(tree::Msg::Toggle(id, kind, value.to_owned())));
}

fn shown(app: &App) -> usize {
    app.table.model.order().len()
}

fn maps_marked(app: &App, id: &str) -> Vec<String> {
    let mut v: Vec<String> = app.tree.marks(id).maps.iter().cloned().collect();
    v.sort();
    v
}

#[test]
fn tree_shows_mods_and_maps_after_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &tree_servers());
    assert_eq!(leaf_text(&mut app, "cod2", Kind::Mods, "pam"), "pam · 2");
    assert_eq!(leaf_text(&mut app, "cod2", Kind::Maps, "mp_a"), "mp_a · 2");
}

#[test]
fn marking_map_filters_table() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        complete_now(&mut app, &tree_servers());
        app.table.selected = Some(tree_servers()[0].address.clone()); // mp_a: se oculta
        toggle(&mut app, "cod2", Kind::Maps, "mp_b");
        assert_eq!(shown(&app), 1);
        assert_eq!(app.table.selected, None);
        assert_eq!(
            app.count_text(),
            "1 servidor · filtro de mod/mapa/país activo"
        );
        toggle(&mut app, "cod2", Kind::Maps, "mp_b");
        assert_eq!(shown(&app), 3);
        assert_eq!(app.count_text(), "3 servidores");
    });
}

#[test]
fn marks_are_per_game_and_restored_on_switch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &tree_servers());
    toggle(&mut app, "cod2", Kind::Maps, "mp_b");
    select(&mut app, "ut99");
    complete_now(&mut app, &[mk("9.9.9.9", "u", 60, 3, "451")]);
    assert_eq!(shown(&app), 1);
    assert!(!app.count_text().contains("filtro"));
    select(&mut app, "cod2");
    assert_eq!((shown(&app), app.servers().len()), (1, 3));
    assert_eq!(maps_marked(&app, "cod2"), ["mp_b"]);
}

#[test]
fn tree_marks_survive_live_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &tree_servers());
    toggle(&mut app, "cod2", Kind::Maps, "mp_b");
    let _ = app.update(Message::Refresh); // la tabla queda vacía y el escaneo en curso
    assert!(app.is_scanning());
    assert_eq!(maps_marked(&app, "cod2"), ["mp_b"]);
    assert_eq!(leaf_text(&mut app, "cod2", Kind::Maps, "mp_b"), "mp_b · 0");
    complete_now(&mut app, &tree_servers());
    assert_eq!(maps_marked(&app, "cod2"), ["mp_b"]);
    assert_eq!(leaf_text(&mut app, "cod2", Kind::Maps, "mp_b"), "mp_b · 1");
    assert_eq!(shown(&app), 1);
}

#[test]
fn marked_leaf_gone_after_refresh_drops_mark() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &tree_servers());
    toggle(&mut app, "cod2", Kind::Maps, "mp_b");
    let _ = app.update(Message::Refresh);
    assert_eq!(leaf_text(&mut app, "cod2", Kind::Maps, "mp_b"), "mp_b · 0");
    complete_now(
        &mut app,
        &[mkm("1.1.1.1", "mp_a", ""), mkm("2.2.2.2", "mp_a", "")],
    );
    assert!(app.tree.marks("cod2").is_empty());
    assert!(leaf(&mut app, "cod2", Kind::Maps, "mp_b").is_none());
    assert_eq!(shown(&app), 2);
    assert!(!app.count_text().contains("filtro"));
}

#[test]
fn count_label_mentions_tree_filter() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        complete_now(&mut app, &tree_servers());
        assert!(!app.count_text().contains("filtro"));
        toggle(&mut app, "cod2", Kind::Mods, "pam");
        assert!(
            app.count_text()
                .ends_with(" · filtro de mod/mapa/país activo")
        );
        assert_eq!(shown(&app), 2);
    });
}

#[test]
fn double_refresh_keeps_live_mark() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &tree_servers());
    let _ = app.update(Message::Refresh);
    let generation = app.scan.generation();
    event(
        &mut app,
        generation,
        ScanEvent::Server(mkm("1.1.1.1", "mp_new", "")),
    );
    // El desglose en vivo llega con el temporizador de 300 ms.
    assert!(leaf(&mut app, "cod2", Kind::Maps, "mp_new").is_none());
    let _ = app.update(Message::TreeTick);
    assert!(leaf(&mut app, "cod2", Kind::Maps, "mp_new").is_some());
    toggle(&mut app, "cod2", Kind::Maps, "mp_new");
    let _ = app.update(Message::Refresh); // segundo F5 antes de terminar el primero
    assert_eq!(maps_marked(&app, "cod2"), ["mp_new"]);
    let _ = app.update(Message::Stop);
    assert_eq!(maps_marked(&app, "cod2"), ["mp_new"]);
}

#[test]
fn failed_scan_keeps_mark_and_shows_model_breakdown() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &tree_servers());
    toggle(&mut app, "cod2", Kind::Maps, "mp_b");
    let _ = app.update(Message::Refresh);
    no_master_now(&mut app);
    assert!(!app.is_scanning());
    assert_eq!(maps_marked(&app, "cod2"), ["mp_b"]);
    assert_eq!(leaf_text(&mut app, "cod2", Kind::Maps, "mp_b"), "mp_b · 0");
    // No el desglose de la caché vieja.
    assert!(leaf(&mut app, "cod2", Kind::Maps, "mp_a").is_none());
}

#[test]
fn stopped_scan_tree_count_uses_table_rows() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let generation = app.scan.generation();
    for info in cod2() {
        event(&mut app, generation, ScanEvent::Server(info));
    }
    let _ = app.update(Message::Stop);
    assert_eq!(app.servers().len(), 2);
    let row = app
        .tree
        .rows()
        .into_iter()
        .find(|r| r.game == "cod2")
        .unwrap();
    assert_eq!(row.text, "Call of Duty 2 · 2");
}

#[test]
fn tree_shows_bf1942_mod() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let mut info = mk("1.1.1.1", "a", 40, 3, "1.3");
    info.rules.insert("gameId".into(), "DC_Final".into());
    let mut base = mk("2.2.2.2", "b", 40, 3, "1.3");
    base.rules.insert("gameId".into(), "bf1942".into());
    select(&mut app, "bf1942");
    complete_now(&mut app, &[info, base]);
    assert_eq!(
        leaf_text(&mut app, "bf1942", Kind::Mods, "dc_final"),
        "dc_final · 1"
    );
}

#[test]
fn tree_countries_folder_filters_table() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        app.table
            .model
            .set_country_lookup(Some(Arc::new(|ip: &str| {
                (ip == "1.1.1.1").then(|| "CL".to_owned())
            })));
        complete_now(&mut app, &cod2());
        assert_eq!(
            leaf_text(&mut app, "cod2", Kind::Countries, "cl"),
            "Chile · 1"
        );
        toggle(&mut app, "cod2", Kind::Countries, "cl");
        assert_eq!(shown(&app), 1);
        assert!(
            app.count_text()
                .ends_with(" · filtro de mod/mapa/país activo")
        );
        app.clear_tree_marks();
        assert_eq!(shown(&app), 2);
        assert!(app.tree.marks("cod2").is_empty());
    });
}

/// Los juegos dados y los que conectan por Steam (listos sin ruta) primero, en el orden de
/// `GAMES`; luego el resto.
fn then_rest(first: &[&str]) -> Vec<&'static str> {
    let ready = |g: &GameDef| first.contains(&g.id) || g.launch_style == LaunchStyle::Steam;
    let mut ids: Vec<&'static str> = GAMES.iter().filter(|g| ready(g)).map(|g| g.id).collect();
    ids.extend(GAMES.iter().filter(|g| !ready(g)).map(|g| g.id));
    ids
}

fn with_paths(paths: &[(&str, &str)]) -> Config {
    Config {
        game_paths: paths
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        ..Config::default()
    }
}

#[test]
fn sidebar_configured_games_first() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = with_paths(&[("cod1", r"C:\CoD")]);
    assert_eq!(App::sorted_game_ids(&cfg), then_rest(&["cod1"]));
    let (app, _) = fake_app(dir.path(), cfg);
    assert_eq!(app.tree.game_ids(), then_rest(&["cod1"]));
    let names: Vec<String> = app.tree.rows().into_iter().map(|r| r.tooltip).collect();
    let expected: Vec<&str> = then_rest(&["cod1"])
        .iter()
        .map(|id| game(id).unwrap().name)
        .collect();
    assert_eq!(names, expected);
}

#[test]
fn sidebar_without_paths_steam_launch_games_first() {
    let cfg = with_paths(&[]);
    let steam: Vec<&str> = GAMES
        .iter()
        .filter(|g| g.launch_style == LaunchStyle::Steam)
        .map(|g| g.id)
        .collect();
    let ids = App::sorted_game_ids(&cfg);
    assert!(!steam.is_empty());
    assert_eq!(ids[..steam.len()], steam[..]);
    assert_eq!(ids, then_rest(&[]));
}

#[test]
fn sidebar_default_config_order() {
    assert_eq!(
        App::sorted_game_ids(&Config::default()),
        then_rest(&["cod2", "ut99", "mohaa"])
    );
}

#[test]
fn tree_uses_badge_without_path() {
    // shortcut: el icono del exe llega en la 5e; hasta entonces cada juego lleva su insignia.
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fake_app(dir.path(), with_paths(&[]));
    let q3 = app
        .tree
        .rows()
        .into_iter()
        .find(|r| r.game == "q3")
        .unwrap();
    assert_eq!(q3.item, Item::Game);
    assert!(palette(app.theme_id).badge("q3").is_some());
    let _ = app.view();
}

// ------------------------------------- filtros rápidos, texto, versión, países y foco

fn side(app: &mut App, m: sidebar::Msg) -> Task<Message> {
    app.update(Message::Sidebar(m))
}

fn saved(dir: &Path) -> Config {
    load_config(&dir.join("config.json"))
}

#[test]
fn hide_empty_checkbox_filters_and_persists() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        let empty = mk("2.2.2.2", "b", 50, 0, "1.3");
        complete_now(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3"), empty]);
        assert_eq!(shown(&app), 2);
        let _ = side(&mut app, sidebar::Msg::HideEmpty(true));
        assert_eq!(shown(&app), 1);
        assert_eq!(app.count_text(), "1 servidor");
        assert!(app.quick_filters().hide_empty);
        assert!(saved(dir.path()).quick_filters.hide_empty);
    });
}

#[test]
fn quick_filter_widgets_reflect_config() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            quick_filters: QuickFilters {
                hide_full: true,
                hide_password: true,
                max_ping: 150,
                ..QuickFilters::default()
            },
            ..Config::default()
        };
        let (app, _) = fake_app(dir.path(), cfg);
        let q = app.quick_filters();
        assert!(!q.hide_empty && q.hide_full && q.hide_password);
        // El filtro ya se aplica al arrancar.
        assert_eq!(app.table.model.filter().quick, *q);
        let labels: Vec<String> = ping_options(q.max_ping)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(labels, ["Sin límite", "50", "100", "150", "250"]);
        let _ = app.view();
    });
}

#[test]
fn max_ping_combo_filters_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(
        &mut app,
        &[
            mk("1.1.1.1", "a", 40, 3, "1.3"),
            mk("2.2.2.2", "b", 120, 3, "1.3"),
        ],
    );
    let _ = side(&mut app, sidebar::Msg::MaxPing(100));
    assert_eq!(shown(&app), 1);
    assert_eq!(saved(dir.path()).quick_filters.max_ping, 100);
    let _ = side(&mut app, sidebar::Msg::MaxPing(0));
    assert_eq!(shown(&app), 2);
    assert_eq!(saved(dir.path()).quick_filters.max_ping, 0);
}

type GeoCalls = Arc<Mutex<Vec<(PathBuf, bool)>>>;

/// App con una base de países falsa (`FakeGeo`): `db` es el CSV, `None` = falla al bajarla.
fn geo_app(dir: &Path, db: Option<&str>) -> (support::TestApp, Task<Message>, GeoCalls) {
    let lang = support::lang::Spanish::hold();
    let csv = dir.join("fake.csv");
    if let Some(text) = db {
        std::fs::write(&csv, text).unwrap();
    }
    let calls: GeoCalls = Arc::default();
    let seen = calls.clone();
    let fake = support::FakeScan::default();
    let mut deps = support::fake_deps(&fake, "");
    let ok = db.is_some();
    deps.country_db = Some(Arc::new(move |path: &Path, refresh| {
        seen.lock().unwrap().push((path.to_owned(), refresh));
        if ok {
            CountryDB::from_csv(&csv).map_err(|e| e.to_string())
        } else {
            Err("No se pudo descargar la base de países".to_owned())
        }
    }));
    let (app, task) = App::new(Config::default(), dir.join("config.json"), deps);
    drop(lang);
    (support::TestApp::new(app), task, calls)
}

fn country_db_msgs(task: Task<Message>) -> Vec<Message> {
    support::run_task(task)
        .into_iter()
        .filter(|m| matches!(m, Message::CountryDb { .. }))
        .collect()
}

#[test]
fn country_db_loads_in_background_from_cache_dir() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, task, calls) = geo_app(dir.path(), Some("1.1.1.1,1.1.1.1,CL\n"));
        complete_now(&mut app, &cod2());
        // Nada en `new` ni en `update`: aún sin base, la columna sigue vacía.
        assert!(calls.lock().unwrap().is_empty());
        let first = app.servers()[0].clone();
        assert_eq!(app.table.model.country_code(&first), None);
        let msgs = country_db_msgs(task);
        assert_eq!(
            *calls.lock().unwrap(),
            [(
                dir.path().join("cache").join("dbip-country-lite.csv"),
                false
            )]
        );
        for m in msgs {
            let _ = app.update(m);
        }
        assert_eq!(app.table.model.country_code(&first), Some("cl"));
        assert_eq!(
            leaf_text(&mut app, "cod2", Kind::Countries, "cl"),
            "Chile · 1"
        );
    });
}

#[test]
fn country_db_late_result_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, task, _) = geo_app(dir.path(), Some("1.1.1.1,1.1.1.1,CL\n"));
    complete_now(&mut app, &cod2());
    let old = app.country_request().wrapping_sub(1);
    for m in country_db_msgs(task) {
        let Message::CountryDb { result, .. } = m else {
            unreachable!()
        };
        let _ = app.update(Message::CountryDb {
            request: old,
            result,
        });
    }
    let first = app.servers()[0].clone();
    assert_eq!(app.table.model.country_code(&first), None);
}

#[test]
fn country_db_failure_only_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, task, _) = geo_app(dir.path(), None);
    for m in country_db_msgs(task) {
        // Dura 8 s, como `showMessage(error, 8000)`.
        let timer = app.update(m);
        assert!(iced_runtime::task::into_stream(timer).is_some());
    }
    assert_eq!(app.status(), "No se pudo descargar la base de países");
    let seq = app.status_seq();
    let _ = app.update(Message::StatusExpired(seq));
    assert_eq!(app.status(), "");
    complete_now(&mut app, &cod2());
    let first = app.servers()[0].clone();
    assert_eq!(app.table.model.country_code(&first), None);
}

#[test]
fn country_combo_lists_loaded_countries_and_filters() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, task, _) = geo_app(dir.path(), Some("1.1.1.1,1.1.1.1,CL\n"));
        for m in country_db_msgs(task) {
            let _ = app.update(m);
        }
        complete_now(&mut app, &cod2());
        let _ = side(&mut app, sidebar::Msg::CountryOpened);
        let labels: Vec<String> = app
            .country_options()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(labels, ["Todos los países", "Chile (1)"]);
        let _ = side(&mut app, sidebar::Msg::Country("cl".into()));
        assert_eq!(shown(&app), 1);
        assert_eq!(app.quick_filters().country, "cl");
        assert_eq!(saved(dir.path()).quick_filters.country, "cl");
    });
}

#[test]
fn country_filter_and_card() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, task, _) = geo_app(dir.path(), Some("1.1.1.1,1.1.1.1,CL\n"));
    for m in country_db_msgs(task) {
        let _ = app.update(m);
    }
    complete_now(&mut app, &cod2());
    let _ = side(&mut app, sidebar::Msg::Text("cl".into()));
    assert_eq!(shown(&app), 1);
}

fn version_labels(app: &App) -> Vec<String> {
    app.version_options()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn selected_version_label(app: &App) -> String {
    let key = app.table.model.filter().version.clone();
    app.version_options()
        .into_iter()
        .find(|o| o.key == key)
        .map(|o| o.to_string())
        .unwrap_or_default()
}

fn cod2x_servers() -> Vec<ServerInfo> {
    vec![
        mk("1.1.1.1", "a", 40, 3, "1.3"),
        mk("2.2.2.2", "b", 40, 3, "1.4"),
        mk("3.3.3.3", "c", 40, 3, "1.4"),
    ]
}

#[test]
fn version_filter_default_hides_other_versions() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(
        &mut app,
        &[
            mk("1.1.1.1", "a", 40, 3, "1.3"),
            mk("2.2.2.2", "b", 40, 3, "1.0"),
        ],
    );
    assert_eq!(app.servers().len(), 2);
    assert_eq!(shown(&app), 1);
}

#[test]
fn version_combo_shows_cod2x_label_and_filters() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2x_servers());
    let labels = version_labels(&app);
    assert!(labels.contains(&"1.4 (CoD2x)".to_owned()));
    assert!(!labels.contains(&"1.4".to_owned()));
    let _ = side(&mut app, sidebar::Msg::Version("1.4".into()));
    assert_eq!(shown(&app), 2);
    assert_eq!(saved(dir.path()).version_filter["cod2"], "1.4");
}

#[test]
fn version_combo_cod2x_label_english() {
    with_language("en", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        complete_now(&mut app, &cod2x_servers());
        assert!(version_labels(&app).contains(&"1.4 (CoD2x)".to_owned()));
    });
}

#[test]
fn saved_cod2x_filter_selects_label() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.version_filter.insert("cod2".into(), "1.4".into());
    let (mut app, _) = fake_app(dir.path(), cfg);
    complete_now(&mut app, &cod2x_servers());
    assert_eq!(selected_version_label(&app), "1.4 (CoD2x)");
    assert_eq!(shown(&app), 2);
}

#[test]
fn version_combo_lists_observed_versions_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(
        &mut app,
        &[
            mk("1.1.1.1", "a", 40, 3, "1.3"),
            mk("2.2.2.2", "b", 40, 3, "1.01b"),
        ],
    );
    let labels = version_labels(&app);
    assert_eq!(labels[..4], ["Todas", "1.0", "1.2", "1.3"]);
    assert!(labels.contains(&"1.01b".to_owned()));
    assert_eq!(selected_version_label(&app), "1.3");
    let _ = side(&mut app, sidebar::Msg::Version("Todas".into()));
    assert_eq!(shown(&app), 2);
    assert_eq!(saved(dir.path()).version_filter["cod2"], "Todas");
}

#[test]
fn text_filter_updates_selection_state() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        complete_now(&mut app, &[mk("1.1.1.1", "alpha", 40, 3, "1.3")]);
        let addr = app.servers()[0].address.clone();
        let _ = app.update(Message::Table(table::Msg::RowClicked(addr)));
        assert!(app.table.selected.is_some());
        let _ = side(&mut app, sidebar::Msg::Text("zzz".into()));
        assert_eq!(shown(&app), 0);
        assert!(app.table.selected.is_none());
        assert_eq!(app.count_text(), "0 servidores");
    });
}

fn only_installed(paths: &[(&str, String)]) -> Config {
    Config {
        only_installed_games: true,
        game_paths: paths
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        ..Config::default()
    }
}

/// Entrega a la app las respuestas de la búsqueda de exes que trae `task`.
fn deliver_installed(app: &mut App, task: Task<Message>) {
    for m in support::run_task(task) {
        if matches!(m, Message::Installed { .. }) {
            let _ = app.update(m);
        }
    }
}

#[test]
fn installed_check_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = only_installed(&[("cod2", "/no/existe".into())]);
    let (mut app, fake, task) = fake_app_task(dir.path(), cfg);
    select(&mut app, "ut99");
    let _ = side(&mut app, sidebar::Msg::OnlyInstalled(false));
    let _ = side(&mut app, sidebar::Msg::OnlyInstalled(true));
    let _ = app.view();
    assert!(fake.installed_calls().is_empty());
    deliver_installed(&mut app, task);
    assert_eq!(fake.installed_calls(), ["cod2"]);
}

#[test]
fn installed_tree_waits_optimistically_then_updates() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("vacia").display().to_string();
    let (mut app, _, task) = fake_app_task(dir.path(), only_installed(&[("cod2", empty)]));
    select(&mut app, "ut99");
    // Sin respuesta todavía: con ruta cuenta como listo.
    assert!(app.tree.game_ids().contains(&"cod2"));
    deliver_installed(&mut app, task);
    assert!(!app.tree.game_ids().contains(&"cod2"));
}

#[test]
fn only_installed_checks_disk_once_per_path_change() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = only_installed(&[("cod2", "/no/existe".into())]);
    let (mut app, fake, task) = fake_app_task(dir.path(), cfg);
    deliver_installed(&mut app, task);
    assert_eq!(fake.installed_calls().len(), 1);
    // Cambiar de juego no vuelve al disco (las tareas que devuelve tampoco).
    let a = app.update(Message::SelectGame("ut99"));
    let b = app.update(Message::SelectGame("cod2"));
    deliver_installed(&mut app, Task::batch([a, b]));
    assert_eq!(fake.installed_calls().len(), 1);
}

#[test]
fn theme_change_keeps_installed_answers() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<(&str, String)> = ["q3", "rtcw", "et", "cod1", "cod2"]
        .iter()
        .map(|g| (*g, dir.path().join(g).display().to_string()))
        .collect();
    let cfg = Config {
        theme: "system".into(),
        ..only_installed(&paths)
    };
    let (mut app, _, task) = fake_app_task(dir.path(), cfg);
    select(&mut app, "tf2");
    deliver_installed(&mut app, task);
    assert!(!app.tree.game_ids().contains(&"cod2"));
    let generation = app.installed_generation();
    let t = app.update(Message::SystemMode(Mode::Light));
    deliver_installed(&mut app, t);
    assert_eq!(app.installed_generation(), generation);
    assert!(!app.tree.game_ids().contains(&"cod2"));
}

#[test]
fn only_installed_games_filters_tree() {
    let dir = tempfile::tempdir().unwrap();
    let exe_dir = dir.path().join("cod2");
    std::fs::create_dir(&exe_dir).unwrap();
    std::fs::write(exe_dir.join("CoD2MP_s.exe"), b"").unwrap();
    let mut cfg = only_installed(&[
        ("cod2", exe_dir.display().to_string()),
        ("ut99", dir.path().join("nope").display().to_string()),
    ]);
    cfg.only_installed_games = false;
    let fake = support::FakeScan::default();
    let mut deps = support::fake_deps(&fake, "");
    deps.game_installed = Arc::new(jase::scan::game_installed);
    let (app, task) = App::new(cfg, dir.path().join("config.json"), deps);
    let mut app = support::TestApp::new(app);
    assert!(!app.cfg_only_installed());
    assert_eq!(app.tree.game_ids().len(), GAMES.len());
    select(&mut app, "cod2");
    let _ = side(&mut app, sidebar::Msg::OnlyInstalled(true));
    deliver_installed(&mut app, task);
    // Con ruta válida o por Steam; la ruta sin exe (ut99) no cuenta.
    let ids = app.tree.game_ids();
    assert!(!ids.contains(&"ut99"));
    assert!(ids.contains(&"cod2") && ids.contains(&"tf2"));
    assert!(saved(dir.path()).only_installed_games);
    // El juego actual sigue visible aunque no esté instalado.
    select(&mut app, "ut99");
    assert!(app.tree.game_ids().contains(&"ut99"));
    let _ = side(&mut app, sidebar::Msg::OnlyInstalled(false));
    assert_eq!(app.tree.game_ids().len(), GAMES.len());
}

#[test]
fn only_installed_games_restored_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = only_installed(&[]);
    cfg.game_paths.clear();
    let (app, _) = fake_app(dir.path(), cfg);
    assert!(app.cfg_only_installed());
    let current = app.tree.current();
    assert!(app.tree.game_ids().iter().all(|id| {
        game(id).unwrap().launch_style == LaunchStyle::Steam || Some(*id) == current
    }));
}

fn press(app: &mut App, key: Named) -> bool {
    match app.key_message(Key::Named(key)) {
        Some(m) => {
            let _ = app.update(m);
            true
        }
        None => false,
    }
}

#[test]
fn arrow_keys_follow_focus() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    assert_eq!(app.focus(), Focus::Sidebar);
    let order: Vec<_> = app
        .table
        .model
        .order()
        .iter()
        .map(|&r| app.servers()[r].address.clone())
        .collect();
    let _ = app.update(Message::Table(table::Msg::RowClicked(order[0].clone())));
    assert_eq!(app.focus(), Focus::Table);
    assert!(press(&mut app, Named::ArrowDown));
    assert_eq!(app.table.selected.as_ref(), Some(&order[1]));
    // Escribiendo en el campo, ↓ no mueve nada.
    let _ = app.update(Message::FilterFocused(true));
    assert_eq!(app.focus(), Focus::Filter);
    assert!(!press(&mut app, Named::ArrowUp));
    assert_eq!(app.table.selected.as_ref(), Some(&order[1]));
    // Un clic en el árbol le da las flechas: ↓ cambia de juego.
    let _ = app.update(Message::Tree(tree::Msg::Activate("cod2")));
    assert_eq!(app.focus(), Focus::Tree);
    assert!(press(&mut app, Named::ArrowDown));
    assert_ne!(app.tree.current(), Some("cod2"));
}

#[test]
fn focus_off_table_unfocuses_selection_style() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    let addr = app.servers()[0].address.clone();
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr.clone())));
    assert!(app.table.focused);
    let _ = side(&mut app, sidebar::Msg::HideFull(true));
    assert!(!app.table.focused);
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr)));
    let _ = app.update(Message::FilterFocused(true));
    assert!(!app.table.focused);
    // Perder el foco del campo no da las flechas a nadie (Qt deja el foco donde estaba).
    let _ = app.update(Message::FilterFocused(false));
    assert_eq!(app.focus(), Focus::Sidebar);
}

fn preset(app: &mut App, m: presets::Msg) -> Task<Message> {
    app.update(Message::Presets(m))
}

/// «Guardar filtros actuales…» con ese nombre en el modal.
fn save_preset(app: &mut App, name: &str) {
    let _ = preset(app, presets::Msg::Pick(PresetItem::Save));
    assert_eq!(app.focus(), Focus::Dialog);
    let _ = preset(app, presets::Msg::NameChanged(name.to_owned()));
    let _ = preset(app, presets::Msg::Submit);
    assert!(app.preset_name().is_none());
}

/// El menú «Filtros guardados» en texto: «—» es el separador y «Borrar ▸ a, b» el submenú.
fn preset_labels(app: &App) -> Vec<String> {
    entries(app, Menu::Presets)
        .iter()
        .map(|e| match e {
            Entry::Item { label, .. } => label.clone(),
            Entry::Separator => "—".to_owned(),
            Entry::Sub { label, entries } => {
                let names: Vec<String> = entries
                    .iter()
                    .filter_map(|e| match e {
                        Entry::Item { label, .. } => Some(label.clone()),
                        _ => None,
                    })
                    .collect();
                format!("{label} ▸ {}", names.join(", "))
                    .trim_end()
                    .to_owned()
            }
        })
        .collect()
}

#[test]
fn filter_presets_save_and_apply() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        let _ = side(&mut app, sidebar::Msg::HideEmpty(true));
        let _ = side(&mut app, sidebar::Msg::MaxPing(77));
        let _ = side(&mut app, sidebar::Msg::Country("de".into()));
        let _ = side(&mut app, sidebar::Msg::Text("dm".into()));
        save_preset(&mut app, "  Mi mezcla ");
        let names: Vec<&str> = app
            .filter_presets()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["Mi mezcla"]);
        assert_eq!(saved(dir.path()).filter_presets, app.filter_presets());
        assert_eq!(
            preset_labels(&app),
            [
                "Mi mezcla",
                "—",
                "Guardar filtros actuales…",
                "Borrar ▸ Mi mezcla"
            ]
        );
        let _ = side(&mut app, sidebar::Msg::HideEmpty(false));
        let _ = side(&mut app, sidebar::Msg::HideFull(true));
        let _ = side(&mut app, sidebar::Msg::MaxPing(0));
        let _ = side(&mut app, sidebar::Msg::Country(String::new()));
        let _ = side(&mut app, sidebar::Msg::Text(String::new()));
        let _ = preset(
            &mut app,
            presets::Msg::Pick(PresetItem::Apply("Mi mezcla".into())),
        );
        let q = app.quick_filters().clone();
        assert!(q.hide_empty && !q.hide_full);
        assert_eq!((q.max_ping, q.country.as_str()), (77, "de"));
        assert_eq!(app.filter_text(), "dm");
        assert_eq!(app.table.model.filter().quick, q);
        assert_eq!(app.table.model.filter().text, "dm");
        assert_eq!(saved(dir.path()).quick_filters.max_ping, 77);
        let _ = preset(
            &mut app,
            presets::Msg::Pick(PresetItem::Delete("mi mezcla".into())),
        );
        assert!(app.filter_presets().is_empty());
        assert!(saved(dir.path()).filter_presets.is_empty());
        assert_eq!(
            preset_labels(&app),
            ["Guardar filtros actuales…", "Borrar ▸"]
        );
        let _ = app.view();
    });
}

#[test]
fn hide_bots_check_mirrors_menu_and_presets() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = side(&mut app, sidebar::Msg::HideBots(true));
    save_preset(&mut app, "B");
    let _ = side(&mut app, sidebar::Msg::HideBots(false));
    let _ = preset(&mut app, presets::Msg::Pick(PresetItem::Apply("B".into())));
    assert!(app.quick_filters().hide_bots);
    assert!(app.table.model.filter().quick.hide_bots);
}

#[test]
fn preset_cap_keeps_list_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let full: Vec<FilterPreset> = (0..MAX_PRESETS)
        .map(|i| FilterPreset {
            name: format!("p{i}"),
            quick: QuickFilters::default(),
            text: String::new(),
        })
        .collect();
    let cfg = Config {
        filter_presets: full.clone(),
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    let path = dir.path().join("config.json");
    let before = std::fs::read(&path).ok();
    save_preset(&mut app, "nuevo");
    assert_eq!(app.filter_presets(), full);
    assert_eq!(std::fs::read(&path).ok(), before);
    // Uno con el nombre de otro sí entra: lo reemplaza en su sitio.
    save_preset(&mut app, "P3");
    assert_eq!(app.filter_presets()[3].name, "P3");
    assert_eq!(app.filter_presets().len(), MAX_PRESETS);
}

fn key_event(key: Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(key),
        modified_key: Key::Named(key),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    })
}

#[test]
fn modal_escape_and_empty_name_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::Table(table::Msg::RowClicked(
        mk("1.1.1.1", "a", 40, 3, "1.3").address,
    )));
    let _ = preset(&mut app, presets::Msg::Pick(PresetItem::Save));
    assert_eq!(app.focus(), Focus::Dialog);
    assert_eq!(app.preset_name(), Some(""));
    // Con el modal, las flechas no llegan a la tabla ni al árbol.
    assert!(!press(&mut app, Named::ArrowDown));
    let _ = preset(&mut app, presets::Msg::NameChanged("x".into()));
    let _ = app.view();
    // Esc dentro del campo del modal: el `text_input` lo captura, `listen_with` igual lo ve.
    let esc = event_message(
        key_event(Named::Escape),
        Status::Captured,
        window::Id::unique(),
    );
    let esc = esc.expect("Esc capturado llega");
    let _ = app.update(esc);
    assert!(app.preset_name().is_none());
    assert_eq!(app.focus(), Focus::Table); // vuelve a quien lo tenía
    assert!(app.filter_presets().is_empty());
    // Nombre vacío (o solo espacios) = Cancelar.
    let _ = preset(&mut app, presets::Msg::Pick(PresetItem::Save));
    let _ = preset(&mut app, presets::Msg::NameChanged("   ".into()));
    let _ = preset(&mut app, presets::Msg::Submit);
    assert!(app.preset_name().is_none());
    assert!(app.filter_presets().is_empty());
    let _ = preset(&mut app, presets::Msg::Pick(PresetItem::Save));
    let _ = preset(&mut app, presets::Msg::Cancel);
    assert!(app.preset_name().is_none());
}

#[test]
fn escape_and_click_ask_who_has_focus() {
    let id = window::Id::unique();
    let click = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    for status in [Status::Ignored, Status::Captured] {
        assert!(matches!(
            event_message(click.clone(), status, id),
            Some(Message::MousePressed)
        ));
        assert!(matches!(
            event_message(key_event(Named::Escape), status, id),
            Some(Message::Escape)
        ));
    }
    assert!(event_message(key_event(Named::ArrowDown), Status::Captured, id).is_none());
    // Fuera del modal, Esc pregunta al campo (como el clic) y la respuesta mueve el foco: el
    // Esc que el campo capturó lo dejó sin foco y las flechas no van a nadie, como en Qt.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let task = app.update(Message::MousePressed);
    let msgs = support::run_task_with(task, Some(true));
    assert!(matches!(msgs.as_slice(), [Message::FilterFocused(true)]));
    for m in msgs {
        let _ = app.update(m);
    }
    assert_eq!(app.focus(), Focus::Filter);
    let task = app.update(Message::Escape);
    let msgs = support::run_task_with(task, Some(false));
    assert!(matches!(msgs.as_slice(), [Message::FilterFocused(false)]));
    for m in msgs {
        let _ = app.update(m);
    }
    assert_eq!(app.focus(), Focus::Sidebar);
    let current = app.tree.current();
    assert!(!press(&mut app, Named::ArrowDown));
    assert_eq!(app.tree.current(), current);
}

#[test]
fn startup_focus_is_sidebar() {
    // Qt arranca con el foco en una casilla de la barra lateral: las flechas no mueven nada.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    assert_eq!(app.focus(), Focus::Sidebar);
    assert!(!press(&mut app, Named::ArrowDown));
    assert!(!press(&mut app, Named::Space));
    assert_eq!(app.tree.current(), Some("cod2"));
    assert!(app.table.selected.is_none());
}

#[test]
fn checkbox_click_keeps_arrows_off_tree() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete_now(&mut app, &cod2());
    let _ = app.update(Message::Tree(tree::Msg::Activate("cod2")));
    assert_eq!(app.focus(), Focus::Tree);
    let _ = side(&mut app, sidebar::Msg::HideFull(true));
    assert_eq!(app.focus(), Focus::Sidebar);
    assert!(!press(&mut app, Named::ArrowDown));
    assert_eq!(app.tree.current(), Some("cod2"));
    // Con un combo abierto (`on_open`), flechas y Espacio no llegan al árbol ni a la tabla.
    let addr = app.servers()[0].address.clone();
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr.clone())));
    let _ = side(&mut app, sidebar::Msg::ComboOpened);
    assert_eq!(app.focus(), Focus::Sidebar);
    assert!(!press(&mut app, Named::ArrowDown));
    assert!(!press(&mut app, Named::Space));
    assert_eq!(app.table.selected.as_ref(), Some(&addr));
    // Un clic en la zona vacía de la tabla le da el foco, como Qt.
    let _ = app.update(Message::Table(table::Msg::BlankClicked));
    assert_eq!(app.focus(), Focus::Table);
    assert!(press(&mut app, Named::ArrowDown));
}

#[test]
fn dialog_enum_escape_cancels() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::Tree(tree::Msg::Activate("cod2")));
    let _ = preset(&mut app, presets::Msg::Pick(PresetItem::Save));
    let _ = preset(&mut app, presets::Msg::NameChanged("x".into()));
    assert_eq!(app.dialog(), Some(&Dialog::SavePreset("x".into())));
    let _ = app.view();
    let _ = app.update(Message::Escape);
    assert_eq!(app.dialog(), None);
    assert_eq!(app.focus(), Focus::Tree);
    assert!(app.filter_presets().is_empty());
}
