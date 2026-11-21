//! Aviso de versión nueva y actualización desde la app (`tests/test_app_update.py`,
//! `test_app_update_install.py` y `test_portable_update.py`). Todo es falso: `update_fetch`
//! responde de una cola, `update_stream` lee de memoria y `spawn` solo anota; nunca se baja
//! nada de Internet ni se lanza un proceso.

mod support;

use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use iced::Task;
use jase::actions::{self, Action, Menu, entries};
use jase::app::{
    App, BackgroundMsg, Dialog, Message, PromptMsg, SettingsMsg, UPDATE_POLL, UpdateChoice,
};
use jase::menu::Entry;
use jase::scan::StreamFn;
use jase::startup::update_version;
use jase::theme::VARIANTS;
use jase::update::{BAD_ZIP, SelfUpdate, download_error_text, download_update};
use jase_core::app_update::{RELEASE_PAGE_PREFIX, Release, newest_release};
use jase_core::config::Config;
use jase_core::portable_update::SCRIPT_NAME;
use serde_json::{Value, json};
use support::lang::with_language;
use support::{FakeFetch, FakeScan, TestApp, run_task, update_app, view_texts, window_requests};

const PAGE: &str = RELEASE_PAGE_PREFIX;
const DL: &str = "https://github.com/JotaJazz/JASE/releases/download/v99.0.0/JASE-99.0.0-setup.exe";
const ZIP_URL: &str =
    "https://github.com/JotaJazz/JASE/releases/download/v99.0.0/JASE-99.0.0-win64.zip";
const SETUP: &str = "JASE-99.0.0-setup.exe";
const ZIP: &str = "JASE-99.0.0-win64.zip";

fn body() -> Vec<u8> {
    b"installer-bytes".repeat(5000)
}

