//! El diálogo de Ajustes en la ventana (`window_settings.py`, `tests/test_main_window.py` y
//! `tests/test_masters_tab.py`): se llena con `SettingsMsg` y se acepta con `Accept`, como los
//! `fake_exec` de Python; el disco, el llavero, el selector y la URL corren al ejecutar la `Task`.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use iced::Task;
use iced_runtime::Action;
use jase::app::{
    App, Dialog, DirectMsg, DirectView, MastersMsg, Message, PATH_DEBOUNCE, PromptMsg,
    STEAM_KEY_URL, SettingsMsg,
};
use jase::passwords::STEAM_KEY_USER;
use jase::scan::ScanMsg;
use jase::settings::{CountryButton, Form, PathStatus, Tab, swatch_colors, tab_labels};
use jase::startup::system_locale_name;
use jase::table;
use jase::theme::{TERMINAL_COLORS, VARIANTS, palette};
use jase_core::config::{Config, load_config};
use jase_core::geo::CountryDB;
use jase_core::i18n::resolve_language;
use jase_core::masters::{CUSTOM_FILENAME, load_custom};
use jase_core::models::ServerInfo;
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::lang::with_language;
use support::{FakeScan, TestApp, fake_app, fake_app_with_key, mk, run_task, view_texts};

fn set(app: &mut App, m: SettingsMsg) -> Task<Message> {
    app.update(Message::Settings(m))
}

fn masters(app: &mut App, m: MastersMsg) -> Task<Message> {
    set(app, SettingsMsg::Masters(m))
}

fn form(app: &App) -> &Form {
    app.settings_form().expect("Ajustes abierto")
}

/// Corre la `Task` y entrega a la app todo lo que vuelve (como el runtime de Iced).
fn drive(app: &mut App, task: Task<Message>) {
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        for m in run_task(task) {
            pending.push(app.update(m));
        }
    }
}

/// Abre Ajustes y deja correr lo que pide (el estado de las rutas).
fn open(app: &mut App) {
    let task = set(app, SettingsMsg::Open);
    drive(app, task);
}

fn accept(app: &mut App) {
    let task = set(app, SettingsMsg::Accept);
    drive(app, task);
}

fn status(app: &App, game: &str) -> PathStatus {
    form(app).status[game].clone()
}

fn show(app: &mut App, id: &'static str, servers: &[ServerInfo]) {
    let _ = app.update(Message::SelectGame(id));
    let generation = app.scan.generation();
    for s in servers {
        let msg = ScanMsg::Event(ScanEvent::Server(s.clone()));
        let _ = app.update(Message::Scan { generation, msg });
    }
    let msg = ScanMsg::Finished(ScanResult {
        masters_ok: 1,
        servers_listed: servers.len(),
        servers_responded: servers.len(),
        errors: vec![],
    });
    let _ = app.update(Message::Scan { generation, msg });
}

fn custom_path(dir: &Path) -> PathBuf {
    dir.join(CUSTOM_FILENAME)
}

fn add_mine(app: &mut App) {
    let _ = masters(app, MastersMsg::Game("cod2"));
    let _ = masters(app, MastersMsg::Add);
    let _ = masters(app, MastersMsg::AddText("mine.example.com:1234".into()));
    let _ = masters(app, MastersMsg::AddSubmit);
}

// ------------------------------------------------------------------ pestañas y masters

#[test]
fn settings_has_six_tabs() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut app, _) = fake_app(dir.path(), Config::default());
        assert_eq!(
            tab_labels(),
            [
                "Juegos",
                "Masters",
                "General",
                "Apariencia",
                "Barra",
                "Avisos"
            ]
        );
        open(&mut app);
        assert_eq!(form(&app).tab, Tab::Games);
    }
    with_language("en", || {
        assert_eq!(
            tab_labels(),
            [
                "Games",
                "Masters",
                "General",
                "Appearance",
                "Toolbar",
                "Notifications"
            ]
        );
        let (mut app, _) = fake_app(dir.path(), Config::default());
        open(&mut app);
        let _ = set(&mut app, SettingsMsg::Tab(Tab::Masters));
        assert!(view_texts(&app).iter().any(|t| t == "Source"));
    });
}

#[test]
fn open_settings_saves_custom_masters() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    open(&mut app);
    add_mine(&mut app);
    accept(&mut app);
    let added = &app.custom_masters().added;
    assert_eq!(added["cod2"], [("mine.example.com".to_owned(), 1234)]);
    assert_eq!(&load_custom(&custom_path(dir.path())).added, added);
    assert!(app.dialog().is_none() && app.settings_form().is_none());
}

#[test]
fn saving_changed_masters_refreshes_current_game() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    show(&mut app, "cod2", &[]);
    let before = fake.calls().len();
    open(&mut app);
    add_mine(&mut app);
    accept(&mut app);
    assert_eq!(fake.calls().len(), before + 1);
}

