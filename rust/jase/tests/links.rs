//! Enlaces `jase://` en la ventana (`tests/test_window_links.py`): consultar, confirmar en
//! texto plano y conectar; uno a la vez, el último espera, y nada debajo de otro diálogo.

mod support;

use std::path::Path;

use iced::Task;
use iced::keyboard::{Key, key::Named};
use jase::actions::{self, Action};
use jase::app::{App, Dialog, LinkMsg, Message};
use jase::theme::VARIANTS;
use jase_core::config::Config;
use jase_core::models::{ServerAddress, ServerInfo};
use support::{FakeScan, TestApp, fake_app, fake_deps, run_task, window_requests};

const URL: &str = "jase://connect/cod2/9.9.9.9:28960";

fn addr() -> ServerAddress {
    ServerAddress::new("9.9.9.9", 28960)
}

fn server(a: &ServerAddress, name: &str) -> ServerInfo {
    let mut info = ServerInfo::new(a.clone(), a.clone());
    info.name = name.to_owned();
    info.version = "1.3".into();
    info.max_players = 8;
    info
}

/// La app con ruta para cod2 (si no, conectar pide la ruta en vez de lanzar).
fn app(dir: &Path) -> (TestApp, FakeScan) {
    let mut cfg = Config::default();
    cfg.game_paths.insert("cod2".into(), "C:/cod2".into());
    fake_app(dir, cfg)
}

fn link(app: &mut App, m: LinkMsg) -> Task<Message> {
    app.update(Message::Link(m))
}

fn open(url: &str, interactive: bool) -> LinkMsg {
    LinkMsg::Open {
        url: url.to_owned(),
        interactive,
    }
}

/// Corre la `Task` y entrega a la app las respuestas de las consultas y de conectar (como el
/// runtime de Iced); devuelve lo demás.
fn drive(app: &mut App, task: Task<Message>) -> Vec<Message> {
    let (mut pending, mut rest) = (vec![task], Vec::new());
    while let Some(task) = pending.pop() {
        for m in run_task(task) {
            match m {
                Message::Link(LinkMsg::Found { .. }) | Message::Connect(_) => {
                    pending.push(app.update(m));
                }
                m => rest.push(m),
            }
        }
    }
    rest
}

fn found(messages: Vec<Message>) -> Vec<Message> {
    messages
        .into_iter()
        .filter(|m| matches!(m, Message::Link(LinkMsg::Found { .. })))
        .collect()
}

fn retries(messages: &[Message]) -> usize {
    messages
        .iter()
        .filter(|m| matches!(m, Message::Link(LinkMsg::Retry)))
        .count()
}

fn link_text(app: &App) -> Option<String> {
    match app.dialog() {
        Some(Dialog::Link { text }) => Some(text.clone()),
        _ => None,
    }
}

fn launched(fake: &FakeScan) -> Vec<ServerAddress> {
    fake.launches().into_iter().map(|a| a.address).collect()
}

fn queried(fake: &FakeScan) -> Vec<ServerAddress> {
    fake.queries().into_iter().map(|(_, a)| a).collect()
}

fn about(app: &mut App) {
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::About)));
    assert_eq!(app.dialog(), Some(&Dialog::About));
}

#[test]
fn invalid_link_warns_and_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let task = link(&mut app, open("jase://connect/cod2/;rm -rf:1", true));
    drive(&mut app, task);
    match app.dialog() {
        Some(Dialog::Message { title, text, .. }) => {
            assert_eq!(title, "Enlace jase://");
            assert_eq!(text, "El enlace no es válido.");
        }
        d => panic!("esperaba el aviso, no {d:?}"),
    }
    assert!(fake.queries().is_empty() && fake.launches().is_empty());
    assert!(!app.link_busy());
}

#[test]
fn socket_invalid_link_is_status_message() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let task = link(&mut app, LinkMsg::Received("jase://nope".into()));
    drive(&mut app, task);
    assert_eq!(app.dialog(), None);
    assert_eq!(app.status(), "El enlace no es válido.");
    assert!(fake.queries().is_empty());
}

#[test]
fn link_connects_only_after_confirm() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let _ = app.update(Message::SelectGame("ut99"));
    fake.answer_for(&addr(), Some(server(&addr(), "Mi servidor")));
    let task = link(&mut app, open(URL, true));
    assert!(app.link_busy());
    assert_eq!(app.status(), "Consultando 9.9.9.9:28960…");
    drive(&mut app, task);
    assert_eq!(app.current_game().map(|g| g.id), Some("cod2"));
    assert_eq!(app.table.selected, Some(addr()));
    assert!(link_text(&app).is_some());
    assert!(fake.launches().is_empty());
    let task = link(&mut app, LinkMsg::Confirm(true));
    drive(&mut app, task);
    assert_eq!(launched(&fake), [addr()]);
    assert_eq!(app.dialog(), None);
    assert!(!app.link_busy());
}

