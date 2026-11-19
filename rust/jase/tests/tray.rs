//! La bandeja del lado de la app (`tests/test_tray.py`): minimizar a la bandeja, los globos y
//! sus clics, la pregunta que espera a la ventana y el cierre. Con una bandeja falsa: nunca un
//! icono ni una notificación reales.

mod support;

use std::path::Path;
use std::sync::atomic::Ordering;

use iced::Task;
use iced::window::Id;
use jase::actions::{self, Action};
use jase::app::{App, AutoMsg, Click, Dialog, LinkMsg, Message, PromptMsg, SettingsMsg, TrayMsg};
use jase::scan::ScanMsg;
use jase::tray::{TrayCommand, TrayEvent};
use jase_core::config::Config;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{
    FAKE_TRAY_CALLS, FakeScan, FakeTray, TestApp, fake_app, mk, run_task, tray_app, tray_handle,
    window_actions,
};

type Actions = Vec<(&'static str, Option<Id>)>;

fn addr(host: &str) -> ServerAddress {
    ServerAddress::new(host, 28960)
}

fn srv(host: &str, players: i32) -> ServerInfo {
    mk(host, &format!("srv {host}"), 40, players, "1.3")
}

fn cfg(tray: bool) -> Config {
    let mut cfg = Config {
        minimize_to_tray: tray,
        ..Config::default()
    };
    cfg.game_paths.insert("cod2".into(), "C:/cod2".into());
    cfg
}

/// cod2 con 1.1.1.1 lleno (20/20) y 2.2.2.2 con sitio, ya escaneados.
fn app(dir: &Path, tray: bool) -> (TestApp, FakeScan) {
    let (mut app, fake) = tray_app(dir, cfg(tray));
    let generation = app.scan.generation();
    for s in [srv("1.1.1.1", 20), srv("2.2.2.2", 3)] {
        let _ = app.update(Message::Scan {
            generation,
            msg: ScanMsg::Event(ScanEvent::Server(s)),
        });
    }
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Finished(ScanResult {
            masters_ok: 1,
            ..ScanResult::default()
        }),
    });
    (app, fake)
}

fn tray(app: &mut App, m: TrayMsg) -> Task<Message> {
    app.update(Message::Tray(m))
}

/// El icono aparece (el `Ready` del backend).
fn ready(app: &mut App, messages: bool) -> FakeTray {
    let t = tray_handle(messages);
    let _ = tray(app, TrayMsg::Event(TrayEvent::Ready(t.handle.clone())));
    t
}

/// Minimizar: pregunta si está minimizada y, si lo está, la oculta.
fn minimize(app: &mut App) -> Actions {
    let main = Some(app.main_window());
    assert_eq!(
        window_actions(tray(app, TrayMsg::CheckMinimized)),
        [("minimized", main)]
    );
    window_actions(tray(app, TrayMsg::Minimized(Some(true))))
}

fn hide(app: &mut App) {
    let main = Some(app.main_window());
    assert_eq!(minimize(app), [("hidden", main)]);
    assert!(app.in_tray());
}

/// Corre la `Task` y lo que traiga.
fn drive(app: &mut App, task: Task<Message>) {
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        for m in run_task(task) {
            pending.push(app.update(m));
        }
    }
}

fn notice(title: &str, text: &str) -> TrayCommand {
    TrayCommand::Notify {
        title: title.into(),
        text: text.into(),
    }
}

fn restored(actions: &Actions, main: Id) -> bool {
    actions.contains(&("windowed", Some(main))) && actions.contains(&("focus", Some(main)))
}

/// La vigilancia de 1.1.1.1 encuentra sitio (como `_on_slot_free`).
fn slot_free(app: &mut App, fake: &FakeScan) {
    let _ = app.update(Message::Table(jase::table::Msg::RowClicked(addr(
        "1.1.1.1",
    ))));
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::WatchSlot)));
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 19)));
    for m in run_task(app.update(Message::Watch(jase::app::WatchMsg::SlotTick))) {
        let _ = app.update(m);
    }
}

