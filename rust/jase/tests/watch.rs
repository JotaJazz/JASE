//! *Avisar cuando haya sitio*, *Avisar cuando se arme partida* y la revisión de fondo
//! (`tests/test_slot_watch.py`, `test_party.py`, `test_friend_watch.py` y lo suyo de
//! `test_main_window.py`). Ni consultas reales ni ventanas: todo con el escáner falso.

mod support;

use std::path::Path;

use iced::Task;
use iced::advanced::subscription::into_recipes;
use jase::actions::{self, Action, enabled};
use jase::app::{App, Dialog, DirectMsg, DirectView, LinkMsg, Message, PromptMsg, SettingsMsg};
use jase::app::{SLOT_INTERVAL, SLOT_MAX, WATCH_INTERVAL, WatchMsg, has_free_slot, party_value};
use jase::scan::ScanMsg;
use jase::theme::VARIANTS;
use jase_core::config::{Config, load_config};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{FakeScan, TestApp, fake_app, mk, run_task, view_texts, window_actions};

fn addr(host: &str) -> ServerAddress {
    ServerAddress::new(host, 28960)
}

/// `host` con `players` de 20.
fn srv(host: &str, players: i32) -> ServerInfo {
    mk(host, &format!("srv {host}"), 40, players, "1.3")
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
    let _ = app.update(Message::Table(jase::table::Msg::RowClicked(addr(host))));
}

fn run(app: &mut App, a: Action) -> Task<Message> {
    app.update(Message::Actions(actions::Msg::Run(a)))
}

fn watch(app: &mut App, m: WatchMsg) -> Task<Message> {
    app.update(Message::Watch(m))
}

fn feed(app: &mut App, messages: Vec<Message>) {
    for m in messages {
        let _ = app.update(m);
    }
}

/// Un `SlotTick` y su respuesta, sin entregarla.
fn tick(app: &mut App) -> Vec<Message> {
    run_task(watch(app, WatchMsg::SlotTick))
}

/// cod2 (con ruta) con 1.1.1.1 lleno (20/20) y 2.2.2.2 con sitio; el lleno, elegido.
fn cod2_app(dir: &Path, mut cfg: Config) -> (TestApp, FakeScan) {
    cfg.game_paths.insert("cod2".into(), "C:/cod2".into());
    let (mut app, fake) = fake_app(dir, cfg);
    complete(&mut app, &[srv("1.1.1.1", 20), srv("2.2.2.2", 3)]);
    select(&mut app, "1.1.1.1");
    (app, fake)
}

/// Empieza a vigilar el elegido (la `Task` del plazo de 30 min no se corre).
fn start_watch(app: &mut App) {
    let _ = run(app, Action::WatchSlot);
    assert!(app.watch_label().is_some());
}

fn slot_text(app: &App) -> Option<String> {
    match app.dialog() {
        Some(Dialog::SlotFree { text }) => Some(text.clone()),
        _ => None,
    }
}

#[test]
fn has_free_slot_rules() {
    assert!(has_free_slot(&srv("1.1.1.1", 19)));
    assert!(!has_free_slot(&srv("1.1.1.1", 20)));
    let mut zero = srv("1.1.1.1", 0);
    zero.max_players = 0;
    assert!(!has_free_slot(&zero));
    let mut offline = srv("1.1.1.1", 1);
    offline.online = false;
    assert!(!has_free_slot(&offline));
    assert_eq!(SLOT_INTERVAL.as_secs(), 10);
    assert_eq!(SLOT_MAX.as_secs(), 30 * 60);
    assert_eq!(WATCH_INTERVAL.as_secs(), 120);
}

