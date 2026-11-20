//! Vistas Favoritos y Recientes (`window_favorites.py` y sus tests de
//! `tests/test_main_window.py`): consulta directa sin master, filtros apagados en la vista y
//! la lista lateral. Las respuestas se meten a mano con `DirectMsg::Found/Finished` y el
//! `direct_request()` vigente, o salen de correr la `Task` con la consulta falsa.

mod support;

use std::path::Path;

use iced::Task;
use jase::actions::{self, Action, Menu, enabled};
use jase::app::{App, DIRECT_CONCURRENCY, DirectMsg, DirectView, Message};
use jase::bars;
use jase::menu::Entry;
use jase::presets::{self, PresetItem};
use jase::scan::ScanMsg;
use jase::sidebar;
use jase::table::{self, Column, ServerFilter};
use jase::theme::VARIANTS;
use jase::tree::{self, Kind};
use jase_core::config::Config;
use jase_core::favorites::{offline_server, remove_favorite};
use jase_core::filter_presets::FilterPreset;
use jase_core::games::ALL_VERSIONS;
use jase_core::models::{QuickFilters, ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{FakeScan, TestApp, fake_app, fake_app_task, mk, run_task};

fn addr(host: &str) -> ServerAddress {
    ServerAddress::new(host, 28960)
}

fn srv(host: &str) -> ServerInfo {
    mk(host, &format!("srv {host}"), 40, 3, "1.3")
}

fn mkm(host: &str, map: &str) -> ServerInfo {
    let mut info = srv(host);
    info.map = map.to_owned();
    info
}

fn cfg_with(favorites: &[&str], recent: &[&str], quick: QuickFilters) -> Config {
    let list = |l: &[&str]| l.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
    let mut cfg = Config {
        quick_filters: quick,
        ..Config::default()
    };
    if !favorites.is_empty() {
        cfg.favorites.insert("cod2".into(), list(favorites));
    }
    if !recent.is_empty() {
        cfg.recent.insert("cod2".into(), list(recent));
    }
    cfg
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

/// `favorites_window`: cod2 con su lista normal ya escaneada.
fn favorites_app(dir: &Path, cfg: Config, servers: &[ServerInfo]) -> (TestApp, FakeScan) {
    let (mut app, fake) = fake_app(dir, cfg);
    complete(&mut app, servers);
    (app, fake)
}

fn show(app: &mut App, view: DirectView) -> Task<Message> {
    app.update(Message::Direct(DirectMsg::Show(view)))
}

/// Corre la `Task` y entrega a la app lo que vuelve de la consulta directa.
fn drive(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        if matches!(m, Message::Direct(_)) {
            let _ = app.update(m);
        }
    }
}

fn found(app: &mut App, request: u64, info: ServerInfo) {
    let address = info.address.clone();
    let result = Ok(Some(info));
    let _ = app.update(Message::Direct(DirectMsg::Found {
        request,
        address,
        result,
    }));
}

fn finished(app: &mut App, request: u64) {
    let _ = app.update(Message::Direct(DirectMsg::Finished { request }));
}

fn rows(app: &App) -> Vec<(String, bool)> {
    app.servers()
        .iter()
        .map(|s| (s.address.host.clone(), s.online))
        .collect()
}

fn hosts(app: &App) -> Vec<String> {
    rows(app).into_iter().map(|r| r.0).collect()
}

fn shown(app: &App) -> usize {
    app.table.model.order().len()
}

fn select(app: &mut App, host: &str) {
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr(host))));
}

fn toggle(app: &mut App, kind: Kind, value: &str) {
    let msg = tree::Msg::Toggle("cod2", kind, value.to_owned());
    let _ = app.update(Message::Tree(msg));
}

fn activate(app: &mut App) {
    let _ = app.update(Message::Tree(tree::Msg::Activate("cod2")));
}

