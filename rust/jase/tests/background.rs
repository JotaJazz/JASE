//! Tareas de fondo de la ventana (`window_background.py`; tests de `test_masters_update.py`,
//! `test_masters_tab.py` y `test_main_window.py`): la lista oficial de masters (al arrancar,
//! como mucho cada 24 h, y a mano desde Ajustes) y la base de países. El `masters_fetch` es
//! falso: nunca se baja nada de Internet.

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};

use iced::Task;
use jase::app::{App, BackgroundMsg, MastersMsg, Message, SettingsMsg};
use jase_core::config::Config;
use jase_core::geo::CountryDB;
use jase_core::masters::{META_FILENAME, check_official, parse_official};
use support::lang::with_language;
use support::{FakeFetch, FakeScan, TestApp, fake_deps, fetch_app, mk, run_task};

fn doc(updated: &str, host: &str) -> Vec<u8> {
    format!(
        r#"{{"schema": 1, "updated": "{updated}", "games": {{"cod2": [{{"host": "{host}", "port": 20510}}]}}}}"#
    )
    .into_bytes()
}

fn new_doc() -> Vec<u8> {
    doc("2030-01-01", "new.example.com")
}

/// Corre la `Task` y entrega lo que produce, sin correr lo que piden esas respuestas (así el
/// plazo de la barra de estado, que con el reloj en pausa vence enseguida, no la borra).
fn deliver(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        let _ = app.update(m);
    }
}

fn settings(app: &mut App, m: SettingsMsg) -> Task<Message> {
    app.update(Message::Settings(m))
}

fn tab_status(app: &App) -> String {
    app.settings_form().unwrap().masters.update_status.clone()
}

/// Lo que dice el estado de la pestaña tras un *Buscar actualización* (Ajustes abierto).
fn forced_check(app: &mut App) -> Task<Message> {
    let open = settings(app, SettingsMsg::Open);
    deliver(app, open);
    settings(app, SettingsMsg::Masters(MastersMsg::CheckUpdate))
}

#[test]
fn window_applies_update_and_shows_status() {
    let dir = tempfile::tempdir().unwrap();
    let new = new_doc();
    let fetch = FakeFetch::new(&[Some(&new)]);
    let (mut app, _, task) = fetch_app(dir.path(), Config::default(), Some(&fetch));
    deliver(&mut app, task);
    let expected = vec![("new.example.com".to_owned(), 20510)];
    assert_eq!(app.official().games.get("cod2"), Some(&expected));
    assert_eq!(app.official().updated, "2030-01-01");
    assert_eq!(app.status(), "Lista de masters actualizada (2030-01-01)");
    assert!(!app.masters_busy());
    // Revisada hace instantes: no vuelve a bajarla.
    let again = app.check_masters_update(false);
    deliver(&mut app, again);
    assert_eq!(fetch.calls().len(), 1);
}

#[test]
fn stale_update_result_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, _) = fetch_app(dir.path(), Config::default(), None);
    let (before, status) = (app.official().clone(), app.status().to_owned());
    let list = parse_official(&new_doc()).unwrap();
    let request = app.masters_request() + 7;
    let _ = app.update(Message::Background(BackgroundMsg::Masters {
        request,
        force: false,
        result: Some((Some(list), true)),
    }));
    assert_eq!(*app.official(), before);
    assert_eq!(app.status(), status);
}

#[test]
fn masters_update_status_in_english() {
    with_language("en", || {
        let dir = tempfile::tempdir().unwrap();
        let new = new_doc();
        let fetch = FakeFetch::new(&[Some(&new)]);
        let (mut app, _, task) = fetch_app(dir.path(), Config::default(), Some(&fetch));
        deliver(&mut app, task);
        assert_eq!(app.status(), "Masters list updated (2030-01-01)");
    });
}

