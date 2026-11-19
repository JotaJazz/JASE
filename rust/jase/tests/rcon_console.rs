//! Consola RCON (`tests/test_rcon.py`, la parte de la ventana): clave en el llavero, historial,
//! salida con tope y respuestas tardías. Ni ventanas reales, ni llavero real, ni RCON real.

mod support;

use std::path::Path;

use iced::keyboard::{Key, key::Named};
use iced::window;
use jase::actions::{Action, enabled};
use jase::app::{App, Focus, Message, RconMsg, Tool};
use jase::passwords::{Secret, rcon_user};
use jase::rcon_console::{Console, HISTORY_MAX, RCON_MAX_LINES, error_text};
use jase::scan::ScanMsg;
use jase::theme::VARIANTS;
use jase_core::config::Config;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::rcon::RconError;
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{FakeScan, TestApp, element_texts, fake_app, mk, run_task, tool_ids};

fn addr() -> ServerAddress {
    ServerAddress::new("1.1.1.1", 28960)
}

fn complete(app: &mut App, servers: &[ServerInfo]) {
    let generation = app.scan.generation();
    for s in servers {
        let _ = app.update(Message::Scan {
            generation,
            msg: ScanMsg::Event(ScanEvent::Server(s.clone())),
        });
    }
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Finished(ScanResult {
            masters_ok: 1,
            ..ScanResult::default()
        }),
    });
}

fn select(app: &mut App, host: &str) {
    let address = ServerAddress::new(host, 28960);
    let _ = app.update(Message::Table(jase::table::Msg::RowClicked(address)));
}

/// cod2 con «Clan Nox» (1.1.1.1) en línea y elegido.
fn cod2_app(dir: &Path) -> (TestApp, FakeScan) {
    let (mut app, fake) = fake_app(dir, Config::default());
    complete(
        &mut app,
        &[
            mk("1.1.1.1", "Clan Nox", 40, 3, "1.3"),
            mk("2.2.2.2", "Otro", 40, 3, "1.3"),
        ],
    );
    select(&mut app, "1.1.1.1");
    (app, fake)
}

/// Abre la consola del elegido y entrega la clave del llavero.
fn open(app: &mut App) -> window::Id {
    let task = app.update(Message::Rcon(RconMsg::Open));
    let (id, Tool::Rcon(_)) = app.tool_windows().last().cloned().unwrap() else {
        panic!("sin consola");
    };
    for m in run_task(task) {
        let _ = app.update(m);
    }
    id
}

fn send(app: &mut App, id: window::Id, command: &str) -> Vec<Message> {
    let _ = app.update(Message::Rcon(RconMsg::Command(id, command.to_owned())));
    run_task(app.update(Message::Rcon(RconMsg::Send(id))))
}

fn output(app: &App, id: window::Id) -> String {
    app.console(id).unwrap().output.text()
}

#[test]
fn console_sends_and_remembers_password() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let _ = app.update(Message::Shortcut(jase::bars::Shortcut::Find));
    assert_eq!(app.focus(), Focus::Filter);
    let task = app.update(Message::Rcon(RconMsg::Open));
    let id = tool_ids(&app)[0];
    assert_eq!(app.title(id), "Consola RCON: Clan Nox");
    assert!(!app.console(id).unwrap().loaded);
    let user = rcon_user("cod2", &addr());
    assert_eq!(user, "rcon:cod2:1.1.1.1:28960");
    let msgs = run_task(task);
    let calls = fake.keyring.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, user);
    assert!(matches!(
        msgs.as_slice(),
        [Message::Rcon(RconMsg::Saved { saved: None, .. })]
    ));
    for m in msgs {
        let _ = app.update(m);
    }
    // La consola enfoca su campo: el filtro de la principal suelta el caret.
    assert_eq!(app.focus(), Focus::Sidebar);
    let c = app.console(id).unwrap();
    assert!(c.loaded && !c.remember);
    let _ = app.update(Message::Rcon(RconMsg::Password(id, Secret("pw".into()))));
    let _ = app.update(Message::Rcon(RconMsg::Remember(id, true)));
    fake.rcon.answer(Ok("map mp_carentan\n".into()));
    let msgs = send(&mut app, id, "status");
    assert!(app.console(id).unwrap().busy);
    assert_eq!(
        fake.keyring.entries.lock().unwrap().get(&user).unwrap(),
        "pw"
    );
    let calls = fake.rcon.calls();
    assert_eq!(calls.len(), 1);
    let (game, address, pw, cmd, thread) = &calls[0];
    assert_eq!(
        (*game, address, pw.as_str(), cmd.as_str()),
        ("cod2", &addr(), "pw", "status")
    );
    assert_ne!(*thread, std::thread::current().id());
    for m in msgs {
        let _ = app.update(m);
    }
    assert!(
        output(&app, id).ends_with("> status\nmap mp_carentan"),
        "{:?}",
        output(&app, id)
    );
    let c = app.console(id).unwrap();
    assert!(!c.busy && c.command.is_empty());
}