#[test]
fn favorites_view_shows_online_and_offline() {
    let dir = tempfile::tempdir().unwrap();
    let quick = QuickFilters {
        hide_empty: true,
        hide_full: true,
        max_ping: 50,
        ..QuickFilters::default()
    };
    let cfg = cfg_with(&["2.2.2.2:28960", "3.3.3.3:28960"], &[], quick);
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    // Otra versión: el filtro no la oculta aquí.
    fake.answer_for(
        &addr("2.2.2.2"),
        Some(mk("2.2.2.2", "srv 2.2.2.2", 40, 3, "1.0")),
    );
    assert_eq!(shown(&app), 1); // vista normal: 1.1.1.1 con 3/20 jugadores
    let task = show(&mut app, DirectView::Favorites);
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    assert_eq!(app.current_game().map(|g| g.id), Some("cod2"));
    drive(&mut app, task);
    assert_eq!(fake.calls(), ["cod2"]); // sin master
    assert_eq!(
        fake.queries(),
        [("cod2", addr("2.2.2.2")), ("cod2", addr("3.3.3.3"))]
    );
    let offline = app
        .servers()
        .iter()
        .find(|s| s.address.host == "3.3.3.3")
        .unwrap();
    assert!(!offline.online);
    assert_eq!(offline.name, "3.3.3.3:28960");
    assert!(rows(&app).contains(&("2.2.2.2".into(), true)));
    assert_eq!(shown(&app), 2); // filtros rápidos y versión no ocultan favoritos
    assert!(!enabled(&app, Action::HideEmpty));
    assert_eq!(bars::version_tip(&app), "No aplica en Favoritos");
    let last = *app.table.model.order().last().unwrap();
    assert_eq!(app.servers()[last].address.host, "3.3.3.3"); // sin respuesta al final
    assert!(app.status().contains("1 de 2"), "{}", app.status());
    assert_eq!(app.direct_list_labels()[0], "★ Favoritos · 2");

    // Volver al juego: la lista de la sesión con los filtros repuestos.
    activate(&mut app);
    assert_eq!(app.direct_view(), None);
    assert_eq!(hosts(&app), ["1.1.1.1"]); // la vista Favoritos no tocó la caché
    assert_eq!(shown(&app), 1);
    assert!(enabled(&app, Action::HideEmpty));
    assert_eq!(app.table.model.filter().version, "1.3");
    assert_eq!(bars::version_tip(&app), "Versión del juego");
    app.table
        .model
        .add_or_update(offline_server(&addr("4.4.4.4")));
    assert_eq!(shown(&app), 1); // filtros activos otra vez
    assert_eq!(fake.calls(), ["cod2"]); // se usó la lista guardada
}

#[test]
fn favorites_refresh_requeries_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2")));
    let task = show(&mut app, DirectView::Favorites);
    drive(&mut app, task);
    assert_eq!(fake.queries().len(), 1);
    let task = app.update(Message::Refresh);
    drive(&mut app, task);
    assert_eq!(fake.queries().len(), 2);
    assert_eq!(fake.calls(), ["cod2"]);
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    assert_eq!(rows(&app), [("2.2.2.2".into(), true)]);
}

#[test]
fn favorites_view_without_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = favorites_app(dir.path(), Config::default(), &[srv("1.1.1.1")]);
    let task = show(&mut app, DirectView::Favorites);
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    assert!(app.servers().is_empty());
    assert!(
        app.status().contains("No hay favoritos"),
        "{}",
        app.status()
    );
    assert!(!app.is_querying_direct());
    drive(&mut app, task);
    assert!(fake.queries().is_empty());
}

#[test]
fn selecting_other_game_leaves_favorites_view() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2")));
    let request = {
        let _ = show(&mut app, DirectView::Favorites);
        app.direct_request()
    };
    let _ = app.update(Message::SelectGame("ut99"));
    assert_eq!(app.direct_view(), None);
    assert_eq!(app.current_game().map(|g| g.id), Some("ut99"));
    found(&mut app, request, srv("2.2.2.2"));
    finished(&mut app, request);
    assert!(app.servers().is_empty()); // el resultado tardío de Favoritos se descarta
    assert_eq!(fake.calls(), ["cod2", "ut99"]);
}

