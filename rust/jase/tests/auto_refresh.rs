//! Refresco automático y selección pendiente (`window_auto_refresh.py` y sus tests de
//! `tests/test_main_window.py`): los mensajes del escaneo se meten a mano con la generación
//! vigente; los de la vista directa salen de correr la `Task` con la consulta falsa.

mod support;

use iced::Task;
use jase::actions::{Action, Msg as ActionMsg};
use jase::app::{App, AutoMsg, DirectMsg, DirectView, Message, SettingsMsg};
use jase::scan::ScanMsg;
use jase::table;
use jase_core::config::Config;
use jase_core::games::game;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{fake_app, mk, run_task, widget_ops, window_requests};

fn srv(host: &str) -> ServerInfo {
    mk(host, &format!("srv {host}"), 40, 3, "1.3")
}

fn send(app: &mut App, msg: ScanMsg) -> Task<Message> {
    let generation = app.scan.generation();
    app.update(Message::Scan { generation, msg })
}

fn server(app: &mut App, info: &ServerInfo) -> Task<Message> {
    send(app, ScanMsg::Event(ScanEvent::Server(info.clone())))
}

fn finish(app: &mut App, ok: bool) {
    let result = ScanResult {
        masters_ok: usize::from(ok),
        ..ScanResult::default()
    };
    let _ = send(app, ScanMsg::Finished(result));
}

/// Entrega `servers` como el escaneo terminado (bien) del juego actual.
fn complete(app: &mut App, servers: &[ServerInfo]) {
    for info in servers {
        let _ = server(app, info);
    }
    finish(app, true);
}

fn deliver(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        let _ = app.update(m);
    }
}

fn run(app: &mut App) -> Task<Message> {
    app.update(Message::AutoRefresh(AutoMsg::Run {
        minimized: Some(false),
    }))
}

fn addresses(app: &App) -> Vec<ServerAddress> {
    app.servers().iter().map(|s| s.address.clone()).collect()
}

/// cod2 con A y B ya escaneados.
fn two_rows(dir: &std::path::Path, cfg: Config) -> (support::TestApp, support::FakeScan) {
    let (mut app, fake) = fake_app(dir, cfg);
    complete(&mut app, &[srv("1.1.1.1"), srv("2.2.2.2")]);
    (app, fake)
}

#[test]
fn auto_refresh_follows_config() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        auto_refresh_minutes: 2,
        ..Config::default()
    };
    let (mut app, _) = fake_app(dir.path(), cfg);
    assert_eq!(app.auto_refresh_minutes(), 2);
    let open = app.update(Message::Settings(SettingsMsg::Open));
    deliver(&mut app, open);
    let _ = app.update(Message::Settings(SettingsMsg::AutoRefresh(0)));
    let _ = app.update(Message::Settings(SettingsMsg::Accept));
    assert_eq!(app.auto_refresh_minutes(), 0);
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        fake_app(dir.path(), Config::default())
            .0
            .auto_refresh_minutes(),
        0
    );
}

#[test]
fn auto_refresh_keeps_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = two_rows(dir.path(), Config::default());
    let b = srv("2.2.2.2");
    app.table.selected = Some(b.address.clone());
    let _ = run(&mut app);
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
    assert!(app.is_scanning());
    assert_eq!(app.table.selected, Some(b.address.clone()));
    complete(&mut app, &[srv("1.1.1.1"), b.clone()]);
    assert_eq!(app.servers().len(), 2);
    assert_eq!(app.table.selected, Some(b.address));
}

#[test]
fn auto_refresh_skips_when_busy() {
    for block in ["scanning", "modal", "files", "minimized"] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake) = two_rows(dir.path(), Config::default());
        let mut minimized = Some(false);
        match block {
            "scanning" => {
                let _ = app.update(Message::Refresh);
            }
            "modal" => {
                let _ = app.update(Message::Actions(ActionMsg::Run(Action::About)));
            }
            "files" => {
                let _ = app.update(Message::Actions(ActionMsg::Run(Action::ExportFavorites)));
                assert!(app.files_open());
            }
            _ => minimized = Some(true),
        }
        let scans = fake.calls().len();
        let task = app.update(Message::AutoRefresh(AutoMsg::Run { minimized }));
        assert_eq!(task.units(), 0, "{block}");
        assert_eq!(fake.calls().len(), scans, "{block}");
        if block != "scanning" {
            assert_eq!(app.servers().len(), 2, "{block}");
        }
    }
}