fn sha(data: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn rel(tag: &str) -> Value {
    json!({"tag_name": tag, "draft": false, "prerelease": false, "html_url": format!("{PAGE}{tag}")})
}

fn with_asset(name: &str, url: &str, data: &[u8]) -> Value {
    let mut r = rel("v99.0.0");
    r["assets"] = json!([{
        "name": name,
        "size": data.len(),
        "digest": format!("sha256:{}", sha(data)),
        "browser_download_url": url,
    }]);
    r
}

fn installer_release() -> Value {
    with_asset(SETUP, DL, &body())
}

fn payload(r: Value) -> Vec<u8> {
    serde_json::to_vec(&json!([r])).unwrap()
}

fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(*name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn good_zip() -> Vec<u8> {
    make_zip(&[("JASE/JASE.exe", b"exe"), ("JASE/_internal/x.dll", b"dll")])
}

fn bg(app: &mut App, m: BackgroundMsg) -> Task<Message> {
    app.update(Message::Background(m))
}

/// Lo que pasó al correr las `Task`: las peticiones a la ventana (cerrar) y lo demás.
#[derive(Default)]
struct Driven {
    window: usize,
    progress: Vec<u8>,
}

/// Corre la `Task` como el runtime de Iced y entrega a la app lo de fondo (también lo que
/// piden sus respuestas); el cierre de la ventana solo se cuenta.
fn drive(app: &mut App, task: Task<Message>) -> Driven {
    let mut out = Driven::default();
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        for m in run_task(task) {
            match m {
                Message::Background(BackgroundMsg::Launched { .. }) => {
                    let next = app.update(m);
                    out.window += window_requests(next);
                }
                Message::Background(b) => {
                    if let BackgroundMsg::Progress { percent, .. } = b {
                        out.progress.push(percent);
                    }
                    pending.push(bg(app, b));
                }
                _ => {}
            }
        }
    }
    out
}

/// App con la release `r` ya avisada en la barra (la consulta del arranque corrió).
fn app_with_release(dir: &Path, r: Value, how: SelfUpdate) -> (TestApp, FakeScan) {
    let data = payload(r);
    let fetch = FakeFetch::new(&[Some(&data)]);
    let (mut app, fake, task) = update_app(dir, Config::default(), Some(&fetch), how);
    drive(&mut app, task);
    assert!(app.update_link_text().is_some());
    (app, fake)
}

fn opened(fake: &FakeScan) -> Vec<String> {
    fake.opened
        .lock()
        .unwrap()
        .iter()
        .map(|(u, _)| u.clone())
        .collect()
}

fn spawned(fake: &FakeScan) -> Vec<(std::path::PathBuf, Vec<String>)> {
    let calls = fake.spawned.lock().unwrap();
    calls
        .iter()
        .map(|(p, a, _)| (p.clone(), a.clone()))
        .collect()
}

fn set_body(fake: &FakeScan, data: Vec<u8>) {
    *fake.stream_body.lock().unwrap() = Some(data);
}

/// Clic en el enlace, *Descargar e instalar* y la descarga hasta el final.
fn install(app: &mut App) -> Driven {
    let link = bg(app, BackgroundMsg::UpdateLink);
    assert!(iced_runtime::task::into_stream(link).is_none());
    assert_eq!(
        app.dialog(),
        Some(&Dialog::UpdateAsk {
            version: "99.0.0".into()
        })
    );
    let task = bg(app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    assert!(app.downloading());
    drive(app, task)
}

fn failed_text(app: &App) -> String {
    match app.dialog() {
        Some(Dialog::UpdateFailed { text }) => text.clone(),
        d => panic!("esperaba el aviso de la actualización, no {d:?}"),
    }
}

fn manual(app: &mut App) -> Task<Message> {
    app.update(Message::Actions(actions::Msg::Run(Action::CheckUpdates)))
}

#[test]
fn window_shows_update_link() {
    let dir = tempfile::tempdir().unwrap();
    let (app, fake) = app_with_release(dir.path(), rel("v99.0.0"), SelfUpdate::Unsupported);
    let release = app.update_release().unwrap();
    assert_eq!(release.version, "99.0.0");
    assert_eq!(release.url, format!("{PAGE}v99.0.0"));
    assert_eq!(
        app.update_link_text().as_deref(),
        Some("Nueva versión 99.0.0 disponible")
    );
    // La automática solo avisa en la barra: nada se abre solo.
    assert_eq!(app.dialog(), None);
    assert!(opened(&fake).is_empty());
}

#[test]
fn window_label_in_english() {
    with_language("en", || {
        let dir = tempfile::tempdir().unwrap();
        let (app, _) = app_with_release(dir.path(), rel("v99.0.0"), SelfUpdate::Unsupported);
        assert_eq!(
            app.update_link_text().as_deref(),
            Some("New version 99.0.0 available")
        );
    });
}

#[test]
fn window_hidden_by_default_and_no_network_without_fetch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, task) =
        update_app(dir.path(), Config::default(), None, SelfUpdate::Unsupported);
    drive(&mut app, task);
    assert_eq!(app.update_link_text(), None);
    let status = app.status().to_owned();
    assert!(iced_runtime::task::into_stream(app.check_app_update(false)).is_none());
    assert!(iced_runtime::task::into_stream(app.check_app_update(true)).is_none());
    assert_eq!(app.status(), status); // la manual sin red no dice nada
}

#[test]
fn window_respects_config_switch() {
    let dir = tempfile::tempdir().unwrap();
    let fetch = FakeFetch::new(&[Some(b"[]")]);
    let cfg = Config {
        check_app_updates: false,
        ..Config::default()
    };
    let (mut app, _, task) = update_app(dir.path(), cfg, Some(&fetch), SelfUpdate::Unsupported);
    drive(&mut app, task);
    assert!(iced_runtime::task::into_stream(app.check_app_update(false)).is_none());
    assert!(fetch.calls().is_empty());
    assert_eq!(app.update_link_text(), None);
}

#[test]
fn update_ignores_foreign_release_url() {
    let dir = tempfile::tempdir().unwrap();
    let fetch = FakeFetch::new(&[]);
    let (mut app, _, _startup) = update_app(
        dir.path(),
        Config::default(),
        Some(&fetch),
        SelfUpdate::Unsupported,
    );
    assert!(app.update_busy());
    let evil = Release {
        version: "99.0.0".into(),
        url: "https://evil.example/".into(),
        installer: None,
        portable: None,
    };
    let request = app.update_request();
    let _ = bg(
        &mut app,
        BackgroundMsg::Update {
            request,
            manual: false,
            result: Some((Some(evil), true)),
        },
    );
    assert_eq!(app.update_release(), None);
    assert_eq!(app.update_link_text(), None);
    assert!(!app.update_busy());
}