#[test]
fn saved_password_fills_and_focuses_command() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let user = rcon_user("cod2", &addr());
    fake.keyring
        .entries
        .lock()
        .unwrap()
        .insert(user, "x".into());
    let id = open(&mut app);
    let c = app.console(id).unwrap();
    assert!(c.loaded && c.remember);
    assert_eq!(c.password.0, "x");
}

#[test]
fn error_texts() {
    let _lang = support::lang::Spanish::hold();
    let cases = [
        ("timeout", "El servidor no respondió al RCON."),
        ("bad_password", "Clave RCON incorrecta."),
        (
            "no_challenge",
            "El servidor no dio un challenge de RCON (¿tiene RCON activado?).",
        ),
        ("connect", "No se pudo conectar al RCON (TCP) del servidor."),
        ("invalid", "Respuesta RCON inválida."),
        ("closed", "Se cortó la conexión RCON."),
        ("unsupported", "Este juego no tiene RCON en JASE."),
    ];
    for (code, text) in cases {
        assert_eq!(error_text(code, ""), text);
    }
    assert_eq!(
        error_text("connect", "refused"),
        "No se pudo conectar al RCON (TCP) del servidor. (refused)"
    );
    assert_eq!(error_text("network", "x"), "Error de red: x");
    assert_eq!(error_text("network", ""), "Error de red: network");
}

#[test]
fn rcon_answer_is_verbatim_and_capped() {
    let _lang = support::lang::Spanish::hold();
    let mut c = Console::new("s".into(), "1.1.1.1:28960".into());
    c.on_saved(None);
    c.command = "a".into();
    assert!(c.send().is_some());
    c.show_answer("^1hola\r\n");
    let text = c.output.text();
    assert!(text.contains("> a\n^1hola"), "{text:?}");
    assert!(!text.ends_with('\n'), "{text:?}");
    assert!(!c.busy);
    c.show_answer("");
    assert!(c.output.text().ends_with("\n(sin respuesta de texto)"));
    c.show_error("bad_password", "");
    assert!(c.output.text().ends_with("\n! Clave RCON incorrecta."));
    let many: Vec<String> = (0..6000).map(|i| format!("l{i}")).collect();
    c.show_answer(&many.join("\n"));
    assert_eq!(c.output.line_count(), RCON_MAX_LINES);
    let text = c.output.text();
    assert!(text.starts_with("l1000\n") && text.ends_with("\nl5999"));
}

#[test]
fn history_arrows() {
    let mut c = Console::new("s".into(), "a".into());
    c.on_saved(None);
    for cmd in ["status", "map x", "map x", " kick y "] {
        c.command = cmd.into();
        assert!(c.send().is_some());
        c.show_answer("ok");
    }
    c.history_step(true);
    assert_eq!(c.command, "kick y");
    c.history_step(true);
    assert_eq!(c.command, "map x");
    c.history_step(true);
    assert_eq!(c.command, "status");
    c.history_step(true);
    assert_eq!(c.command, "status");
    c.history_step(false);
    c.history_step(false);
    assert_eq!(c.command, "kick y");
    c.history_step(false);
    assert_eq!(c.command, "");
    for i in 0..HISTORY_MAX + 5 {
        c.command = format!("c{i}");
        c.send();
        c.show_answer("ok");
    }
    for _ in 0..HISTORY_MAX + 5 {
        c.history_step(true);
    }
    assert_eq!(c.command, "c5");
}

