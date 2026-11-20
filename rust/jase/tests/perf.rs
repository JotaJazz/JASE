//! Rendimiento con 5.000 filas, sin red: cada operación se mide con 1.000 y con 5.000 filas
//! (el mínimo de 3 corridas) y debe escalar casi lineal (`t(5000) ≤ 12·t(1000) + 5 ms`) y
//! quedar bajo un tope holgado (10× lo medido en WSL en debug: el CI de Windows es lento).
//! `cargo test --release -p jase --test perf -- --nocapture` imprime la tabla.
//!
//! A 5.000 filas, con operaciones baratas por fila, esa proporción no atrapa lo cuadrático:
//! recorrer `order` entero por cada respuesta (el `order.retain` de antes) daba 10,4×. El
//! benchmark a mano `perf_model_update_no_full_walk` (1.000 y 20.000 filas) lo muestra, pero no
//! es una guarda automática: el memmove del `Vec` al mover una fila (O(n), aceptado en
//! `deuda.md`) varía con la caché de cada máquina.

mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use iced::Task;
use jase::app::{App, AutoMsg, Message, SearchMsg};
use jase::scan::ScanMsg;
use jase::sidebar;
use jase::table::{self, Column, TableModel};
use jase::theme::VARIANTS;
use jase_core::config::Config;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{TestApp, fake_app, run_task};

/// Cuenta los bytes vivos de todo el proceso (solo en este binario de test).
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

// SAFETY: delega todo en `System`; solo suma y resta tamaños.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Relaxed);
        }
        p
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size() as isize, Relaxed);
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            LIVE.fetch_add(new as isize - l.size() as isize, Relaxed);
        }
        q
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Uno a la vez: los tiempos no compiten y el contador de memoria no suma lo de otro test.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

// Topes con 5.000 filas: 10× lo medido en WSL en debug, hacia arriba a 100 ms.
/// Medido: 78 ms.
const CAP_ARRIVALS: Duration = Duration::from_millis(800);
/// Medido: 120 ms (332 ms con `order.retain` y `remove` por fila).
const CAP_AUTO_REFRESH: Duration = Duration::from_millis(1_200);
/// Medido: 6,1 ms.
const CAP_TREE_TICK: Duration = Duration::from_millis(100);
/// Medido: 23,5 ms.
const CAP_TEXT_FILTER: Duration = Duration::from_millis(300);
/// Medido: 7,6 ms.
const CAP_SORT: Duration = Duration::from_millis(100);
/// Medido: 8,8 ms (la tabla solo arma las filas visibles).
const CAP_VIEW: Duration = Duration::from_millis(100);
/// Medido: 222 ms (4 listas de 5.000).
const CAP_PLAYER_SEARCH: Duration = Duration::from_millis(2_300);
/// Bytes por fila: 2× lo medido, hacia arriba a 1 KiB. Medido: 3.850.
const CAP_BYTES_PER_ROW: isize = 8 * 1024;

const MAPS: [&str; 8] = [
    "mp_toujane",
    "mp_carentan",
    "mp_dawnville",
    "mp_brecourt",
    "mp_burgundy",
    "mp_matmata",
    "mp_railyard",
    "mp_leningrad",
];

fn address(i: usize) -> ServerAddress {
    ServerAddress::new(
        format!("10.{}.{}.{}", i / 65_536, (i / 256) % 256, i % 256),
        28960,
    )
}

fn synthetic(n: usize) -> Vec<ServerInfo> {
    (0..n)
        .map(|i| {
            let a = address(i);
            let mut info = ServerInfo::new(a.clone(), a);
            info.raw_name = format!("^1Perf ^7{i}");
            info.name = format!("Perf {i}");
            info.map = MAPS[i % MAPS.len()].to_owned();
            info.gametype = "tdm".to_owned();
            info.num_players = (i % 21) as i32;
            info.max_players = 20;
            info.ping_ms = (i % 300) as u32;
            info.version = "1.3".to_owned();
            info.players = (0..12)
                .map(|k| Player::new(format!("Perf {i}-{k}"), k, 50))
                .collect();
            info.rules = (0..25)
                .map(|k| (format!("rule{k}"), format!("value {i} {k}")))
                .collect();
            info
        })
        .collect()
}

