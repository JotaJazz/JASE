//! Búsqueda en la red local y «Refrescar este servidor» (`test_lan.py`, `test_main_window.py`):
//! los mensajes que traería la `Task` se meten a mano con la generación o el pedido vigente.

mod support;

use std::io;
use std::path::Path;

use futures::StreamExt;
use futures::executor::block_on;
use jase::app::{App, Message};
use jase::scan::{ScanMsg, ScanState, lan_finish, scan_stream_with};
use jase::table;
use jase_core::config::Config;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use jase_core::server_cache;
use support::lang::with_language;
use support::{FakeScan, TestApp, fake_app, mk, run_task};

fn complete(app: &mut App, servers: &[ServerInfo]) {
    let generation = app.scan.generation();
    for info in servers {
        let _ = app.update(Message::Scan {
            generation,
            msg: ScanMsg::Event(ScanEvent::Server(info.clone())),
        });
    }
    let result = ScanResult {
        masters_ok: 1,
        servers_listed: servers.len(),
        servers_responded: servers.len(),
        errors: vec![],
    };
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Finished(result),
    });
}

fn names(app: &App) -> Vec<String> {
    let mut v: Vec<String> = app.servers().iter().map(|s| s.name.clone()).collect();
    v.sort();
    v
}

fn select_host(app: &mut App, host: &str) -> ServerAddress {
    let a = app
        .servers()
        .iter()
        .find(|s| s.address.host == host)
        .unwrap()
        .address
        .clone();
    let _ = app.update(Message::Table(table::Msg::RowClicked(a.clone())));
    a
}

/// Pide refrescar el seleccionado y entrega lo que traiga la `Task`.
fn refresh_selected(app: &mut App) {
    let task = app.update(Message::RefreshSelected);
    for m in run_task(task) {
        let _ = app.update(m);
    }
}

fn query_msg(app: &App, result: Result<Option<ServerInfo>, String>) -> Message {
    Message::Query {
        request: app.scan.query_request(),
        game: "cod2",
        result,
    }
}

/// App con cod2 escaneado (lista completa de internet).
fn scanned(dir: &Path, servers: &[ServerInfo]) -> (TestApp, FakeScan) {
    let (mut app, fake) = fake_app(dir, Config::default());
    complete(&mut app, servers);
    (app, fake)
}

#[test]
fn window_lan_scan_does_not_replace_saved_list() {
    let dir = tempfile::tempdir().unwrap();
    let internet = [
        mk("8.8.8.8", "Internet", 5, 1, "1.3"),
        mk("8.8.4.4", "Internet 2", 5, 1, "1.3"),
    ];
    let (mut app, fake) = scanned(dir.path(), &internet);
    let _ = app.update(Message::ScanLan);
    assert_eq!(fake.lan_calls(), ["cod2"]);
    assert_eq!(fake.calls(), ["cod2"]); // sin masters
    assert!(app.is_scanning() && app.scan.lan());
    assert!(app.servers().is_empty());
    assert_eq!(
        app.status(),
        "Buscando servidores de Call of Duty 2 en la red local…"
    );
    complete(&mut app, &[mk("192.168.1.20", "Casa", 5, 1, "1.3")]);
    assert!(!app.is_scanning());
    assert_eq!(names(&app), ["Casa"]);
    // La caché y el disco siguen con la lista de internet.
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
    app.saved.flush();
    let index = server_cache::read_index(&dir.path().join("cache").join("servers"));
    assert_eq!(index.get("cod2").map(|e| e.0), Some(2));
    // F5 vuelve a escanear internet.
    let _ = app.update(Message::Refresh);
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
    assert!(!app.scan.lan());
}

#[test]
fn lan_scan_needs_no_masters_or_key() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::SelectGame("l4d")); // sin clave de Steam: no escanea
    assert_eq!(fake.calls(), ["cod2"]);
    let _ = app.update(Message::ScanLan);
    assert_eq!(fake.lan_calls(), ["l4d"]);
    assert!(app.is_scanning());
    // La clave que llega tarde no reemplaza la búsqueda en la red local.
    let _ = app.update(Message::SteamKey("K".into()));
    assert_eq!(fake.calls(), ["cod2"]);
}