#[test]
fn slot_free_once_on_third_answer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    start_watch(&mut app);
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 20)));
    for _ in 0..2 {
        let answers = tick(&mut app);
        feed(&mut app, answers);
        assert!(app.dialog().is_none());
        assert!(app.watch_label().is_some());
    }
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 19)));
    let answers = tick(&mut app);
    feed(&mut app, answers.clone());
    assert_eq!(
        slot_text(&app).as_deref(),
        Some("Hay sitio en srv 1.1.1.1 (19/20). ¿Conectar ahora?")
    );
    assert!(app.watch_label().is_none());
    // La cuarta (ni el tic ni la respuesta repetida) no hace nada.
    assert!(tick(&mut app).is_empty());
    let _ = app.update(Message::Prompt(PromptMsg::Cancel));
    feed(&mut app, answers);
    assert!(app.dialog().is_none());
    assert_eq!(fake.queries().len(), 3);
}

#[test]
fn no_answer_never_frees() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    start_watch(&mut app);
    for _ in 0..5 {
        let answers = tick(&mut app);
        feed(&mut app, answers);
    }
    assert_eq!(fake.queries().len(), 5);
    assert!(app.dialog().is_none() && app.watch_label().is_some());
}

#[test]
fn zero_max_players_never_frees() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    start_watch(&mut app);
    let mut zero = srv("1.1.1.1", 0);
    zero.max_players = 0;
    fake.answer_for(&addr("1.1.1.1"), Some(zero));
    for _ in 0..3 {
        let answers = tick(&mut app);
        feed(&mut app, answers);
    }
    assert!(app.dialog().is_none() && app.watch_label().is_some());
}

#[test]
fn slot_expires() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = cod2_app(dir.path(), Config::default());
    // El reloj de los tests está en pausa: el plazo vence al instante.
    let expiry = run_task(run(&mut app, Action::WatchSlot));
    assert!(matches!(
        expiry.as_slice(),
        [Message::Watch(WatchMsg::SlotExpired(_))]
    ));
    feed(&mut app, expiry);
    assert_eq!(
        app.status(),
        "Se dejó de vigilar srv 1.1.1.1: 30 min sin sitio"
    );
    assert!(app.watch_label().is_none());
}

#[test]
fn slot_watch_expired_message_uses_constant() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = cod2_app(dir.path(), Config::default());
    let expiry = run_task(run(&mut app, Action::WatchSlot));
    feed(&mut app, expiry);
    let minutes = format!("{} min sin sitio", SLOT_MAX.as_secs() / 60);
    assert!(app.status().contains(&minutes), "{}", app.status());
}

#[test]
fn stop_aborts_running_query() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 19)));
    start_watch(&mut app);
    let query = watch(&mut app, WatchMsg::SlotTick);
    // Con una consulta en curso, otro tic no lanza otra.
    assert!(run_task(watch(&mut app, WatchMsg::SlotTick)).is_empty());
    let _ = run(&mut app, Action::WatchSlot);
    assert!(app.watch_label().is_none());
    assert!(run_task(query).is_empty());
    assert!(fake.queries().is_empty());
}

#[test]
fn slot_answer_after_stop_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 19)));
    start_watch(&mut app);
    let late = tick(&mut app);
    let _ = run(&mut app, Action::WatchSlot);
    let status = app.status().to_owned();
    feed(&mut app, late);
    assert!(app.dialog().is_none());
    assert_eq!(app.status(), status);
    // Reemplazada por otra vigilancia: la respuesta de la anterior tampoco llega.
    start_watch(&mut app);
    let late = tick(&mut app);
    let generation = app.scan.generation();
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Event(ScanEvent::Server(srv("3.3.3.3", 20))),
    });
    select(&mut app, "3.3.3.3");
    start_watch(&mut app);
    assert_eq!(app.watch_label().as_deref(), Some("Vigilando srv 3.3.3.3…"));
    feed(&mut app, late);
    assert!(app.dialog().is_none());
    assert_eq!(app.watch_label().as_deref(), Some("Vigilando srv 3.3.3.3…"));
}

