//! *Buscar juegos* y el ofrecimiento del primer arranque (`tests/test_find_games.py` y
//! `tests/test_main_window.py`): la búsqueda va por el `find_games` falso y corre al ejecutar
//! la `Task`, nunca en `update`.

mod support;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::Task;
use indexmap::IndexMap;
use jase::actions::{Action, Msg as ActionMsg, enabled};
use jase::app::{App, Dialog, Message, PromptMsg, SettingsMsg};
use jase::find_games::{RowState, Search, option_label, source_label};
use jase::theme::VARIANTS;
use jase_core::config::{Config, load_config};
use jase_core::detect::Finding;
use jase_core::paths::normpath;
use support::{FakeScan, Found, TestApp, fake_app, run_task, view_texts};

fn set(app: &mut App, m: SettingsMsg) -> Task<Message> {
    app.update(Message::Settings(m))
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

fn deliver(app: &mut App, msgs: Vec<Message>) {
    for m in msgs {
        let task = app.update(m);
        drive(app, task);
    }
}

fn finding(game: &'static str, path: &str, source: &'static str) -> Finding {
    Finding {
        game_id: game,
        path: path.to_owned(),
        source,
    }
}

fn found(list: &[Finding]) -> Found {
    let mut out = Found::new();
    for f in list {
        out.entry(f.game_id.to_owned()).or_default().push(f.clone());
    }
    out
}

fn no_paths() -> Config {
    let mut cfg = Config::default();
    cfg.game_paths.clear();
    cfg
}

/// *Buscar juegos…* del menú, sin correr su `Task`.
fn find_from_menu(app: &mut App) -> Task<Message> {
    app.update(Message::Actions(ActionMsg::Run(Action::FindGames)))
}

fn search(app: &App) -> &Search {
    app.find_search().expect("Buscar juegos abierto")
}

/// Abre *Buscar juegos* desde el menú y deja llegar el resultado.
fn open_find(app: &mut App) {
    let task = find_from_menu(app);
    drive(app, task);
}

fn rows(app: &App) -> Vec<&'static str> {
    search(app).rows.iter().map(|r| r.game).collect()
}

fn expected(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(g, p)| ((*g).to_owned(), (*p).to_owned()))
        .collect()
}

/// Una carpeta con el exe de cod2 que el `game_installed` falso da por buena.
fn cod2_dir(dir: &Path, fake: &FakeScan) -> String {
    let d = dir.join("g");
    std::fs::create_dir_all(&d).unwrap();
    let exe = d.join("CoD2MP_s.exe");
    std::fs::write(&exe, b"").unwrap();
    fake.exes
        .lock()
        .unwrap()
        .insert("cod2", exe.display().to_string());
    d.display().to_string()
}

fn cfg_path(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

#[test]
fn dialog_rows_and_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    let ok = cod2_dir(dir.path(), &fake);
    let gone = dir.path().join("gone").display().to_string();
    let paths = [("cod2", ok.as_str()), ("cod1", &gone), ("cod4", "")];
    app.config_mut().game_paths = expected(&paths);
    fake.find.set(
        None,
        found(&[
            finding("cod2", &ok, "registro"),
            finding("cod1", "X:\\CoD", "steam"),
            finding("cod4", "X:\\CoD4", "registro"),
        ]),
    );
    open_find(&mut app);
    let s = search(&app);
    let by_game: HashMap<_, _> = s.rows.iter().map(|r| (r.game, r)).collect();
    let states: HashMap<_, _> = by_game.iter().map(|(g, r)| (*g, r.state)).collect();
    assert_eq!(
        states,
        HashMap::from([
            ("cod2", RowState::Configured),
            ("cod1", RowState::SavedMissing),
            ("cod4", RowState::New),
        ])
    );
    let texts: HashMap<_, _> = by_game.iter().map(|(g, r)| (*g, r.state.text())).collect();
    assert_eq!(texts["cod2"], "ya configurado");
    assert_eq!(texts["cod1"], "ruta guardada no encontrada");
    assert_eq!(texts["cod4"], "nuevo");
    let checked: HashMap<_, _> = by_game.iter().map(|(g, r)| (*g, r.checked)).collect();
    assert_eq!(
        checked,
        HashMap::from([("cod2", false), ("cod1", true), ("cod4", true)])
    );
    assert_eq!(
        s.selected_paths(),
        expected(&[("cod1", "X:\\CoD"), ("cod4", "X:\\CoD4")])
    );
    // Las filas en el orden de `GAMES`.
    assert_eq!(rows(&app), ["cod2", "cod1", "cod4"]);
}