#[test]
fn history_keys_from_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = cod2_app(dir.path());
    let id = open(&mut app);
    for m in send(&mut app, id, "status") {
        let _ = app.update(m);
    }
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::ArrowUp)));
    assert_eq!(app.console(id).unwrap().command, "status");
    let _ = app.update(Message::ToolKey(id, Key::Named(Named::ArrowDown)));
    assert_eq!(app.console(id).unwrap().command, "");
}

#[test]
fn send_ignored_while_busy_or_empty_or_loading() {
    let mut c = Console::new("s".into(), "a".into());
    c.command = "status".into();
    assert!(c.send().is_none(), "clave cargando");
    c.on_saved(Some(Secret("pw".into())));
    c.command = "   ".into();
    assert!(c.send().is_none(), "vacío");
    c.command = "status".into();
    let (pw, cmd, remember) = c.send().unwrap();
    assert_eq!(
        (pw.0.as_str(), cmd.as_str(), remember),
        ("pw", "status", true)
    );
    c.command = "otro".into();
    assert!(c.send().is_none(), "ocupado");
    assert_eq!(c.command, "otro");
    // En la app: sin tarea mientras espera.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let id = open(&mut app);
    let _ = send(&mut app, id, "status");
    assert!(send(&mut app, id, "otro").is_empty());
    assert_eq!(fake.rcon.calls().len(), 1);
}

#[test]
fn one_console_per_server() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let a = open(&mut app);
    let _ = run_task(app.update(Message::Rcon(RconMsg::Open)));
    assert_eq!(tool_ids(&app), [a]);
    assert_eq!(
        fake.keyring.calls().len(),
        1,
        "la abierta no relee el llavero"
    );
    select(&mut app, "2.2.2.2");
    let b = open(&mut app);
    assert_ne!(a, b);
    assert_eq!(tool_ids(&app), [a, b]);
    assert_eq!(app.title(b), "Consola RCON: Otro");
}

#[test]
fn can_rcon_needs_supported_online_server() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    assert!(!app.can_rcon());
    let mut off = mk("3.3.3.3", "caído", 40, 0, "1.3");
    off.online = false;
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3"), off]);
    select(&mut app, "3.3.3.3");
    assert!(!app.can_rcon(), "desconectado");
    assert!(!enabled(&app, Action::Rcon));
    select(&mut app, "1.1.1.1");
    assert!(app.can_rcon());
    assert!(enabled(&app, Action::Rcon));
    let task = app.update(Message::Shortcut(jase::bars::Shortcut::Rcon));
    assert_eq!(support::window_actions(task)[0].0, "open");
    for (game, yes) in [("ut99", false), ("l4d2", true)] {
        let _ = app.update(Message::SelectGame(game));
        complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
        select(&mut app, "1.1.1.1");
        assert_eq!(app.can_rcon(), yes, "{game}");
    }
    // Sin consola si no se puede.
    let _ = app.update(Message::SelectGame("ut99"));
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select(&mut app, "1.1.1.1");
    let _ = app.update(Message::Rcon(RconMsg::Open));
    assert_eq!(tool_ids(&app).len(), 1, "solo la del atajo");
}

#[test]
fn late_rcon_answer_after_close_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = cod2_app(dir.path());
    let task = app.update(Message::Rcon(RconMsg::Open));
    let id = tool_ids(&app)[0];
    let saved = run_task(task);
    let _ = app.update(Message::WindowClosed(id));
    for m in saved {
        let _ = app.update(m); // la clave llega a una consola cerrada
    }
    assert!(app.console(id).is_none());
    let _ = app.update(Message::Rcon(RconMsg::Answered {
        window: id,
        request: 1,
        result: Ok("x".into()),
    }));
    assert!(app.console(id).is_none() && tool_ids(&app).is_empty());
}

#[test]
fn reopened_console_ignores_old_answer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let old = open(&mut app);
    fake.rcon.answer(Ok("vieja".into()));
    let late = send(&mut app, old, "status");
    let _ = app.update(Message::WindowClosed(old));
    let new = open(&mut app);
    assert_ne!(old, new);
    for m in late {
        let _ = app.update(m);
    }
    let c = app.console(new).unwrap();
    assert!(!c.output.text().contains("vieja"));
    // Y en la misma consola, la de otro pedido tampoco.
    let _ = send(&mut app, new, "status");
    let _ = app.update(Message::Rcon(RconMsg::Answered {
        window: new,
        request: 0,
        result: Ok("otra".into()),
    }));
    let c = app.console(new).unwrap();
    assert!(c.busy && !c.output.text().contains("otra"));
}

