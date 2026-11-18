//! Conectar (`window_connect.py` y sus tests de `tests/test_main_window.py`): ruta que falta,
//! servidor sin respuesta, contraseña con el llavero, lanzador y recientes.

mod support;

use std::path::Path;

use iced::Task;
use iced::keyboard::{Key, key::Named};
use jase::actions::{self, Action, enabled};
use jase::app::{App, ConnectMsg, Dialog, Message, PromptMsg};
use jase::launch::{LaunchArgs, launch_args, launch_with};
use jase::scan::ScanMsg;
use jase::table;
use jase::theme::VARIANTS;
use jase_core::config::{Config, load_config};
use jase_core::favorites::offline_server;
use jase_core::games::game;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::recent::recent_addresses;
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{FakeScan, TestApp, fake_app, mk, run_task};

/// El servidor de UT con contraseña: consulta en :7778, conexión en :7777.
fn ut_locked() -> ServerInfo {
    let mut info = ServerInfo::new(
        ServerAddress::new("5.5.5.5", 7778),
        ServerAddress::new("5.5.5.5", 7777),
    );
    info.name = "UT con clave".into();
    info.version = "451".into();
    info.max_players = 16;
    info.password = true;
    info
}

fn ut_user() -> String {
    "ut99:5.5.5.5:7778".to_owned()
}

/// Elige el juego y le entrega `servers` como un escaneo terminado.
fn show(app: &mut App, id: &'static str, servers: &[ServerInfo]) {
    let _ = app.update(Message::SelectGame(id));
    let generation = app.scan.generation();
    for s in servers {
        let msg = ScanMsg::Event(ScanEvent::Server(s.clone()));
        let _ = app.update(Message::Scan { generation, msg });
    }
    let result = ScanResult {
        masters_ok: 1,
        servers_listed: servers.len(),
        servers_responded: servers.len(),
        errors: vec![],
    };
    let msg = ScanMsg::Finished(result);
    let _ = app.update(Message::Scan { generation, msg });
}

fn select(app: &mut App, address: &ServerAddress) {
    let _ = app.update(Message::Table(table::Msg::RowClicked(address.clone())));
}

/// La app con `servers` del juego `id` en la tabla y el primero elegido.
fn shown(dir: &Path, cfg: Config, id: &'static str, servers: &[ServerInfo]) -> (TestApp, FakeScan) {
    let (mut app, fake) = fake_app(dir, cfg);
    if game(id).is_some_and(|g| g.uses_steam_api()) {
        let _ = app.update(Message::SteamKey("K".into()));
    }
    show(&mut app, id, servers);
    if let Some(first) = servers.first() {
        select(&mut app, &first.address);
    }
    (app, fake)
}

/// Corre la `Task` y entrega a la app lo que vuelve de conectar (como el runtime de Iced).
fn drive(app: &mut App, task: Task<Message>) {
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        for m in run_task(task) {
            if matches!(m, Message::Connect(_)) {
                pending.push(app.update(m));
            }
        }
    }
}

fn connect(app: &mut App) {
    let task = app.connect_selected();
    drive(app, task);
}

fn prompt(app: &mut App, m: PromptMsg) {
    let task = app.update(Message::Prompt(m));
    drive(app, task);
}

/// Juego, carpeta, dirección y contraseña de cada lanzamiento (`FakeLaunch.calls`).
fn calls(fake: &FakeScan) -> Vec<(&'static str, Option<String>, ServerAddress, Option<String>)> {
    fake.launches()
        .into_iter()
        .map(|a| (a.game.id, a.install_dir, a.address, a.password))
        .collect()
}

fn password_dialog(app: &App) -> Option<(String, String, bool)> {
    match app.dialog() {
        Some(Dialog::Password { name, typed, saved }) => {
            Some((name.clone(), typed.clone(), *saved))
        }
        _ => None,
    }
}

fn message_dialog(app: &App) -> Option<(String, String, String, bool)> {
    match app.dialog() {
        Some(Dialog::Message {
            title,
            text,
            informative,
            settings,
        }) => Some((title.clone(), text.clone(), informative.clone(), *settings)),
        _ => None,
    }
}

const UT_DIR: &str = r"D:\Juegos\UnrealTournament";