#[test]
fn refreshing_a_lan_server_keeps_the_internet_list() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = scanned(dir.path(), &[mk("8.8.8.8", "Internet", 5, 1, "1.3")]);
    let _ = app.update(Message::ScanLan);
    complete(&mut app, &[mk("192.168.1.20", "Casa", 5, 1, "1.3")]);
    select_host(&mut app, "192.168.1.20");
    fake.answer(Ok(Some(mk(
        "192.168.1.20",
        "Casa (refrescado)",
        5,
        1,
        "1.3",
    ))));
    refresh_selected(&mut app);
    assert_eq!(names(&app), ["Casa (refrescado)"]);
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 1");
    let _ = app.update(Message::SelectGame("q3"));
    let _ = app.update(Message::SelectGame("cod2")); // vuelve la lista de internet guardada
    assert_eq!(names(&app), ["Internet"]);
    assert!(!app.scan.lan());
}

#[test]
fn refresh_selected_updates_row_and_card() {
    let dir = tempfile::tempdir().unwrap();
    let old = mk("1.1.1.1", "Viejo", 40, 3, "1.3");
    let (mut app, fake) = scanned(dir.path(), &[old.clone(), mk("2.2.2.2", "b", 50, 3, "1.3")]);
    select_host(&mut app, "1.1.1.1");
    let task = app.update(Message::RefreshSelected);
    assert_eq!(app.status(), "Consultando 1.1.1.1:28960…");
    fake.answer(Ok(Some(mk("1.1.1.1", "Nuevo", 7, 3, "1.3"))));
    for m in run_task(task) {
        let _ = app.update(m);
    }
    let row = app
        .servers()
        .iter()
        .find(|s| s.address == old.address)
        .unwrap();
    assert_eq!((row.ping_ms, row.name.as_str()), (7, "Nuevo"));
    assert_eq!(app.servers().len(), 2);
    assert_eq!(fake.queries(), [("cod2", old.address.clone())]);
    assert_eq!(app.table.selected.as_ref(), Some(&old.address));
    assert_eq!(app.status(), "Servidor actualizado: 1.1.1.1:28960");
}

#[test]
fn refresh_selected_no_answer_keeps_row() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = scanned(dir.path(), &[mk("1.1.1.1", "a", 42, 3, "1.3")]);
    select_host(&mut app, "1.1.1.1");
    refresh_selected(&mut app);
    assert_eq!(app.status(), "El servidor no respondió");
    assert_eq!(app.servers().len(), 1);
    assert_eq!(app.servers()[0].ping_ms, 42);
}

#[test]
fn refresh_selected_ignored_after_game_switch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = scanned(dir.path(), &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_host(&mut app, "1.1.1.1");
    let _ = app.update(Message::RefreshSelected);
    let late = query_msg(&app, Ok(Some(mk("1.1.1.1", "a", 7, 3, "1.3"))));
    let _ = app.update(Message::SelectGame("ut99"));
    let _ = app.update(late);
    // Con el pedido vigente pero de otro juego, tampoco.
    let current = query_msg(&app, Ok(None));
    let _ = app.update(current);
    assert!(app.servers().is_empty());
    assert_ne!(app.status(), "El servidor no respondió");
    assert!(!app.status().starts_with("Servidor actualizado"));
}

#[test]
fn refresh_selected_after_stopped_scan_keeps_full_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (
        mk("1.1.1.1", "a", 40, 3, "1.3"),
        mk("2.2.2.2", "b", 50, 3, "1.3"),
    );
    let (mut app, fake) = scanned(dir.path(), &[a.clone(), b]);
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
    let _ = app.update(Message::Refresh);
    let generation = app.scan.generation();
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Event(ScanEvent::Server(a.clone())),
    });
    let _ = app.update(Message::Stop);
    select_host(&mut app, "1.1.1.1");
    fake.answer(Ok(Some(a)));
    refresh_selected(&mut app);
    assert_eq!(app.status(), "Servidor actualizado: 1.1.1.1:28960");
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
}