fn valid_release() -> Release {
    newest_release(&payload(rel("v99.0.0"))).unwrap()
}

#[test]
fn stale_result_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let fetch = FakeFetch::new(&[]);
    let (mut app, _, _startup) = update_app(
        dir.path(),
        Config::default(),
        Some(&fetch),
        SelfUpdate::Unsupported,
    );
    let request = app.update_request() + 7;
    let _ = bg(
        &mut app,
        BackgroundMsg::Update {
            request,
            manual: false,
            result: Some((Some(valid_release()), true)),
        },
    );
    assert_eq!(app.update_link_text(), None);
    assert!(app.update_busy()); // la del arranque sigue en curso
}

#[test]
fn periodic_tick_checks_without_duplicating() {
    assert_eq!(UPDATE_POLL, std::time::Duration::from_secs(3600));
    let dir = tempfile::tempdir().unwrap();
    let fetch = FakeFetch::new(&[Some(b"[]")]);
    let (mut app, _, startup) = update_app(
        dir.path(),
        Config::default(),
        Some(&fetch),
        SelfUpdate::Unsupported,
    );
    let first = app.update_request();
    // Con la del arranque en curso, el tic no pide otra.
    let tick = bg(&mut app, BackgroundMsg::CheckUpdates);
    assert!(iced_runtime::task::into_stream(tick).is_none());
    assert_eq!(app.update_request(), first);
    drive(&mut app, startup);
    assert!(!app.update_busy());
    assert_eq!(fetch.calls().len(), 1);
    let tick = bg(&mut app, BackgroundMsg::CheckUpdates);
    assert_eq!(app.update_request(), first + 1);
    let again = bg(&mut app, BackgroundMsg::CheckUpdates);
    assert!(iced_runtime::task::into_stream(again).is_none());
    assert_eq!(app.update_request(), first + 1);
    // Consultada hace instantes: `should_check` dice que aún no (sin red).
    drive(&mut app, tick);
    assert_eq!(fetch.calls().len(), 1);
    assert!(!app.update_busy());
}

#[test]
fn manual_check_reports_up_to_date_even_when_switched_off() {
    let dir = tempfile::tempdir().unwrap();
    let data = payload(rel("v0.0.1"));
    let fetch = FakeFetch::new(&[Some(&data)]);
    let cfg = Config {
        check_app_updates: false,
        ..Config::default()
    };
    let (mut app, _, _) = update_app(dir.path(), cfg, Some(&fetch), SelfUpdate::Unsupported);
    let help: Vec<(String, bool)> = entries(&app, Menu::Help)
        .iter()
        .filter_map(|e| match e {
            Entry::Item { label, on, .. } => Some((label.clone(), on.is_some())),
            _ => None,
        })
        .collect();
    assert_eq!(
        help,
        [
            "Ayuda de JASE",
            "Licencia",
            "Reportar un problema",
            "Sitio web",
            "Buscar actualizaciones…",
            "Acerca de"
        ]
        .map(|l| (l.to_owned(), true))
    );
    let task = manual(&mut app);
    assert_eq!(app.status(), "Buscando actualizaciones…");
    drive(&mut app, task);
    assert_eq!(app.status(), "");
    match app.dialog() {
        Some(Dialog::Message { title, text, .. }) => {
            assert_eq!(title, "Buscar actualizaciones");
            assert_eq!(text, "Tienes la última versión (1.3.0).");
        }
        d => panic!("esperaba el aviso, no {d:?}"),
    }
    assert_eq!(app.update_link_text(), None);
}

#[test]
fn manual_check_failure_and_new_version() {
    let dir = tempfile::tempdir().unwrap();
    let data = payload(rel("v99.0.0"));
    let fetch = FakeFetch::new(&[None, Some(&data)]);
    let (mut app, fake, _) = update_app(
        dir.path(),
        Config::default(),
        Some(&fetch),
        SelfUpdate::Unsupported,
    );
    let task = manual(&mut app);
    drive(&mut app, task);
    match app.dialog() {
        Some(Dialog::Message { text, .. }) => assert!(text.contains("GitHub"), "{text}"),
        d => panic!("esperaba el aviso, no {d:?}"),
    }
    let _ = app.update(Message::Prompt(PromptMsg::Cancel));
    // Ignora el plazo que dejó la consulta fallida y la ofrece enseguida.
    let task = manual(&mut app);
    drive(&mut app, task);
    assert!(app.update_link_text().is_some());
    assert_eq!(opened(&fake), [format!("{PAGE}v99.0.0")]);
    assert_eq!(fetch.calls().len(), 2);
}