#[test]
fn connect_passes_connect_address_and_password() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    assert!(enabled(&app, Action::Connect));
    connect(&mut app);
    assert!(fake.launches().is_empty());
    assert_eq!(
        password_dialog(&app),
        Some(("UT con clave".into(), String::new(), false))
    );
    prompt(&mut app, PromptMsg::Input("pw".into()));
    prompt(&mut app, PromptMsg::Submit);
    assert_eq!(app.dialog(), None);
    assert_eq!(
        calls(&fake),
        [(
            "ut99",
            Some(UT_DIR.to_owned()),
            ServerAddress::new("5.5.5.5", 7777),
            Some("pw".to_owned())
        )]
    );
    assert_eq!(
        fake.keyring
            .entries
            .lock()
            .unwrap()
            .get(&ut_user())
            .cloned(),
        Some("pw".to_owned())
    );
    // La segunda vez el diálogo trae la contraseña guardada.
    connect(&mut app);
    assert_eq!(
        password_dialog(&app),
        Some(("UT con clave".into(), "pw".into(), true))
    );
}

#[test]
fn forget_password() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    fake.keyring
        .entries
        .lock()
        .unwrap()
        .insert(ut_user(), "old".into());
    connect(&mut app);
    assert_eq!(password_dialog(&app).map(|d| d.2), Some(true));
    prompt(&mut app, PromptMsg::Forget);
    assert_eq!(app.dialog(), None);
    assert!(fake.keyring.entries.lock().unwrap().is_empty());
    assert!(fake.launches().is_empty());
    assert_eq!(app.status(), "Contraseña olvidada");
}

#[test]
fn empty_password_submit_forgets_saved_and_launches_without() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    fake.keyring
        .entries
        .lock()
        .unwrap()
        .insert(ut_user(), "old".into());
    connect(&mut app);
    assert_eq!(password_dialog(&app).map(|d| d.1), Some("old".into()));
    prompt(&mut app, PromptMsg::Input(String::new()));
    prompt(&mut app, PromptMsg::Submit);
    assert!(fake.keyring.entries.lock().unwrap().is_empty());
    assert_eq!(
        calls(&fake),
        [(
            "ut99",
            Some(UT_DIR.to_owned()),
            ServerAddress::new("5.5.5.5", 7777),
            None
        )]
    );
}

#[test]
fn password_dialog_forget_button_only_with_initial() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    connect(&mut app);
    assert_eq!(password_dialog(&app).map(|d| d.2), Some(false));
    // Sin guardada no hay *Olvidar contraseña*: el mensaje no hace nada.
    prompt(&mut app, PromptMsg::Forget);
    assert_eq!(password_dialog(&app).map(|d| d.2), Some(false));
    prompt(&mut app, PromptMsg::Cancel);
    assert_eq!(app.dialog(), None);
    fake.keyring
        .entries
        .lock()
        .unwrap()
        .insert(ut_user(), "abc".into());
    connect(&mut app);
    assert_eq!(
        password_dialog(&app),
        Some(("UT con clave".into(), "abc".into(), true))
    );
    // Esc cancela sin lanzar.
    let _ = app.update(Message::Escape);
    assert_eq!(app.dialog(), None);
    assert!(fake.launches().is_empty());
}

#[test]
fn connect_tells_the_launcher_the_tacops_version() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config {
        wine_command: String::new(),
        ..Config::default()
    };
    cfg.game_paths
        .insert("tacops".into(), r"I:\TacticalOps".into());
    let mut info = mk("5.5.5.5", "t", 40, 1, "3.5");
    info.address = ServerAddress::new("5.5.5.5", 7778);
    info.connect_address = ServerAddress::new("5.5.5.5", 7777);
    let (mut app, fake) = shown(dir.path(), cfg, "tacops", &[info]);
    connect(&mut app);
    let launches = fake.launches();
    assert_eq!(launches.len(), 1);
    assert_eq!(launches[0].version, "3.5");
    assert_eq!(launches[0].mod_name, "");
    assert_eq!(launches[0].wrapper, None);
}