#[test]
fn saving_unchanged_masters_does_not_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    show(&mut app, "cod2", &[]);
    let before = fake.calls().len();
    open(&mut app);
    accept(&mut app);
    assert_eq!(fake.calls().len(), before);
    assert!(!app.is_scanning());
}

#[test]
fn saving_changed_masters_in_favorites_does_not_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    show(&mut app, "cod2", &[]);
    let _ = app.update(Message::Direct(DirectMsg::Show(DirectView::Favorites)));
    let before = fake.calls().len();
    open(&mut app);
    add_mine(&mut app);
    accept(&mut app);
    assert_eq!(fake.calls().len(), before);
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
}

#[test]
fn open_settings_cancel_keeps_custom() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    open(&mut app);
    add_mine(&mut app);
    let _ = set(&mut app, SettingsMsg::Cancel);
    assert!(app.custom_masters().added.is_empty());
    assert!(!custom_path(dir.path()).exists());
    assert!(app.dialog().is_none() && app.settings_form().is_none());
}

#[test]
fn pending_migration_cleared_after_custom_saved() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.masters
        .insert("cod2".into(), vec![("mine.lan".to_owned(), 9)]);
    // Una carpeta donde va el archivo: la migración no se puede guardar.
    std::fs::create_dir(custom_path(dir.path())).unwrap();
    let (mut app, _) = fake_app(dir.path(), cfg);
    assert!(!app.config().masters.is_empty()); // migración pendiente
    std::fs::remove_dir(custom_path(dir.path())).unwrap();
    open(&mut app);
    let _ = masters(&mut app, MastersMsg::Add);
    let _ = masters(
        &mut app,
        MastersMsg::AddText("other.example.com:1234".into()),
    );
    let _ = masters(&mut app, MastersMsg::AddSubmit);
    accept(&mut app);
    assert!(app.config().masters.is_empty());
    assert!(
        load_config(&dir.path().join("config.json"))
            .masters
            .is_empty()
    );
    let saved = load_custom(&custom_path(dir.path()));
    assert!(saved.added["cod2"].contains(&("other.example.com".to_owned(), 1234)));
}

#[test]
fn masters_tab_binds_steam_key() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _) = fake_app_with_key(dir.path(), Config::default(), "");
    let _ = app.update(Message::SteamKey("K".into()));
    open(&mut app);
    let _ = masters(&mut app, MastersMsg::Game("l4d2"));
    let _ = masters(&mut app, MastersMsg::Select(0));
    let task = masters(&mut app, MastersMsg::Probe);
    drive(&mut app, task);
    let probes = fake.probes.lock().unwrap().clone();
    assert_eq!(probes.len(), 1);
    assert_eq!((probes[0].0, probes[0].2.as_str()), ("l4d2", "K"));
}

#[test]
fn toggling_official_master_selects_its_row() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    open(&mut app);
    let _ = masters(&mut app, MastersMsg::Toggle(1));
    // Como `QTableWidget`: el clic en la casilla también elige la fila.
    let tab = &form(&app).masters;
    assert_eq!(tab.selected, Some(1));
    assert!(!tab.rows()[1].active);
}

#[test]
fn masters_add_prompt_and_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    open(&mut app);
    let _ = masters(&mut app, MastersMsg::Add);
    assert_eq!(form(&app).masters.adding.as_deref(), Some(""));
    let _ = masters(&mut app, MastersMsg::AddText("bad".into()));
    let _ = app.update(Message::Prompt(PromptMsg::Submit)); // Enter = *Guardar* del prompt
    assert_eq!(
        form(&app).masters.warning.as_deref(),
        Some("Dirección no válida; usa host:puerto (puerto 1-65535).")
    );
    let _ = app.view();
    let _ = app.update(Message::Escape);
    assert!(form(&app).masters.warning.is_none());
    assert!(form(&app).masters.adding.is_some());
    let _ = app.update(Message::Escape);
    assert!(form(&app).masters.adding.is_none());
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
    // Uno repetido: el otro aviso.
    let first = form(&app).masters.rows()[0].master.clone();
    let _ = masters(&mut app, MastersMsg::Add);
    let _ = masters(
        &mut app,
        MastersMsg::AddText(format!("{}:{}", first.0, first.1)),
    );
    let _ = masters(&mut app, MastersMsg::AddSubmit);
    assert_eq!(
        form(&app).masters.warning.as_deref(),
        Some("Ese master ya está en la lista.")
    );
    let _ = masters(&mut app, MastersMsg::CloseWarning);
    let _ = masters(&mut app, MastersMsg::AddCancel);
    let _ = app.update(Message::Escape); // ya sin internas: cancela Ajustes
    assert!(app.dialog().is_none());
}