#[test]
fn manual_check_offers_install_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let data = payload(installer_release());
    let fetch = FakeFetch::new(&[Some(&data)]);
    let (mut app, fake, _) = update_app(
        dir.path(),
        Config::default(),
        Some(&fetch),
        SelfUpdate::Installer,
    );
    let task = manual(&mut app);
    drive(&mut app, task);
    assert_eq!(
        app.dialog(),
        Some(&Dialog::UpdateAsk {
            version: "99.0.0".into()
        })
    );
    assert!(opened(&fake).is_empty());
}

#[test]
fn manual_check_replaces_automatic() {
    let dir = tempfile::tempdir().unwrap();
    let data = payload(rel("v99.0.0"));
    let fetch = FakeFetch::new(&[Some(&data)]);
    let (mut app, fake, _startup) = update_app(
        dir.path(),
        Config::default(),
        Some(&fetch),
        SelfUpdate::Unsupported,
    );
    let automatic = app.update_request();
    let task = manual(&mut app);
    assert_eq!(app.update_request(), automatic + 1);
    // La automática reemplazada llega tarde: se descarta.
    let _ = bg(
        &mut app,
        BackgroundMsg::Update {
            request: automatic,
            manual: false,
            result: Some((Some(valid_release()), true)),
        },
    );
    assert_eq!(app.update_link_text(), None);
    assert_eq!(app.status(), "Buscando actualizaciones…");
    drive(&mut app, task);
    assert!(app.update_link_text().is_some());
    assert_eq!(opened(&fake), [format!("{PAGE}v99.0.0")]);
}

#[test]
fn click_off_windows_opens_page() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) =
        app_with_release(dir.path(), installer_release(), SelfUpdate::Unsupported);
    let task = bg(&mut app, BackgroundMsg::UpdateLink);
    assert_eq!(app.dialog(), None);
    drive(&mut app, task);
    assert_eq!(opened(&fake), [format!("{PAGE}v99.0.0")]);
    assert_eq!(app.dialog(), None);
}

#[test]
fn click_windows_without_installer_opens_page() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), rel("v99.0.0"), SelfUpdate::Installer);
    let task = bg(&mut app, BackgroundMsg::UpdateLink);
    drive(&mut app, task);
    assert_eq!(opened(&fake), [format!("{PAGE}v99.0.0")]);
    assert_eq!(app.dialog(), None);
}

#[test]
fn click_windows_page_choice() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let task = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Page));
    assert_eq!(app.dialog(), None);
    drive(&mut app, task);
    assert_eq!(opened(&fake), [format!("{PAGE}v99.0.0")]);
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let task = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Cancel));
    drive(&mut app, task);
    assert_eq!(opened(&fake).len(), 1);
    assert_eq!(app.dialog(), None);
    assert!(!app.downloading());
}

#[test]
fn click_windows_install_runs_installer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    set_body(&fake, body());
    let driven = install(&mut app);
    let expected = dir.path().join("cache").join("updates").join(SETUP);
    assert_eq!(spawned(&fake), [(expected.clone(), Vec::<String>::new())]);
    assert_eq!(std::fs::read(&expected).unwrap(), body());
    // Lanzado fuera del hilo de la UI; la ventana se cierra por el camino normal.
    let thread = fake.spawned.lock().unwrap()[0].2;
    assert_ne!(thread, std::thread::current().id());
    assert!(driven.window > 0);
    assert_eq!(driven.progress.last(), Some(&100));
    assert!(opened(&fake).is_empty());
    assert_eq!(fake.streams.lock().unwrap()[0].0, DL);
}