const SLOT_TEXT: &str = "Hay sitio en srv 1.1.1.1 (19/20). ¿Conectar ahora?";

#[test]
fn no_tray_without_option() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!tray_app(dir.path(), cfg(false)).0.want_tray());
    assert!(!fake_app(dir.path(), cfg(true)).0.want_tray());
    assert!(tray_app(dir.path(), cfg(true)).0.want_tray());
}

#[test]
fn minimize_hides_window_with_tray() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    hide(&mut app);
    let main = app.main_window();
    let actions = window_actions(tray(&mut app, TrayMsg::Event(TrayEvent::Show)));
    assert!(restored(&actions, main), "{actions:?}");
    assert!(!app.in_tray());
}

#[test]
fn minimize_without_option_keeps_window() {
    let dir = tempfile::tempdir().unwrap();
    for on in [false, true] {
        // Sin icono (casilla apagada, o aún sin `Ready`) no se pregunta ni se oculta.
        let (mut app, _) = app(dir.path(), on);
        assert!(window_actions(tray(&mut app, TrayMsg::CheckMinimized)).is_empty());
        assert!(window_actions(tray(&mut app, TrayMsg::Minimized(Some(true)))).is_empty());
        assert!(!app.in_tray());
    }
}

#[test]
fn minimized_unknown_never_hides() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    // Wayland no dice si está minimizada: nunca se oculta (no habría cómo saberlo).
    assert!(window_actions(tray(&mut app, TrayMsg::Minimized(None))).is_empty());
    assert!(window_actions(tray(&mut app, TrayMsg::Minimized(Some(false)))).is_empty());
    assert!(!app.in_tray());
}

#[test]
fn notify_without_tray_uses_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), false);
    let main = app.main_window();
    let task = app.notify("Hola", "mundo", Some(Click::Restore));
    assert_eq!(app.status(), "Hola: mundo");
    assert!(window_actions(task).contains(&("attention", Some(main))));
    // Con icono pero visible, igual.
    let (mut app, _) = self::app(dir.path(), true);
    let mut t = ready(&mut app, true);
    let _ = app.notify("Hola", "mundo", None);
    assert_eq!(app.status(), "Hola: mundo");
    assert!(t.commands().is_empty());
}

#[test]
fn message_click_calls_callback() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let mut t = ready(&mut app, true);
    hide(&mut app);
    let main = app.main_window();
    let _ = app.notify("T", "x", Some(Click::Reveal("cod2", addr("2.2.2.2"))));
    assert_eq!(t.commands(), [notice("T", "x")]);
    assert_eq!(app.table.selected, None);
    let task = tray(&mut app, TrayMsg::Event(TrayEvent::MessageClicked));
    assert_eq!(app.table.selected, Some(addr("2.2.2.2")));
    assert!(restored(&window_actions(task), main));
    // Un solo uso.
    let _ = app.update(Message::Table(jase::table::Msg::RowClicked(addr(
        "1.1.1.1",
    ))));
    let task = tray(&mut app, TrayMsg::Event(TrayEvent::MessageClicked));
    assert!(!window_actions(task).contains(&("windowed", Some(main))));
    assert_eq!(app.table.selected, Some(addr("1.1.1.1")));
}

/// Abre Ajustes, apaga la casilla y acepta.
fn tray_option_off(app: &mut App) -> Task<Message> {
    let _ = app.update(Message::Settings(SettingsMsg::Open));
    let _ = app.update(Message::Settings(SettingsMsg::Tray(false)));
    app.update(Message::Settings(SettingsMsg::Accept))
}

#[test]
fn apply_tray_toggles_at_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), false);
    assert!(!app.want_tray());
    let _ = app.update(Message::Settings(SettingsMsg::Open));
    let _ = app.update(Message::Settings(SettingsMsg::Tray(true)));
    let _ = app.update(Message::Settings(SettingsMsg::Accept));
    assert!(app.want_tray());
    let t = ready(&mut app, true);
    let _ = tray_option_off(&mut app);
    assert!(!app.want_tray());
    assert_eq!(t.shutdowns.load(Ordering::SeqCst), 1);
    // Ya sin icono, minimizar no oculta.
    assert!(window_actions(tray(&mut app, TrayMsg::CheckMinimized)).is_empty());
}