/// El filtro por número de `SlotAnswer`: la respuesta (con sitio) de la vigilancia reemplazada
/// no hace nada.
#[test]
fn slot_answer_with_old_serial_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    let expiry = run_task(run(&mut app, Action::WatchSlot));
    let [Message::Watch(WatchMsg::SlotExpired(old))] = expiry.as_slice() else {
        panic!("{expiry:?}");
    };
    let generation = app.scan.generation();
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Event(ScanEvent::Server(srv("3.3.3.3", 20))),
    });
    select(&mut app, "3.3.3.3");
    start_watch(&mut app);
    let old = WatchMsg::SlotAnswer {
        serial: *old,
        result: Ok(Some(srv("1.1.1.1", 3))),
    };
    let task = watch(&mut app, old);
    assert!(run_task(task).is_empty());
    assert!(app.dialog().is_none());
    assert_eq!(app.watch_label().as_deref(), Some("Vigilando srv 3.3.3.3…"));
    assert!(fake.queries().is_empty());
}

/// El plazo de 30 min de una vigilancia parada o reemplazada se aborta.
#[test]
fn slot_expiry_aborted_on_stop_and_replace() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = cod2_app(dir.path(), Config::default());
    let stopped = run(&mut app, Action::WatchSlot);
    let _ = run(&mut app, Action::WatchSlot);
    assert!(run_task(stopped).is_empty());
    let replaced = run(&mut app, Action::WatchSlot);
    let generation = app.scan.generation();
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Event(ScanEvent::Server(srv("3.3.3.3", 20))),
    });
    select(&mut app, "3.3.3.3");
    let current = run(&mut app, Action::WatchSlot);
    assert!(run_task(replaced).is_empty());
    assert_eq!(run_task(current).len(), 1);
}

#[test]
fn watch_slot_action_only_for_full_online() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    let mut offline = srv("4.4.4.4", 20);
    offline.online = false;
    complete(&mut app, &[srv("1.1.1.1", 20), srv("2.2.2.2", 3), offline]);
    assert!(app.selected_server().is_none());
    assert!(!enabled(&app, Action::WatchSlot));
    select(&mut app, "2.2.2.2");
    assert!(!enabled(&app, Action::WatchSlot));
    select(&mut app, "4.4.4.4");
    assert!(!enabled(&app, Action::WatchSlot));
    select(&mut app, "1.1.1.1");
    assert!(enabled(&app, Action::WatchSlot));
    assert_eq!(Action::WatchSlot.label(&app), "Avisar cuando haya sitio");
}

#[test]
fn watch_slot_shows_label_and_toggles() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = cod2_app(dir.path(), Config::default());
    start_watch(&mut app);
    assert_eq!(app.watch_label().as_deref(), Some("Vigilando srv 1.1.1.1…"));
    assert!(view_texts(&app).contains(&"Vigilando srv 1.1.1.1…".to_owned()));
    assert_eq!(Action::WatchSlot.label(&app), "Dejar de vigilar");
    assert!(app.is_watching(app.selected_server()));
    // Vigilado, sigue encendida aunque ya no esté lleno.
    complete(&mut app, &[srv("1.1.1.1", 3), srv("2.2.2.2", 3)]);
    select(&mut app, "1.1.1.1");
    assert!(enabled(&app, Action::WatchSlot));
    let _ = run(&mut app, Action::WatchSlot);
    assert!(app.watch_label().is_none());
    assert_eq!(Action::WatchSlot.label(&app), "Avisar cuando haya sitio");
}