#[test]
fn install_error_offers_page() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    set_body(&fake, vec![b'x'; body().len()]);
    let driven = install(&mut app);
    assert!(spawned(&fake).is_empty());
    assert_eq!(driven.window, 0);
    assert_eq!(
        failed_text(&app),
        "No se pudo descargar la actualización: la suma SHA-256 no coincide"
    );
    let updates = dir.path().join("cache").join("updates");
    assert_eq!(std::fs::read_dir(&updates).unwrap().count(), 0); // ni el `.part`
    assert!(!app.downloading());
    let task = bg(&mut app, BackgroundMsg::OpenPage);
    assert_eq!(app.dialog(), None);
    drive(&mut app, task);
    assert_eq!(opened(&fake), [format!("{PAGE}v99.0.0")]);
}

#[test]
fn install_hash_error_is_translated() {
    with_language("en", || {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake) =
            app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
        set_body(&fake, vec![b'x'; body().len()]);
        install(&mut app);
        assert_eq!(
            failed_text(&app),
            "The update could not be downloaded: the SHA-256 checksum does not match"
        );
    });
}

#[test]
fn installer_launch_failure_has_own_message() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    set_body(&fake, body());
    *fake.spawn_fails.lock().unwrap() = true;
    let driven = install(&mut app);
    assert_eq!(spawned(&fake).len(), 1);
    assert_eq!(failed_text(&app), "No se pudo ejecutar el instalador.");
    assert_eq!(driven.window, 0); // la app sigue abierta
    assert!(!app.downloading());
}

#[test]
fn download_clears_previous_updates() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("updates");
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("viejo.zip"), b"viejo").unwrap();
    let asset = newest_release(&payload(installer_release()))
        .unwrap()
        .installer
        .unwrap();
    let fake = FakeScan::default();
    set_body(&fake, body());
    let stream = support::fake_deps(&fake, "").update_stream;
    let path = download_update(&asset, &dest, None, &stream, &mut |_, _| {}, &|| false);
    assert_eq!(path.unwrap(), dest.join(SETUP));
    assert!(!dest.join("viejo.zip").exists());
    assert!(dest.join(SETUP).exists());
}

/// Un cuerpo que pone `eof` al terminar: la cancelación llega justo entre la descarga y la
/// extracción.
struct CancelAtEnd {
    data: Cursor<Vec<u8>>,
    eof: Arc<AtomicBool>,
}

impl Read for CancelAtEnd {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.data.read(buf)?;
        if n == 0 {
            self.eof.store(true, Ordering::SeqCst);
        }
        Ok(n)
    }
}

#[test]
fn cancel_after_download_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let good = good_zip();
    let asset = newest_release(&payload(with_asset(ZIP, ZIP_URL, &good)))
        .unwrap()
        .portable
        .unwrap();
    for extract in [true, false] {
        let eof = Arc::new(AtomicBool::new(false));
        let (data, flag) = (good.clone(), eof.clone());
        let stream: StreamFn = Arc::new(move |_| {
            let body = CancelAtEnd {
                data: Cursor::new(data.clone()),
                eof: flag.clone(),
            };
            Ok(Box::new(body) as Box<dyn Read + Send>)
        });
        let dest = dir.path().join("updates");
        let to = dest.join(format!("portable-{ZIP}"));
        let cancelled = || eof.load(Ordering::SeqCst);
        let result = download_update(
            &asset,
            &dest,
            extract.then_some(to.as_path()),
            &stream,
            &mut |_, _| {},
            &cancelled,
        );
        assert_eq!(result, Err("cancelled".to_owned()), "extract={extract}");
        assert!(!to.exists());
    }
}

/// Un cuerpo que se detiene tras el primer trozo hasta que el test lo suelta, y avisa al
/// llegar ahí.
struct Gated {
    data: Cursor<Vec<u8>>,
    reached: Option<mpsc::Sender<()>>,
    gate: mpsc::Receiver<()>,
}

impl Read for Gated {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.data.position() > 0
            && let Some(reached) = self.reached.take()
        {
            let _ = reached.send(());
            let _ = self.gate.recv();
        }
        let n = buf.len().min(1000);
        self.data.read(&mut buf[..n])
    }
}

type Gates = (mpsc::Receiver<()>, mpsc::Sender<()>);