#[test]
fn double_click_row_connects() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let info = mk("1.1.1.1", "a", 40, 3, "1.3");
    show(&mut app, "cod2", std::slice::from_ref(&info));
    let task = app.update(Message::Table(table::Msg::RowDoubleClicked(
        info.address.clone(),
    )));
    drive(&mut app, task);
    assert_eq!(
        calls(&fake),
        [(
            "cod2",
            Some(r"D:\Juegos\CoD2".to_owned()),
            ServerAddress::new("1.1.1.1", 28960),
            None
        )]
    );
}

fn press_enter(app: &mut App) {
    if let Some(m) = app.key_message(Key::Named(Named::Enter)) {
        let task = app.update(m);
        drive(app, task);
    }
}

#[test]
fn enter_on_table_connects() {
    let dir = tempfile::tempdir().unwrap();
    let info = mk("1.1.1.1", "a", 40, 3, "1.3");
    let (mut app, fake) = shown(dir.path(), Config::default(), "cod2", &[info]);
    press_enter(&mut app);
    assert_eq!(fake.launches().len(), 1);
}

#[test]
fn enter_without_selection_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    show(&mut app, "cod1", &[mk("1.1.1.1", "a", 40, 3, "1.5")]);
    let _ = app.update(Message::Table(table::Msg::BlankClicked));
    assert_eq!(app.selected_server(), None);
    press_enter(&mut app);
    // Sin selección no llega a conectar: ni el aviso de la ruta que falta.
    assert_eq!(app.dialog(), None);
    assert!(fake.launches().is_empty());
}

#[test]
fn keyring_error_still_launches() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    *fake.keyring.set_error.lock().unwrap() = Some("llavero bloqueado".into());
    connect(&mut app);
    prompt(&mut app, PromptMsg::Input("pw".into()));
    prompt(&mut app, PromptMsg::Submit);
    let (title, text, _, settings) = message_dialog(&app).expect("aviso del llavero");
    assert_eq!(title, "Contraseña");
    assert!(text.to_lowercase().contains("contraseña"));
    assert!(text.contains("llavero bloqueado"));
    assert!(!settings);
    let launches = fake.launches();
    assert_eq!(launches[0].password.as_deref(), Some("pw"));
}

#[test]
fn connect_without_path_shows_settings_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let info = mk("3.3.3.3", "a", 40, 3, "1.5");
    let (mut app, fake) = shown(dir.path(), Config::default(), "cod1", &[info]);
    assert!(!enabled(&app, Action::Connect));
    connect(&mut app);
    let (title, text, informative, settings) = message_dialog(&app).expect("aviso de la ruta");
    assert_eq!(title, "Ruta no configurada");
    assert_eq!(text, "No hay ruta configurada para Call of Duty.");
    assert_eq!(informative, "Configúrala en Ajustes para poder conectarte.");
    assert!(settings);
    assert!(fake.launches().is_empty());
    // Cerrar el aviso deja todo como estaba.
    prompt(&mut app, PromptMsg::Cancel);
    assert_eq!(app.dialog(), None);
}

#[test]
fn connect_without_selection_and_without_path_shows_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    show(&mut app, "cod1", &[]);
    connect(&mut app);
    assert_eq!(
        message_dialog(&app).map(|d| d.0),
        Some("Ruta no configurada".into())
    );
}

fn l4d2() -> ServerInfo {
    let mut info = mk("3.3.3.3", "s", 40, 3, "2.2.4.3");
    info.address = ServerAddress::new("3.3.3.3", 27015);
    info.connect_address = info.address.clone();
    info
}

#[test]
fn steam_game_connects_without_path() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "l4d2", &[l4d2()]);
    assert!(enabled(&app, Action::Connect));
    connect(&mut app);
    assert_eq!(app.dialog(), None);
    let launches = fake.launches();
    assert_eq!(launches.len(), 1);
    assert_eq!(launches[0].game.id, "l4d2");
    assert_eq!(launches[0].install_dir, None);
    assert_eq!(launches[0].wrapper, None);
}

