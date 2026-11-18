//! Escaneo cableado en la app (`test_main_window.py` y `ScanWorker` de Python): los mensajes
//! que traería la `Task` se meten a mano con la generación vigente o una vieja.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use futures::StreamExt;
use futures::executor::block_on;
use futures::task::noop_waker_ref;
use iced::theme::Mode;
use jase::app::{App, Message};
use jase::scan::{ScanMsg, scan_stream, scan_stream_with};
use jase::theme::palette;
use jase::tree::{self, Item, Kind, Node, TreeRow};
use jase_core::config::{Config, load_config};
use jase_core::games::{GAMES, GameDef, LaunchStyle, game};
use jase_core::masters::{
    CustomMasters, effective_masters, load_official, master_key, save_custom,
};
use jase_core::models::ServerInfo;
use jase_core::scanner::scan::{MasterError, ScanEvent, ScanOptions, ScanResult};
use support::lang::with_language;
use support::{fake_app, mk};

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
    let generation = app.generation();
    complete(app, generation, servers);
}

fn no_master_now(app: &mut App) {
    let generation = app.generation();
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
    let old = app.generation();
    select(&mut app, "ut99");
    assert_ne!(app.generation(), old);
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
    let old = app.generation();
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
    let stuck = app.generation();
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
    let generation = app.generation();
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
    assert_eq!(app.progress(), None);
    let generation = app.generation();
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
    assert_eq!(app.progress(), Some((2, 3)));
    assert_eq!(app.progress_text(), "Consultando 2 / 3");
    send(
        &mut app,
        generation,
        ScanMsg::Finished(ScanResult::default()),
    );
    assert_eq!(app.progress(), None);
}

#[test]
fn scan_progress_bar_hidden_when_scan_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let generation = app.generation();
    event(
        &mut app,
        generation,
        ScanEvent::Progress { done: 0, total: 2 },
    );
    let _ = app.update(Message::Stop);
    assert!(!app.is_scanning());
    assert_eq!(app.progress(), None);
    event(
        &mut app,
        generation,
        ScanEvent::Progress { done: 1, total: 2 },
    ); // tardío
    assert_eq!(app.progress(), None);
}

#[test]
fn status_messages_are_shown() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let generation = app.generation();
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
    let generation = app.generation();
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
    let generation = app.generation();
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
    let generation = app.generation();
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
    let generation = app.generation();
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