/// En Rust siempre hay un juego (`initial_game`): de la guarda de `_show_direct` queda la
/// otra mitad, mostrar la vista vigente no hace nada.
#[test]
fn favorites_view_needs_a_game() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    let task = show(&mut app, DirectView::Favorites);
    let request = app.direct_request();
    let again = show(&mut app, DirectView::Favorites);
    assert_eq!(app.direct_request(), request);
    drive(&mut app, again);
    assert!(fake.queries().is_empty());
    drive(&mut app, task);
    assert_eq!(fake.queries().len(), 1);
}

#[test]
fn favorite_removed_mid_query_is_not_shown_offline() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(
        &["2.2.2.2:28960", "3.3.3.3:28960"],
        &[],
        QuickFilters::default(),
    );
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    let _ = show(&mut app, DirectView::Favorites);
    let request = app.direct_request();
    found(&mut app, request, srv("2.2.2.2"));
    assert_eq!(app.servers().len(), 1);
    select(&mut app, "2.2.2.2");
    let _ = app.toggle_favorite_selected(); // quita la fila
    finished(&mut app, request);
    assert_eq!(rows(&app), [("3.3.3.3".into(), false)]);
    assert!(app.status().contains("0 de 1"), "{}", app.status());
}

#[test]
fn late_answer_of_removed_favorite_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(
        &["2.2.2.2:28960", "3.3.3.3:28960"],
        &[],
        QuickFilters::default(),
    );
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    let _ = show(&mut app, DirectView::Favorites);
    let request = app.direct_request();
    remove_favorite(app.config_mut(), "cod2", &addr("2.2.2.2"));
    found(&mut app, request, srv("2.2.2.2"));
    finished(&mut app, request);
    assert_eq!(rows(&app), [("3.3.3.3".into(), false)]);
}

#[test]
fn entering_favorites_during_scan_discards_scan_rows() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["9.9.9.9:28960"], &[], QuickFilters::default());
    let (mut app, _fake) = fake_app(dir.path(), cfg);
    assert!(app.is_scanning());
    let generation = app.scan.generation();
    let task = show(&mut app, DirectView::Favorites);
    assert!(!app.is_scanning());
    let msg = ScanMsg::Event(ScanEvent::Server(srv("1.1.1.1")));
    let _ = app.update(Message::Scan { generation, msg });
    let msg = ScanMsg::Finished(ScanResult {
        masters_ok: 1,
        ..ScanResult::default()
    });
    let _ = app.update(Message::Scan { generation, msg });
    drive(&mut app, task);
    assert_eq!(hosts(&app), ["9.9.9.9"]);
    assert!(app.game_label("cod2").ends_with(" · —")); // nada en la caché
}

#[test]
fn activating_current_game_leaves_favorites_view() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    let _ = show(&mut app, DirectView::Favorites);
    activate(&mut app);
    assert_eq!(app.direct_view(), None);
    assert_eq!(hosts(&app), ["1.1.1.1"]);
}

#[test]
fn country_column_width_and_version_tooltip_in_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    assert_eq!(Column::Country.initial_width(), Some(48.0));
    assert_eq!(bars::version_tip(&app), "Versión del juego");
    let _ = show(&mut app, DirectView::Favorites);
    assert_eq!(bars::version_tip(&app), "No aplica en Favoritos");
    // El combo apagado muestra la versión guardada, no «todas».
    assert_eq!(app.table.model.filter().version, ALL_VERSIONS);
    assert_eq!(bars::version_shown(&app).as_deref(), Some("1.3"));
    let _ = app.update(Message::SelectGame("cod2"));
    assert_eq!(bars::version_tip(&app), "Versión del juego");
    let _ = show(&mut app, DirectView::Recent);
    assert_eq!(bars::version_tip(&app), "No aplica en Recientes");
}