#[test]
fn ready_after_option_off_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _ = tray_option_off(&mut app);
    // El icono que se estaba creando llega tarde: se quita y no se usa.
    let t = ready(&mut app, true);
    assert_eq!(t.shutdowns.load(Ordering::SeqCst), 1);
    assert!(window_actions(tray(&mut app, TrayMsg::CheckMinimized)).is_empty());
    assert!(!app.in_tray());
}

#[test]
fn option_off_restores_hidden_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    hide(&mut app);
    let main = app.main_window();
    let actions = window_actions(tray_option_off(&mut app));
    assert!(restored(&actions, main), "{actions:?}");
    assert!(!app.in_tray());
}

#[test]
fn tray_gone_restores_hidden_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    hide(&mut app);
    let main = app.main_window();
    // El icono desaparece (se cayó el panel): sin él, la ventana oculta no tendría salida.
    let actions = window_actions(tray(&mut app, TrayMsg::Gone));
    assert!(restored(&actions, main), "{actions:?}");
    assert!(!app.in_tray());
    assert!(window_actions(tray(&mut app, TrayMsg::CheckMinimized)).is_empty());
}

#[test]
fn close_shuts_down_tray() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path(), true);
    let t = ready(&mut app, true);
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(t.shutdowns.load(Ordering::SeqCst), 1);
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
}

#[test]
fn tray_quit_closes_hidden_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path(), true);
    let t = ready(&mut app, true);
    hide(&mut app);
    let main = Some(app.main_window());
    let task = tray(&mut app, TrayMsg::Event(TrayEvent::Quit));
    assert_eq!(window_actions(task), [("maximized", main)]);
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
    assert_eq!(t.shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn file_menu_quit_quits_application() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path(), true);
    let t = ready(&mut app, true);
    let main = Some(app.main_window());
    let task = app.update(Message::Actions(actions::Msg::Run(Action::Quit)));
    assert_eq!(window_actions(task), [("maximized", main)]);
    let _ = app.update(Message::Close { maximized: true });
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
    assert_eq!(t.shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn restore_never_unmaximizes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    let main = app.main_window();
    // Visible (restore_keeps_maximized_when_visible): ni se muestra ni se maximiza.
    let actions = window_actions(tray(&mut app, TrayMsg::Event(TrayEvent::Show)));
    assert!(actions.contains(&("focus", Some(main))), "{actions:?}");
    assert!(
        !actions
            .iter()
            .any(|(a, _)| *a == "maximize" || *a == "windowed")
    );
    // Desde la bandeja (restore_keeps_maximized_from_tray): se muestra sin tocar el maximizado.
    hide(&mut app);
    let actions = window_actions(tray(&mut app, TrayMsg::Event(TrayEvent::Show)));
    assert!(restored(&actions, main), "{actions:?}");
    assert!(
        !actions.iter().any(|(a, _)| *a == "maximize"),
        "{actions:?}"
    );
}

#[test]
fn slot_free_in_tray_notifies_then_asks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path(), true);
    let mut t = ready(&mut app, true);
    hide(&mut app);
    slot_free(&mut app, &fake);
    assert_eq!(t.commands(), [notice("Hay sitio", SLOT_TEXT)]);
    assert_eq!(app.dialog(), None);
    let task = tray(&mut app, TrayMsg::Event(TrayEvent::MessageClicked));
    assert!(restored(&window_actions(task), app.main_window()));
    assert_eq!(
        app.dialog(),
        Some(&Dialog::SlotFree {
            text: SLOT_TEXT.into()
        })
    );
}