#[test]
fn launch_error_shows_message() {
    let dir = tempfile::tempdir().unwrap();
    let info = mk("1.1.1.1", "a", 40, 3, "1.3");
    let (mut app, fake) = shown(dir.path(), Config::default(), "cod2", &[info]);
    fake.fail_launch("No se encontró X");
    connect(&mut app);
    assert_eq!(fake.launches().len(), 1);
    let (title, text, informative, settings) = message_dialog(&app).expect("error");
    assert_eq!(title, "No se pudo conectar");
    assert_eq!(text, "No se encontró X");
    assert_eq!(informative, "Revisa la ruta del juego en Ajustes.");
    assert!(settings);
}

#[test]
fn steam_launch_error_has_no_path_hint() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "l4d2", &[l4d2()]);
    fake.fail_launch("No se pudo abrir Steam. ¿Está instalado?");
    connect(&mut app);
    let (title, text, informative, settings) = message_dialog(&app).expect("error");
    assert_eq!(title, "No se pudo conectar");
    assert!(text.contains("Steam") && !text.contains("ruta"));
    assert!(informative.is_empty());
    assert!(!settings);
}

#[test]
fn tree_shows_bf1942_mod_and_connect_passes_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config {
        wine_command: String::new(),
        ..Config::default()
    };
    cfg.game_paths
        .insert("bf1942".into(), r"D:\Juegos\BF1942".into());
    let mut info = mk("1.1.1.1", "dc", 40, 3, "");
    info.rules.insert("gameId".into(), "DC_Final".into());
    let mut base = mk("2.2.2.2", "base", 40, 3, "");
    base.rules.insert("gameId".into(), "bf1942".into());
    let (mut app, fake) = shown(dir.path(), cfg, "bf1942", &[info.clone(), base.clone()]);
    connect(&mut app);
    select(&mut app, &base.address);
    connect(&mut app);
    let mods: Vec<String> = fake.launches().into_iter().map(|a| a.mod_name).collect();
    assert_eq!(mods, ["DC_Final", ""]); // valor crudo del servidor
}

#[test]
fn connect_records_recent() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let a = mk("1.1.1.1", "a", 40, 3, "1.3");
    let task = app.connect_to(game("cod2").unwrap(), a.clone());
    drive(&mut app, task);
    assert_eq!(recent_addresses(app.config(), "cod2"), [a.address]);
    let saved = load_config(&dir.path().join("config.json"));
    assert_eq!(saved.recent["cod2"], ["1.1.1.1:28960"]);
    assert_eq!(app.status(), "Lanzando Call of Duty 2 → 1.1.1.1:28960");
}

#[test]
fn failed_launch_not_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    fake.fail_launch("x");
    let task = app.connect_to(game("cod2").unwrap(), mk("1.1.1.1", "a", 40, 3, "1.3"));
    drive(&mut app, task);
    assert!(recent_addresses(app.config(), "cod2").is_empty());
}

#[test]
fn offline_favorite_connect() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.favorites
        .insert("cod2".into(), vec!["3.3.3.3:28960".into()]);
    cfg.game_paths.insert("cod2".into(), "C:/cod2".into());
    let address = ServerAddress::new("3.3.3.3", 28960);
    let (mut app, fake) = shown(dir.path(), cfg, "cod2", &[offline_server(&address)]);
    assert_eq!(app.selected_server().map(|s| s.online), Some(false));
    // La dirección guardada es la de consulta: no se lanza contra ella.
    assert!(!enabled(&app, Action::Connect));
    let task = app.update(Message::Table(table::Msg::RowDoubleClicked(
        address.clone(),
    )));
    drive(&mut app, task);
    assert!(fake.launches().is_empty());
    assert_eq!(
        app.status(),
        "El servidor no responde; refresca para conectar"
    );
    let _ = app.update(Message::Refresh); // la barra de estado cambia
    show(&mut app, "cod2", &[offline_server(&address)]);
    select(&mut app, &address);
    press_enter(&mut app);
    assert!(fake.launches().is_empty());
    assert_eq!(
        app.status(),
        "El servidor no responde; refresca para conectar"
    );
    // Si responde (p. ej. tras refrescar), se puede conectar.
    show(&mut app, "cod2", &[mk("3.3.3.3", "a", 40, 3, "1.3")]);
    select(&mut app, &address);
    assert!(enabled(&app, Action::Connect));
    connect(&mut app);
    assert_eq!(calls(&fake)[0].2, address);
}