#[test]
fn link_cancel_does_not_connect() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr(), Some(server(&addr(), "Mi servidor")));
    let cancels: [fn(&mut App) -> Task<Message>; 3] = [
        |app| link(app, LinkMsg::Confirm(false)),
        // Enter con el foco en el diálogo: *Cancelar*, el botón por defecto.
        |app| {
            let enter = app.key_message(Key::Named(Named::Enter)).unwrap();
            app.update(enter)
        },
        |app| app.update(Message::Escape),
    ];
    for cancel in cancels {
        let task = link(&mut app, open(URL, true));
        drive(&mut app, task);
        assert!(link_text(&app).is_some());
        let task = cancel(&mut app);
        drive(&mut app, task);
        assert_eq!(app.dialog(), None);
        assert!(!app.link_busy());
    }
    assert!(fake.launches().is_empty());
}

#[test]
fn link_no_answer_shows_message() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    assert_eq!(app.status(), "El servidor no respondió");
    assert!(app.dialog().is_none() && !app.link_busy());
    fake.answer(Err("x".into()));
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    assert_eq!(app.status(), "Error al consultar: x");
    assert!(app.dialog().is_none() && !app.link_busy());
    assert!(fake.launches().is_empty());
}

#[test]
fn confirmation_is_plain_text_with_address() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr(), Some(server(&addr(), "^1<b>Oficial</b>")));
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    let text = link_text(&app).unwrap();
    assert!(
        text.contains("<b>Oficial</b>") && !text.contains("^1"),
        "{text}"
    );
    assert!(text.contains("9.9.9.9:28960") && text.contains("Call of Duty 2"));
    // Sin nombre (o solo colores), la dirección.
    let _ = link(&mut app, LinkMsg::Confirm(false));
    fake.answer_for(&addr(), Some(server(&addr(), "^1^7")));
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    assert!(
        link_text(&app)
            .unwrap()
            .starts_with("¿Conectar a 9.9.9.9:28960?")
    );
}

#[test]
fn second_link_waits_for_first_dialog_then_latest_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let (other, third) = (
        ServerAddress::new("8.8.8.8", 1),
        ServerAddress::new("7.7.7.7", 1),
    );
    for a in [addr(), other.clone(), third.clone()] {
        fake.answer_for(&a, Some(server(&a, &format!("s{}", a.host))));
    }
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    let first = link_text(&app).unwrap();
    // Con la confirmación abierta llegan dos más: ninguno pregunta ni consulta.
    for url in [
        "jase://connect/cod2/8.8.8.8:1",
        "jase://connect/cod2/7.7.7.7:1",
    ] {
        let task = link(&mut app, LinkMsg::Received(url.into()));
        drive(&mut app, task);
    }
    assert_eq!(link_text(&app), Some(first));
    assert_eq!(queried(&fake), [addr()]);
    let task = link(&mut app, LinkMsg::Confirm(true));
    drive(&mut app, task);
    assert!(link_text(&app).unwrap().contains("7.7.7.7:1"));
    let task = link(&mut app, LinkMsg::Confirm(true));
    drive(&mut app, task);
    assert_eq!(launched(&fake), [addr(), third.clone()]); // el intermedio se descartó
    assert_eq!(queried(&fake), [addr(), third]);
}

#[test]
fn link_waits_while_a_modal_dialog_is_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let other = ServerAddress::new("8.8.8.8", 27960);
    fake.answer_for(&other, Some(server(&other, "otro")));
    about(&mut app);
    let mut rest = Vec::new();
    for url in [
        "jase://connect/q3/7.7.7.7:27960",
        "jase://connect/q3/8.8.8.8:27960",
    ] {
        let task = link(&mut app, LinkMsg::Received(url.into()));
        rest.extend(drive(&mut app, task));
    }
    // Ni cambia de juego ni consulta; espera con un reintento (uno solo en marcha).
    assert_eq!(app.current_game().map(|g| g.id), Some("cod2"));
    assert!(fake.queries().is_empty());
    assert_eq!(retries(&rest), 1);
    let task = link(&mut app, LinkMsg::Retry);
    let rest = drive(&mut app, task);
    assert_eq!(app.dialog(), Some(&Dialog::About));
    assert!(fake.queries().is_empty() && !app.link_busy());
    assert_eq!(retries(&rest), 1); // sigue esperando
    let _ = app.update(Message::CloseDialog);
    let task = link(&mut app, LinkMsg::Retry);
    drive(&mut app, task);
    assert_eq!(app.current_game().map(|g| g.id), Some("q3"));
    assert_eq!(queried(&fake), [other]); // solo el último
    assert!(link_text(&app).is_some());
}