fn send(app: &mut App, msg: ScanMsg) -> Task<Message> {
    let generation = app.scan.generation();
    app.update(Message::Scan { generation, msg })
}

fn arrive(app: &mut App, servers: &[ServerInfo]) {
    for info in servers {
        let _ = send(app, ScanMsg::Event(ScanEvent::Server(info.clone())));
    }
}

fn finish(app: &mut App) {
    let result = ScanResult {
        masters_ok: 1,
        ..ScanResult::default()
    };
    let _ = send(app, ScanMsg::Finished(result));
}

/// Una app de CoD2 (con su carpeta temporal) y `n` filas ya escaneadas.
fn loaded(n: usize) -> (TestApp, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    arrive(&mut app, &synthetic(n));
    finish(&mut app);
    (app, dir)
}

/// El mínimo de 3 corridas de `op`, cada una sobre un `setup` nuevo (que no se cuenta).
fn measure<T>(mut setup: impl FnMut() -> T, mut op: impl FnMut(&mut T)) -> Duration {
    (0..3)
        .map(|_| {
            let mut state = setup();
            let start = Instant::now();
            op(&mut state);
            start.elapsed()
        })
        .min()
        .unwrap()
}

/// Casi lineal (cuadrático, sin costos fijos, daría 25×); los 5 ms absorben tiempos casi nulos.
fn scales(t1000: Duration, t5000: Duration) -> bool {
    t5000 <= t1000 * 12 + Duration::from_millis(5)
}

fn assert_scales(name: &str, t1000: Duration, t5000: Duration, cap: Duration) {
    println!("{name:<24} {t1000:>12.2?} {t5000:>12.2?}");
    assert!(
        scales(t1000, t5000),
        "{name}: no escala ({t1000:?} → {t5000:?})"
    );
    assert!(t5000 <= cap, "{name}: {t5000:?} pasa el tope {cap:?}");
}

/// Mide `op` sobre una app cargada con 1.000 y con 5.000 filas.
fn on_loaded(name: &str, cap: Duration, op: impl Fn(&mut App, usize)) {
    let _serial = serial();
    let t = |n| measure(|| loaded(n), |(app, _)| op(app, n));
    assert_scales(name, t(1_000), t(5_000), cap);
}

#[test]
fn perf_ratio_ignores_tiny_times() {
    let _serial = serial();
    let ms = Duration::from_millis;
    assert!(scales(Duration::ZERO, Duration::ZERO));
    assert!(scales(Duration::ZERO, ms(4)));
    assert!(scales(ms(10), ms(60)));
    assert!(!scales(ms(10), ms(250))); // cuadrático
    assert_scales("tiny", Duration::ZERO, ms(1), ms(100));
}

/// Filas sin jugadores ni reglas: lo que pesa es el orden, no copiar datos.
fn light(n: usize, ping: impl Fn(usize) -> u32) -> Vec<ServerInfo> {
    (0..n)
        .map(|i| {
            let a = address(i);
            let mut info = ServerInfo::new(a.clone(), a);
            info.name = format!("Perf {i}");
            info.ping_ms = ping(i);
            info
        })
        .collect()
}

// Benchmark a mano (`cargo test -p jase --test perf -- --ignored`): el memmove aceptado de
// `order` depende de la caché de cada máquina. En WSL, en debug, el recorrido completo daba
// 350× y el código actual 65×; en el runner de Linux del CI el código actual dio 428×, así que
// la proporción no sirve de guarda entre máquinas. La corrección la cubren los proptests de
// `table_model.rs`.
#[test]
#[ignore = "benchmark a mano: la proporción depende de la caché de la máquina (428× en el CI)"]
fn perf_model_update_no_full_walk() {
    let _serial = serial();
    let t = |n| {
        let again = light(n, |i| ((i * 7) % 300) as u32);
        measure(
            || {
                let mut m = TableModel::default();
                for info in light(n, |i| (i % 300) as u32) {
                    m.add_or_update(info);
                }
                (m, again.clone())
            },
            |(m, again)| {
                for info in again.drain(..) {
                    m.add_or_update(info);
                }
            },
        )
    };
    let (t1k, t20k) = (t(1_000), t(20_000));
    println!("{:<24} {t1k:>12.2?} {t20k:>12.2?}", "model_update 1k/20k");
    // Lineal ≈ 20×, n·log n ≈ 29×; el recorrido completo de `order` ≈ 350×.
    assert!(
        t20k <= t1k * 150 + Duration::from_millis(5),
        "add_or_update no escala ({t1k:?} → {t20k:?})"
    );
}

