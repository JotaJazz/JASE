//! Listas guardadas entre sesiones (`tests/test_saved_lists.py`): al abrir, cada juego muestra
//! su último escaneo completo leído del disco en un hilo, hasta que se refresca.

mod support;

use std::path::{Path, PathBuf};

use iced::Task;
use jase::app::{App, Message};
use jase::saved_lists::{SavedLists, short_datetime};
use jase::scan::ScanMsg;
use jase_core::config::Config;
use jase_core::models::ServerInfo;
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use jase_core::server_cache;
use support::lang::with_language;
use support::{fake_app, fake_app_task, mk, run_task};

fn saved_dir(dir: &Path) -> PathBuf {
    dir.join("cache").join("servers")
}

fn s(host: &str) -> ServerInfo {
    mk(host, host, 40, 3, "1.3")
}

fn finish(app: &mut App, servers: &[ServerInfo], ok: bool) {
    let generation = app.scan.generation();
    for info in servers {
        let _ = app.update(Message::Scan {
            generation,
            msg: ScanMsg::Event(ScanEvent::Server(info.clone())),
        });
    }
    let result = ScanResult {
        masters_ok: usize::from(ok),
        servers_listed: servers.len(),
        servers_responded: servers.len(),
        errors: vec![],
    };
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Finished(result),
    });
}

/// Primera sesión: escanea cod2 y espera a que la lista quede en disco.
fn scanned(dir: &Path, servers: &[ServerInfo]) {
    let (mut app, _) = fake_app(dir, Config::default());
    finish(&mut app, servers, true);
    app.saved.flush();
    assert!(server_cache::read_index(&saved_dir(dir)).contains_key("cod2"));
}

/// Corre la `Task` del arranque y entrega a la app las listas leídas del disco.
fn deliver_saved(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        if matches!(m, Message::SavedList { .. }) {
            let _ = app.update(m);
        }
    }
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

#[test]
fn next_session_shows_saved_list_without_scanning() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        scanned(dir.path(), &[s("1.1.1.1"), s("2.2.2.2")]);
        let (mut app, fake, task) = fake_app_task(dir.path(), Config::default());
        assert!(fake.calls().is_empty()); // no se escaneó al abrir
        assert_eq!(
            app.status(),
            "Cargando la lista guardada de Call of Duty 2…"
        );
        deliver_saved(&mut app, task);
        assert_eq!(hosts(&app), ["1.1.1.1", "2.2.2.2"]);
        assert!(fake.calls().is_empty());
        assert!(
            app.status().contains("lista guardada del"),
            "{}",
            app.status()
        );
        assert!(app.status().ends_with("; F5 para refrescar"));
        let _ = app.update(Message::Refresh); // F5: la nueva lista reemplaza a la guardada
        assert_eq!(fake.calls(), ["cod2"]);
        finish(&mut app, &[s("3.3.3.3")], true);
        assert_eq!(hosts(&app), ["3.3.3.3"]);
    });
}

#[test]
fn corrupt_saved_list_falls_back_to_scan() {
    let dir = tempfile::tempdir().unwrap();
    scanned(dir.path(), &[s("1.1.1.1")]);
    std::fs::write(saved_dir(dir.path()).join("cod2.json"), b"{roto").unwrap();
    let (mut app, fake, task) = fake_app_task(dir.path(), Config::default());
    assert!(fake.calls().is_empty());
    deliver_saved(&mut app, task);
    assert_eq!(fake.calls(), ["cod2"]);
    assert!(app.is_scanning());
    finish(&mut app, &[s("3.3.3.3")], true);
    assert_eq!(hosts(&app), ["3.3.3.3"]);
}

#[test]
fn failed_scan_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    finish(&mut app, &[s("1.1.1.1")], false);
    app.saved.flush();
    assert!(!saved_dir(dir.path()).join("cod2.json").exists());
}

#[test]
fn sidebar_counts_saved_lists_of_other_games() {
    let dir = tempfile::tempdir().unwrap();
    scanned(dir.path(), &[s("1.1.1.1"), s("2.2.2.2")]);
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::SelectGame("ut99"));
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 2");
}

#[test]
fn failed_f5_is_not_replaced_by_late_saved_list() {
    with_language("es", || {
        let dir = tempfile::tempdir().unwrap();
        scanned(dir.path(), &[s("1.1.1.1")]);
        let (mut app, _, task) = fake_app_task(dir.path(), Config::default());
        // La carga del disco queda en camino; F5 antes de que llegue y el master falla.
        let _ = app.update(Message::Refresh);
        finish(&mut app, &[], false);
        deliver_saved(&mut app, task);
        assert!(app.servers().is_empty());
        assert!(!app.status().contains("lista guardada del"));
    });
}

#[test]
fn writer_keeps_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut saved = SavedLists::open(dir.path());
    // La primera, grande, tarda más en escribirse: aun así queda la segunda.
    let big: Vec<ServerInfo> = (0..3000)
        .map(|i| s(&format!("10.0.{}.{}", i / 250, i % 250)))
        .collect();
    saved.save("cod2", &big, 1.0);
    saved.save("cod2", &[s("2.2.2.2")], 2.0);
    saved.flush();
    let (at, servers) = server_cache::load(&saved_dir(dir.path()), "cod2").unwrap();
    assert_eq!(at, 2.0);
    assert_eq!(servers.len(), 1);
    assert_eq!(
        server_cache::read_index(&saved_dir(dir.path())).get("cod2"),
        Some(&(1, 2.0))
    );
}

#[test]
fn short_datetime_uses_offset() {
    assert_eq!(short_datetime(0.0, 0), "1970-01-01 00:00");
    // 2000-02-29 01:02:03 UTC (bisiesto de siglo).
    assert_eq!(short_datetime(951_786_123.9, 0), "2000-02-29 01:02");
    assert_eq!(short_datetime(1_735_689_599.0, 0), "2024-12-31 23:59");
    // Una hora más: corre la hora y cruza el día (y el año); una menos, hacia atrás.
    assert_eq!(short_datetime(1_735_689_599.0, 3600), "2025-01-01 00:59");
    assert_eq!(short_datetime(951_786_123.9, -3 * 3600), "2000-02-28 22:02");
}
