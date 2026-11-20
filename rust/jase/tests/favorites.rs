//! Estrella, notas, quitar de recientes y grupos de favoritos (`window_favorites.py`,
//! `window_groups.py` y sus tests de `tests/test_main_window.py`, `tests/test_favorites.py` y
//! `tests/test_groups.py`).

mod support;

use std::path::Path;

use iced::Task;
use indexmap::IndexMap;
use jase::actions::{self, Action, Menu, enabled, entries};
use jase::app::{App, Dialog, DirectMsg, DirectView, Focus, Message, PROMPT_ID, PromptMsg};
use jase::bars;
use jase::card;
use jase::menu::{Check, Entry};
use jase::scan::ScanMsg;
use jase::table::{self, Column, Table, visible_rows};
use jase::theme::{VARIANTS, WidgetStyle, palette};
use jase_core::config::{Config, load_config};
use jase_core::favorites::{GROUP_MAX, NOTE_MAX, get_group, offline_server};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{FakeScan, TestApp, fake_app, field_ops, mk, run_task};

fn addr(host: &str) -> ServerAddress {
    ServerAddress::new(host, 28960)
}

fn srv(host: &str) -> ServerInfo {
    mk(host, &format!("srv {host}"), 40, 3, "1.3")
}

/// Entrega `servers` como el escaneo terminado del juego actual.
fn complete(app: &mut App, servers: &[ServerInfo]) {
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

fn app_with(dir: &Path, cfg: Config, servers: &[ServerInfo]) -> (TestApp, FakeScan) {
    let (mut app, fake) = fake_app(dir, cfg);
    complete(&mut app, servers);
    (app, fake)
}

/// Corre la `Task` y entrega a la app todo lo que vuelve.
fn drive(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        let _ = app.update(m);
    }
}

fn show(app: &mut App, view: DirectView) {
    let task = app.update(Message::Direct(DirectMsg::Show(view)));
    drive(app, task);
}

fn select(app: &mut App, host: &str) {
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr(host))));
}

fn run(app: &mut App, a: Action) -> Task<Message> {
    app.update(Message::Actions(actions::Msg::Run(a)))
}

fn prompt(app: &mut App, m: PromptMsg) -> Task<Message> {
    app.update(Message::Prompt(m))
}

fn hosts(app: &App) -> Vec<String> {
    app.servers()
        .iter()
        .map(|s| s.address.host.clone())
        .collect()
}

fn saved(dir: &Path) -> Config {
    load_config(&dir.join("config.json"))
}

/// `{"cod2": [...]}` como lo guarda la config.
fn cod2(list: &[&str]) -> IndexMap<String, Vec<String>> {
    let list = list.iter().map(|a| (*a).to_owned()).collect();
    [("cod2".to_owned(), list)].into_iter().collect()
}

fn star(app: &App) -> bool {
    card::vm(app).unwrap().favorite
}

fn labels(es: &[Entry<Message>]) -> Vec<String> {
    es.iter()
        .filter_map(|e| match e {
            Entry::Item { label, .. } | Entry::Sub { label, .. } => Some(label.clone()),
            Entry::Separator => None,
        })
        .collect()
}