#[test]
fn no_check_without_fetch_or_before_24h() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut app, _, task) = fetch_app(dir.path(), Config::default(), None);
        deliver(&mut app, task);
        assert!(!app.masters_busy());
        assert_eq!(app.check_masters_update(false).units(), 0);
        assert!(!app.masters_busy());
    }
    // Revisada hace una hora (el reloj falso de `Deps.now`).
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    let hour_ago = FakeScan::default().clock.0.lock().unwrap().0 - 3600.0;
    let old = doc("2029-12-01", "old.example.com");
    let _ = check_official(&cache, hour_ago, |_| Some(old));
    let fetch = FakeFetch::new(&[Some(&new_doc())]);
    let (mut app, _, task) = fetch_app(dir.path(), Config::default(), Some(&fetch));
    deliver(&mut app, task);
    assert!(fetch.calls().is_empty());
    assert!(!app.masters_busy());
    assert_eq!(app.official().updated, "2029-12-01");
    assert_eq!(app.status(), "Consultando masters de Call of Duty 2…");
}

#[test]
fn forced_check_reports_outcome_in_tab() {
    let first = doc("2030-01-01", "new.example.com");
    let other = doc("2030-02-01", "other.example.com");
    let cases: [(&[u8], &str); 3] = [
        (&other, "Lista de masters actualizada (2030-02-01)"),
        (&first, "La lista de masters está al día"),
        (b"{roto", "No se pudo descargar la lista de masters"),
    ];
    for (second, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::new(&[Some(&first), Some(second)]);
        let (mut app, _, task) = fetch_app(dir.path(), Config::default(), Some(&fetch));
        deliver(&mut app, task); // la revisión del arranque
        assert!(!app.masters_busy());
        let task = forced_check(&mut app);
        assert_eq!(tab_status(&app), "Buscando actualización…");
        assert!(app.masters_busy());
        deliver(&mut app, task);
        assert_eq!(tab_status(&app), expected);
        assert_eq!(fetch.calls().len(), 2);
    }
}

#[test]
fn forced_check_without_network_reports_failure() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, _) = fetch_app(dir.path(), Config::default(), None);
    let task = forced_check(&mut app);
    assert_eq!(task.units(), 0);
    assert_eq!(tab_status(&app), "No se pudo descargar la lista de masters");
    // Sin `masters_fetch` el botón está apagado.
    assert!(!app.settings_form().unwrap().masters.has_check());
}

#[test]
fn automatic_check_only_reports_updated_to_open_editor() {
    let dir = tempfile::tempdir().unwrap();
    let fetch = FakeFetch::default();
    let (mut app, _, _startup) = fetch_app(dir.path(), Config::default(), Some(&fetch));
    let open = settings(&mut app, SettingsMsg::Open);
    deliver(&mut app, open);
    let list = parse_official(&new_doc()).unwrap();
    let cases = [
        (false, (None, false)),
        (false, (None, true)),
        (true, (None, false)),
        (true, (None, true)),
        (false, (Some(list), true)),
    ];
    let mut seen = Vec::new();
    for (force, result) in cases {
        // La revisión del arranque (o la de recién) sigue en curso: su respuesta llega a mano.
        if !app.masters_busy() {
            let _ = app.check_masters_update(force);
        }
        let request = app.masters_request();
        let _ = app.update(Message::Background(BackgroundMsg::Masters {
            request,
            force,
            result: Some(result),
        }));
        let status = tab_status(&app);
        if seen.last() != Some(&status) && !status.is_empty() {
            seen.push(status);
        }
    }
    assert_eq!(
        seen,
        [
            "No se pudo descargar la lista de masters",
            "La lista de masters está al día",
            "Lista de masters actualizada (2030-01-01)",
        ]
    );
    assert_eq!(
        app.settings_form().unwrap().masters.official.updated,
        "2030-01-01"
    );
}

