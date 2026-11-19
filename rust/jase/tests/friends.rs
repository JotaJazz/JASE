//! *Amigos…*: la lista, su diálogo, el aviso al aparecer uno y la fuente de la tabla de
//! jugadores (`tests/test_friends.py`, lo suyo de `test_friend_watch.py`). Ni consultas reales
//! ni ventanas: todo con el escáner falso.

mod support;

use iced::Task;
use iced::advanced::subscription::into_recipes;
use iced::font::Weight;
use jase::actions::{self, Action, Menu, enabled, entries};
use jase::app::{App, Dialog, FriendsMsg, Message, NOTIFY_BATCH, WatchMsg};
use jase::details::player_font;
use jase::friends_dialog::FriendsForm;
use jase::menu::Entry;
use jase::scan::ScanMsg;
use jase::theme::VARIANTS;
use jase_core::config::{Config, load_config};
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::scanner::scan::ScanEvent;
use support::{FakeScan, TestApp, fake_app, mk, run_task, view_texts};

fn addr(host: &str) -> ServerAddress {
    ServerAddress::new(host, 28960)
}

/// `host` llamado `name`, con esos jugadores (nombre con colores en `raw_name`).
fn srv(host: &str, name: &str, players: &[&str]) -> ServerInfo {
    let mut info = mk(host, name, 40, players.len() as i32, "1.3");
    info.players = players
        .iter()
        .map(|n| {
            let mut p = Player::new(jase_core::protocols::quake3::strip_colors(n), 0, 0);
            p.raw_name = (*n).to_owned();
            p
        })
        .collect();
    info
}

fn friends_app(dir: &std::path::Path, friends: &[&str]) -> (TestApp, FakeScan) {
    let cfg = Config {
        friends: friends.iter().map(|f| (*f).to_owned()).collect(),
        ..Config::default()
    };
    fake_app(dir, cfg)
}

/// Un servidor que llega del escaneo; devuelve los mensajes de su `Task` (el plazo vence al
/// instante con el reloj en pausa), solo los de amigos.
fn scan_in(app: &mut App, info: ServerInfo) -> Vec<Message> {
    let generation = app.scan.generation();
    let task = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Event(ScanEvent::Server(info)),
    });
    friend_msgs(task)
}

fn friend_msgs(task: Task<Message>) -> Vec<Message> {
    run_task(task)
        .into_iter()
        .filter(|m| matches!(m, Message::Friends(_)))
        .collect()
}

fn feed(app: &mut App, messages: Vec<Message>) {
    for m in messages {
        let _ = app.update(m);
    }
}

fn friends(app: &mut App, m: FriendsMsg) -> Task<Message> {
    app.update(Message::Friends(m))
}

fn round_subs(app: &App) -> usize {
    into_recipes(app.subscription()).len()
}

#[test]
fn single_notice_for_one_friend() {
    assert_eq!(NOTIFY_BATCH.as_millis(), 1500);
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = friends_app(dir.path(), &["Jane"]);
    let flush = scan_in(&mut app, srv("1.1.1.1", "S", &["^1Jane", "x"]));
    assert!(matches!(
        flush.as_slice(),
        [Message::Friends(FriendsMsg::Flush(_))]
    ));
    feed(&mut app, flush);
    assert_eq!(
        app.status(),
        "Amigo en línea: Jane está en S (Call of Duty 2)"
    );
    // La misma aparición no avisa de nuevo.
    assert!(scan_in(&mut app, srv("1.1.1.1", "S", &["^1Jane", "x"])).is_empty());
}

#[test]
fn batched_into_one_notice() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = friends_app(dir.path(), &["Jane", "Bob"]);
    let first = scan_in(&mut app, srv("1.1.1.1", "A", &["Jane"]));
    // Con el plazo corriendo, las siguientes no piden otro.
    assert!(scan_in(&mut app, srv("2.2.2.2", "B", &["Jane"])).is_empty());
    assert!(scan_in(&mut app, srv("3.3.3.3", "C", &["Bob"])).is_empty());
    assert_eq!(first.len(), 1);
    feed(&mut app, first);
    assert_eq!(
        app.status(),
        "Amigo en línea: 3 amigos están en línea: Jane, Bob"
    );
}