fn item_on(es: &[Entry<Message>], name: &str) -> Option<Message> {
    es.iter()
        .find_map(|e| match e {
            Entry::Item { label, on, .. } if label == name => Some(on.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("sin ítem {name:?}"))
}

/// Las entradas del submenú *Grupo* del contextual.
fn group_menu(app: &App) -> Vec<Entry<Message>> {
    entries(app, Menu::Context)
        .into_iter()
        .find_map(|e| match e {
            Entry::Sub { label, entries, .. } if label == "Grupo" => Some(entries),
            _ => None,
        })
        .unwrap()
}

fn name_tip(t: &Table, row: usize) -> Option<String> {
    visible_rows(t, palette("dark"), WidgetStyle::Flat)[row]
        .cells
        .iter()
        .find(|(c, _)| *c == Column::Name)
        .and_then(|(_, cell)| cell.tooltip.clone())
}

#[test]
fn context_menu_adds_and_removes_favorite() {
    let dir = tempfile::tempdir().unwrap();
    let mut info = srv("1.1.1.1");
    info.connect_address = ServerAddress::new("1.1.1.1", 28961);
    let (mut app, fake) = app_with(dir.path(), Config::default(), &[info]);
    select(&mut app, "1.1.1.1");
    assert!(!star(&app));
    let menu = entries(&app, Menu::Context);
    let msg = item_on(&menu, "Agregar a favoritos").expect("encendida");
    let _ = app.update(msg);
    // Se guarda la dirección de consulta, no la de conexión.
    let want = cod2(&["1.1.1.1:28960"]);
    assert_eq!(app.config().favorites, want);
    assert_eq!(saved(dir.path()).favorites, want);
    assert!(star(&app));
    assert_eq!(app.status(), "Agregado a favoritos: 1.1.1.1:28960");
    assert!(fake.queries().is_empty()); // agregar en la vista normal no consulta nada
    assert!(!app.is_scanning() && app.direct_view().is_none());
    let menu = entries(&app, Menu::Context);
    let _ = app.update(item_on(&menu, "Quitar de favoritos").expect("encendida"));
    assert!(app.config().favorites.is_empty());
    assert!(saved(dir.path()).favorites.is_empty());
    assert!(!star(&app));
    assert_eq!(app.status(), "Quitado de favoritos: 1.1.1.1:28960");
}

#[test]
fn card_star_toggles_favorite() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[srv("1.1.1.1")]);
    select(&mut app, "1.1.1.1");
    // El mensaje que manda la estrella de la ficha.
    let _ = run(&mut app, Action::Favorite);
    assert_eq!(
        app.config().favorites.get("cod2"),
        Some(&vec!["1.1.1.1:28960".to_owned()])
    );
    assert!(star(&app));
    assert!(card::chart(&app).is_some()); // el gráfico aparece al marcarlo
    let _ = run(&mut app, Action::Favorite);
    assert!(app.config().favorites.is_empty());
    assert!(!star(&app));
}

fn favorites_cfg(favs: &[&str]) -> Config {
    let mut cfg = Config::default();
    let list = favs.iter().map(|a| (*a).to_owned()).collect();
    cfg.favorites.insert("cod2".into(), list);
    cfg
}

#[test]
fn removing_favorite_in_favorites_view_drops_row() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = favorites_cfg(&["2.2.2.2:28960", "3.3.3.3:28960"]);
    let (mut app, fake) = app_with(dir.path(), cfg, &[srv("1.1.1.1")]);
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2")));
    show(&mut app, DirectView::Favorites);
    assert_eq!(app.servers().len(), 2);
    select(&mut app, "3.3.3.3");
    let menu = entries(&app, Menu::Context);
    let _ = app.update(item_on(&menu, "Quitar de favoritos").expect("encendida"));
    assert_eq!(hosts(&app), ["2.2.2.2"]);
    assert_eq!(
        app.config().favorites.get("cod2"),
        Some(&vec!["2.2.2.2:28960".to_owned()])
    );
    assert!(app.selected_server().is_none());
}

#[test]
fn remove_from_recent_context_action() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    let list = vec!["2.2.2.2:28960".to_owned(), "3.3.3.3:28960".to_owned()];
    cfg.recent.insert("cod2".into(), list);
    let (mut app, fake) = app_with(dir.path(), cfg, &[srv("1.1.1.1")]);
    select(&mut app, "1.1.1.1");
    assert!(!labels(&entries(&app, Menu::Context)).contains(&"Quitar de recientes".into()));
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2")));
    show(&mut app, DirectView::Recent);
    assert_eq!(app.servers().len(), 2);
    select(&mut app, "3.3.3.3");
    let menu = entries(&app, Menu::Context);
    let msg = item_on(&menu, "Quitar de recientes").expect("encendida");
    let _ = app.update(msg);
    assert_eq!(hosts(&app), ["2.2.2.2"]);
    let want = cod2(&["2.2.2.2:28960"]);
    assert_eq!(app.config().recent, want);
    assert_eq!(saved(dir.path()).recent, want);
    assert_eq!(app.status(), "Quitado de recientes: 3.3.3.3:28960");
}