#[test]
fn favorites_view_bypasses_tree_marks() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let servers = [mkm("1.1.1.1", "mp_a"), mkm("4.4.4.4", "mp_b")];
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &servers);
    toggle(&mut app, Kind::Maps, "mp_b");
    assert_eq!(shown(&app), 1);
    let task = show(&mut app, DirectView::Favorites);
    drive(&mut app, task);
    assert_eq!(shown(&app), 1); // el favorito (sin mapa) se ve pese a la marca
    assert_eq!(hosts(&app), ["2.2.2.2"]);
    let count = app.count_text();
    assert!(count.ends_with(" · filtro de mod/mapa/país (no aplica en Favoritos)"));
    assert!(!count.contains("activo"));
    let _ = app.update(Message::SelectGame("cod2"));
    assert_eq!(shown(&app), 1);
    assert!(app.count_text().contains("filtro de mod/mapa/país activo"));
}

#[test]
fn marks_in_favorites_show_notice() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let servers = [mkm("1.1.1.1", "mp_a"), mkm("4.4.4.4", "mp_b")];
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &servers);
    let task = show(&mut app, DirectView::Favorites);
    drive(&mut app, task);
    assert!(!app.count_text().contains("filtro"));
    toggle(&mut app, Kind::Maps, "mp_b");
    assert!(
        app.count_text()
            .ends_with(" · filtro de mod/mapa/país (no aplica en Favoritos)")
    );
    assert_eq!(shown(&app), 1);
}

#[test]
fn clear_marks_action_follows_marks_and_view() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let servers = [mkm("1.1.1.1", "mp_a"), mkm("4.4.4.4", "mp_b")];
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &servers);
    assert!(!enabled(&app, Action::ClearTreeMarks));
    toggle(&mut app, Kind::Maps, "mp_b");
    assert!(enabled(&app, Action::ClearTreeMarks));
    let _ = show(&mut app, DirectView::Favorites);
    assert!(!enabled(&app, Action::ClearTreeMarks));
    let _ = app.update(Message::SelectGame("cod2"));
    assert!(enabled(&app, Action::ClearTreeMarks));
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::ClearTreeMarks)));
    assert!(!enabled(&app, Action::ClearTreeMarks));
    assert!(app.tree.marks("cod2").is_empty());
}

/// ¿Están encendidos los radios de Filtro → Ping máx.?
fn ping_radios(app: &App) -> Vec<bool> {
    let entries = actions::entries(app, Menu::Filter);
    let Some(Entry::Sub { entries, .. }) = entries
        .into_iter()
        .find(|e| matches!(e, Entry::Sub { label, .. } if label == "Ping máx."))
    else {
        panic!("sin submenú Ping máx.");
    };
    entries
        .iter()
        .map(|e| matches!(e, Entry::Item { on: Some(_), .. }))
        .collect()
}

#[test]
fn ping_actions_disabled_in_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    assert!(ping_radios(&app).iter().all(|&on| on));
    let _ = show(&mut app, DirectView::Favorites);
    assert!(ping_radios(&app).iter().all(|&on| !on));
    for a in [
        Action::HideEmpty,
        Action::HideFull,
        Action::HidePassword,
        Action::HideBots,
    ] {
        assert!(!enabled(&app, a));
    }
    let _ = app.update(Message::SelectGame("cod2"));
    assert!(ping_radios(&app).iter().all(|&on| on));
}

#[test]
fn favorites_query_status_singular_spanish() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[]);
    let _ = show(&mut app, DirectView::Favorites);
    assert_eq!(app.status(), "Consultando 1 favorito de Call of Duty 2…");
    assert_eq!(app.scanning_text(), "Consultando favoritos…");
    assert!(app.is_querying_direct());
}

