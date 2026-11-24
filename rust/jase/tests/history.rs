//! Historial de ocupación (`test_window_history.py`, `test_occupancy_chart.py`): registro de
//! favoritos y recientes, carga y guardado fuera del hilo de la UI, gráfico de la ficha y hora
//! local.

mod support;

use std::path::{Path, PathBuf};

use iced::Task;
use jase::actions::Msg as A;
use jase::app::{App, Message};
use jase::card;
use jase::chart::{self, ChartVm};
use jase::localtime::local_offset_s;
use jase::scan::ScanMsg;
use jase::table;
use jase::theme::palette;
use jase_core::config::Config;
use jase_core::history::History;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::ScanEvent;
use support::{FakeScan, TestApp, fake_app, fake_app_task, run_task};

const T0: f64 = 1_700_000_000.0;

fn addr(host: &str) -> ServerAddress {
    ServerAddress::new(host, 28960)
}

/// El `_info` de Python: 3 jugadores de `maxp`.
fn info(host: &str, online: bool, maxp: i32) -> ServerInfo {
    let a = addr(host);
    let mut i = ServerInfo::new(a.clone(), a);
    i.name = "s".into();
    i.map = "m".into();
    i.gametype = "dm".into();
    i.num_players = 3;
    i.max_players = maxp;
    i.version = "1.3".into();
    i.ping_ms = 20;
    i.online = online;
    i
}

fn cfg(favorites: &[&str], recent: &[&str]) -> Config {
    let mut c = Config::default();
    let list = |hs: &[&str]| hs.iter().map(|h| addr(h).to_string()).collect();
    c.favorites.insert("cod2".into(), list(favorites));
    c.recent.insert("cod2".into(), list(recent));
    c
}

fn history_path(dir: &Path) -> PathBuf {
    dir.join("cache").join("history.json")
}

/// Corre la `Task` del arranque y entrega a la app el historial leído.
fn deliver_history(app: &mut App, task: Task<Message>) {
    let mut loaded = false;
    for m in run_task(task) {
        if matches!(m, Message::HistoryLoaded(_)) {
            loaded = true;
            let _ = app.update(m);
        }
    }
    assert!(loaded, "el arranque no leyó el historial");
}

/// App con el historial ya leído y el reloj falso en `T0`.
fn loaded_app(dir: &Path, c: Config) -> (TestApp, FakeScan) {
    let (mut app, fake, task) = fake_app_task(dir, c);
    deliver_history(&mut app, task);
    (app, fake)
}

fn scan(app: &mut App, s: ServerInfo) {
    let generation = app.scan.generation();
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Event(ScanEvent::Server(s)),
    });
}

fn samples(app: &App, host: &str) -> Vec<[i64; 3]> {
    app.history.samples("cod2", &addr(host).to_string())
}

fn write_history(dir: &Path, json: &str) {
    std::fs::create_dir_all(dir.join("cache")).unwrap();
    std::fs::write(history_path(dir), json).unwrap();
}

#[test]
fn records_only_favorites_and_recents() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = loaded_app(dir.path(), cfg(&["9.9.9.9"], &["7.7.7.7"]));
    scan(&mut app, info("9.9.9.9", true, 8));
    scan(&mut app, info("8.8.8.8", true, 8));
    assert_eq!(samples(&app, "9.9.9.9"), vec![[T0 as i64, 3, 8]]);
    assert!(samples(&app, "8.8.8.8").is_empty());
    scan(&mut app, info("7.7.7.7", false, 8));
    scan(&mut app, info("7.7.7.7", true, 0));
    assert!(samples(&app, "7.7.7.7").is_empty());
    scan(&mut app, info("7.7.7.7", true, 8));
    assert_eq!(samples(&app, "7.7.7.7").len(), 1);
}

#[test]
fn query_response_is_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = loaded_app(dir.path(), cfg(&["9.9.9.9"], &[]));
    scan(&mut app, info("9.9.9.9", false, 8));
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr("9.9.9.9"))));
    fake.answer(Ok(Some(info("9.9.9.9", true, 8))));
    let task = app.update(Message::RefreshSelected);
    for m in run_task(task) {
        let _ = app.update(m);
    }
    assert_eq!(samples(&app, "9.9.9.9").len(), 1);
}

#[test]
fn saved_on_close() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = loaded_app(dir.path(), cfg(&["9.9.9.9"], &[]));
    scan(&mut app, info("9.9.9.9", true, 8));
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(*fake.exits.lock().unwrap(), vec![0]);
    let mut h = History::new(history_path(dir.path()));
    h.load(T0);
    assert_eq!(h.samples("cod2", "9.9.9.9:28960").len(), 1);
}

/// Review Focus 3: hasta que se lee el historial no se guarda (no se pisa con uno vacío).
#[test]
fn close_before_history_loads_keeps_file() {
    let dir = tempfile::tempdir().unwrap();
    let json = r#"{"cod2":{"9.9.9.9:28960":[[1699999000,5,8]]}}"#;
    write_history(dir.path(), json);
    let (mut app, _fake, _task) = fake_app_task(dir.path(), cfg(&["9.9.9.9"], &[]));
    scan(&mut app, info("9.9.9.9", true, 8));
    let _ = app.update(Message::HistorySave);
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(
        std::fs::read_to_string(history_path(dir.path())).unwrap(),
        json
    );
}