#[test]
fn probe_runs_in_task_and_late_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    *fake.probe_answer.lock().unwrap() = Some(3);
    open(&mut app);
    let _ = masters(&mut app, MastersMsg::Select(0));
    let task = masters(&mut app, MastersMsg::Probe);
    assert!(fake.probes.lock().unwrap().is_empty());
    assert_eq!(form(&app).masters.rows()[0].probe, "Probando…");
    let late = run_task(task);
    assert_eq!(fake.probes.lock().unwrap().len(), 1);
    // Otra fila antes de que llegue: la respuesta ya no vale.
    let _ = masters(&mut app, MastersMsg::Select(1));
    for m in late {
        let _ = app.update(m);
    }
    assert!(form(&app).masters.rows().iter().all(|r| r.probe.is_empty()));
    // Sin cambiar de fila, sí.
    let task = masters(&mut app, MastersMsg::Probe);
    drive(&mut app, task);
    assert_eq!(form(&app).masters.rows()[1].probe, "✓ 3 servidores");
}

// --------------------------------------------------------------------------- rutas

fn exe_dir(dir: &Path) -> PathBuf {
    let d = dir.join("cod2");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn with_cod2(dir: &Path, fake: &FakeScan) -> String {
    let d = exe_dir(dir);
    let exe = d.join("CoD2MP_s.exe").display().to_string();
    fake.exes.lock().unwrap().insert("cod2", exe);
    d.display().to_string()
}

fn cod2_cfg(path: &str) -> Config {
    let mut cfg = Config::default();
    cfg.game_paths.insert("cod2".into(), path.to_owned());
    cfg
}

#[test]
fn settings_shows_path_status() {
    let dir = tempfile::tempdir().unwrap();
    let path = exe_dir(dir.path()).display().to_string();
    let (mut app, fake) = fake_app(dir.path(), cod2_cfg(&path));
    with_cod2(dir.path(), &fake);
    let task = set(&mut app, SettingsMsg::Open);
    assert_eq!(status(&app, "cod2"), PathStatus::Searching);
    let text = |app: &App| {
        let s = status(app, "cod2");
        jase::settings::path_status_text(jase_core::games::game("cod2").unwrap(), &s)
    };
    assert_eq!(text(&app), "Buscando…");
    drive(&mut app, task);
    assert_eq!(text(&app), "\u{2713} CoD2MP_s.exe");
    let other = dir.path().join("otra").display().to_string();
    let task = set(
        &mut app,
        SettingsMsg::Path {
            game: "cod2",
            text: other,
        },
    );
    drive(&mut app, task);
    assert!(text(&app).starts_with('\u{26a0}') && text(&app).contains("CoD2MP_s.exe"));
    // Sin ruta y no de Steam: el aviso sin tocar el disco.
    assert_eq!(status(&app, "cod1"), PathStatus::Missing);
}

#[test]
fn settings_path_status_resolved_off_ui_thread() {
    let dir = tempfile::tempdir().unwrap();
    let path = exe_dir(dir.path()).display().to_string();
    let (mut app, fake) = fake_app(dir.path(), cod2_cfg(&path));
    with_cod2(dir.path(), &fake);
    fake.installed_at.lock().unwrap().clear();
    let task = set(&mut app, SettingsMsg::Open);
    assert!(fake.installed_at.lock().unwrap().is_empty());
    drive(&mut app, task);
    let calls = fake.installed_at.lock().unwrap().clone();
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|(_, t)| *t != std::thread::current().id()));
    assert_eq!(
        status(&app, "cod2"),
        PathStatus::Found(
            dir.path()
                .join("cod2")
                .join("CoD2MP_s.exe")
                .display()
                .to_string()
        )
    );
}

#[test]
fn cleared_path_ignores_late_answer() {
    let dir = tempfile::tempdir().unwrap();
    let path = exe_dir(dir.path()).display().to_string();
    let (mut app, fake) = fake_app(dir.path(), cod2_cfg(&path));
    with_cod2(dir.path(), &fake);
    let late = run_task(set(&mut app, SettingsMsg::Open));
    let task = set(
        &mut app,
        SettingsMsg::Path {
            game: "cod2",
            text: String::new(),
        },
    );
    drive(&mut app, task);
    assert_eq!(status(&app, "cod2"), PathStatus::Missing);
    for m in late {
        let _ = app.update(m); // la respuesta tardía de la ruta vieja
    }
    assert_eq!(status(&app, "cod2"), PathStatus::Missing);
}

#[test]
fn paths_after_close_are_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let path = exe_dir(dir.path()).display().to_string();
    let (mut app, fake) = fake_app(dir.path(), cod2_cfg(&path));
    with_cod2(dir.path(), &fake);
    let late = run_task(set(&mut app, SettingsMsg::Open));
    let _ = set(&mut app, SettingsMsg::Cancel);
    for m in late {
        let _ = app.update(m);
    }
    assert!(app.settings_form().is_none());
    // Uno nuevo no toma la respuesta del anterior.
    let late = run_task(set(&mut app, SettingsMsg::Open));
    let again = run_task(set(&mut app, SettingsMsg::Cancel));
    assert!(again.is_empty());
    let _ = set(&mut app, SettingsMsg::Open);
    for m in late {
        let _ = app.update(m);
    }
    assert_eq!(status(&app, "cod2"), PathStatus::Searching);
}