#[test]
fn dialog_keeps_valid_saved_path_unchecked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    let ok = cod2_dir(dir.path(), &fake);
    app.config_mut().game_paths = expected(&[("cod2", &ok)]);
    fake.find
        .set(None, found(&[finding("cod2", "Z:\\other", "steam")]));
    open_find(&mut app);
    assert!(!search(&app).selected_paths().contains_key("cod2"));
    let task = set(&mut app, SettingsMsg::FindApply);
    drive(&mut app, task);
    assert_eq!(app.config().game_paths, expected(&[("cod2", &ok)]));
    assert!(app.find_search().is_none());
    assert_eq!(app.dialog(), None);
}

#[test]
fn dialog_multiple_paths_combo_ordered_by_source() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    fake.find.set(
        None,
        found(&[
            finding("cod2", "A", "registro"),
            finding("cod2", "B", "steam"),
        ]),
    );
    open_find(&mut app);
    let labels: Vec<String> = search(&app).rows[0]
        .findings
        .iter()
        .map(option_label)
        .collect();
    assert_eq!(labels, ["A (registro)", "B (steam)"]);
    let _ = set(&mut app, SettingsMsg::FindChoose("cod2", 1));
    assert_eq!(search(&app).selected_paths()["cod2"], "B");
}

#[test]
fn source_labels_are_translated() {
    let _lang = support::lang::Spanish::hold();
    for s in ["registro", "steam", "carpeta típica", "carpeta elegida"] {
        assert_eq!(source_label(s), s);
    }
    assert_eq!(source_label("otro"), "otro");
}

#[test]
fn stale_search_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    fake.find
        .set(None, found(&[finding("cod2", "OLD", "registro")]));
    fake.find.set(
        Some("D:\\Juegos"),
        found(&[finding("cod1", "NEW", "carpeta elegida")]),
    );
    fake.find.hold();
    let first = find_from_menu(&mut app);
    let first = std::thread::spawn(move || run_task(first));
    fake.find.wait_calls(1);
    *fake.files.folder.lock().unwrap() = Some(PathBuf::from("D:\\Juegos"));
    let task = set(&mut app, SettingsMsg::FindInFolder);
    let calls = fake.files.calls();
    assert!(calls.is_empty());
    deliver(&mut app, run_task(task));
    assert_eq!(fake.files.calls()[0].1, "Buscar juegos en la carpeta");
    assert_eq!(rows(&app), ["cod1"]);
    fake.find.release();
    deliver(&mut app, first.join().unwrap());
    assert_eq!(rows(&app), ["cod1"]);
    assert_eq!(fake.find.extras(), [None, Some("D:\\Juegos".to_owned())]);
}

#[test]
fn cancel_search_discards_result() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    fake.find
        .set(None, found(&[finding("cod2", "P", "registro")]));
    fake.find.hold();
    let task = find_from_menu(&mut app);
    let pending = std::thread::spawn(move || run_task(task));
    assert_eq!(search(&app).status, "Buscando…");
    assert!(search(&app).searching);
    let _ = set(&mut app, SettingsMsg::FindCancelSearch);
    fake.find.release();
    deliver(&mut app, pending.join().unwrap());
    assert!(search(&app).rows.is_empty());
    assert_eq!(search(&app).status, "Búsqueda cancelada");
    assert!(!search(&app).searching);
}

#[test]
fn settings_find_games_fills_edits_without_saving() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    fake.find
        .set(None, found(&[finding("cod1", "X:\\CoD", "registro")]));
    let task = set(&mut app, SettingsMsg::Open);
    drive(&mut app, task);
    let task = set(&mut app, SettingsMsg::FindGames);
    drive(&mut app, task);
    assert!(search(&app).from_settings);
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
    let task = set(&mut app, SettingsMsg::FindApply);
    drive(&mut app, task);
    assert!(app.find_search().is_none());
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
    let form = app.settings_form().unwrap();
    assert_eq!(form.paths["cod1"], "X:\\CoD");
    assert_ne!(
        app.config().game_paths.get("cod1").map(String::as_str),
        Some("X:\\CoD")
    );
    let task = set(&mut app, SettingsMsg::Accept);
    drive(&mut app, task);
    assert_eq!(app.config().game_paths["cod1"], "X:\\CoD");
}