/// Decisión: lo que respondió antes de leer el historial no se registra (a lo sumo una
/// muestra por servidor, `MIN_GAP_S` es 300 s); al llegar, el historial es el del archivo.
#[test]
fn history_loaded_late_keeps_nothing_recorded_before() {
    let dir = tempfile::tempdir().unwrap();
    write_history(
        dir.path(),
        r#"{"cod2":{"9.9.9.9:28960":[[1699999000,5,8]]}}"#,
    );
    let (mut app, _fake, task) = fake_app_task(dir.path(), cfg(&["9.9.9.9"], &[]));
    assert!(!app.history_loaded);
    scan(&mut app, info("9.9.9.9", true, 8));
    assert!(samples(&app, "9.9.9.9").is_empty());
    deliver_history(&mut app, task);
    assert!(app.history_loaded);
    assert_eq!(samples(&app, "9.9.9.9"), vec![[1_699_999_000, 5, 8]]);
    scan(&mut app, info("9.9.9.9", true, 8));
    assert_eq!(samples(&app, "9.9.9.9").len(), 2);
}

#[test]
fn history_save_tick_writes_off_ui_thread() {
    let dir = tempfile::tempdir().unwrap();
    let json = r#"{"cod2":{"6.6.6.6:28960":[[1699999000,5,8]]}}"#;
    write_history(dir.path(), json);
    let (mut app, _) = loaded_app(dir.path(), cfg(&["9.9.9.9"], &[]));
    assert_eq!(samples(&app, "6.6.6.6").len(), 1);
    scan(&mut app, info("9.9.9.9", true, 8));
    let task = app.update(Message::HistorySave);
    assert_eq!(
        std::fs::read_to_string(history_path(dir.path())).unwrap(),
        json
    );
    let _ = run_task(task);
    let mut h = History::new(history_path(dir.path()));
    h.load(T0);
    assert_eq!(h.samples("cod2", "9.9.9.9:28960").len(), 1);
    // 6.6.6.6 ya no es favorito ni reciente: se poda.
    assert!(h.samples("cod2", "6.6.6.6:28960").is_empty());
    assert_eq!(app.history_keep().len(), 1);
}

/// Un guardado periódico que corre después del cierre (en vuelo al cerrar) no pisa el
/// archivo del cierre con datos más viejos.
#[test]
fn late_periodic_save_does_not_overwrite_close() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = loaded_app(dir.path(), cfg(&["9.9.9.9", "8.8.8.8"], &[]));
    scan(&mut app, info("9.9.9.9", true, 8));
    let stale = app.update(Message::HistorySave);
    scan(&mut app, info("8.8.8.8", true, 8));
    let _ = app.update(Message::Close { maximized: false });
    let _ = run_task(stale);
    let mut h = History::new(history_path(dir.path()));
    h.load(T0);
    assert_eq!(h.samples("cod2", "8.8.8.8:28960").len(), 1);
}

#[test]
fn chart_heights_and_empty() {
    assert!(!chart::has_data(&[None; 24]));
    let mut values = [None; 24];
    values[0] = Some(4.0);
    values[23] = Some(8.0);
    assert!(chart::has_data(&values));
    let h = chart::bar_heights(&values, 8, 40.0);
    assert_eq!((h[0], h[23], h[5]), (20.0, 40.0, 0.0));
    // El tope es el mayor promedio si supera el máximo; sin tope, todo en 0.
    assert_eq!(chart::bar_heights(&values, 4, 40.0)[0], 20.0);
    assert_eq!(chart::bar_heights(&[Some(0.0); 24], 0, 40.0), [0.0; 24]);
}

fn select(app: &mut App, host: &str) {
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr(host))));
}

#[test]
fn card_history_visibility() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), cfg(&["9.9.9.9"], &[]));
    scan(&mut app, info("9.9.9.9", true, 8));
    scan(&mut app, info("8.8.8.8", true, 8));
    // El gráfico va al lado de los datos: la ficha no crece con él.
    select(&mut app, "8.8.8.8");
    assert!(card::chart(&app).is_none());
    assert_eq!(card::height(&app), card::CARD_HEIGHT);
    let _ = card::view(&app);
    select(&mut app, "9.9.9.9");
    assert_eq!(card::height(&app), card::CARD_HEIGHT);
    let header = table::header_height(app.theme_id);
    assert_eq!(
        card::lower_min_height(&app),
        card::CARD_HEIGHT + 6.0 + header + table::ROW_HEIGHT
    );
    // Sin historial todavía: se muestra igual, vacío.
    let chart = card::chart(&app).unwrap();
    assert!(!chart::has_data(&chart.values));
    let _ = card::view(&app);
}