#[test]
fn auto_refresh_keeps_rows_and_drops_missing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = two_rows(dir.path(), Config::default());
    let a = srv("1.1.1.1");
    app.table.selected = Some(srv("2.2.2.2").address);
    let _ = run(&mut app);
    assert_eq!(app.servers().len(), 2); // no se vació al empezar
    let _ = server(&mut app, &a);
    assert_eq!(app.servers().len(), 2);
    finish(&mut app, true);
    assert_eq!(addresses(&app), std::slice::from_ref(&a.address));
    assert_eq!(app.table.selected, None); // la elegida ya no está
    // La caché también: al volver al juego se ve la lista nueva.
    let _ = app.update(Message::SelectGame("ut99"));
    let _ = app.update(Message::SelectGame("cod2"));
    assert_eq!(addresses(&app), [a.address]);
}

#[test]
fn auto_refresh_failed_master_keeps_rows() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = two_rows(dir.path(), Config::default());
    let _ = run(&mut app);
    finish(&mut app, false);
    assert_eq!(app.servers().len(), 2);
    // Detener a mitad tampoco quita nada.
    let _ = run(&mut app);
    let _ = server(&mut app, &srv("1.1.1.1"));
    let _ = app.update(Message::Stop);
    assert_eq!(app.servers().len(), 2);
}

#[test]
fn manual_refresh_still_clears() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = two_rows(dir.path(), Config::default());
    let _ = app.update(Message::Refresh);
    assert!(app.servers().is_empty());
}

#[test]
fn auto_refresh_does_not_scroll_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = two_rows(dir.path(), Config::default());
    let b = srv("2.2.2.2");
    app.table.selected = Some(b.address.clone());
    app.table.offset_y = 500.0;
    let task = run(&mut app);
    assert_eq!(widget_ops(task), 0);
    let mut ops = 0;
    for info in [srv("1.1.1.1"), b.clone()] {
        ops += widget_ops(server(&mut app, &info));
    }
    finish(&mut app, true);
    assert_eq!(ops, 0);
    assert_eq!(app.table.offset_y, 500.0);
    assert_eq!(app.table.selected, Some(b.address));
}

#[test]
fn auto_refresh_from_lan_view_starts_clean() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let (lan_row, net) = (srv("192.168.1.5"), srv("8.8.8.8"));
    complete(&mut app, std::slice::from_ref(&net));
    let _ = app.update(Message::ScanLan);
    complete(&mut app, std::slice::from_ref(&lan_row));
    assert!(app.scan.lan());
    app.table.selected = Some(lan_row.address.clone());
    let _ = run(&mut app);
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
    assert!(!addresses(&app).contains(&lan_row.address));
    assert_eq!(app.pending_select(), Some(&lan_row.address));
}

fn favorites_cfg(addresses: &[&str]) -> Config {
    let mut cfg = Config::default();
    let list = addresses.iter().map(|a| (*a).to_owned()).collect();
    cfg.favorites.insert("cod2".into(), list);
    cfg
}

fn show_favorites(app: &mut App) {
    let task = app.update(Message::Direct(DirectMsg::Show(DirectView::Favorites)));
    deliver(app, task);
}

#[test]
fn auto_refresh_in_favorites_requeries_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), favorites_cfg(&["2.2.2.2:28960"]));
    let online = srv("2.2.2.2");
    fake.answer_for(&online.address, Some(online.clone()));
    show_favorites(&mut app);
    assert_eq!(fake.queries().len(), 1);
    let task = run(&mut app);
    deliver(&mut app, task);
    assert_eq!(fake.queries().len(), 2);
    assert_eq!(fake.calls(), ["cod2"]);
    assert_eq!(app.direct_view(), Some(DirectView::Favorites));
    assert_eq!(addresses(&app), [online.address]);
}

#[test]
fn auto_refresh_keeps_rows_when_game_cannot_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::SelectGame("l4d2"));
    for host in ["1.1.1.1", "2.2.2.2"] {
        app.table.model.add_or_update(srv(host));
    }
    let l4d2 = game("l4d2").unwrap();
    assert!(!app.can_scan(l4d2));
    let scans = fake.calls().len();
    let _ = run(&mut app);
    assert_eq!(app.servers().len(), 2);
    assert_eq!(fake.calls().len(), scans);
    let _ = app.update(Message::SteamKey("K".into()));
    assert!(app.can_scan(l4d2));
}