#[test]
fn settings_find_games_esc_closes_only_the_search() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), no_paths());
    let task = set(&mut app, SettingsMsg::Open);
    drive(&mut app, task);
    let task = set(&mut app, SettingsMsg::FindGames);
    drive(&mut app, task);
    let _ = app.update(Message::Escape);
    assert!(app.find_search().is_none());
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
}

#[test]
fn folder_choice_is_normalized() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    open_find(&mut app);
    *fake.files.folder.lock().unwrap() = Some(PathBuf::from("/a//b/../c/"));
    let task = set(&mut app, SettingsMsg::FindInFolder);
    drive(&mut app, task);
    assert_eq!(fake.find.extras(), [None, Some(normpath("/a//b/../c/"))]);
}

#[test]
fn dialog_found_count_singular_and_plural() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    open_find(&mut app);
    assert_eq!(search(&app).status, "No se encontraron juegos");
    let _ = set(&mut app, SettingsMsg::FindClose);
    let one = [finding("cod2", "A", "registro")];
    fake.find.set(None, found(&one));
    open_find(&mut app);
    assert_eq!(search(&app).status, "1 juego encontrado");
    let _ = set(&mut app, SettingsMsg::FindClose);
    let two = [one[0].clone(), finding("cod1", "B", "steam")];
    fake.find.set(None, found(&two));
    open_find(&mut app);
    assert_eq!(search(&app).status, "2 juegos encontrados");
}

#[test]
fn find_games_action_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fake_app(dir.path(), Config::default());
    assert!(enabled(&app, Action::FindGames));
}

#[test]
fn main_find_games_applies_and_resorts() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    let g = cod2_dir(dir.path(), &fake);
    fake.find.set(None, found(&[finding("cod2", &g, "steam")]));
    let generation = app.installed_generation();
    open_find(&mut app);
    assert_eq!(app.dialog(), Some(&Dialog::FindGames));
    let task = set(&mut app, SettingsMsg::FindApply);
    drive(&mut app, task);
    assert_eq!(app.config().game_paths["cod2"], g);
    assert_eq!(app.tree.game_ids()[0], "cod2");
    assert_eq!(app.installed_generation(), generation + 1);
    assert_eq!(load_config(&cfg_path(dir.path())).game_paths["cod2"], g);
    assert_eq!(app.dialog(), None);
}

#[test]
fn enter_applies_and_esc_closes_menu_search() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    fake.find
        .set(None, found(&[finding("cod1", "X:\\CoD", "steam")]));
    open_find(&mut app);
    let _ = app.update(Message::Escape);
    assert_eq!(app.dialog(), None);
    assert!(app.find_search().is_none());
    assert!(!app.config().game_paths.contains_key("cod1"));
    open_find(&mut app);
    let task = app.update(Message::Prompt(PromptMsg::Submit));
    drive(&mut app, task);
    assert_eq!(app.config().game_paths["cod1"], "X:\\CoD");
}

#[test]
fn find_games_runs_in_the_task() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    let ok = cod2_dir(dir.path(), &fake);
    app.config_mut().game_paths = expected(&[("cod2", &ok)]);
    fake.find
        .set(None, found(&[finding("cod2", &ok, "registro")]));
    fake.installed_at.lock().unwrap().clear();
    let task = find_from_menu(&mut app);
    assert!(fake.find.calls().is_empty());
    assert!(fake.installed_at.lock().unwrap().is_empty());
    let msgs = run_task(task);
    let me = std::thread::current().id();
    let calls = fake.find.calls();
    assert_eq!(calls.len(), 1);
    assert_ne!(calls[0].1, me);
    let installed = fake.installed_at.lock().unwrap().clone();
    assert!(!installed.is_empty() && installed.iter().all(|(_, t)| *t != me));
    deliver(&mut app, msgs);
    assert_eq!(search(&app).rows[0].state, RowState::Configured);
    // El ofrecimiento también pregunta por los exes en la `Task`.
    app.config_mut().find_games_offered = false;
    let _ = app.update(Message::CloseDialog);
    fake.installed_at.lock().unwrap().clear();
    let task = app.offer_find_games();
    assert!(fake.installed_at.lock().unwrap().is_empty());
    let _ = run_task(task);
    assert!(!fake.installed_at.lock().unwrap().is_empty());
}