/// *Conectar* lanza el juego vigilado aunque la tabla muestre otro; *Cancelar*, nada.
#[test]
fn slot_free_offers_connect() {
    for (answer, launches) in [(PromptMsg::Submit, 1), (PromptMsg::Cancel, 0)] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake) = cod2_app(dir.path(), Config::default());
        start_watch(&mut app);
        let _ = app.update(Message::SelectGame("q3"));
        fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 19)));
        let answers = tick(&mut app);
        feed(&mut app, answers);
        assert!(slot_text(&app).unwrap().contains("19/20"));
        let mut pending = vec![app.update(Message::Prompt(answer))];
        while let Some(task) = pending.pop() {
            for m in run_task(task) {
                pending.push(app.update(m));
            }
        }
        assert!(app.dialog().is_none());
        let launched = fake.launches();
        assert_eq!(launched.len(), launches);
        if let Some(l) = launched.first() {
            assert_eq!((l.game.id, &l.address), ("cod2", &addr("1.1.1.1")));
        }
        assert!(app.watch_label().is_none());
    }
}

#[test]
fn slot_free_with_dialog_open_goes_to_status() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    start_watch(&mut app);
    let _ = app.update(Message::Settings(SettingsMsg::Open));
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 19)));
    let answers = tick(&mut app);
    feed(&mut app, answers);
    assert_eq!(app.dialog(), Some(&Dialog::Settings));
    assert_eq!(
        app.status(),
        "Hay sitio: Hay sitio en srv 1.1.1.1 (19/20). ¿Conectar ahora?"
    );
}

#[test]
fn slot_watch_responses_reach_server_response_hook() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.favorites
        .insert("cod2".into(), vec!["1.1.1.1:28960".into()]);
    let (mut app, fake) = cod2_app(dir.path(), cfg);
    app.history_loaded = true;
    start_watch(&mut app);
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 20)));
    let answers = tick(&mut app);
    feed(&mut app, answers);
    assert_eq!(app.history.samples("cod2", "1.1.1.1:28960").len(), 1);
}

#[test]
fn watch_label_is_plain_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = cod2_app(dir.path(), Config::default());
    let mut bold = srv("5.5.5.5", 20);
    bold.name = "<b>x</b>".into();
    complete(&mut app, &[bold]);
    select(&mut app, "5.5.5.5");
    start_watch(&mut app);
    assert_eq!(app.watch_label().as_deref(), Some("Vigilando <b>x</b>…"));
    assert!(view_texts(&app).contains(&"Vigilando <b>x</b>…".to_owned()));
}

#[test]
fn watch_io_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.party_alerts.insert(
        "cod2".into(),
        [("1.1.1.1:28960".to_owned(), 4)].into_iter().collect(),
    );
    let (mut app, fake) = cod2_app(dir.path(), cfg);
    let tasks = [
        run(&mut app, Action::WatchSlot),
        watch(&mut app, WatchMsg::SlotTick),
        watch(&mut app, WatchMsg::RoundTick),
    ];
    assert!(fake.queries().is_empty());
    for t in tasks {
        let _ = run_task(t);
    }
    assert_eq!(fake.queries().len(), 2);
}

// ------------------------------------------------------------------- partida

/// «Clan Nox» (1.1.1.1) con `humans` personas de 16.
fn nox(humans: i32) -> ServerInfo {
    let mut info = mk("1.1.1.1", "Clan Nox", 30, humans, "1.3");
    info.max_players = 16;
    info
}

fn nox_app(dir: &Path, cfg: Config) -> (TestApp, FakeScan) {
    let (mut app, fake) = fake_app(dir, cfg);
    complete(&mut app, &[nox(1)]);
    select(&mut app, "1.1.1.1");
    (app, fake)
}

fn party_text(app: &App) -> Option<String> {
    match app.dialog() {
        Some(Dialog::PartyAlert { text, .. }) => Some(text.clone()),
        _ => None,
    }
}

fn round_subs(app: &App) -> usize {
    into_recipes(app.subscription()).len()
}