#[test]
fn recent_view_lists_and_marks_offline() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(
        &[],
        &["2.2.2.2:28960", "3.3.3.3:28960"],
        QuickFilters::default(),
    );
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2")));
    let task = show(&mut app, DirectView::Recent);
    assert_eq!(app.direct_view(), Some(DirectView::Recent));
    assert_eq!(
        app.status(),
        "Consultando 2 servidores recientes de Call of Duty 2…"
    );
    assert_eq!(app.scanning_text(), "Consultando recientes…");
    drive(&mut app, task);
    assert_eq!(
        rows(&app),
        [("2.2.2.2".into(), true), ("3.3.3.3".into(), false)]
    );
    assert_eq!(app.status(), "Recientes: 1 de 2 respondieron");
    assert_eq!(app.scanning_text(), "");
    assert_eq!(
        app.direct_list_labels(),
        ["★ Favoritos · 0".to_owned(), "🕘 Recientes · 2".to_owned()]
    );
    // Cambiar de vista directa vuelve a consultar con la otra lista (aquí vacía).
    let _ = show(&mut app, DirectView::Favorites);
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    assert!(app.servers().is_empty());
    assert!(
        app.status()
            .starts_with("No hay favoritos de Call of Duty 2")
    );
}

#[test]
fn filter_preset_in_favorites_does_not_hide_offline() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = cfg_with(&["9.9.9.9:28960"], &[], QuickFilters::default());
    cfg.filter_presets.push(FilterPreset {
        name: "vacíos".into(),
        quick: QuickFilters {
            hide_empty: true,
            ..QuickFilters::default()
        },
        text: String::new(),
    });
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[]);
    let task = show(&mut app, DirectView::Favorites);
    drive(&mut app, task); // el favorito no responde: fila sin conexión con 0 jugadores
    assert_eq!(shown(&app), 1);
    let pick = presets::Msg::Pick(PresetItem::Apply("vacíos".into()));
    let _ = app.update(Message::Presets(pick));
    assert_eq!(shown(&app), 1);
    assert!(app.quick_filters().hide_empty); // queda para la lista normal
    let _ = app.update(Message::SelectGame("cod2"));
    assert!(app.table.model.filter().quick.hide_empty);
}

#[test]
fn stop_action_cancels_direct_query() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2")));
    let task = show(&mut app, DirectView::Favorites);
    let request = app.direct_request();
    assert!(enabled(&app, Action::Stop));
    let _ = app.update(Message::Actions(actions::Msg::Run(Action::Stop)));
    assert!(!app.is_querying_direct());
    assert!(!enabled(&app, Action::Stop));
    assert_eq!(app.scanning_text(), "");
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    drive(&mut app, task);
    found(&mut app, request, srv("2.2.2.2"));
    finished(&mut app, request);
    assert!(app.servers().is_empty());
}

/// `test_favorites.py`: los filtros rápidos y la versión ocultan la fila sin conexión, salvo
/// con los de la vista directa.
#[test]
fn quick_filters_and_version_hide_offline_unless_bypassed() {
    let mut t = table::Table::default();
    t.model
        .add_or_update(offline_server(&ServerAddress::new("9.9.9.9", 1)));
    let mut f = ServerFilter {
        version: "1.3".into(),
        ..ServerFilter::default()
    };
    f.quick.hide_empty = true;
    table::set_filter(&mut t, f);
    assert!(t.model.order().is_empty());
    table::set_filter(&mut t, ServerFilter::default());
    assert_eq!(t.model.order().len(), 1);
}

#[test]
fn direct_queries_run_in_the_task() {
    let dir = tempfile::tempdir().unwrap();
    let favorites: Vec<String> = (0..100).map(|i| format!("10.0.0.{i}:28960")).collect();
    let favorites: Vec<&str> = favorites.iter().map(String::as_str).collect();
    let cfg = cfg_with(&favorites, &[], QuickFilters::default());
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[]);
    let task = show(&mut app, DirectView::Favorites);
    assert!(fake.queries().is_empty()); // nada en `update`
    drive(&mut app, task);
    assert_eq!(fake.queries().len(), 100);
    let max = fake.query_max.load(std::sync::atomic::Ordering::SeqCst);
    assert!(max > 1 && max <= DIRECT_CONCURRENCY, "{max} a la vez");
    assert_eq!(app.servers().len(), 100);
}