/// Corre la `Task` con el reloj en pausa y da sus mensajes y el tiempo (virtual) que tardó.
fn timed(task: Task<Message>) -> (Vec<Message>, Duration) {
    let stream = iced_runtime::task::into_stream(task).expect("una tarea");
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(async move {
            let start = tokio::time::Instant::now();
            let msgs: Vec<Message> = stream
                .filter_map(|a| async move {
                    match a {
                        Action::Output(m) => Some(m),
                        _ => None,
                    }
                })
                .collect()
                .await;
            (msgs, start.elapsed())
        })
}

#[test]
fn settings_status_is_debounced() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let task = set(&mut app, SettingsMsg::Open);
    drive(&mut app, task);
    let initial = fake.installed_at.lock().unwrap().len();
    let mut waits = Vec::new();
    for text in ["a", "ab", "abc", "abcd"] {
        let task = set(
            &mut app,
            SettingsMsg::Path {
                game: "cod2",
                text: text.into(),
            },
        );
        assert_eq!(status(&app, "cod2"), PathStatus::Searching);
        waits.push(task);
    }
    assert_eq!(fake.installed_at.lock().unwrap().len(), initial);
    for task in waits {
        let (msgs, took) = timed(task);
        assert!(took >= PATH_DEBOUNCE, "{took:?}");
        for m in msgs {
            let task = app.update(m);
            drive(&mut app, task);
        }
    }
    let calls = fake.installed_at.lock().unwrap().clone();
    assert_eq!(calls.len(), initial + 1);
    assert_eq!(calls.last().unwrap().0, "abcd");
    assert_eq!(status(&app, "cod2"), PathStatus::Missing);
}

#[test]
fn browse_fills_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = exe_dir(dir.path()).display().to_string();
    let (mut app, fake) = fake_app(dir.path(), cod2_cfg(&path));
    with_cod2(dir.path(), &fake);
    open(&mut app);
    let other = dir.path().join("otra");
    *fake.files.folder.lock().unwrap() = Some(other.clone());
    let task = set(&mut app, SettingsMsg::Browse("cod2"));
    assert!(fake.files.calls().is_empty());
    let msgs = run_task(task);
    let calls = fake.files.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].0, calls[0].1.as_str()),
        ("folder", "Carpeta de Call of Duty 2")
    );
    assert_ne!(calls[0].2, std::thread::current().id());
    assert_eq!(fake.files.folder_starts.lock().unwrap()[0], path);
    for m in msgs {
        let task = app.update(m);
        drive(&mut app, task);
    }
    assert_eq!(form(&app).paths["cod2"], other.display().to_string());
    assert_eq!(status(&app, "cod2"), PathStatus::Missing);
    // Cancelado: nada cambia.
    let _ = set(
        &mut app,
        SettingsMsg::Browsed {
            game: "cod2",
            path: None,
        },
    );
    assert_eq!(form(&app).paths["cod2"], other.display().to_string());
}

#[test]
fn settings_accept_rerequests_card_image() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let info = mk("1.1.1.1", "a", 40, 3, "1.3");
    show(&mut app, "cod2", std::slice::from_ref(&info));
    let task = app.update(Message::Table(table::Msg::RowClicked(info.address.clone())));
    drive(&mut app, task);
    fake.maps.wait_calls(1);
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::DownloadMaps(true));
    accept(&mut app);
    fake.maps.wait_calls(2);
    let downloads: Vec<bool> = fake.maps.calls().iter().map(|c| c.3).collect();
    assert_eq!(downloads, [false, true]);
}

#[test]
fn sidebar_resorts_when_paths_change_and_keeps_selection() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.game_paths.clear();
    let (mut app, _) = fake_app(dir.path(), cfg);
    let _ = app.update(Message::SelectGame("ut99"));
    let before = app.tree.game_ids();
    open(&mut app);
    for g in jase_core::games::GAMES.iter() {
        let _ = set(
            &mut app,
            SettingsMsg::Path {
                game: g.id,
                text: String::new(),
            },
        );
    }
    let _ = set(
        &mut app,
        SettingsMsg::Path {
            game: "ut99",
            text: r"D:\UT".into(),
        },
    );
    accept(&mut app);
    let ids = app.tree.game_ids();
    assert_ne!(ids, before);
    assert_eq!(ids, App::sorted_game_ids(app.config()));
    let ut = ids.iter().position(|&i| i == "ut99").unwrap();
    assert!(ids[..ut].iter().all(|&i| {
        jase_core::games::game(i)
            .is_some_and(|g| g.launch_style == jase_core::games::LaunchStyle::Steam)
    }));
    assert_eq!(app.current_game().map(|g| g.id), Some("ut99"));
    assert_eq!(app.tree.current(), Some("ut99"));
}