#[test]
fn launch_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let info = mk("1.1.1.1", "a", 40, 3, "1.3");
    let (mut app, fake) = shown(dir.path(), Config::default(), "cod2", &[info]);
    let _task = app.connect_selected();
    let _ = app.update(Message::Connect(ConnectMsg::Enter));
    let _ = app.update(Message::Table(table::Msg::RowDoubleClicked(
        ServerAddress::new("1.1.1.1", 28960),
    )));
    assert!(fake.launches().is_empty());

    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    let _task = app.connect_selected();
    let request = app.connect_request();
    let _ = app.update(Message::Connect(ConnectMsg::Stored {
        request,
        password: None,
    }));
    let _ = app.update(Message::Prompt(PromptMsg::Input("pw".into())));
    let _ = app.update(Message::Prompt(PromptMsg::Submit));
    assert!(fake.launches().is_empty());
    // Solo la clave de Steam (de `App::new`, sin correr) podría haberlo tocado.
    assert!(fake.keyring.calls().is_empty());
}

#[test]
fn stale_stored_password_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let mut other = ut_locked();
    other.address = ServerAddress::new("6.6.6.6", 7778);
    other.connect_address = ServerAddress::new("6.6.6.6", 7777);
    other.name = "Otro".into();
    let (mut app, _fake) = shown(
        dir.path(),
        Config::default(),
        "ut99",
        &[ut_locked(), other.clone()],
    );
    let first = app.connect_selected();
    select(&mut app, &other.address);
    let second = app.connect_selected();
    // La respuesta del primer pedido llega tarde: no abre nada.
    drive(&mut app, first);
    assert_eq!(app.dialog(), None);
    // Con otro diálogo abierto, la vigente tampoco abre el suyo.
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::About)));
    drive(&mut app, second);
    assert_eq!(app.dialog(), Some(&Dialog::About));
    let _ = app.update(Message::CloseDialog);
    connect(&mut app);
    assert_eq!(password_dialog(&app).map(|d| d.0), Some("Otro".into()));
}

#[test]
fn password_never_leaks() {
    const SECRET: &str = "s3cr3t";
    // Guardar y lanzar fallan: el error del lanzamiento es el que se muestra.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    *fake.keyring.set_error.lock().unwrap() = Some("bloqueado".into());
    fake.fail_launch("No se encontró UnrealTournament.exe");
    connect(&mut app);
    prompt(&mut app, PromptMsg::Input(SECRET.into()));
    prompt(&mut app, PromptMsg::Submit);
    let shown_dialog = message_dialog(&app).expect("error del lanzamiento");
    assert_eq!(shown_dialog.0, "No se pudo conectar");
    assert!(!format!("{shown_dialog:?}").contains(SECRET));
    assert!(!app.status().contains(SECRET));
    // Guardar falla pero lanza: el aviso del llavero y los recientes (config en disco).
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    *fake.keyring.set_error.lock().unwrap() = Some("bloqueado".into());
    connect(&mut app);
    prompt(&mut app, PromptMsg::Input(SECRET.into()));
    prompt(&mut app, PromptMsg::Submit);
    let shown_dialog = message_dialog(&app).expect("aviso del llavero");
    assert!(!format!("{shown_dialog:?}").contains(SECRET));
    assert!(!app.status().contains(SECRET));
    let saved = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(saved.contains("5.5.5.5:7778") && !saved.contains(SECRET));
    assert_eq!(fake.launches()[0].password.as_deref(), Some(SECRET));
}

#[test]
fn password_dialog_shows_remote_name_plain() {
    let dir = tempfile::tempdir().unwrap();
    let mut info = ut_locked();
    info.name = "a\n<b>x</b>".into();
    let (mut app, _fake) = shown(dir.path(), Config::default(), "ut99", &[info]);
    connect(&mut app);
    assert_eq!(
        password_dialog(&app).map(|d| d.0),
        Some("a\n<b>x</b>".into())
    );
    for (id, _, _) in VARIANTS.iter() {
        app.theme_id = id;
        let _ = app.view();
    }
}