/// Un `StreamFn` de una sola descarga detenida a mitad.
fn gated_stream(data: Vec<u8>) -> (StreamFn, Gates) {
    let (reached_tx, reached_rx) = mpsc::channel();
    let (gate_tx, gate_rx) = mpsc::channel();
    let body = Mutex::new(Some(Gated {
        data: Cursor::new(data),
        reached: Some(reached_tx),
        gate: gate_rx,
    }));
    let stream: StreamFn = Arc::new(move |_| {
        let body = body.lock().unwrap().take().unwrap();
        Ok(Box::new(body) as Box<dyn Read + Send>)
    });
    (stream, (reached_rx, gate_tx))
}

/// Cancelar y volver a instalar enseguida: el hilo viejo, al limpiar, no toca la descarga nueva.
#[test]
fn cancelled_thread_does_not_break_the_next_download() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("updates");
    let asset = newest_release(&payload(installer_release()))
        .unwrap()
        .installer
        .unwrap();
    let (old_stream, (old_reached, old_gate)) = gated_stream(body());
    let (new_stream, (new_reached, new_gate)) = gated_stream(body());
    let cancel_old = Arc::new(AtomicBool::new(false));
    let old = {
        let (asset, dest, flag) = (asset.clone(), dest.clone(), cancel_old.clone());
        std::thread::spawn(move || {
            let cancelled = || flag.load(Ordering::SeqCst);
            download_update(&asset, &dest, None, &old_stream, &mut |_, _| {}, &cancelled)
        })
    };
    old_reached.recv().unwrap();
    cancel_old.store(true, Ordering::SeqCst);
    let new = {
        let (asset, dest) = (asset.clone(), dest.clone());
        std::thread::spawn(move || {
            download_update(&asset, &dest, None, &new_stream, &mut |_, _| {}, &|| false)
        })
    };
    new_reached.recv().unwrap();
    old_gate.send(()).unwrap();
    assert_eq!(old.join().unwrap(), Err("cancelled".to_owned()));
    new_gate.send(()).unwrap();
    assert_eq!(new.join().unwrap(), Ok(dest.join(SETUP)));
    assert_eq!(std::fs::read(dest.join(SETUP)).unwrap(), body());
    // Solo queda lo descargado: ni `.part` ni carpetas de trabajo.
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 1);
}

#[test]
fn replacing_the_download_dialog_cancels_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    set_body(&fake, body());
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let task = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    assert!(app.downloading());
    // Otro diálogo que abre sin pasar por el menú reemplaza al de la descarga.
    let _ = app.update(Message::Settings(SettingsMsg::Open));
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
    assert!(!app.downloading());
    drive(&mut app, task);
    assert!(spawned(&fake).is_empty());
}

#[test]
fn close_during_download_cancels_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    set_body(&fake, body());
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let _task = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    let flag = app.download_cancel_flag().unwrap();
    assert!(!flag.load(Ordering::SeqCst));
    let _ = app.update(Message::Close { maximized: false });
    assert!(flag.load(Ordering::SeqCst));
    assert!(!app.downloading());
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
}

#[test]
fn portable_launch_failure_has_own_message() {
    let dir = tempfile::tempdir().unwrap();
    let exe_dir = dir.path().join("portable");
    std::fs::create_dir(&exe_dir).unwrap();
    let good = good_zip();
    let (mut app, fake) = app_with_release(
        dir.path(),
        with_asset(ZIP, ZIP_URL, &good),
        SelfUpdate::Portable(exe_dir),
    );
    set_body(&fake, good);
    *fake.spawn_fails.lock().unwrap() = true;
    let driven = install(&mut app);
    assert_eq!(spawned(&fake).len(), 1);
    assert_eq!(
        failed_text(&app),
        "No se pudo reemplazar la carpeta de JASE; actualízala a mano."
    );
    assert_eq!(driven.window, 0);
    assert!(!app.downloading());
}