#[test]
fn refresh_selected_after_full_scan_updates_cache() {
    let dir = tempfile::tempdir().unwrap();
    let a = mk("1.1.1.1", "a", 40, 3, "1.3");
    let (mut app, fake) = scanned(dir.path(), &[a, mk("2.2.2.2", "b", 50, 3, "1.3")]);
    select_host(&mut app, "1.1.1.1");
    fake.answer(Ok(Some(mk("1.1.1.1", "Nuevo", 40, 3, "1.3"))));
    refresh_selected(&mut app);
    let _ = app.update(Message::SelectGame("ut99"));
    let _ = app.update(Message::SelectGame("cod2"));
    assert_eq!(names(&app), ["Nuevo", "b"]);
}

#[test]
fn refresh_selected_error_reports_exception() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = scanned(dir.path(), &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_host(&mut app, "1.1.1.1");
    fake.answer(Err("explotó".into()));
    refresh_selected(&mut app);
    assert_eq!(app.status(), "Error al consultar: explotó");
    assert_eq!(app.servers().len(), 1);
}

#[test]
fn stop_action_cancels_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    assert!(app.is_scanning());
    let _ = app.update(Message::Stop);
    assert!(!app.is_scanning());
    assert!(app.scan.stopped());
}

#[test]
fn scan_state_stop_drops_old_generation() {
    let mut s = ScanState::default();
    assert!(!s.stop()); // sin escaneo, nada que detener
    assert_eq!(s.generation(), 0);
    let _task = s.begin(futures::stream::empty().boxed(), true);
    assert!(s.is_scanning() && s.lan() && !s.stopped());
    assert_eq!(s.generation(), 1);
    assert!(s.stop());
    assert!(!s.is_scanning() && s.stopped());
    assert_eq!(s.generation(), 2); // lo que llegue con la 1 ya es viejo
    // Y en la app: lo de la generación detenida no entra.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let old = app.scan.generation();
    let _ = app.update(Message::Stop);
    let _ = app.update(Message::Scan {
        generation: old,
        msg: ScanMsg::Event(ScanEvent::Server(mk("1.1.1.1", "a", 40, 3, "1.3"))),
    });
    assert!(app.servers().is_empty());
}

#[test]
fn lan_bind_error_finishes_with_status() {
    with_language("es", || {
        let msgs: Vec<ScanMsg> = block_on(
            scan_stream_with(|mut emit| async move {
                lan_finish(Err(io::Error::other("puerto ocupado")), &mut emit)
            })
            .collect(),
        );
        assert_eq!(
            msgs,
            [
                ScanMsg::Event(ScanEvent::Status(
                    "Error en el escaneo: puerto ocupado".into()
                )),
                ScanMsg::Finished(ScanResult::default()),
            ]
        );
    });
}

#[test]
fn query_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = scanned(dir.path(), &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_host(&mut app, "1.1.1.1");
    let task = app.update(Message::RefreshSelected);
    assert!(fake.queries().is_empty());
    let _ = run_task(task);
    assert_eq!(fake.queries().len(), 1);
}

#[test]
fn f5_cancels_pending_query() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = scanned(dir.path(), &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_host(&mut app, "1.1.1.1");
    let _ = app.update(Message::RefreshSelected);
    let late = query_msg(&app, Ok(Some(mk("1.1.1.1", "a", 7, 3, "1.3"))));
    let _ = app.update(Message::Refresh);
    let _ = app.update(late);
    assert!(app.servers().is_empty());
    assert!(!app.status().starts_with("Servidor actualizado"));
}

#[test]
fn lan_error_keeps_status_and_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = scanned(dir.path(), &[mk("8.8.8.8", "Internet", 5, 1, "1.3")]);
    let _ = app.update(Message::ScanLan);
    let generation = app.scan.generation();
    let error = "Error en el escaneo: puerto ocupado".to_owned();
    for msg in [
        ScanMsg::Event(ScanEvent::Status(error.clone())),
        ScanMsg::Finished(ScanResult::default()),
    ] {
        let _ = app.update(Message::Scan { generation, msg });
    }
    assert!(!app.is_scanning());
    assert_eq!(app.status(), error);
    assert!(app.servers().is_empty());
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 1");
    let _ = app.update(Message::SelectGame("ut99"));
    let _ = app.update(Message::SelectGame("cod2"));
    assert_eq!(names(&app), ["Internet"]);
}