#[test]
fn friend_notice_uses_plain_names() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = friends_app(dir.path(), &["Jane"]);
    let flush = scan_in(&mut app, srv("1.1.1.1", "^1S^7rv", &["^4Jane"]));
    feed(&mut app, flush);
    assert_eq!(
        app.status(),
        "Amigo en línea: Jane está en Srv (Call of Duty 2)"
    );
}

#[test]
fn add_friend_persists_and_bolds() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = friends_app(dir.path(), &["bob"]);
    let _ = friends(&mut app, FriendsMsg::AddFriend("Jane".into()));
    assert_eq!(app.config().friends, ["bob", "Jane"]);
    let saved = load_config(&dir.path().join("config.json")).friends;
    assert_eq!(saved, ["bob", "Jane"]);
    assert!(app.is_friend("^1jane"));
    assert!(!app.is_friend("x"));
    assert_eq!(player_font(&app, "^1jane").weight, Weight::Bold);
    assert_eq!(player_font(&app, "x").weight, Weight::Normal);
    // El aviso del recién agregado ya funciona.
    let flush = scan_in(&mut app, srv("1.1.1.1", "S", &["jane"]));
    assert_eq!(flush.len(), 1);
}

#[test]
fn dialog_add_remove() {
    let mut form = FriendsForm::new(&["a".to_owned()], false);
    form.text = " Jane ".into();
    form.add();
    assert_eq!(form.text, "");
    form.text = "jane".into();
    form.add(); // repetida
    form.text = "   ".into();
    form.add(); // vacía
    assert_eq!(form.names, ["a", "Jane"]);
    form.selected = Some(0);
    form.remove_selected();
    assert_eq!(form.names(), ["Jane"]);
    assert_eq!(form.selected, None);
    form.remove_selected(); // sin elegida, nada
    assert_eq!(form.names(), ["Jane"]);
}

#[test]
fn close_stops_pending_friend_batch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = friends_app(dir.path(), &["Jane"]);
    let generation = app.scan.generation();
    let pending = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Event(ScanEvent::Server(srv("1.1.1.1", "S", &["Jane"]))),
    });
    let status = app.status().to_owned();
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
    let late = friend_msgs(pending);
    assert_eq!(late.len(), 1);
    feed(&mut app, late);
    assert_eq!(app.status(), status);
}

/// cod2 con 1.1.1.1 favorito y 2.2.2.2 reciente; `watch` = la casilla.
fn watch_app(dir: &std::path::Path, watch: bool) -> (TestApp, FakeScan) {
    let mut cfg = Config {
        friends: vec!["Jane".into()],
        watch_friends: watch,
        ..Config::default()
    };
    cfg.favorites
        .insert("cod2".into(), vec!["1.1.1.1:28960".into()]);
    cfg.recent
        .insert("cod2".into(), vec!["2.2.2.2:28960".into()]);
    fake_app(dir, cfg)
}

#[test]
fn window_runs_watcher_and_feeds_check() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = watch_app(dir.path(), true);
    assert!(app.watching_friends());
    let targets = app.watch_targets();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].0.id, "cod2");
    assert_eq!(targets[0].1, vec![addr("1.1.1.1"), addr("2.2.2.2")]);
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2", "Rec", &["Jane"])));
    let seen = run_task(app.update(Message::Watch(WatchMsg::RoundTick)));
    let mut flush = Vec::new();
    for m in seen {
        flush.extend(friend_msgs(app.update(m)));
    }
    feed(&mut app, flush);
    assert_eq!(
        app.status(),
        "Amigo en línea: Jane está en Rec (Call of Duty 2)"
    );
}

#[test]
fn window_toggle_stops_and_close_returns_worker() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = watch_app(dir.path(), true);
    fake.answer(Ok(Some(srv("1.1.1.1", "Fav", &["Jane"]))));
    let on = round_subs(&app);
    let late = app.update(Message::Watch(WatchMsg::RoundTick));
    let _ = friends(&mut app, FriendsMsg::Open);
    let _ = friends(&mut app, FriendsMsg::Watch(false));
    let _ = friends(&mut app, FriendsMsg::Accept);
    assert!(!app.config().watch_friends);
    assert_eq!(round_subs(&app), on - 1);
    assert!(run_task(late).is_empty());
    assert!(fake.queries().is_empty());
    assert!(app.watch_targets().is_empty());
}