#[test]
fn paths_changed_helper_runs_whole_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let generation = app.installed_generation();
    let before = fake.installed_calls().len();
    let mut paths = app.config().game_paths.clone();
    paths.insert("ut99".into(), r"D:\UT".into());
    let task = app.set_game_paths(paths);
    assert_eq!(
        load_config(&dir.path().join("config.json")).game_paths["ut99"],
        r"D:\UT"
    );
    assert_eq!(app.tree.game_ids(), App::sorted_game_ids(app.config()));
    assert_eq!(app.installed_generation(), generation + 1);
    assert_eq!(fake.installed_calls().len(), before);
    drive(&mut app, task);
    assert!(fake.installed_calls().len() > before);
}

#[test]
fn missing_path_prompt_opens_settings() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::SelectGame("cod1"));
    let _ = app.connect_selected();
    assert!(matches!(
        app.dialog(),
        Some(Dialog::Message { settings: true, .. })
    ));
    let _ = app.view();
    open(&mut app); // el botón *Abrir ajustes*
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
    let _ = set(&mut app, SettingsMsg::Cancel);
    assert!(app.dialog().is_none());
    // Enter en el aviso también (el botón de aceptar por defecto).
    let _ = app.connect_selected();
    let task = app.update(Message::Prompt(PromptMsg::Submit));
    drive(&mut app, task);
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
}

/// Abre con `old`, elige `new` y acepta: ¿avisa del reinicio?
fn restart_notice(old: &str, new: &str) -> bool {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        language: old.into(),
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::Language(new.into()));
    accept(&mut app);
    assert_eq!(app.config().language, new);
    match app.dialog() {
        Some(Dialog::LanguageRestart) => {
            let _ = app.view();
            assert!(
                view_texts(&app)
                    .iter()
                    .any(|t| t == "El idioma cambiará al reiniciar JASE.")
            );
            assert!(view_texts(&app).iter().any(|t| t == "Entendido"));
            let _ = app.update(Message::Prompt(PromptMsg::Submit));
            assert!(app.dialog().is_none());
            true
        }
        None => false,
        other => panic!("{other:?}"),
    }
}

#[test]
fn language_change_shows_restart_notice() {
    assert!(restart_notice("es", "en"));
    assert!(!restart_notice("en", "en"));
}

#[test]
fn restart_notice_compares_resolved_language() {
    // El aviso compara lo resuelto con el idioma del sistema (el de esta máquina).
    let system = system_locale_name();
    for (old, new) in [
        ("auto", "en"),
        ("auto", "es"),
        ("en", "auto"),
        ("es", "auto"),
    ] {
        let expected = resolve_language(old, &system) != resolve_language(new, &system);
        assert_eq!(
            restart_notice(old, new),
            expected,
            "{old} → {new} ({system})"
        );
    }
}

#[test]
fn settings_appearance_tab_applies_theme() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        theme: "dark".into(),
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    open(&mut app);
    assert_eq!(form(&app).theme, "dark");
    let _ = set(&mut app, SettingsMsg::Tab(Tab::Appearance));
    let _ = app.view();
    let _ = set(&mut app, SettingsMsg::Theme("classic".into()));
    assert_eq!(app.theme_id, "dark"); // se aplica al guardar
    accept(&mut app);
    assert_eq!(app.theme_id, "classic");
    assert_eq!(
        load_config(&dir.path().join("config.json")).theme,
        "classic"
    );
}

/// Pestaña Barra: ocultar, mover y agregar un separador se guardan y cambian la barra; Restablecer
/// vuelve a la de fábrica (y la config queda sin `toolbar`).
#[test]
fn settings_toolbar_tab_reorders_and_saves() {
    use jase::actions::Action as A;
    use jase::bars::{ToolItem::Button as B, ToolItem::Separator as S, visible_toolbar};
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::Tab(Tab::Toolbar));
    let _ = app.view();
    let _ = set(&mut app, SettingsMsg::ToolbarCheck(1, false)); // Detener
    let _ = set(&mut app, SettingsMsg::ToolbarSelect(0)); // Refrescar
    let _ = set(&mut app, SettingsMsg::ToolbarMove(true)); // ya es el primero: nada
    let _ = set(&mut app, SettingsMsg::ToolbarMove(false));
    assert_eq!(form(&app).toolbar_selected, Some(1));
    let _ = set(&mut app, SettingsMsg::ToolbarSeparator);
    assert_eq!(form(&app).toolbar[2], (S, true));
    accept(&mut app);
    let shown = visible_toolbar(&jase::bars::toolbar_layout(&app.config().toolbar));
    assert_eq!(
        shown[..5],
        [B(A::Refresh), S, B(A::RefreshSelected), S, B(A::Connect)]
    );
    let saved = load_config(&dir.path().join("config.json")).toolbar;
    assert_eq!(
        saved[..2],
        [("stop".to_owned(), false), ("refresh".to_owned(), true)]
    );
    let _ = app.view();
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::ToolbarReset);
    accept(&mut app);
    assert!(app.config().toolbar.is_empty());
    assert!(
        load_config(&dir.path().join("config.json"))
            .toolbar
            .is_empty()
    );
}