#[test]
fn edit_note_via_context_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[srv("1.1.1.1")]);
    select(&mut app, "1.1.1.1");
    assert!(item_on(&entries(&app, Menu::Context), "Editar nota…").is_none());
    let _ = app.toggle_favorite_selected();
    let msg = item_on(&entries(&app, Menu::Context), "Editar nota…").expect("encendida");
    let _ = app.update(msg);
    assert!(matches!(app.dialog(), Some(Dialog::Note { text, .. }) if text.is_empty()));
    let _ = prompt(&mut app, PromptMsg::Input("mi nota".into()));
    let _ = prompt(&mut app, PromptMsg::Submit);
    assert!(app.dialog().is_none());
    let note = |cfg: &Config| cfg.favorite_notes.get("cod2").cloned().unwrap_or_default();
    assert_eq!(note(app.config()).get("1.1.1.1:28960").unwrap(), "mi nota");
    assert_eq!(note(&saved(dir.path())), note(app.config()));
    assert_eq!(card::vm(&app).unwrap().note, "mi nota");
    assert!(name_tip(&app.table, 0).unwrap().contains("mi nota"));
    // Al abrir de nuevo trae la nota actual; vacía la borra.
    let _ = app.edit_note_selected();
    assert!(matches!(app.dialog(), Some(Dialog::Note { text, .. }) if text == "mi nota"));
    let _ = prompt(&mut app, PromptMsg::Input(String::new()));
    let _ = prompt(&mut app, PromptMsg::Submit);
    assert!(app.config().favorite_notes.is_empty());
    assert_eq!(name_tip(&app.table, 0), None);
    // *Cancelar* no cambia nada.
    let _ = app.edit_note_selected();
    let _ = prompt(&mut app, PromptMsg::Input("z".into()));
    let _ = prompt(&mut app, PromptMsg::Cancel);
    assert!(app.dialog().is_none());
    assert!(app.config().favorite_notes.is_empty());
}

#[test]
fn model_note_in_name_tooltip() {
    let mut t = Table::default();
    let info = srv("1.1.1.1");
    t.model.add_or_update(info.clone());
    assert_eq!(name_tip(&t, 0), None);
    t.model
        .set_notes([(info.address.clone(), "mi nota".to_owned())].into());
    assert_eq!(name_tip(&t, 0).as_deref(), Some("srv 1.1.1.1\nmi nota"));
}

#[test]
fn offline_favorite_tooltip_says_it_did_not_answer() {
    support::lang::with_language("es", || {
        let mut t = Table::default();
        let a = addr("2.2.2.2");
        t.model.add_or_update(offline_server(&a));
        assert_eq!(
            name_tip(&t, 0).as_deref(),
            Some("2.2.2.2:28960: el favorito no respondió")
        );
        t.model.set_notes([(a, "clan".to_owned())].into());
        assert_eq!(
            name_tip(&t, 0).as_deref(),
            Some("2.2.2.2:28960: el favorito no respondió\nclan")
        );
    });
}

#[test]
fn favorite_label_follows_selection() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = favorites_cfg(&["1.1.1.1:28960"]);
    let (mut app, _) = app_with(dir.path(), cfg, &[srv("1.1.1.1"), srv("2.2.2.2")]);
    for (host, want) in [
        ("1.1.1.1", "Quitar de favoritos"),
        ("2.2.2.2", "Agregar a favoritos"),
    ] {
        select(&mut app, host);
        assert_eq!(Action::Favorite.label(&app), want);
        assert!(labels(&entries(&app, Menu::Server)).contains(&want.to_owned()));
        assert!(labels(&entries(&app, Menu::Context)).contains(&want.to_owned()));
        assert!(enabled(&app, Action::Favorite));
    }
    // Sin selección, apagada y con el texto de agregar.
    app.table.selected = None;
    assert_eq!(Action::Favorite.label(&app), "Agregar a favoritos");
    assert!(!enabled(&app, Action::Favorite));
}