#[test]
fn perf_arrivals() {
    let _serial = serial();
    let t = |n| {
        let servers = synthetic(n);
        measure(
            || {
                let dir = tempfile::tempdir().unwrap();
                (fake_app(dir.path(), Config::default()).0, dir)
            },
            |(app, _)| {
                arrive(app, &servers);
                finish(app);
            },
        )
    };
    assert_scales("arrivals", t(1_000), t(5_000), CAP_ARRIVALS);
}

#[test]
fn perf_auto_refresh() {
    let _serial = serial();
    let t = |n| {
        // Otro ping y el 10 % (cada décima) sin responder.
        let again: Vec<ServerInfo> = synthetic(n)
            .into_iter()
            .enumerate()
            .filter(|(i, _)| i % 10 != 9)
            .map(|(i, mut s)| {
                s.ping_ms = ((i * 7) % 300) as u32;
                s
            })
            .collect();
        measure(
            || loaded(n),
            |(app, _)| {
                let _ = app.update(Message::AutoRefresh(AutoMsg::Run {
                    minimized: Some(false),
                }));
                arrive(app, &again);
                finish(app);
                assert_eq!(app.servers().len(), n - n / 10);
            },
        )
    };
    assert_scales("auto_refresh", t(1_000), t(5_000), CAP_AUTO_REFRESH);
}

#[test]
fn perf_tree_tick() {
    on_loaded("tree_tick", CAP_TREE_TICK, |app, _| {
        let _ = app.update(Message::TreeTick);
    });
}

#[test]
fn perf_text_filter() {
    on_loaded("text_filter", CAP_TEXT_FILTER, |app, n| {
        let text = "perf 4";
        let typed = (1..=text.len()).chain((0..text.len()).rev());
        for len in typed {
            let t = text[..len].to_owned();
            let _ = app.update(Message::Sidebar(sidebar::Msg::Text(t)));
        }
        assert_eq!(app.table.model.order().len(), n);
    });
}

#[test]
fn perf_sort_every_column() {
    on_loaded("sort_every_column", CAP_SORT, |app, _| {
        for col in Column::ALL {
            let _ = app.update(Message::Table(table::Msg::HeaderClicked(col)));
        }
    });
}

#[test]
fn perf_view_every_theme() {
    on_loaded("view_every_theme", CAP_VIEW, |app, n| {
        app.table.selected = Some(address(n / 2));
        for (id, _, _) in VARIANTS.iter() {
            app.theme_id = id;
            drop(app.view());
        }
    });
}

#[test]
fn perf_player_search() {
    let _serial = serial();
    let t = |n| {
        measure(
            || {
                let dir = tempfile::tempdir().unwrap();
                let (mut app, _) = fake_app(dir.path(), Config::default());
                let servers = synthetic(n);
                for game in ["cod2", "cod4", "q3", "ut99"] {
                    let _ = app.update(Message::SelectGame(game));
                    arrive(&mut app, &servers);
                    finish(&mut app);
                }
                let _ = app.update(Message::Search(SearchMsg::Open));
                (app, dir)
            },
            |(app, _)| {
                let wait = app.update(Message::Search(SearchMsg::Query("perf 49".into())));
                for m in run_task(wait) {
                    let _ = app.update(m); // `Debounced`
                }
                assert!(!app.player_search().unwrap().hits.is_empty());
            },
        )
    };
    assert_scales("player_search", t(1_000), t(5_000), CAP_PLAYER_SEARCH);
}

#[test]
fn perf_memory_per_row() {
    let _serial = serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let servers = synthetic(5_000);
    let before = LIVE.load(Relaxed);
    arrive(&mut app, &servers);
    let per_row = (LIVE.load(Relaxed) - before) / 5_000;
    println!("{:<24} {per_row:>12} B/fila", "memory_per_row");
    assert!(
        per_row <= CAP_BYTES_PER_ROW,
        "{per_row} B por fila, tope {CAP_BYTES_PER_ROW}"
    );
    finish(&mut app);
}