#[test]
fn forced_check_after_settings_closed_goes_to_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let first = new_doc();
    let fetch = FakeFetch::new(&[Some(&first), Some(&first)]);
    let (mut app, _, task) = fetch_app(dir.path(), Config::default(), Some(&fetch));
    deliver(&mut app, task);
    let task = forced_check(&mut app);
    let _ = settings(&mut app, SettingsMsg::Cancel);
    deliver(&mut app, task);
    assert_eq!(app.status(), "La lista de masters está al día");
    // Sin red, con Ajustes cerrado, también a la barra.
    let fetch = FakeFetch::new(&[Some(&first), None]);
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, task) = fetch_app(dir.path(), Config::default(), Some(&fetch));
    deliver(&mut app, task);
    let task = forced_check(&mut app);
    let _ = settings(&mut app, SettingsMsg::Cancel);
    deliver(&mut app, task);
    assert_eq!(app.status(), "No se pudo descargar la lista de masters");
}

#[test]
fn background_io_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    let now = FakeScan::default().clock.0.lock().unwrap().0;
    // Revisada recién: si `should_check` corriera en `new`, no habría pedido.
    let old = doc("2029-12-01", "old.example.com");
    let _ = check_official(&cache, now, |_| Some(old));
    let fetch = FakeFetch::new(&[Some(&new_doc())]);
    let (mut app, _, task) = fetch_app(dir.path(), Config::default(), Some(&fetch));
    assert!(fetch.calls().is_empty());
    // Ahora toca revisar: lo decide la `Task`, al correr.
    std::fs::write(cache.join(META_FILENAME), "basura").unwrap();
    let _ = app.view();
    deliver(&mut app, task);
    let calls = fetch.calls();
    assert_eq!(calls.len(), 1);
    assert_ne!(calls[0].1, std::thread::current().id());
    assert_eq!(app.official().updated, "2030-01-01");
    // El forzado tampoco baja nada en `update`.
    let task = forced_check(&mut app);
    assert_eq!(fetch.calls().len(), 1);
    deliver(&mut app, task);
    assert_eq!(fetch.calls().len(), 2);
}

// ------------------------------------------------------------------- base de países

type GeoCalls = Arc<Mutex<Vec<bool>>>;

/// Base de países falsa: la primera carga da `1.1.1.1` → CL; las siguientes fallan.
fn geo_app(dir: &Path) -> (TestApp, GeoCalls) {
    let lang = support::lang::Spanish::hold();
    let csv = dir.join("fake.csv");
    std::fs::write(&csv, "1.1.1.1,1.1.1.1,CL\n").unwrap();
    let calls: GeoCalls = Arc::default();
    let seen = calls.clone();
    let fake = FakeScan::default();
    let mut deps = fake_deps(&fake, "");
    deps.country_db = Some(Arc::new(move |_: &Path, refresh| {
        let mut seen = seen.lock().unwrap();
        seen.push(refresh);
        if seen.len() == 1 {
            CountryDB::from_csv(&csv).map_err(|e| e.to_string())
        } else {
            Err("No se pudo descargar la base de países".to_owned())
        }
    }));
    let (app, task) = App::new(Config::default(), dir.join("config.json"), deps);
    drop(lang);
    let mut app = TestApp::new(app);
    deliver(&mut app, task);
    (app, calls)
}

#[test]
fn failed_country_refresh_keeps_db() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, calls) = geo_app(dir.path());
    let server = mk("1.1.1.1", "a", 40, 3, "1.3");
    app.table.model.add_or_update(server.clone());
    assert_eq!(app.table.model.country_code(&server), Some("cl"));
    let task = app.start_country_db(true).unwrap();
    deliver(&mut app, task);
    assert_eq!(*calls.lock().unwrap(), [false, true]);
    assert_eq!(app.table.model.country_code(&server), Some("cl"));
    assert!(!app.country_busy());
    assert_eq!(app.status(), "No se pudo descargar la base de países");
    assert_eq!(app.dialog(), None);
}

#[test]
fn country_refresh_refused_while_busy() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, calls) = geo_app(dir.path());
    let task = app.start_country_db(true);
    assert!(task.is_some() && app.country_busy());
    assert!(app.start_country_db(true).is_none());
    deliver(&mut app, task.unwrap());
    assert_eq!(calls.lock().unwrap().len(), 2);
    assert!(!app.country_busy());
}