#[test]
fn notify_hidden_without_balloons_restores_and_calls() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let mut t = ready(&mut app, false);
    hide(&mut app);
    let main = app.main_window();
    let task = app.notify("T", "x", Some(Click::Reveal("cod2", addr("2.2.2.2"))));
    assert!(restored(&window_actions(task), main));
    assert!(!app.in_tray());
    assert_eq!(app.table.selected, Some(addr("2.2.2.2")));
    hide(&mut app);
    let _ = app.notify("T", "y", None);
    assert_eq!(app.status(), "T: y");
    assert!(t.commands().is_empty());
}

#[test]
fn friend_notice_does_not_drop_slot_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path(), true);
    let mut t = ready(&mut app, true);
    hide(&mut app);
    slot_free(&mut app, &fake);
    let friend = Some(Click::Reveal("cod2", addr("2.2.2.2")));
    let _ = app.notify("Amigo en línea", "pepe está en srv", friend);
    assert_eq!(t.commands().len(), 2);
    assert_eq!(app.dialog(), None);
    // P. ej. clic en el icono: el globo no se tocó.
    let _ = tray(&mut app, TrayMsg::Event(TrayEvent::Show));
    assert!(matches!(app.dialog(), Some(Dialog::SlotFree { .. })));
    assert_eq!(app.status(), "Amigo en línea: pepe está en srv");
    // El globo del amigo ya no vale.
    let _ = tray(&mut app, TrayMsg::Event(TrayEvent::MessageClicked));
    assert_eq!(app.table.selected, Some(addr("1.1.1.1")));
    // La pregunta se hace una sola vez.
    let _ = app.update(Message::Prompt(PromptMsg::Cancel));
    hide(&mut app);
    let _ = tray(&mut app, TrayMsg::Event(TrayEvent::Show));
    assert_eq!(app.dialog(), None);
}

#[test]
fn restore_shows_last_notice_in_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    hide(&mut app);
    let _ = app.notify("A", "uno", None);
    let _ = app.notify("B", "dos", None);
    let _ = tray(&mut app, TrayMsg::Event(TrayEvent::Show));
    assert_eq!(app.status(), "B: dos");
}

#[test]
fn auto_refresh_skips_in_tray() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    hide(&mut app);
    let scans = fake.calls().len();
    let task = app.update(Message::AutoRefresh(AutoMsg::Run {
        minimized: Some(false),
    }));
    assert_eq!(task.units(), 0);
    assert_eq!(fake.calls().len(), scans);
    assert_eq!(app.servers().len(), 2);
}

#[test]
fn show_from_second_instance_restores() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(dir.path(), true);
    let _t = ready(&mut app, true);
    hide(&mut app);
    let main = app.main_window();
    let task = app.update(Message::Link(LinkMsg::Received("show".into())));
    assert!(restored(&window_actions(task), main));
    assert!(!app.in_tray());
}

#[test]
fn tray_never_starts_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path(), true);
    let t = ready(&mut app, true);
    hide(&mut app);
    slot_free(&mut app, &fake);
    let _ = app.notify("A", "uno", None);
    let _ = app.subscription();
    drive(
        &mut app,
        Task::done(Message::Tray(TrayMsg::Event(TrayEvent::Show))),
    );
    let _ = tray_option_off(&mut app);
    assert_eq!(FAKE_TRAY_CALLS.load(Ordering::SeqCst), 0);
    drop(t);
}

#[test]
fn pure_helpers() {
    use jase::tray::{escape_markup, rgba_to_argb, wide_truncated};
    assert_eq!(escape_markup("a<b>&c"), "a&lt;b&gt;&amp;c");
    assert_eq!(escape_markup("ñandú"), "ñandú");
    let w = wide_truncated(&"😀".repeat(300), 63);
    assert_eq!(w.len(), 63); // 62 unidades (31 pares) y el 0
    assert_eq!(String::from_utf16(&w[..62]).unwrap(), "😀".repeat(31));
    assert_eq!(
        wide_truncated("ñandú", 63),
        "ñandú\0".encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(rgba_to_argb(&[1, 2, 3, 4]), [4, 1, 2, 3]);
    let side = 32;
    let rgba = jase::icons::app_icon_rgba(side).unwrap();
    assert_eq!(rgba.len(), (side * side * 4) as usize);
}