#[test]
fn alert_notifies_and_starts_background_watch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = nox_app(dir.path(), Config::default());
    let idle = round_subs(&app);
    let _ = app.set_party_alert("cod2", &nox(1), 4);
    assert_eq!(round_subs(&app), idle + 1);
    let saved = load_config(&dir.path().join("config.json")).party_alerts;
    assert_eq!(saved["cod2"]["1.1.1.1:28960"], 4);
    assert_eq!(app.status(), "Avisaré cuando Clan Nox tenga 4 jugadores");
    let targets = app.watch_targets();
    assert_eq!(targets[0].0.id, "cod2");
    assert_eq!(targets[0].1, vec![addr("1.1.1.1")]);
    let quiet = app.on_server_response("cod2", &nox(2));
    assert!(window_actions(quiet).is_empty());
    assert_eq!(app.status(), "Avisaré cuando Clan Nox tenga 4 jugadores");
    let mut busy = nox(5);
    busy.bots = 1;
    let task = app.on_server_response("cod2", &busy);
    assert_eq!(
        app.status(),
        "Se armó partida: Clan Nox: 4 jugadores en Call of Duty 2"
    );
    let main = app.main_window();
    assert_eq!(window_actions(task), vec![("attention", Some(main))]);
    let _ = app.set_party_alert("cod2", &nox(5), 0);
    assert!(app.config().party_alerts.is_empty());
    assert_eq!(round_subs(&app), idle);
    assert_eq!(app.status(), "Ya no se avisa de partidas en Clan Nox");
}

#[test]
fn edit_party_alert_uses_selected_server() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = nox_app(dir.path(), Config::default());
    let _ = run(&mut app, Action::PartyAlert);
    assert_eq!(party_text(&app).as_deref(), Some("4"));
    assert!(view_texts(&app).contains(
        &"Avisar cuando Clan Nox tenga al menos estos jugadores (0 = no avisar):".to_owned()
    ));
    let _ = app.update(Message::Prompt(PromptMsg::Input("6".into())));
    let _ = app.update(Message::Prompt(PromptMsg::Submit));
    assert!(app.dialog().is_none());
    assert_eq!(app.party_threshold("cod2", &addr("1.1.1.1")), 6);
    assert_eq!(
        Action::PartyAlert.label(&app),
        "Avisar de partida: 6 jugadores…"
    );
    let _ = run(&mut app, Action::PartyAlert);
    assert_eq!(party_text(&app).as_deref(), Some("6"));
    let _ = app.update(Message::Prompt(PromptMsg::Input("0".into())));
    let _ = app.update(Message::Prompt(PromptMsg::Submit));
    assert_eq!(app.status(), "Ya no se avisa de partidas en Clan Nox");
    assert!(!app.config().party_alerts.contains_key("cod2"));
}

/// Si al *Guardar* el servidor ya no está en la tabla, el aviso usa el nombre del diálogo.
#[test]
fn party_save_without_row_uses_dialog_name() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = nox_app(dir.path(), Config::default());
    let _ = run(&mut app, Action::PartyAlert);
    let _ = app.update(Message::SelectGame("q3"));
    assert!(app.servers().iter().all(|s| s.address != addr("1.1.1.1")));
    let _ = app.update(Message::Prompt(PromptMsg::Submit));
    assert_eq!(app.status(), "Avisaré cuando Clan Nox tenga 4 jugadores");
    assert_eq!(app.party_threshold("cod2", &addr("1.1.1.1")), 4);
}

#[test]
fn party_input_validates() {
    for bad in ["abc", "-1", "129", ""] {
        assert_eq!(party_value(bad), None, "{bad}");
    }
    assert_eq!(party_value(" 7 "), Some(7));
    assert_eq!(party_value("0"), Some(0));
    assert_eq!(party_value("128"), Some(128));
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = nox_app(dir.path(), Config::default());
    let _ = run(&mut app, Action::PartyAlert);
    for bad in ["abc", "-1", "129", ""] {
        let _ = app.update(Message::Prompt(PromptMsg::Input(bad.into())));
        let _ = app.update(Message::Prompt(PromptMsg::Submit));
        assert_eq!(party_text(&app).as_deref(), Some(bad));
        assert_eq!(app.party_threshold("cod2", &addr("1.1.1.1")), 0);
    }
    let _ = app.update(Message::Prompt(PromptMsg::Input(" 7 ".into())));
    let _ = app.update(Message::Prompt(PromptMsg::Submit));
    assert!(app.dialog().is_none());
    assert_eq!(app.party_threshold("cod2", &addr("1.1.1.1")), 7);
}