#[test]
fn dialogs_view_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    let views = |app: &mut App| {
        for (id, _, _) in VARIANTS.iter() {
            app.theme_id = id;
            let _ = app.view();
        }
    };
    connect(&mut app); // sin *Olvidar*
    views(&mut app);
    prompt(&mut app, PromptMsg::Cancel);
    fake.keyring
        .entries
        .lock()
        .unwrap()
        .insert(ut_user(), "abc".into());
    connect(&mut app); // con *Olvidar*
    views(&mut app);
    prompt(&mut app, PromptMsg::Cancel);
    fake.fail_launch("x");
    *fake.keyring.set_error.lock().unwrap() = None;
    let _ = app.update(Message::SelectGame("cod1"));
    connect(&mut app); // con *Abrir ajustes*
    assert_eq!(message_dialog(&app).map(|d| d.3), Some(true));
    views(&mut app);
    prompt(&mut app, PromptMsg::Cancel);
    let _ = app.update(Message::SteamKey("K".into()));
    show(&mut app, "l4d2", &[l4d2()]);
    select(&mut app, &l4d2().address);
    connect(&mut app); // solo *Cerrar*
    assert_eq!(message_dialog(&app).map(|d| d.3), Some(false));
    views(&mut app);
}

#[test]
fn launch_args_wrapper_by_platform() {
    let cfg = Config::default();
    let cod2 = launch_args(
        game("cod2").unwrap(),
        &mk("1.1.1.1", "a", 40, 3, "1.3"),
        &cfg,
        None,
    );
    assert_eq!(
        cod2.wrapper,
        (!cfg!(windows)).then(|| vec!["wine".to_owned()])
    );
    assert_eq!(cod2.install_dir.as_deref(), Some(r"D:\Juegos\CoD2"));
    assert_eq!(cod2.version, ""); // solo los juegos con una copia por versión
    let l4d2 = launch_args(game("l4d2").unwrap(), &l4d2(), &cfg, Some("pw".into()));
    assert_eq!(l4d2.wrapper, None);
    assert_eq!(l4d2.install_dir, None);
    assert_eq!(l4d2.password.as_deref(), Some("pw"));
}

/// Lanza con un `spawn` falso que anota el argv.
fn argv_of(args: &LaunchArgs) -> Vec<String> {
    let mut argv = Vec::new();
    let mut spawn = |cmd: &jase_core::launcher::LaunchCommand| {
        argv = cmd.argv.clone();
        Ok(())
    };
    let mut open = |_: &str| Ok(());
    launch_with(args, &mut spawn, &mut open).unwrap();
    argv
}

#[test]
fn hostile_server_values_do_not_reach_argv() {
    let cfg = Config::default();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("BF1942.exe"), b"").unwrap();
    std::fs::write(dir.path().join("zandronum.exe"), b"").unwrap();
    let folder = dir.path().to_string_lossy().into_owned();

    let bf = game("bf1942").unwrap();
    let mut info = mk("1.1.1.1", "x", 40, 3, "");
    info.rules.insert("gameId".into(), "DC +exec x".into());
    let mut args = launch_args(bf, &info, &cfg, None);
    assert_eq!(args.mod_name, "DC +exec x"); // el valor crudo: lo filtra el lanzador
    args.install_dir = Some(folder.clone());
    args.wrapper = None;
    let argv = argv_of(&args);
    assert!(!argv.iter().any(|a| a == "+game" || a.contains("exec")));

    let zand = game("zandronum").unwrap();
    let mut info = mk("1.1.1.1", "x", 40, 3, "");
    info.rules.insert("iwad".into(), "../../x.wad".into());
    let mut args = launch_args(zand, &info, &cfg, None);
    args.install_dir = Some(folder);
    args.wrapper = None;
    let argv = argv_of(&args);
    assert!(!argv.iter().any(|a| a == "-iwad" || a.contains("x.wad")));
}