#[test]
fn rcon_error_shows_code_and_detail() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let id = open(&mut app);
    fake.rcon.answer(Err(RconError::Connect("refused".into())));
    for m in send(&mut app, id, "status") {
        let _ = app.update(m);
    }
    assert!(
        output(&app, id).ends_with("! No se pudo conectar al RCON (TCP) del servidor. (refused)")
    );
    assert!(!app.console(id).unwrap().busy);
}

#[test]
fn rcon_password_never_leaks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let id = open(&mut app);
    let _ = app.update(Message::Rcon(RconMsg::Password(id, Secret("pw".into()))));
    let _ = app.update(Message::Rcon(RconMsg::Remember(id, true)));
    fake.rcon.answer(Err(RconError::BadPassword));
    for m in send(&mut app, id, "status") {
        assert!(!format!("{m:?}").contains("pw"), "{m:?}");
        let _ = app.update(m);
    }
    assert!(!app.status().contains("pw"));
    let _ = app.update(Message::Close { maximized: false });
    let cfg = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(!cfg.contains("\"pw\""), "{cfg}");
    let m = RconMsg::Password(id, Secret("pw".into()));
    assert!(!format!("{m:?}").contains("pw"));
    assert_eq!(format!("{:?}", Secret("pw".into())), "***");
}

#[test]
fn rcon_remember_off_deletes_saved() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let user = rcon_user("cod2", &addr());
    fake.keyring
        .entries
        .lock()
        .unwrap()
        .insert(user.clone(), "vieja".into());
    let id = open(&mut app);
    let _ = app.update(Message::Rcon(RconMsg::Remember(id, false)));
    let _ = send(&mut app, id, "status");
    assert!(!fake.keyring.entries.lock().unwrap().contains_key(&user));
    assert_eq!(fake.rcon.calls()[0].2, "vieja");
}

#[test]
fn rcon_keyring_error_still_sends() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    *fake.keyring.set_error.lock().unwrap() = Some("sin llavero".into());
    let id = open(&mut app);
    let _ = app.update(Message::Rcon(RconMsg::Password(id, Secret("pw".into()))));
    let _ = app.update(Message::Rcon(RconMsg::Remember(id, true)));
    fake.rcon.answer(Ok("ok".into()));
    for m in send(&mut app, id, "status") {
        let _ = app.update(m);
    }
    assert_eq!(fake.rcon.calls().len(), 1);
    assert!(output(&app, id).ends_with("\nok"));
    assert!(!app.status().contains("sin llavero"));
}

#[test]
fn rcon_keyring_runs_in_task() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    let before = fake.keyring.calls().len();
    let open = app.update(Message::Rcon(RconMsg::Open));
    assert_eq!(fake.keyring.calls().len(), before);
    let id = tool_ids(&app)[0];
    for m in run_task(open) {
        let _ = app.update(m);
    }
    let _ = app.update(Message::Rcon(RconMsg::Command(id, "status".into())));
    let task = app.update(Message::Rcon(RconMsg::Send(id)));
    assert_eq!(fake.keyring.calls().len(), before + 1);
    assert!(fake.rcon.calls().is_empty());
    let _ = run_task(task);
    assert_eq!(fake.keyring.calls().len(), before + 2);
    assert_eq!(fake.rcon.calls().len(), 1);
}

#[test]
fn rcon_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path());
    // Con la clave cargando.
    let _ = app.update(Message::Rcon(RconMsg::Open));
    let loading = tool_ids(&app)[0];
    select(&mut app, "2.2.2.2");
    let id = open(&mut app);
    fake.rcon.answer(Ok("map mp_carentan".into()));
    for m in send(&mut app, id, "status") {
        let _ = app.update(m);
    }
    for (theme, _, _) in VARIANTS.iter() {
        app.theme_id = theme;
        let texts = element_texts(app.view_window(id));
        for t in [
            "Servidor:",
            "Clave RCON:",
            "Recordar la clave",
            "Enviar",
            "2.2.2.2:28960",
        ] {
            assert!(texts.iter().any(|x| x == t), "{theme} {t}: {texts:?}");
        }
        let texts = element_texts(app.view_window(loading));
        assert!(texts.iter().any(|x| x == "1.1.1.1:28960"), "{theme}");
    }
}