#[test]
fn group_menu_disabled_without_favorite() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = favorites_cfg(&["1.1.1.1:28960"]);
    cfg.favorite_groups.insert(
        "cod2".into(),
        [("1.1.1.1:28960".to_owned(), "Clan".to_owned())].into(),
    );
    let (mut app, _) = app_with(dir.path(), cfg, &[srv("1.1.1.1"), srv("2.2.2.2")]);
    select(&mut app, "2.2.2.2");
    assert!(group_menu(&app).is_empty());
    assert!(!enabled(&app, Action::Group));
    assert!(!enabled(&app, Action::EditNote));
    select(&mut app, "1.1.1.1");
    assert!(enabled(&app, Action::Group));
    let menu = group_menu(&app);
    assert_eq!(labels(&menu), ["Clan", "Sin grupo", "Nuevo grupo…"]);
    let checks: Vec<bool> = menu
        .iter()
        .filter_map(|e| match e {
            Entry::Item {
                check: Check::Check(c),
                ..
            } => Some(*c),
            _ => None,
        })
        .collect();
    assert_eq!(checks, [true, false]);
    // El de la barra de menús es el mismo.
    let server = entries(&app, Menu::Server);
    assert!(server.iter().any(
        |e| matches!(e, Entry::Sub { label, entries, .. } if label == "Grupo" && entries.len() == 4)
    ));
}

const A: &str = "1.1.1.1";
const B: &str = "2.2.2.2";
const C: &str = "3.3.3.3";

/// `make` de `test_groups.py`: A en «Clan», B en «LAN», C suelto, y cada uno responde.
fn groups_app(dir: &Path) -> (TestApp, FakeScan) {
    let mut cfg = favorites_cfg(&["1.1.1.1:28960", "2.2.2.2:28960", "3.3.3.3:28960"]);
    cfg.favorite_groups.insert(
        "cod2".into(),
        [
            ("1.1.1.1:28960".to_owned(), "Clan".to_owned()),
            ("2.2.2.2:28960".to_owned(), "LAN".to_owned()),
        ]
        .into(),
    );
    let (app, fake) = app_with(dir, cfg, &[]);
    for (host, name) in [(A, "Clan Nox"), (B, "Casa"), (C, "Suelto")] {
        fake.answer_for(&addr(host), Some(mk(host, name, 40, 3, "1.3")));
    }
    (app, fake)
}

fn group_labels(app: &App) -> Vec<String> {
    app.group_options().into_iter().map(|o| o.label).collect()
}

fn pick_group(app: &mut App, group: Option<&str>) {
    let task = app.update(Message::Direct(DirectMsg::Group(group.map(str::to_owned))));
    drive(app, task);
}

#[test]
fn group_combo_filters_favorites_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = groups_app(dir.path());
    assert!(app.group_options().is_empty()); // fuera de Favoritos no se ve
    show(&mut app, DirectView::Favorites);
    assert_eq!(app.servers().len(), 3);
    assert_eq!(
        group_labels(&app),
        ["Todos los grupos", "Clan", "LAN", "Sin grupo"]
    );
    fake.queries.lock().unwrap().clear();
    pick_group(&mut app, Some("Clan"));
    assert_eq!(app.current_group(), Some("Clan"));
    assert_eq!(fake.queries(), [("cod2", addr(A))]); // solo se consulta el grupo
    assert_eq!(hosts(&app), [A]);
    pick_group(&mut app, Some(""));
    assert_eq!(hosts(&app), [C]);
    let _ = app.update(Message::SelectGame("ut99"));
    assert!(app.group_options().is_empty());
    assert_eq!(app.current_group(), None);
}

/// Como el combo de versión de la barra: elegir un grupo deja el foco en el combo.
#[test]
fn group_combo_takes_focus_like_version_combo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _fake) = groups_app(dir.path());
    show(&mut app, DirectView::Favorites);
    select(&mut app, A);
    assert_eq!(app.focus(), Focus::Table);
    pick_group(&mut app, Some("Clan"));
    assert_eq!(app.focus(), Focus::Sidebar);
}