// ------------------------------------------------------------------- primer arranque

fn offer(app: &mut App) {
    let task = app.offer_find_games();
    drive(app, task);
}

#[test]
fn first_run_offer_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let (mut first, _) = fake_app(dir.path(), no_paths());
    offer(&mut first);
    assert_eq!(first.dialog(), Some(&Dialog::OfferFindGames));
    let texts = view_texts(&first);
    assert!(
        texts
            .iter()
            .any(|t| t == "No hay juegos configurados. ¿Buscar juegos instalados?")
    );
    let buttons: Vec<&String> = texts
        .iter()
        .filter(|t| *t == "Buscar juegos" || *t == "Ahora no")
        .collect();
    assert_eq!(buttons, ["Buscar juegos", "Buscar juegos", "Ahora no"]); // título y botones
    assert!(first.config().find_games_offered);
    let saved = load_config(&cfg_path(dir.path()));
    assert!(saved.find_games_offered);
    drop(first);
    let (mut second, _) = fake_app(dir.path(), saved);
    let task = second.offer_find_games();
    assert!(iced_runtime::task::into_stream(task).is_none());
    assert_eq!(second.dialog(), None);
}

#[test]
fn no_offer_when_a_game_is_configured() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    let ok = cod2_dir(dir.path(), &fake);
    app.config_mut().game_paths = expected(&[("cod2", &ok)]);
    offer(&mut app);
    assert_eq!(app.dialog(), None);
    assert!(app.config().find_games_offered); // se marca igual
    assert!(load_config(&cfg_path(dir.path())).find_games_offered);
}

#[test]
fn offer_waits_when_a_dialog_is_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), no_paths());
    let _ = app.update(Message::Actions(ActionMsg::Run(Action::About)));
    offer(&mut app);
    assert_eq!(app.dialog(), Some(&Dialog::About));
    assert!(!app.config().find_games_offered);
}

#[test]
fn first_run_accept_finds_and_saves() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    let g = cod2_dir(dir.path(), &fake);
    fake.find.set(None, found(&[finding("cod2", &g, "steam")]));
    offer(&mut app);
    // Enter = *Buscar juegos*.
    let task = app.update(Message::Prompt(PromptMsg::Submit));
    drive(&mut app, task);
    assert_eq!(app.dialog(), Some(&Dialog::FindGames));
    let task = set(&mut app, SettingsMsg::FindApply);
    drive(&mut app, task);
    assert_eq!(app.config().game_paths["cod2"], g);
    assert_eq!(load_config(&cfg_path(dir.path())).game_paths["cod2"], g);
}

#[test]
fn first_run_decline_does_not_search() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    offer(&mut app);
    let task = set(&mut app, SettingsMsg::OfferDecline);
    drive(&mut app, task);
    assert_eq!(app.dialog(), None);
    assert!(fake.find.calls().is_empty());
    // Esc también es *Ahora no*.
    let (mut app, fake): (TestApp, FakeScan) = fake_app(dir.path(), no_paths());
    offer(&mut app);
    let _ = app.update(Message::Escape);
    assert_eq!(app.dialog(), None);
    assert!(fake.find.calls().is_empty());
}

#[test]
fn find_dialog_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), no_paths());
    fake.find.set(
        None,
        found(&[
            finding("cod2", "A", "registro"),
            finding("cod2", "B", "steam"),
            finding("cod1", "C", "carpeta típica"),
        ]),
    );
    let views = |app: &mut App, want: &str| {
        for (id, _, _) in VARIANTS.iter() {
            app.theme_id = id;
            let texts = view_texts(app);
            assert!(texts.iter().any(|t| t == want), "{id}: {texts:?}");
        }
    };
    open_find(&mut app);
    views(&mut app, "nuevo");
    let _ = set(&mut app, SettingsMsg::FindClose);
    let task = set(&mut app, SettingsMsg::Open);
    drive(&mut app, task);
    let task = set(&mut app, SettingsMsg::FindGames);
    drive(&mut app, task);
    views(&mut app, "Buscar en carpeta…");
    let _ = set(&mut app, SettingsMsg::FindClose);
    let _ = set(&mut app, SettingsMsg::Cancel);
    let (mut app, _) = fake_app(dir.path(), no_paths());
    offer(&mut app);
    views(&mut app, "Ahora no");
}