#[test]
fn refresh_discards_previous_direct_answers() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    let _ = show(&mut app, DirectView::Favorites);
    let old = app.direct_request();
    let _ = app.update(Message::Refresh);
    assert_ne!(app.direct_request(), old);
    found(&mut app, old, srv("2.2.2.2"));
    finished(&mut app, old);
    assert!(app.servers().is_empty());
    assert!(app.is_querying_direct());
}

#[test]
fn refresh_selected_in_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(
        &["2.2.2.2:28960", "3.3.3.3:28960"],
        &[],
        QuickFilters::default(),
    );
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    fake.answer_for(&addr("2.2.2.2"), Some(srv("2.2.2.2")));
    let task = show(&mut app, DirectView::Favorites);
    drive(&mut app, task);
    let label = app.game_label("cod2");
    // Deja de responder: la fila pasa a sin conexión.
    fake.answer_for(&addr("2.2.2.2"), None);
    select(&mut app, "2.2.2.2");
    let task = app.update(Message::RefreshSelected);
    for m in run_task(task) {
        let _ = app.update(m);
    }
    assert!(rows(&app).contains(&("2.2.2.2".into(), false)));
    assert_eq!(app.status(), "El servidor no respondió");
    // Ya no es favorito: su respuesta no entra.
    fake.answer_for(&addr("3.3.3.3"), Some(srv("3.3.3.3")));
    remove_favorite(app.config_mut(), "cod2", &addr("3.3.3.3"));
    select(&mut app, "3.3.3.3");
    let task = app.update(Message::RefreshSelected);
    for m in run_task(task) {
        let _ = app.update(m);
    }
    assert!(rows(&app).contains(&("3.3.3.3".into(), false)));
    assert_eq!(app.game_label("cod2"), label); // la caché no cambia
    let _ = app.update(Message::SelectGame("cod2"));
    assert_eq!(hosts(&app), ["1.1.1.1"]);
}

#[test]
fn lan_leaves_direct_view() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&["2.2.2.2:28960"], &[], QuickFilters::default());
    let (mut app, fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    let _ = show(&mut app, DirectView::Favorites);
    let _ = app.update(Message::ScanLan);
    assert_eq!(app.direct_view(), None);
    assert!(!app.is_querying_direct());
    assert_eq!(fake.lan_calls(), ["cod2"]);
    assert!(enabled(&app, Action::HideEmpty));
}

#[test]
fn steam_key_does_not_scan_in_direct_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.ui_state.insert("last_game".into(), "l4d".into());
    let (mut app, fake, _task) = fake_app_task(dir.path(), cfg);
    let _ = show(&mut app, DirectView::Favorites); // sin favoritos: la tabla queda vacía
    let _ = app.update(Message::SteamKey("K".into()));
    assert!(fake.calls().is_empty());
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
}

#[test]
fn direct_list_and_disabled_controls_build_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(
        &["2.2.2.2:28960"],
        &["3.3.3.3:28960"],
        QuickFilters::default(),
    );
    let (mut app, _fake) = favorites_app(dir.path(), cfg, &[srv("1.1.1.1")]);
    let build = |app: &mut App| {
        for (id, _, _) in VARIANTS.iter() {
            app.theme_id = id;
            let _ = sidebar::view(app);
            let _ = bars::toolbar(app);
            let _ = app.view();
        }
    };
    build(&mut app);
    let task = show(&mut app, DirectView::Favorites);
    drive(&mut app, task);
    build(&mut app);
    let _ = show(&mut app, DirectView::Recent);
    build(&mut app);
}