#[test]
fn move_selected_to_group_from_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = groups_app(dir.path());
    show(&mut app, DirectView::Favorites);
    select(&mut app, C);
    assert_eq!(
        labels(&group_menu(&app)),
        ["Clan", "LAN", "Sin grupo", "Nuevo grupo…"]
    );
    let new = item_on(&group_menu(&app), "Nuevo grupo…").expect("encendida");
    let task = app.update(new);
    drive(&mut app, task);
    assert_eq!(app.dialog(), Some(&Dialog::NewGroup(String::new())));
    let _ = prompt(&mut app, PromptMsg::Input("Probar".into()));
    let task = prompt(&mut app, PromptMsg::Submit);
    drive(&mut app, task);
    assert!(app.dialog().is_none());
    assert_eq!(get_group(app.config(), "cod2", "3.3.3.3:28960"), "Probar");
    assert_eq!(
        get_group(&saved(dir.path()), "cod2", "3.3.3.3:28960"),
        "Probar"
    );
    assert!(group_labels(&app).contains(&"Probar".to_owned()));
    // Con un grupo a la vista, mover fuera de él quita la fila.
    pick_group(&mut app, Some("Probar"));
    assert_eq!(hosts(&app), [C]);
    select(&mut app, C);
    fake.queries.lock().unwrap().clear();
    let task = app.move_selected_to_group("");
    assert_eq!(app.status(), "Suelto quedó sin grupo");
    // «Probar» quedó vacío: vuelve a «Todos los grupos» y se ven los 3.
    assert_eq!(app.current_group(), None);
    drive(&mut app, task);
    assert_eq!(fake.queries().len(), 3);
    assert_eq!(app.servers().len(), 3);
    select(&mut app, A);
    let _ = app.move_selected_to_group("LAN");
    assert_eq!(app.status(), "Clan Nox pasó al grupo «LAN»");
}

#[test]
fn note_and_group_text_is_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = favorites_cfg(&["1.1.1.1:28960"]);
    let (mut app, _) = app_with(dir.path(), cfg, &[srv("1.1.1.1")]);
    select(&mut app, "1.1.1.1");
    let long = format!("a\nb\t{}", "x".repeat(500));
    let _ = app.edit_note_selected();
    let _ = prompt(&mut app, PromptMsg::Input(long.clone()));
    let _ = prompt(&mut app, PromptMsg::Submit);
    let note = &app.config().favorite_notes["cod2"]["1.1.1.1:28960"];
    assert!(note.starts_with("a b x") && !note.contains('\n'));
    assert_eq!(note.chars().count(), NOTE_MAX);
    let _ = app.update(Message::Actions(actions::Msg::NewGroup));
    let _ = prompt(&mut app, PromptMsg::Input(long));
    let _ = prompt(&mut app, PromptMsg::Submit);
    let group = get_group(app.config(), "cod2", "1.1.1.1:28960").to_owned();
    assert!(group.starts_with("a b x") && !group.contains('\n'));
    assert_eq!(group.chars().count(), GROUP_MAX);
    // Un nombre en blanco no mueve.
    let _ = app.update(Message::Actions(actions::Msg::NewGroup));
    let _ = prompt(&mut app, PromptMsg::Input(" \n ".into()));
    let _ = prompt(&mut app, PromptMsg::Submit);
    assert_eq!(get_group(app.config(), "cod2", "1.1.1.1:28960"), group);
}

#[test]
fn favorites_dialogs_build_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = groups_app(dir.path());
    show(&mut app, DirectView::Favorites);
    select(&mut app, A);
    let build = |app: &mut App| {
        for (id, _, _) in VARIANTS.iter() {
            app.theme_id = id;
            let _ = bars::toolbar(app);
            let _ = app.view();
        }
    };
    build(&mut app); // con el combo de grupo
    let _ = app.edit_note_selected();
    build(&mut app);
    let _ = prompt(&mut app, PromptMsg::Cancel);
    let _ = app.update(Message::Actions(actions::Msg::NewGroup));
    build(&mut app);
}

#[test]
fn edit_note_opens_with_text_selected() {
    // C1 (5j): `QInputDialog.getText` abre con el texto elegido, listo para reemplazarlo.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with(dir.path(), Config::default(), &[srv("1.1.1.1")]);
    select(&mut app, "1.1.1.1");
    let _ = app.toggle_favorite_selected();
    let ops = field_ops(app.edit_note_selected(), &PROMPT_ID);
    assert!(ops.contains(&"focus"), "{ops:?}");
    assert!(ops.contains(&"select_all"), "{ops:?}");
}