#[test]
fn peak_is_first_highest_hour() {
    let mut v = [None; 24];
    assert_eq!(chart::peak(&v), None);
    v[3] = Some(2.0);
    v[21] = Some(14.0);
    v[22] = Some(14.0);
    assert_eq!(chart::peak(&v), Some((21, 14.0)));
}

fn on_map(host: &str, map: &str, players: i32) -> ServerInfo {
    let mut i = info(host, true, 20);
    i.map = map.into();
    i.num_players = players;
    i
}

#[test]
fn map_stats_rank_and_top() {
    let mut servers = vec![
        on_map("1.1.1.1", "a", 10),
        on_map("1.1.1.2", "a", 5),
        on_map("1.1.1.3", "b", 12),
        on_map("1.1.1.4", "c", 3),
        on_map("1.1.1.5", "d", 2),
        on_map("1.1.1.6", "e", 0),
        on_map("1.1.1.7", "e", 0),
        on_map("1.1.1.8", "f", 0),
    ];
    // Sin respuesta no cuenta.
    let mut off = on_map("1.1.1.9", "f", 50);
    off.online = false;
    servers.push(off);
    let m = chart::map_stats(&servers, "a").unwrap();
    assert_eq!((m.servers, m.players, m.rank, m.total), (2, 15, 1, 6));
    let names: Vec<_> = m.top.iter().map(|t| (t.0.as_str(), t.1, t.2)).collect();
    assert_eq!(
        names,
        [
            ("a", 15, true),
            ("b", 12, false),
            ("c", 3, false),
            ("d", 2, false)
        ]
    );
    // Fuera de los primeros: ocupa el último lugar; a igual jugadores, más servidores antes.
    let m = chart::map_stats(&servers, "f").unwrap();
    assert_eq!((m.rank, m.total), (6, 6));
    assert_eq!(m.top.last().unwrap(), &("f".to_owned(), 0, true));
    assert_eq!(chart::map_stats(&servers, "e").unwrap().rank, 5);
    assert!(chart::map_stats(&servers, "zz").is_none());
    assert!(chart::map_stats(&servers, "").is_none());
}

#[test]
fn card_map_stats_follow_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), cfg(&[], &[]));
    scan(&mut app, on_map("8.8.8.8", "x", 4));
    scan(&mut app, on_map("7.7.7.7", "x", 6));
    select(&mut app, "8.8.8.8");
    let m = card::map_stats(&app).unwrap();
    assert_eq!((m.map.as_str(), m.servers, m.players), ("x", 2, 10));
    let _ = chart::map_view(Some(m), app.theme_id);
    let _ = chart::map_view(None, app.theme_id);
    let _ = chart::view(None, app.theme_id);
}

#[test]
fn chart_hidden_for_non_history_server() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = loaded_app(dir.path(), cfg(&[], &["7.7.7.7"]));
    scan(&mut app, info("8.8.8.8", true, 8));
    scan(&mut app, info("7.7.7.7", true, 8));
    select(&mut app, "8.8.8.8");
    assert!(card::chart(&app).is_none());
    select(&mut app, "7.7.7.7");
    assert!(card::chart(&app).is_some());
    // Otro juego: la tabla se vacía y el reciente de cod2 ya no está elegido.
    let _ = app.update(Message::SelectGame("cod4"));
    assert!(card::chart(&app).is_none());
}

#[test]
fn chart_hour_uses_local_offset() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = loaded_app(dir.path(), cfg(&["9.9.9.9"], &[]));
    fake.clock.set(T0, -3 * 3600);
    scan(&mut app, info("9.9.9.9", true, 8));
    select(&mut app, "9.9.9.9");
    // T0 es 22:13 UTC: 19 h a UTC−3.
    let c: &ChartVm = card::chart(&app).unwrap();
    assert_eq!(c.current_hour, 19);
    assert_eq!(c.max_players, 8);
    assert_eq!(c.values[19], Some(3.0));
    assert_eq!(c.values.iter().filter(|v| v.is_some()).count(), 1);
}

#[test]
fn chart_follows_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = loaded_app(dir.path(), cfg(&["9.9.9.9"], &[]));
    scan(&mut app, info("9.9.9.9", true, 8));
    select(&mut app, "9.9.9.9");
    let _ = app.update(Message::Actions(A::Theme("light")));
    assert_eq!(app.theme_id, "light");
    let c = card::chart(&app).unwrap().clone();
    let colors = chart::bar_colors(&c, palette(app.theme_id));
    let light = palette("light");
    assert_ne!(light.accent, palette("dark").accent);
    assert_eq!(colors[c.current_hour], light.accent);
    let other = (c.current_hour + 1) % 24;
    assert_eq!(colors[other], light.border); // hora sin datos
    let _ = chart::view(Some(&c), app.theme_id);
    let _ = card::view(&app);
}

#[test]
fn local_offset_is_sane() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    for t in [now, now - 182 * 86_400] {
        let off = local_offset_s(t);
        assert!(off.abs() <= 14 * 3600, "{off}");
        assert_eq!(off % 900, 0, "{off}");
    }
}