/// Pestaña Avisos: las casillas y quitar un umbral se guardan al aceptar.
#[test]
fn settings_notifications_tab_saves() {
    let dir = tempfile::tempdir().unwrap();
    let mut alerts = indexmap::IndexMap::new();
    alerts.insert("1.1.1.1:28960".to_owned(), 4);
    alerts.insert("2.2.2.2:28960".to_owned(), 6);
    let mut party = indexmap::IndexMap::new();
    party.insert("cod2".to_owned(), alerts);
    let cfg = Config {
        party_alerts: party,
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::Tab(Tab::Notifications));
    assert!(view_texts(&app).iter().any(|t| t.contains("2.2.2.2:28960")));
    let _ = set(&mut app, SettingsMsg::NotifyFriends(false));
    let _ = set(&mut app, SettingsMsg::WatchFriends(true));
    let _ = set(&mut app, SettingsMsg::NotifyAttention(false));
    let _ = set(
        &mut app,
        SettingsMsg::RemovePartyAlert("cod2".into(), "1.1.1.1:28960".into()),
    );
    accept(&mut app);
    let saved = load_config(&dir.path().join("config.json"));
    assert!(!saved.notify_friends && saved.watch_friends && !saved.notify_attention);
    assert!(saved.notify_party);
    assert_eq!(saved.party_alerts["cod2"].len(), 1);
    assert_eq!(saved.party_alerts["cod2"]["2.2.2.2:28960"], 6);
}

#[test]
fn settings_theme_swatches_use_theme_colors() {
    let dark = palette("dark");
    assert_eq!(
        swatch_colors("dark"),
        [dark.bg, dark.surface, dark.accent, dark.selection]
    );
    let phosphors: Vec<_> = TERMINAL_COLORS
        .iter()
        .map(|c| palette(&format!("terminal_{c}")).selection)
        .collect();
    assert_eq!(swatch_colors("terminal"), phosphors);
    let light = palette("light");
    assert_eq!(
        swatch_colors("system"),
        [dark.bg, dark.accent, light.bg, light.accent]
    );
}

// ---------------------------------------------------------------------- Steam y países

fn l4d2_app(dir: &Path, key: &str) -> (TestApp, FakeScan) {
    let (mut app, fake, _) = fake_app_with_key(dir, Config::default(), key);
    let _ = app.update(Message::SteamKey(key.into()));
    show(&mut app, "l4d2", &[]);
    (app, fake)
}

#[test]
fn settings_saves_new_steam_key_and_refreshes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = l4d2_app(dir.path(), "OLD");
    assert_eq!(fake.steam_keys().last().map(String::as_str), Some("OLD"));
    open(&mut app);
    assert_eq!(form(&app).steam_key, "OLD");
    let _ = set(&mut app, SettingsMsg::SteamKey(" K ".into()));
    let task = set(&mut app, SettingsMsg::Accept);
    assert_eq!(app.steam_key(), Some("K"));
    assert_eq!(fake.steam_keys().last().map(String::as_str), Some("K"));
    let saved = || {
        fake.keyring
            .entries
            .lock()
            .unwrap()
            .get(STEAM_KEY_USER)
            .cloned()
    };
    assert_eq!(saved().as_deref(), Some("OLD")); // aún no: va en la `Task`
    drive(&mut app, task);
    assert_eq!(saved().as_deref(), Some("K"));
}

#[test]
fn settings_steam_key_keyring_error_uses_key_in_session() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = l4d2_app(dir.path(), "");
    *fake.keyring.set_error.lock().unwrap() = Some("bloqueado".into());
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::SteamKey("NEW".into()));
    accept(&mut app);
    assert_eq!(app.steam_key(), Some("NEW"));
    let Some(Dialog::Message { title, text, .. }) = app.dialog() else {
        panic!("{:?}", app.dialog())
    };
    assert_eq!(title, "Clave de API de Steam");
    assert!(text.starts_with("No se pudo guardar la clave de Steam: bloqueado"));
}

#[test]
fn settings_key_field_is_password_mode() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::SteamKey("SECRETKEY".into()));
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::Tab(Tab::General));
    let texts = view_texts(&app);
    assert!(texts.iter().any(|t| t == "Clave de API de Steam"));
    assert!(texts.iter().all(|t| !t.contains("SECRETKEY")));
    assert!(texts.iter().any(|t| t == &"•".repeat(9)));
    assert_eq!(form(&app).steam_key(), "SECRETKEY");
}