#[test]
fn cancelled_download_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    set_body(&fake, body());
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let task = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    assert_eq!(
        app.dialog(),
        Some(&Dialog::Download {
            name: SETUP.into(),
            percent: 0
        })
    );
    let _ = bg(&mut app, BackgroundMsg::CancelDownload);
    assert_eq!(app.dialog(), None);
    assert!(!app.downloading());
    // La descarga cancelada no entrega nada, y lo que llegue tarde no hace nada.
    drive(&mut app, task);
    let path = dir.path().join("cache").join("updates").join(SETUP);
    for request in 0..4 {
        let late = bg(
            &mut app,
            BackgroundMsg::Downloaded {
                request,
                result: Ok(path.clone()),
            },
        );
        drive(&mut app, late);
    }
    assert!(spawned(&fake).is_empty());
    assert_eq!(app.dialog(), None);
    // Esc también cancela.
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let _ = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    assert!(app.downloading());
    let _ = app.update(Message::Escape);
    assert!(!app.downloading());
    assert_eq!(app.dialog(), None);
}

#[test]
fn portable_click_downloads_extracts_and_runs_script() {
    let dir = tempfile::tempdir().unwrap();
    let exe_dir = dir.path().join("portable");
    std::fs::create_dir(&exe_dir).unwrap();
    let good = good_zip();
    let (mut app, fake) = app_with_release(
        dir.path(),
        with_asset(ZIP, ZIP_URL, &good),
        SelfUpdate::Portable(exe_dir.clone()),
    );
    set_body(&fake, good);
    let driven = install(&mut app);
    let calls = spawned(&fake);
    assert_eq!(calls.len(), 1);
    let (program, args) = &calls[0];
    assert_eq!(program, Path::new("cmd.exe"));
    assert_eq!(args[0], "/c");
    let script = Path::new(&args[1]);
    assert_eq!(script.file_name().unwrap(), SCRIPT_NAME);
    let text = std::fs::read_to_string(script).unwrap();
    let new_dir = dir
        .path()
        .join("cache")
        .join("updates")
        .join(format!("portable-{ZIP}"))
        .join("JASE");
    assert_eq!(std::fs::read(new_dir.join("JASE.exe")).unwrap(), b"exe");
    let copy = format!(
        r#"robocopy "{}" "{}""#,
        new_dir.display(),
        exe_dir.display()
    );
    assert!(text.contains(&copy), "{text}");
    assert!(driven.window > 0);
}

#[test]
fn bad_portable_zip_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let exe_dir = dir.path().join("portable");
    std::fs::create_dir(&exe_dir).unwrap();
    let bad = make_zip(&[("otro/x.txt", b"x")]);
    let (mut app, fake) = app_with_release(
        dir.path(),
        with_asset(ZIP, ZIP_URL, &bad),
        SelfUpdate::Portable(exe_dir),
    );
    set_body(&fake, bad);
    let driven = install(&mut app);
    assert!(spawned(&fake).is_empty());
    assert_eq!(driven.window, 0);
    let expected = format!(
        "No se pudo descargar la actualización: {}",
        download_error_text(BAD_ZIP)
    );
    assert_eq!(failed_text(&app), expected);
    assert!(expected.contains("zip"));
}