#[test]
fn dialog_watch_checkbox() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = watch_app(dir.path(), false);
    let cfg_path = dir.path().join("config.json");
    // *Cancelar* no toca nada.
    let _ = friends(&mut app, FriendsMsg::Open);
    assert_eq!(app.dialog(), Some(&Dialog::Friends));
    let _ = friends(&mut app, FriendsMsg::Watch(true));
    let _ = friends(&mut app, FriendsMsg::Text("Bob".into()));
    let _ = friends(&mut app, FriendsMsg::Add);
    assert_eq!(app.friends_form().unwrap().names, ["Jane", "Bob"]);
    let _ = friends(&mut app, FriendsMsg::Cancel);
    assert!(app.dialog().is_none() && app.friends_form().is_none());
    assert!(!app.config().watch_friends);
    assert_eq!(app.config().friends, ["Jane"]);
    assert!(!cfg_path.exists() || !load_config(&cfg_path).watch_friends);
    // *Guardar* con la casilla: la config y el disco.
    let _ = friends(&mut app, FriendsMsg::Open);
    assert!(!app.friends_form().unwrap().watch);
    let _ = friends(&mut app, FriendsMsg::Watch(true));
    let _ = friends(&mut app, FriendsMsg::Text("Bob".into()));
    let _ = friends(&mut app, FriendsMsg::Add);
    let _ = friends(&mut app, FriendsMsg::Select(0));
    let _ = friends(&mut app, FriendsMsg::Remove);
    let _ = friends(&mut app, FriendsMsg::Accept);
    assert!(app.dialog().is_none() && app.friends_form().is_none());
    let saved = load_config(&cfg_path);
    assert!(saved.watch_friends && app.config().watch_friends);
    assert_eq!(saved.friends, ["Bob"]);
    assert!(app.is_friend("bob") && !app.is_friend("jane"));
}

#[test]
fn friends_action_and_shortcut_open_dialog() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = friends_app(dir.path(), &["Jane"]);
    assert!(enabled(&app, Action::Friends));
    let _ = app.update(Message::Shortcut(jase::bars::Shortcut::Friends));
    assert_eq!(app.dialog(), Some(&Dialog::Friends));
    let _ = app.update(Message::Escape);
    assert!(app.dialog().is_none() && app.friends_form().is_none());
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::Friends)));
    assert_eq!(app.dialog(), Some(&Dialog::Friends));
}

fn menu_items(app: &App) -> Vec<(String, Option<Message>)> {
    entries(app, Menu::Players)
        .into_iter()
        .filter_map(|e| match e {
            Entry::Item { label, on, .. } => Some((label, on)),
            _ => None,
        })
        .collect()
}

#[test]
fn player_menu_offers_add_friend() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = friends_app(dir.path(), &["Jane"]);
    let _ = friends(&mut app, FriendsMsg::PlayerMenu("  ".into()));
    assert_eq!(app.open_menu(), None);
    let _ = friends(&mut app, FriendsMsg::PlayerMenu(" Bob ".into()));
    assert_eq!(app.open_menu(), Some(Menu::Players));
    let items = menu_items(&app);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Agregar a amigos");
    let Some(add) = items[0].1.clone() else {
        panic!("apagado")
    };
    let _ = app.update(add);
    assert_eq!(app.open_menu(), None);
    assert_eq!(app.config().friends, ["Jane", "Bob"]);
    // Ya amigo: apagado.
    let _ = friends(&mut app, FriendsMsg::PlayerMenu("jane".into()));
    assert!(menu_items(&app)[0].1.is_none());
}

#[test]
fn friends_dialog_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = friends_app(dir.path(), &["Jane", "<b>Bob</b>"]);
    let _ = friends(&mut app, FriendsMsg::Open);
    let _ = friends(&mut app, FriendsMsg::Select(1));
    let expected = [
        "Amigos",
        "Jane",
        "<b>Bob</b>",
        "Agregar",
        "Quitar",
        "Revisar favoritos y recientes cada 2 min, también con la ventana en la bandeja",
        "Guardar",
        "Cancelar",
    ];
    for (theme, _, _) in VARIANTS.iter() {
        app.theme_id = theme;
        let texts = view_texts(&app);
        for t in expected {
            assert!(texts.contains(&t.to_owned()), "{theme}: {t} en {texts:?}");
        }
    }
}