#[test]
fn late_launch_result_does_not_replace_open_dialog() {
    let dir = tempfile::tempdir().unwrap();
    let mut open = ut_locked();
    open.address = ServerAddress::new("6.6.6.6", 7778);
    open.connect_address = ServerAddress::new("6.6.6.6", 7777);
    open.password = false;
    let (mut app, fake) = shown(
        dir.path(),
        Config::default(),
        "ut99",
        &[open.clone(), ut_locked()],
    );
    fake.fail_launch("No se encontró X");
    let first = app.connect_selected(); // sin contraseña: lanza
    select(&mut app, &ut_locked().address);
    connect(&mut app);
    prompt(&mut app, PromptMsg::Input("ab".into()));
    // El error del primero llega con el diálogo del segundo abierto: no lo pisa.
    drive(&mut app, first);
    assert_eq!(
        password_dialog(&app),
        Some(("UT con clave".into(), "ab".into(), false))
    );
    assert_eq!(app.status(), "No se pudo conectar: No se encontró X");
    // Y el diálogo sigue conectando a su servidor.
    prompt(&mut app, PromptMsg::Submit);
    let launches = fake.launches();
    assert_eq!(launches.len(), 2);
    assert_eq!(launches[1].address, ServerAddress::new("5.5.5.5", 7777));
}

#[test]
fn enter_presses_the_default_button() {
    let dir = tempfile::tempdir().unwrap();
    let info = mk("3.3.3.3", "a", 40, 3, "1.5");
    let (mut app, _fake) = shown(dir.path(), Config::default(), "cod1", &[info]);
    connect(&mut app);
    assert!(message_dialog(&app).is_some());
    press_enter(&mut app); // *Cancelar* del aviso
    assert_eq!(app.dialog(), None);
    // En el de contraseña, sin el campo enfocado, Enter es *Conectar*.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    connect(&mut app);
    prompt(&mut app, PromptMsg::Input("pw".into()));
    press_enter(&mut app);
    assert_eq!(app.dialog(), None);
    assert_eq!(fake.launches()[0].password.as_deref(), Some("pw"));
}

// --- Revisión final de la 5f ---

#[test]
fn launch_without_password_makes_pending_read_stale() {
    let dir = tempfile::tempdir().unwrap();
    let mut open = ut_locked();
    open.address = ServerAddress::new("6.6.6.6", 7778);
    open.connect_address = ServerAddress::new("6.6.6.6", 7777);
    open.password = false;
    let (mut app, fake) = shown(
        dir.path(),
        Config::default(),
        "ut99",
        &[ut_locked(), open.clone()],
    );
    fake.keyring
        .entries
        .lock()
        .unwrap()
        .insert(ut_user(), "old".into());
    let locked = app.connect_selected(); // lectura del llavero en camino
    select(&mut app, &open.address);
    connect(&mut app); // lanza el que no tiene contraseña
    drive(&mut app, locked);
    assert_eq!(app.dialog(), None);
    assert_eq!(fake.launches().len(), 1);
}

#[test]
fn dialog_closes_open_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = shown(dir.path(), Config::default(), "ut99", &[ut_locked()]);
    let task = app.connect_selected();
    let _ = app.update(Message::Actions(actions::Msg::Open(actions::Menu::Server)));
    assert!(app.open_menu().is_some());
    drive(&mut app, task);
    assert!(password_dialog(&app).is_some());
    assert_eq!(app.open_menu(), None);
}

#[test]
fn lan_search_cancels_wait_for_steam_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.ui_state.insert("last_game".into(), "l4d".into());
    let (mut app, fake) = fake_app(dir.path(), cfg);
    let _ = app.update(Message::ScanLan);
    let generation = app.scan.generation();
    let msg = ScanMsg::Finished(ScanResult::default());
    let _ = app.update(Message::Scan { generation, msg });
    let _ = app.update(Message::SteamKey("K".into()));
    assert!(fake.calls().is_empty()); // ningún escaneo de master sobre la vista LAN
    assert!(app.scan.lan());
}

#[test]
fn enter_closes_about_and_saves_preset() {
    use jase::presets::{self, PresetItem};
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::About)));
    assert_eq!(app.dialog(), Some(&Dialog::About));
    press_enter(&mut app);
    assert_eq!(app.dialog(), None);
    let _ = app.update(Message::Presets(presets::Msg::Pick(PresetItem::Save)));
    press_enter(&mut app); // nombre vacío: nada
    assert_eq!(app.dialog(), Some(&Dialog::SavePreset(String::new())));
    let _ = app.update(Message::Presets(presets::Msg::NameChanged("mío".into())));
    press_enter(&mut app);
    assert_eq!(app.dialog(), None);
    assert_eq!(app.filter_presets()[0].name, "mío");
}