#[test]
fn tick_asks_if_minimized() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = two_rows(dir.path(), Config::default());
    let task = app.update(Message::AutoRefresh(AutoMsg::Tick));
    assert!(window_requests(task) >= 1);
    assert_eq!(fake.calls(), ["cod2"]);
    // Sin saber si está minimizada (`None`), refresca.
    let _ = app.update(Message::AutoRefresh(AutoMsg::Run { minimized: None }));
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
}

// ------------------------------------------------------------------ selección pendiente

#[test]
fn pending_select_applies_when_row_arrives() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let (a, b) = (srv("1.1.1.1"), srv("2.2.2.2"));
    app.set_pending_select(Some(b.address.clone()));
    let _ = server(&mut app, &a);
    assert_eq!(app.table.selected, None);
    let shown = server(&mut app, &b);
    assert_eq!(app.table.selected, Some(b.address));
    assert_eq!(app.pending_select(), None);
    let _ = shown;
    finish(&mut app, true);
}

#[test]
fn user_selection_mid_scan_beats_stale_pending() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = two_rows(dir.path(), Config::default());
    let (a, b) = (srv("1.1.1.1"), srv("2.2.2.2"));
    app.table.selected = Some(a.address.clone());
    app.set_pending_select(Some(a.address.clone()));
    let _ = app.update(Message::Refresh); // F5 vacía la tabla: A queda pendiente
    let _ = server(&mut app, &b);
    let _ = app.update(Message::Table(table::Msg::RowClicked(b.address.clone())));
    assert_eq!(app.pending_select(), None);
    let _ = server(&mut app, &a);
    finish(&mut app, true);
    assert_eq!(app.table.selected, Some(b.address));
}

#[test]
fn pending_select_cleared_when_scan_ends() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    app.set_pending_select(Some(ServerAddress::new("9.9.9.9", 1)));
    complete(&mut app, &[srv("1.1.1.1")]);
    assert_eq!(app.pending_select(), None);
}

#[test]
fn pending_select_cleared_by_select_game() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = two_rows(dir.path(), Config::default());
    app.set_pending_select(Some(ServerAddress::new("9.9.9.9", 1)));
    let _ = app.update(Message::SelectGame("ut99"));
    assert_eq!(app.pending_select(), None);
}

#[test]
fn pending_offline_favorite_stays_selected() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), favorites_cfg(&["3.3.3.3:28960"]));
    let off = ServerAddress::new("3.3.3.3", 28960);
    show_favorites(&mut app);
    assert_eq!(addresses(&app), std::slice::from_ref(&off));
    let _ = app.update(Message::Table(table::Msg::RowClicked(off.clone())));
    let task = run(&mut app); // reconsulta: la fila sin conexión vuelve al terminar
    assert_eq!(app.pending_select(), Some(&off));
    deliver(&mut app, task);
    assert_eq!(fake.queries().len(), 2);
    assert_eq!(app.table.selected, Some(off));
    assert_eq!(app.pending_select(), None);
}

fn numbered(n: usize, ping: impl Fn(usize) -> u32) -> Vec<ServerInfo> {
    (0..n)
        .map(|i| {
            let host = format!("10.0.{}.{}", i / 256, i % 256);
            mk(&host, &format!("srv {i}"), ping(i), 3, "1.3")
        })
        .collect()
}

#[test]
fn refresh_prunes_gone_and_keeps_selection() {
    // La elegida vuelve (fila 10) o no vuelve (fila 4.990, entre las 500 que faltan).
    for (chosen, back) in [(10, true), (4_990, false)] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), Config::default());
        let first = numbered(5_000, |i| (i % 300) as u32);
        complete(&mut app, &first);
        app.table.selected = Some(first[chosen].address.clone());
        let _ = run(&mut app);
        let again = numbered(4_500, |i| ((i * 7) % 300) as u32);
        complete(&mut app, &again);
        assert_eq!(app.servers().len(), 4_500);
        let m = &app.table.model;
        assert_eq!(m.order().len(), 4_500);
        let pings: Vec<(u32, usize)> = m
            .order()
            .iter()
            .map(|&r| (m.servers()[r].ping_ms, r))
            .collect();
        assert!(pings.windows(2).all(|w| w[0] < w[1]), "orden por ping");
        let want = back.then(|| first[chosen].address.clone());
        assert_eq!(app.table.selected, want);
    }
}