#[test]
fn party_action_label_follows_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[nox(1)]);
    assert!(!enabled(&app, Action::PartyAlert));
    select(&mut app, "1.1.1.1");
    assert!(enabled(&app, Action::PartyAlert));
    assert_eq!(
        Action::PartyAlert.label(&app),
        "Avisar cuando se arme partida…"
    );
    let _ = app.set_party_alert("cod2", &nox(1), 1);
    assert_eq!(
        Action::PartyAlert.label(&app),
        "Avisar de partida: 1 jugador…"
    );
}

/// Cuántas alertas a la principal piden las `Task` de entregar estos mensajes.
fn alerts(app: &mut App, messages: Vec<Message>) -> usize {
    let main = app.main_window();
    let mut n = 0;
    for m in messages {
        let task = app.update(m);
        n += window_actions(task)
            .iter()
            .filter(|a| **a == ("attention", Some(main)))
            .count();
    }
    n
}

/// El umbral avisa por cada camino: escaneo, refrescar uno, enlace y vista Favoritos.
#[test]
fn server_response_hook_runs_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.game_paths.insert("cod2".into(), "C:/cod2".into());
    cfg.favorites
        .insert("cod2".into(), vec!["1.1.1.1:28960".into()]);
    let (mut app, fake) = nox_app(dir.path(), cfg);
    fake.answer_for(&addr("1.1.1.1"), Some(nox(5)));
    let arm = |app: &mut App| {
        let _ = app.set_party_alert("cod2", &nox(1), 4);
    };
    arm(&mut app);
    let scan = Message::Scan {
        generation: app.scan.generation(),
        msg: ScanMsg::Event(ScanEvent::Server(nox(5))),
    };
    assert_eq!(alerts(&mut app, vec![scan]), 1, "escaneo");
    arm(&mut app);
    let answers = run_task(app.update(Message::RefreshSelected));
    assert_eq!(alerts(&mut app, answers), 1, "refrescar uno");
    arm(&mut app);
    let link = LinkMsg::Open {
        url: "jase://connect/cod2/1.1.1.1:28960".into(),
        interactive: false,
    };
    let answers = run_task(app.update(Message::Link(link)));
    assert_eq!(alerts(&mut app, answers), 1, "enlace");
    assert!(matches!(app.dialog(), Some(Dialog::Link { .. })));
    let _ = app.update(Message::Prompt(PromptMsg::Cancel));
    arm(&mut app);
    let found: Vec<Message> =
        run_task(app.update(Message::Direct(DirectMsg::Show(DirectView::Favorites))))
            .into_iter()
            .filter(|m| matches!(m, Message::Direct(DirectMsg::Found { .. })))
            .collect();
    assert_eq!(alerts(&mut app, found), 1, "favoritos");
}