#[test]
fn pending_link_resumes_after_its_confirmation_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let other = ServerAddress::new("8.8.8.8", 1);
    for a in [addr(), other.clone()] {
        fake.answer_for(&a, Some(server(&a, "s")));
    }
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    let task = link(
        &mut app,
        LinkMsg::Received("jase://connect/cod2/8.8.8.8:1".into()),
    );
    drive(&mut app, task);
    // Un aviso que llega tarde (exportar que falló) reemplaza la confirmación.
    let failed = Message::Files(jase::files::FilesMsg::Exported {
        path: dir.path().join("f.json"),
        result: Err("x".into()),
    });
    let _ = app.update(failed);
    assert!(matches!(app.dialog(), Some(Dialog::Message { .. })));
    assert!(!app.link_busy());
    // Al cerrarlo, el pendiente se reintenta y pregunta.
    let task = app.update(Message::CloseDialog);
    let rest = drive(&mut app, task);
    assert_eq!(retries(&rest), 1);
    let task = link(&mut app, LinkMsg::Retry);
    drive(&mut app, task);
    assert_eq!(queried(&fake), [addr(), other]);
    assert!(link_text(&app).unwrap().contains("8.8.8.8:1"));
    assert!(fake.launches().is_empty());
}

#[test]
fn link_goes_on_with_a_file_selector_open() {
    // El selector no es modal (y sin portal tarda en volver): el enlace no lo espera.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr(), Some(server(&addr(), "Mi servidor")));
    let _selector = app.update(Message::Actions(actions::Msg::Run(Action::ExportFavorites)));
    assert!(app.files_open());
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    assert_eq!(queried(&fake), [addr()]);
    assert!(link_text(&app).is_some());
}

#[test]
fn show_command_raises_window() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let task = link(&mut app, LinkMsg::Received("show".into()));
    assert!(window_requests(task) >= 1);
    assert!(fake.queries().is_empty() && !app.link_busy());
    // Un enlace válido también trae la ventana al frente.
    let task = link(&mut app, open(URL, true));
    assert!(window_requests(task) >= 1);
}

#[test]
fn link_answer_for_left_game_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr(), Some(server(&addr(), "Mi servidor")));
    let task = link(&mut app, open(URL, true));
    let answers = found(run_task(task));
    assert_eq!(answers.len(), 1);
    let _ = app.update(Message::SelectGame("ut99"));
    for m in answers {
        let task = app.update(m);
        drive(&mut app, task);
    }
    assert!(app.dialog().is_none() && !app.link_busy());
    assert!(app.servers().iter().all(|s| s.address != addr()));
    // El siguiente ya no espera.
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    assert_eq!(queried(&fake).len(), 2);
    assert!(link_text(&app).is_some());
}

#[test]
fn link_answer_with_dialog_open_retries() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr(), Some(server(&addr(), "Mi servidor")));
    let task = link(&mut app, open(URL, true));
    let answers = found(run_task(task));
    about(&mut app);
    let mut rest = Vec::new();
    for m in answers {
        let task = app.update(m);
        rest.extend(drive(&mut app, task));
    }
    // No pregunta encima de *Acerca de*: el enlace queda pendiente con su reintento.
    assert_eq!(app.dialog(), Some(&Dialog::About));
    assert!(!app.link_busy());
    assert_eq!(retries(&rest), 1);
    let _ = app.update(Message::CloseDialog);
    let task = link(&mut app, LinkMsg::Retry);
    drive(&mut app, task);
    assert_eq!(queried(&fake), [addr(), addr()]);
    assert!(link_text(&app).is_some());
}

#[test]
fn startup_link_is_opened_interactively() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    let task = App::link_at_start(Some("jase://connect/cod2/;rm -rf:1".into()));
    for m in run_task(task) {
        let task = app.update(m);
        drive(&mut app, task);
    }
    assert!(matches!(app.dialog(), Some(Dialog::Message { .. })));
    assert!(fake.queries().is_empty());
    assert!(run_task(App::link_at_start(None)).is_empty());
}

#[test]
fn links_never_listen_in_tests() {
    assert!(fake_deps(&FakeScan::default(), "").link_endpoint.is_none());
}

#[test]
fn link_dialog_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app(dir.path());
    fake.answer_for(&addr(), Some(server(&addr(), "^1<b>x</b>\n")));
    let task = link(&mut app, open(URL, true));
    drive(&mut app, task);
    assert!(link_text(&app).is_some());
    for (id, _, _) in VARIANTS.iter() {
        app.theme_id = id;
        let _ = app.view();
    }
}