#[test]
fn progress_messages_are_capped() {
    let dir = tempfile::tempdir().unwrap();
    let big: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect();
    let (mut app, fake) = app_with_release(
        dir.path(),
        with_asset(SETUP, DL, &big),
        SelfUpdate::Installer,
    );
    set_body(&fake, big);
    *fake.stream_chunk.lock().unwrap() = 1000; // más de mil lecturas
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let task = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    let messages = run_task(task);
    let percents: Vec<u8> = messages
        .iter()
        .filter_map(|m| match m {
            Message::Background(BackgroundMsg::Progress { percent, .. }) => Some(*percent),
            _ => None,
        })
        .collect();
    assert!(percents.len() <= 101, "{}", percents.len());
    assert!(percents.len() > 10);
    assert!(percents.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(percents.last(), Some(&100));
    assert!(matches!(
        messages.last(),
        Some(Message::Background(BackgroundMsg::Downloaded {
            result: Ok(_),
            ..
        }))
    ));
}

#[cfg(not(windows))]
#[test]
fn self_update_detect_off_windows() {
    assert_eq!(SelfUpdate::detect(), SelfUpdate::Unsupported);
}

#[test]
fn update_version_is_crate_version() {
    assert_eq!(update_version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(update_version(), "2.1.0");
}

/// Release `tag` con su instalador y su zip portátil.
fn release_with_assets(tag: &str) -> Value {
    let ver = tag.trim_start_matches('v');
    let digest = format!("sha256:{}", sha(b"x"));
    let asset = |name: String| {
        let url = format!("https://github.com/JotaJazz/JASE/releases/download/{tag}/{name}");
        json!({"name": name, "size": 1, "digest": digest, "browser_download_url": url})
    };
    let mut r = rel(tag);
    r["assets"] = json!([
        asset(format!("JASE-{ver}-setup.exe")),
        asset(format!("JASE-{ver}-win64.zip")),
    ]);
    r
}

#[test]
fn installed_2_0_0_sees_only_newer_releases() {
    let dir = tempfile::tempdir().unwrap();
    let check = |list: Vec<Value>| {
        let data = serde_json::to_vec(&list).unwrap();
        jase_core::app_update::check(dir.path(), 0.0, "2.0.0", |_| Some(data))
    };
    let (v200, v130) = (release_with_assets("v2.0.0"), release_with_assets("v1.3.0"));
    assert!(check(vec![v200.clone(), v130.clone()]).is_none());

    let r = check(vec![
        release_with_assets("v2.0.1"),
        v200.clone(),
        v130.clone(),
    ])
    .unwrap();
    assert_eq!(r.version, "2.0.1");
    assert_eq!(r.installer.unwrap().name, "JASE-2.0.1-setup.exe");
    assert_eq!(r.portable.unwrap().name, "JASE-2.0.1-win64.zip");

    let mut draft = release_with_assets("v2.0.1");
    draft["draft"] = json!(true);
    assert!(check(vec![draft, v200, v130]).is_none());
}

#[test]
fn update_io_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let data = payload(installer_release());
    let fetch = FakeFetch::new(&[Some(&data)]);
    let (mut app, fake, _startup) = update_app(
        dir.path(),
        Config::default(),
        Some(&fetch),
        SelfUpdate::Installer,
    );
    set_body(&fake, body());
    let _manual = manual(&mut app);
    let request = app.update_request();
    let _ = bg(
        &mut app,
        BackgroundMsg::Update {
            request,
            manual: true,
            result: Some((newest_release(&data), true)),
        },
    );
    assert!(matches!(app.dialog(), Some(Dialog::UpdateAsk { .. })));
    let _page = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Page));
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let _download = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    let path = dir.path().join("cache").join("updates").join(SETUP);
    let _launch = bg(
        &mut app,
        BackgroundMsg::Downloaded {
            request: 1,
            result: Ok(path),
        },
    );
    assert!(fetch.calls().is_empty());
    assert!(fake.streams.lock().unwrap().is_empty());
    assert!(spawned(&fake).is_empty());
    assert!(opened(&fake).is_empty());
}

#[test]
fn update_dialogs_build_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_release(dir.path(), installer_release(), SelfUpdate::Installer);
    let _ = bg(&mut app, BackgroundMsg::UpdateLink);
    let themes = || VARIANTS.iter().map(|(id, _, _)| *id);
    let check = |app: &mut App, wanted: &str| {
        for id in themes() {
            app.theme_id = id;
            let texts = view_texts(app);
            assert!(texts.iter().any(|t| t.contains(wanted)), "{id}: {texts:?}");
        }
    };
    check(&mut app, "Hay una versión nueva de JASE (99.0.0).");
    let _ = bg(&mut app, BackgroundMsg::UpdateChoice(UpdateChoice::Install));
    check(&mut app, &format!("Descargando {SETUP}…"));
    let _ = bg(
        &mut app,
        BackgroundMsg::Progress {
            request: 1,
            percent: 57,
        },
    );
    assert_eq!(
        app.dialog(),
        Some(&Dialog::Download {
            name: SETUP.into(),
            percent: 57
        })
    );
    check(&mut app, "57%");
    let _ = bg(
        &mut app,
        BackgroundMsg::Downloaded {
            request: 1,
            result: Err("hash".into()),
        },
    );
    check(&mut app, "la suma SHA-256 no coincide");
    let _ = bg(&mut app, BackgroundMsg::OpenPage);
    // El enlace de la barra, sin diálogo.
    assert_eq!(app.dialog(), None);
    for id in themes() {
        app.theme_id = id;
        let _ = view_texts(&app);
    }
}