#[test]
fn round_walks_games_without_overlap() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config {
        watch_friends: true,
        friends: vec!["bob".into()],
        ..Config::default()
    };
    cfg.favorites.insert(
        "cod2".into(),
        vec!["1.1.1.1:28960".into(), "2.2.2.2:28960".into()],
    );
    cfg.recent.insert("q3".into(), vec!["3.3.3.3:27960".into()]);
    let (mut app, fake) = fake_app(dir.path(), cfg);
    fake.answer(Ok(Some(srv("1.1.1.1", 3))));
    let order: Vec<&str> = app.watch_targets().iter().map(|(g, _)| g.id).collect();
    assert_eq!(order.len(), 2);
    let round = watch(&mut app, WatchMsg::RoundTick);
    // Con la vuelta en curso, otro tic no pide nada.
    assert!(run_task(watch(&mut app, WatchMsg::RoundTick)).is_empty());
    let messages = run_task(round);
    let games: Vec<&str> = fake.queries().iter().map(|(g, _)| *g).collect();
    assert_eq!(games.len(), 3);
    let first = games.iter().filter(|g| **g == order[0]).count();
    assert!(games[..first].iter().all(|g| *g == order[0]), "{games:?}");
    assert!(games[first..].iter().all(|g| *g == order[1]), "{games:?}");
    let seen = messages
        .iter()
        .filter(|m| matches!(m, Message::Watch(WatchMsg::RoundSeen { .. })))
        .count();
    assert_eq!(seen, 3);
    assert!(matches!(
        messages.last(),
        Some(Message::Watch(WatchMsg::RoundDone(_)))
    ));
    feed(&mut app, messages);
    let _ = run_task(watch(&mut app, WatchMsg::RoundTick));
    assert_eq!(fake.queries().len(), 6);
}

#[test]
fn round_after_watch_off_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.favorites
        .insert("cod2".into(), vec!["1.1.1.1:28960".into()]);
    let (mut app, fake) = nox_app(dir.path(), cfg);
    app.history_loaded = true;
    fake.answer(Ok(Some(nox(5))));
    // Una vuelta lanzada antes de quitar el último umbral se aborta…
    let _ = app.set_party_alert("cod2", &nox(1), 4);
    let late = watch(&mut app, WatchMsg::RoundTick);
    let _ = app.set_party_alert("cod2", &nox(1), 0);
    assert!(run_task(late).is_empty());
    assert!(fake.queries().is_empty());
    // …y lo que ya llegó de ella no llega al gancho.
    let _ = app.set_party_alert("cod2", &nox(1), 4);
    let stale = run_task(watch(&mut app, WatchMsg::RoundTick));
    assert!(!stale.is_empty());
    let _ = app.set_party_alert("cod2", &nox(1), 0);
    feed(&mut app, stale);
    assert!(app.history.samples("cod2", "1.1.1.1:28960").is_empty());
    assert_eq!(app.status(), "Ya no se avisa de partidas en Clan Nox");
}

#[test]
fn watch_dialogs_build_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = cod2_app(dir.path(), Config::default());
    start_watch(&mut app);
    fake.answer_for(&addr("1.1.1.1"), Some(srv("1.1.1.1", 19)));
    let answers = tick(&mut app);
    feed(&mut app, answers);
    let cases: [(Option<Action>, &[&str]); 2] = [
        (
            None,
            &[
                "Hay sitio",
                "Hay sitio en srv 1.1.1.1 (19/20). ¿Conectar ahora?",
                "Conectar",
                "Cancelar",
            ],
        ),
        (
            Some(Action::PartyAlert),
            &["Avisar cuando se arme partida", "Guardar", "Cancelar", "4"],
        ),
    ];
    for (open, expected) in cases {
        if let Some(a) = open {
            let _ = app.update(Message::Prompt(PromptMsg::Cancel));
            let _ = run(&mut app, a);
        }
        for (theme, _, _) in VARIANTS.iter() {
            app.theme_id = theme;
            let texts = view_texts(&app);
            for t in expected {
                assert!(
                    texts.contains(&(*t).to_owned()),
                    "{theme}: {t} en {texts:?}"
                );
            }
        }
    }
    // La etiqueta, en cada estilo.
    let _ = app.update(Message::Prompt(PromptMsg::Cancel));
    start_watch(&mut app);
    for (theme, _, _) in VARIANTS.iter() {
        app.theme_id = theme;
        assert!(view_texts(&app).contains(&"Vigilando srv 1.1.1.1…".to_owned()));
    }
}