#[test]
fn settings_key_link_opens_externally() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    open(&mut app);
    let task = set(&mut app, SettingsMsg::OpenSteamKeyPage);
    assert!(fake.opened.lock().unwrap().is_empty());
    drive(&mut app, task);
    let opened = fake.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].0, "https://steamcommunity.com/dev/apikey");
    assert_eq!(STEAM_KEY_URL, opened[0].0);
    assert_ne!(opened[0].1, std::thread::current().id());
}

#[test]
fn settings_include_empty_clears_only_steam_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _) = fake_app_with_key(dir.path(), Config::default(), "K");
    let _ = app.update(Message::SteamKey("K".into()));
    show(&mut app, "tf2", &[]);
    show(&mut app, "cod2", &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    app.saved.flush();
    assert!(app.saved.index.contains_key("tf2") && app.saved.index.contains_key("cod2"));
    let tf2_before = app.game_label("tf2");
    assert_eq!(tf2_before, "TF2 · 0");
    let scans = fake.calls().len();
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::SteamIncludeEmpty(true));
    accept(&mut app);
    assert!(app.config().steam_include_empty);
    assert!(!app.saved.index.contains_key("tf2"));
    assert!(app.saved.index.contains_key("cod2"));
    assert_eq!(app.game_label("tf2"), "TF2 · —");
    assert_eq!(app.game_label("cod2"), "Call of Duty 2 · 1");
    // cod2 no es de Steam: no se vuelve a listar.
    assert_eq!(fake.calls().len(), scans);
    app.saved.flush();
    assert!(!jase_core::server_cache::read_index(&app.saved.dir).contains_key("tf2"));
}

type GeoCalls = Arc<std::sync::Mutex<Vec<(PathBuf, bool)>>>;

fn geo_app(dir: &Path) -> (TestApp, GeoCalls) {
    let lang = support::lang::Spanish::hold();
    let csv = dir.join("fake.csv");
    std::fs::write(&csv, "1.1.1.0,1.1.1.255,BR\n").unwrap();
    let calls: GeoCalls = Arc::default();
    let seen = calls.clone();
    let fake = FakeScan::default();
    let mut deps = support::fake_deps(&fake, "");
    deps.country_db = Some(Arc::new(move |path: &Path, refresh| {
        seen.lock().unwrap().push((path.to_owned(), refresh));
        CountryDB::from_csv(&csv).map_err(|e| e.to_string())
    }));
    let (app, task) = App::new(Config::default(), dir.join("config.json"), deps);
    drop(lang);
    let mut app = TestApp::new(app);
    drive(&mut app, task);
    (app, calls)
}

#[test]
fn settings_button_refreshes_country_db() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, calls) = geo_app(dir.path());
    assert_eq!(calls.lock().unwrap().len(), 1);
    open(&mut app);
    assert_eq!(form(&app).countries, CountryButton::Ready);
    let task = set(&mut app, SettingsMsg::UpdateCountries);
    assert_eq!(form(&app).countries, CountryButton::Downloading);
    assert!(app.country_busy());
    assert_eq!(app.status(), "Descargando base de países…");
    let _ = set(&mut app, SettingsMsg::Cancel);
    let msgs = run_task(task);
    let expected = (dir.path().join("cache").join("dbip-country-lite.csv"), true);
    assert_eq!(calls.lock().unwrap().last(), Some(&expected));
    for m in msgs {
        let _ = app.update(m);
    }
    assert!(!app.country_busy());
    assert_eq!(app.status(), "Base de países actualizada");
}

#[test]
fn settings_country_button_disabled_without_geo() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut app, _) = fake_app(dir.path(), Config::default());
        open(&mut app);
        assert_eq!(form(&app).countries, CountryButton::Disabled);
        assert!(set(&mut app, SettingsMsg::UpdateCountries).units() == 0);
    }
    // Con una descarga en curso, también.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = geo_app(dir.path());
    open(&mut app);
    let _task = set(&mut app, SettingsMsg::UpdateCountries);
    let _ = set(&mut app, SettingsMsg::Cancel);
    open(&mut app);
    assert_eq!(form(&app).countries, CountryButton::Disabled);
}

// ----------------------------------------------------------------- hilos y vistas

#[test]
fn settings_io_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let path = exe_dir(dir.path()).display().to_string();
    let (mut app, fake) = fake_app(dir.path(), cod2_cfg(&path));
    with_cod2(dir.path(), &fake);
    fake.installed_at.lock().unwrap().clear();
    *fake.files.folder.lock().unwrap() = Some(dir.path().join("x"));
    let mut tasks = Vec::new();
    let mut step = |app: &mut App, m: SettingsMsg| {
        tasks.push(set(app, m));
        let _ = app.view();
        assert!(fake.installed_at.lock().unwrap().is_empty());
        assert!(fake.files.calls().is_empty());
        assert!(
            fake.keyring
                .calls()
                .iter()
                .all(|(u, _)| u != STEAM_KEY_USER)
        );
        assert!(fake.opened.lock().unwrap().is_empty());
        assert!(fake.probes.lock().unwrap().is_empty());
    };
    step(&mut app, SettingsMsg::Open);
    step(
        &mut app,
        SettingsMsg::Path {
            game: "cod2",
            text: "abc".into(),
        },
    );
    step(&mut app, SettingsMsg::Browse("cod2"));
    step(&mut app, SettingsMsg::OpenSteamKeyPage);
    step(&mut app, SettingsMsg::Masters(MastersMsg::Select(0)));
    step(&mut app, SettingsMsg::Masters(MastersMsg::Probe));
    step(&mut app, SettingsMsg::SteamKey("K".into()));
    step(&mut app, SettingsMsg::Accept);
    let ui = std::thread::current().id();
    for task in tasks {
        for m in run_task(task) {
            let _ = app.update(m);
        }
    }
    let threads = |v: Vec<std::thread::ThreadId>| !v.is_empty() && v.iter().all(|t| *t != ui);
    assert!(threads(
        fake.installed_at
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.1)
            .collect()
    ));
    assert!(threads(fake.files.calls().iter().map(|c| c.2).collect()));
    assert!(threads(
        fake.opened.lock().unwrap().iter().map(|c| c.1).collect()
    ));
    let keyring: Vec<_> = fake
        .keyring
        .calls()
        .into_iter()
        .filter(|(u, _)| u == STEAM_KEY_USER)
        .map(|(_, t)| t)
        .collect();
    assert!(threads(keyring));
    assert_eq!(fake.probes.lock().unwrap().len(), 1);
}

#[test]
fn settings_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    open(&mut app);
    let views = |app: &mut App| {
        for (id, _, _) in VARIANTS.iter() {
            app.theme_id = id;
            let _ = view_texts(app);
        }
    };
    for tab in [
        Tab::Games,
        Tab::Masters,
        Tab::General,
        Tab::Appearance,
        Tab::Toolbar,
        Tab::Notifications,
    ] {
        let _ = set(&mut app, SettingsMsg::Tab(tab));
        views(&mut app);
    }
    let _ = set(&mut app, SettingsMsg::Theme("terminal".into()));
    views(&mut app);
    let _ = set(&mut app, SettingsMsg::Tab(Tab::Masters));
    let _ = masters(&mut app, MastersMsg::Select(0));
    let _ = masters(&mut app, MastersMsg::Add);
    views(&mut app);
    let _ = masters(&mut app, MastersMsg::AddSubmit);
    assert!(form(&app).masters.warning.is_some());
    views(&mut app);
}

#[test]
fn late_keyring_answer_with_settings_open_keeps_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _) = fake_app_with_key(dir.path(), Config::default(), "K");
    open(&mut app); // el llavero aún no respondió: el campo abre vacío
    assert_eq!(form(&app).steam_key, "");
    let _ = app.update(Message::SteamKey("K".into()));
    let before = fake.keyring.calls().len();
    accept(&mut app);
    assert_eq!(fake.keyring.calls().len(), before);
    assert_eq!(app.steam_key(), Some("K"));
    let saved = fake
        .keyring
        .entries
        .lock()
        .unwrap()
        .get(STEAM_KEY_USER)
        .cloned();
    assert_eq!(saved.as_deref(), Some("K"));
}

#[test]
fn probe_uses_the_key_settings_opened_with() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _) = fake_app_with_key(dir.path(), Config::default(), "");
    let _ = app.update(Message::SteamKey("K".into()));
    open(&mut app);
    let _ = set(&mut app, SettingsMsg::SteamKey("OTRA".into()));
    let _ = masters(&mut app, MastersMsg::Game("l4d2"));
    let _ = masters(&mut app, MastersMsg::Select(0));
    let task = masters(&mut app, MastersMsg::Probe);
    drive(&mut app, task);
    let probes = fake.probes.lock().unwrap().clone();
    assert_eq!(probes[0].2, "K");
}

#[test]
fn browse_allows_one_picker_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    open(&mut app);
    let first = set(&mut app, SettingsMsg::Browse("cod2"));
    assert!(app.files_open());
    let second = set(&mut app, SettingsMsg::Browse("cod1"));
    assert_eq!(second.units(), 0);
    let _ = app.view();
    for m in run_task(first) {
        let _ = app.update(m); // cancelado (`None`): vuelve a encenderse
    }
    assert_eq!(fake.files.calls().len(), 1);
    assert!(!app.files_open());
    // Con Ajustes ya cerrado también se enciende.
    let task = set(&mut app, SettingsMsg::Browse("cod2"));
    let _ = set(&mut app, SettingsMsg::Cancel);
    for m in run_task(task) {
        let _ = app.update(m);
    }
    assert!(!app.files_open());
}
