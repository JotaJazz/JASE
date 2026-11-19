mod support;

use jase::colored::color_runs;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use jase::table::{
    Column, CountryLookup, OFFLINE_PING_SORT, ServerFilter, Sort, SortKey, TableModel, cell_text,
    occupancy_ratio, players_text, sort_key,
};
use jase::theme::{palette, ping_color};
use jase_core::games::{ALL_VERSIONS, game};
use jase_core::models::QuickFilters;
use jase_core::models::{ServerAddress, ServerInfo};
use proptest::prelude::*;

fn same(v: &str) -> String {
    v.to_owned()
}

fn mk(host: &str, name: &str, ping: u32, players: i32, version: &str) -> ServerInfo {
    let a = ServerAddress::new(host, 28960);
    let mut info = ServerInfo::new(a.clone(), a);
    info.name = name.to_owned();
    info.map = "mp_toujane".to_owned();
    info.gametype = "tdm".to_owned();
    info.num_players = players;
    info.max_players = 20;
    info.version = version.to_owned();
    info.ping_ms = ping;
    info
}

fn names(m: &TableModel) -> Vec<&str> {
    m.order()
        .iter()
        .map(|&r| m.servers()[r].name.as_str())
        .collect()
}

#[test]
fn add_update_and_display() {
    let mut m = TableModel::default();
    let mut a = mk("1.1.1.1", "A", 50, 3, "1.3");
    a.password = true;
    m.add_or_update(a);
    assert_eq!(m.add_or_update(mk("1.1.1.1", "A2", 40, 4, "1.3")), 0); // misma dirección
    assert_eq!(m.len(), 1);
    let info = &m.servers()[0];
    assert_eq!(cell_text(info, Column::Name, &same), "A2");
    assert_eq!(cell_text(info, Column::Players, &same), "4/20");
    assert_eq!(cell_text(info, Column::Lock, &same), "");
}

#[test]
fn lock_icon_and_clear() {
    let mut m = TableModel::default();
    let mut a = mk("1.1.1.1", "A", 50, 3, "1.3");
    a.password = true;
    m.add_or_update(a);
    assert_eq!(cell_text(&m.servers()[0], Column::Lock, &same), "🔒");
    assert_eq!(m.servers()[0].name, "A");
    assert_eq!(m.servers().len(), 1);
    m.clear();
    assert_eq!(m.len(), 0);
    assert!(m.servers().is_empty() && m.order().is_empty());
}

#[test]
fn numeric_sort_on_ping() {
    let mut m = TableModel::default();
    for (h, ping) in [("1.1.1.1", 100), ("2.2.2.2", 9), ("3.3.3.3", 25)] {
        m.add_or_update(mk(h, h, ping, 1, "1.3"));
    }
    m.set_sort(Sort {
        column: Column::Ping,
        ascending: true,
    });
    let keys: Vec<SortKey> = m
        .order()
        .iter()
        .map(|&r| sort_key(&m.servers()[r], Column::Ping, &same, None))
        .collect();
    assert_eq!(keys, [SortKey::Int(9), SortKey::Int(25), SortKey::Int(100)]);
}

#[test]
fn initial_sort_is_ping_ascending() {
    let mut m = TableModel::default();
    for (h, ping) in [("a", 100), ("b", 9), ("c", 25)] {
        m.add_or_update(mk(h, h, ping, 1, "1.3"));
    }
    assert_eq!(names(&m), ["b", "c", "a"]);
}

#[test]
fn ping_foreground_follows_mode() {
    let info = mk("1.1.1.1", "A", 50, 3, "1.3");
    assert_eq!(
        ping_color(info.ping_ms, palette("dark")),
        palette("dark").ping_good
    );
    assert_eq!(
        ping_color(info.ping_ms, palette("light")),
        palette("light").ping_good
    );
    assert_ne!(palette("dark").ping_good, palette("light").ping_good);
}

#[test]
fn name_column_exposes_color_runs() {
    let mut info = mk("1.1.1.1", "Jimmy TDM", 50, 1, "1.3");
    info.raw_name = "^1Jimmy^7 TDM".to_owned();
    assert_eq!(cell_text(&info, Column::Name, &same), "Jimmy TDM");
    let runs = color_runs(&info.raw_name, palette("dark"));
    let texts: Vec<&str> = runs.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(texts, ["Jimmy", " TDM"]);
}

#[test]
fn occupancy_ratio_is_clamped() {
    assert_eq!(occupancy_ratio(5, 20), 0.25);
    assert_eq!(occupancy_ratio(0, 20), 0.0);
    assert_eq!(occupancy_ratio(25, 20), 1.0); // nunca más de la celda
    assert_eq!(occupancy_ratio(3, 0), 0.0); // sin máximo conocido: sin barra
}

#[test]
fn country_column_between_lock_and_name() {
    let pos = |c| Column::ALL.iter().position(|&x| x == c).unwrap();
    assert!(pos(Column::Lock) < pos(Column::Country));
    assert!(pos(Column::Country) < pos(Column::Name));
    support::lang::with_language("es", || {
        assert_eq!(Column::Country.title(), "País");
        assert_eq!(Column::Name.title(), "Servidor");
    });
    assert_eq!(Column::Lock.title(), "");
    assert_eq!(Column::Name.initial_width(), None);
    assert_eq!(Column::Map.initial_width(), Some(170.0));
}

#[test]
fn row_of() {
    let mut m = TableModel::default();
    let a = mk("1.1.1.1", "A", 50, 3, "1.3");
    let addr = a.address.clone();
    m.add_or_update(a);
    m.add_or_update(mk("2.2.2.2", "B", 50, 3, "1.3"));
    assert_eq!(m.row_of(&addr), Some(0));
    assert_eq!(m.row_of(&ServerAddress::new("9.9.9.9", 28960)), None);
    m.remove(&addr);
    assert_eq!(m.row_of(&ServerAddress::new("2.2.2.2", 28960)), Some(0));
    assert_eq!(m.order(), [0]);
}

#[test]
fn players_column_shows_humans_and_bots() {
    let mut m = TableModel::default();
    let mut bots_only = mk("1.1.1.1", "bots", 50, 6, "1.3");
    bots_only.bots = 6;
    let mut mixed = mk("2.2.2.2", "mixto", 50, 8, "1.3");
    mixed.bots = 6;
    m.add_or_update(bots_only);
    m.add_or_update(mixed);
    m.add_or_update(mk("3.3.3.3", "gente", 50, 3, "1.3"));
    assert_eq!(cell_text(&m.servers()[1], Column::Players, &same), "2+6/20");
    assert_eq!(cell_text(&m.servers()[2], Column::Players, &same), "3/20");
    // se ordena por personas: 3 personas > 2 personas + 6 bots > solo bots
    m.set_sort(Sort {
        column: Column::Players,
        ascending: false,
    });
    assert_eq!(names(&m), ["gente", "mixto", "bots"]);
}

#[test]
fn players_text_with_bots() {
    let mut bots = mk("1.1.1.1", "A", 20, 8, "1.3");
    bots.bots = 6;
    bots.max_players = 16;
    let mut plain = mk("2.2.2.2", "B", 20, 8, "1.3");
    plain.max_players = 16;
    assert_eq!(players_text(&bots), "2+6/16");
    assert_eq!(players_text(&plain), "8/16");
}

#[test]
fn offline_rows_sort_last_by_ping() {
    let mut m = TableModel::default();
    let mut off = mk("1.1.1.1", "off", 0, 0, "1.3");
    off.online = false;
    assert_eq!(
        sort_key(&off, Column::Ping, &same, None),
        SortKey::Int(OFFLINE_PING_SORT)
    );
    assert_eq!(OFFLINE_PING_SORT, (1 << 31) - 1);
    assert_eq!(cell_text(&off, Column::Ping, &same), "—");
    assert_eq!(cell_text(&off, Column::Players, &same), "—");
    m.add_or_update(off);
    m.add_or_update(mk("2.2.2.2", "lejos", 900, 0, "1.3"));
    m.add_or_update(mk("3.3.3.3", "cerca", 5, 0, "1.3"));
    assert_eq!(names(&m), ["cerca", "lejos", "off"]);
}

#[test]
fn version_column_sorts_by_label() {
    let mut m = TableModel::default();
    m.add_or_update(mk("1.1.1.1", "a", 1, 0, "z"));
    m.add_or_update(mk("2.2.2.2", "b", 1, 0, "y"));
    m.set_sort(Sort {
        column: Column::Version,
        ascending: true,
    });
    assert_eq!(names(&m), ["b", "a"]);
    m.set_version_label(|v| if v == "y" { "Z".into() } else { "A".into() });
    assert_eq!(
        cell_text(&m.servers()[0], Column::Version, &|v| m.version_label(v)),
        "A"
    );
    assert_eq!(names(&m), ["a", "b"]);
}

#[derive(Debug, Clone)]
enum Op {
    Upsert(u8, u32, i32, i32, bool, String),
    Remove(u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0u8..12, 0u32..300, 0i32..20, 0i32..20, any::<bool>(), "[a-cA-C]{0,3}")
            .prop_map(|(h, p, n, b, on, s)| Op::Upsert(h, p, n, b, on, s)),
        1 => (0u8..12).prop_map(Op::Remove),
    ]
}

proptest! {
    #[test]
    fn incremental_order_matches_full_sort(
        ops in prop::collection::vec(op(), 0..60),
        col in 0usize..Column::ALL.len(),
        ascending: bool,
    ) {
        let column = Column::ALL[col];
        let mut m = TableModel::default();
        m.set_sort(Sort { column, ascending });
        for op in ops {
            match op {
                Op::Upsert(h, ping, n, bots, online, s) => {
                    let mut info = mk(&format!("10.0.0.{h}"), &s, ping, n, &s);
                    info.bots = bots.min(n);
                    info.online = online;
                    info.password = online && ping % 2 == 0;
                    info.map = s.to_uppercase();
                    m.add_or_update(info);
                }
                Op::Remove(h) => m.remove(&ServerAddress::new(format!("10.0.0.{h}"), 28960)),
            }
            let mut full: Vec<usize> = (0..m.len()).collect();
            full.sort_by(|&a, &b| {
                let (ka, kb) = (
                    sort_key(&m.servers()[a], column, &same, None),
                    sort_key(&m.servers()[b], column, &same, None),
                );
                let o = ka.cmp(&kb);
                (if ascending { o } else { o.reverse() }).then(a.cmp(&b))
            });
            prop_assert_eq!(m.order(), full.as_slice());
            for (i, info) in m.servers().iter().enumerate() {
                prop_assert_eq!(m.row_of(&info.address), Some(i));
            }
        }
    }
}

// ------------------------------------------------------------------ filtros

fn sorted_names(m: &TableModel) -> Vec<&str> {
    let mut v = names(m);
    v.sort();
    v
}

fn quick(q: QuickFilters) -> ServerFilter {
    ServerFilter {
        quick: q,
        ..ServerFilter::default()
    }
}

fn lookup(pairs: &'static [(&'static str, &'static str)]) -> Option<CountryLookup> {
    Some(Arc::new(move |ip: &str| {
        pairs
            .iter()
            .find(|(h, _)| *h == ip)
            .map(|(_, c)| (*c).to_owned())
    }))
}

const COUNTRIES: &[(&str, &str)] = &[("1.1.1.1", "cl"), ("2.2.2.2", "br")];

fn mk_rules(host: &str, name: &str, players: i32, map: &str, fs_game: &str) -> ServerInfo {
    let mut info = mk(host, name, 50, players, "1.3");
    info.map = map.to_owned();
    if !fs_game.is_empty() {
        info.rules.insert("fs_game".into(), fs_game.into());
    }
    info
}

fn set<const N: usize>(v: [&str; N]) -> HashSet<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn default_filter_shows_everything() {
    let f = ServerFilter::default();
    assert_eq!(f.version, ALL_VERSIONS);
    assert!(f.accepts(&mk("1.1.1.1", "a", 9999, 0, "x"), None));
}

#[test]
fn text_and_version_filter() {
    let mut m = TableModel::default();
    m.add_or_update(mk("1.1.1.1", "Clan Hispano", 50, 1, "1.3"));
    let mut b = mk("2.2.2.2", "Euro Vets", 50, 1, "1.0");
    b.map = "mp_carentan".into();
    m.add_or_update(b);
    let mut f = ServerFilter::default();
    f.set_text("  HISP ");
    assert_eq!(f.text, "hisp");
    m.set_filter(f.clone());
    assert_eq!(m.order().len(), 1);
    f.set_text("carentan");
    m.set_filter(f.clone());
    assert_eq!(m.order().len(), 1);
    f.set_text("");
    f.version = "1.3".into();
    m.set_filter(f.clone());
    assert_eq!(names(&m), ["Clan Hispano"]);
    f.version = ALL_VERSIONS.into();
    m.set_filter(f);
    assert_eq!(m.order().len(), 2);
    assert_eq!(m.filter().version, ALL_VERSIONS);
}

fn quick_model() -> TableModel {
    let mut m = TableModel::default();
    m.add_or_update(mk("1.1.1.1", "vacio", 40, 0, "1.3"));
    m.add_or_update(mk("2.2.2.2", "lleno", 40, 20, "1.3"));
    let mut pw = mk("3.3.3.3", "clave", 40, 5, "1.3");
    pw.password = true;
    m.add_or_update(pw);
    m.add_or_update(mk("4.4.4.4", "lejos", 120, 5, "1.3"));
    m
}

#[test]
fn quick_filter_hide_empty() {
    let mut m = quick_model();
    m.set_filter(quick(QuickFilters {
        hide_empty: true,
        ..QuickFilters::default()
    }));
    assert_eq!(sorted_names(&m), ["clave", "lejos", "lleno"]);
}

#[test]
fn quick_filter_hide_full() {
    let mut m = quick_model();
    // max_players == 0 nunca cuenta como lleno
    let mut nomax = mk("5.5.5.5", "sinmax", 40, 0, "1.3");
    nomax.max_players = 0;
    m.add_or_update(nomax);
    m.set_filter(quick(QuickFilters {
        hide_full: true,
        ..QuickFilters::default()
    }));
    assert_eq!(sorted_names(&m), ["clave", "lejos", "sinmax", "vacio"]);
}

#[test]
fn quick_filter_hide_password() {
    let mut m = quick_model();
    m.set_filter(quick(QuickFilters {
        hide_password: true,
        ..QuickFilters::default()
    }));
    assert_eq!(sorted_names(&m), ["lejos", "lleno", "vacio"]);
}

#[test]
fn quick_filter_max_ping() {
    let mut m = quick_model();
    m.set_filter(quick(QuickFilters {
        max_ping: 100,
        ..QuickFilters::default()
    }));
    assert_eq!(sorted_names(&m), ["clave", "lleno", "vacio"]);
    m.set_filter(ServerFilter::default()); // 0 = sin límite
    assert_eq!(m.order().len(), 4);
}

#[test]
fn hide_bots_only_hides_all_bot_servers() {
    let mut m = TableModel::default();
    let mut bots = mk("1.1.1.1", "bots", 40, 4, "1.3");
    bots.bots = 4;
    let mut mixed = mk("2.2.2.2", "mixto", 40, 4, "1.3");
    mixed.bots = 3;
    m.add_or_update(bots);
    m.add_or_update(mixed);
    m.add_or_update(mk("3.3.3.3", "vacio", 40, 0, "1.3")); // 0 jugadores: no es «solo bots»
    m.set_filter(quick(QuickFilters {
        hide_bots: true,
        ..QuickFilters::default()
    }));
    assert_eq!(sorted_names(&m), ["mixto", "vacio"]);
}

#[test]
fn country_quick_filter() {
    let mut m = TableModel::default();
    m.set_country_lookup(lookup(&[("1.1.1.1", "CL"), ("2.2.2.2", "AR")]));
    for h in ["1.1.1.1", "2.2.2.2", "3.3.3.3"] {
        m.add_or_update(mk(h, h, 50, 1, "1.3"));
    }
    m.set_filter(quick(QuickFilters {
        country: "cl".into(),
        ..QuickFilters::default()
    }));
    assert_eq!(names(&m), ["1.1.1.1"]);
    m.set_filter(ServerFilter::default());
    assert_eq!(m.order().len(), 3);
}

#[test]
fn text_filter_matches_country_code() {
    let mut m = TableModel::default();
    m.add_or_update(mk("1.1.1.1", "chileno", 40, 3, "1.3"));
    m.add_or_update(mk("2.2.2.2", "brasileño", 40, 3, "1.3"));
    m.add_or_update(mk("3.3.3.3", "otro", 40, 3, "1.3"));
    let mut f = ServerFilter::default();
    f.set_text("cl");
    m.set_filter(f.clone());
    assert!(names(&m).is_empty()); // sin base todavía
    m.set_country_lookup(lookup(COUNTRIES));
    assert_eq!(names(&m), ["chileno"]); // el filtro se reevalúa al llegar la base
    f.set_text("BR");
    m.set_filter(f);
    assert_eq!(names(&m), ["brasileño"]);
}

#[test]
fn sort_by_country() {
    support::lang::with_language("es", || {
        let mut m = TableModel::default();
        m.set_country_lookup(lookup(&[
            ("1.1.1.1", "cl"),
            ("2.2.2.2", "br"),
            ("4.4.4.4", "de"),
        ]));
        m.add_or_update(mk("1.1.1.1", "chileno", 40, 3, "1.3"));
        m.add_or_update(mk("3.3.3.3", "nada", 40, 3, "1.3"));
        m.add_or_update(mk("2.2.2.2", "brasileño", 40, 3, "1.3"));
        m.add_or_update(mk("4.4.4.4", "alemán", 40, 3, "1.3"));
        m.set_sort(Sort {
            column: Column::Country,
            ascending: true,
        });
        // Por nombre en español (Alemania < Brasil < Chile; «de» iría después de «cl»).
        // Sin código la clave es "": primero en ascendente, como Python 1.3.0.
        assert_eq!(names(&m), ["nada", "alemán", "brasileño", "chileno"]);
        assert_eq!(
            sort_key(&m.servers()[3], Column::Country, &same, Some("de")),
            SortKey::Text("alemania".into())
        );
        m.set_sort(Sort {
            column: Column::Country,
            ascending: false,
        });
        assert_eq!(names(&m), ["chileno", "brasileño", "alemán", "nada"]);
    });
}

#[test]
fn tree_filter_combines_with_text_and_quick() {
    let mut m = TableModel::default();
    m.add_or_update(mk_rules("1.1.1.1", "Alpha", 3, "mp_toujane", "mods/promod"));
    m.add_or_update(mk_rules(
        "2.2.2.2",
        "Alpha vacio",
        0,
        "mp_toujane",
        "mods/promod",
    ));
    m.add_or_update(mk_rules("3.3.3.3", "Beta", 3, "mp_toujane", "mods/promod"));
    m.add_or_update(mk_rules("4.4.4.4", "Alpha otro", 3, "mp_toujane", ""));
    m.add_or_update(mk_rules(
        "5.5.5.5",
        "Alpha mapa",
        3,
        "mp_carentan",
        "mods/promod",
    ));
    let mut f = ServerFilter {
        tree_game: game("cod2"),
        mods: set(["promod"]),
        maps: set(["mp_toujane"]),
        ..ServerFilter::default()
    };
    m.set_filter(f.clone());
    assert_eq!(m.order().len(), 3);
    f.set_text("alpha");
    f.quick.hide_empty = true;
    m.set_filter(f);
    assert_eq!(names(&m), ["Alpha"]);
}

#[test]
fn tree_filter_none_shows_all() {
    let mut m = TableModel::default();
    m.add_or_update(mk_rules("1.1.1.1", "A", 3, "mp_toujane", "mods/promod"));
    m.add_or_update(mk_rules("2.2.2.2", "B", 3, "mp_carentan", ""));
    let mut f = ServerFilter {
        tree_game: game("cod2"),
        mods: set(["promod"]),
        ..ServerFilter::default()
    };
    m.set_filter(f.clone());
    assert_eq!(m.order().len(), 1);
    f.tree_game = None;
    m.set_filter(f.clone());
    assert_eq!(m.order().len(), 2);
    f.tree_game = game("cod2");
    f.mods.clear();
    m.set_filter(f);
    assert_eq!(m.order().len(), 2);
}

#[test]
fn tree_filter_by_countries() {
    let mut m = TableModel::default();
    m.set_country_lookup(lookup(&[("1.1.1.1", "CL"), ("2.2.2.2", "AR")]));
    for (h, n) in [("1.1.1.1", "A"), ("2.2.2.2", "B"), ("3.3.3.3", "C")] {
        m.add_or_update(mk_rules(h, n, 3, "mp_toujane", ""));
    }
    // Los países cuentan aunque no haya juego.
    let mut f = ServerFilter {
        countries: set(["cl"]),
        ..ServerFilter::default()
    };
    m.set_filter(f.clone());
    assert_eq!(names(&m), ["A"]);
    f.countries = set(["cl", "ar"]);
    m.set_filter(f.clone());
    assert_eq!(m.order().len(), 2);
    f.countries.clear();
    m.set_filter(f);
    assert_eq!(m.order().len(), 3);
}

fn counting(calls: &Arc<AtomicUsize>) -> Option<CountryLookup> {
    let calls = calls.clone();
    Some(Arc::new(move |_: &str| {
        calls.fetch_add(1, Ordering::SeqCst);
        Some("cl".to_owned())
    }))
}

#[test]
fn clear_forgets_country_cache() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut m = TableModel::default();
    m.set_country_lookup(counting(&calls));
    m.add_or_update(mk("1.1.1.1", "A", 20, 3, "1.3"));
    assert_eq!(m.country_code(&m.servers()[0]), Some("cl"));
    m.clear();
    assert_eq!(m.country_code(&mk("1.1.1.1", "A", 20, 3, "1.3")), None);
    m.add_or_update(mk("1.1.1.1", "A", 20, 3, "1.3"));
    assert_eq!(calls.load(Ordering::SeqCst), 2); // se vuelve a buscar
}

#[test]
fn set_country_lookup_repaints_country_column() {
    support::lang::with_language("es", || {
        let mut m = TableModel::default();
        m.set_sort(Sort {
            column: Column::Country,
            ascending: true,
        });
        m.add_or_update(mk("1.1.1.1", "a", 40, 3, "1.3"));
        m.add_or_update(mk("2.2.2.2", "b", 40, 3, "1.3"));
        assert_eq!(names(&m), ["a", "b"]);
        assert_eq!(m.country_code(&m.servers()[0]), None);
        m.set_country_lookup(lookup(COUNTRIES));
        assert_eq!(m.country_code(&m.servers()[0]), Some("cl"));
        assert_eq!(names(&m), ["b", "a"]); // Brasil < Chile: se reordena
        m.set_country_lookup(None);
        assert_eq!(m.country_code(&m.servers()[0]), None);
        assert_eq!(names(&m), ["a", "b"]);
    });
}

#[test]
fn country_lookup_runs_once_per_row() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut m = TableModel::default();
    m.set_country_lookup(counting(&calls));
    for i in 0..20 {
        m.add_or_update(mk(&format!("10.0.0.{i}"), "s", i, 1, "1.3"));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 20);
    for col in Column::ALL {
        m.set_sort(Sort {
            column: col,
            ascending: false,
        });
    }
    let mut f = ServerFilter::default();
    f.set_text("cl");
    f.countries = set(["cl"]);
    f.quick.country = "cl".into();
    m.set_filter(f);
    assert_eq!(m.order().len(), 20);
    for info in m.servers() {
        assert_eq!(m.country_code(info), Some("cl"));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 20); // ordenar, filtrar y leer: sin búsquedas
    m.add_or_update(mk("10.0.0.3", "s", 3, 2, "1.3")); // reemplazo: una más
    assert_eq!(calls.load(Ordering::SeqCst), 21);
}

#[test]
fn len_counts_all_rows_and_order_only_visible() {
    let mut m = quick_model();
    m.set_filter(quick(QuickFilters {
        hide_empty: true,
        hide_password: true,
        ..QuickFilters::default()
    }));
    assert_eq!(m.len(), 4);
    assert_eq!(m.servers().len(), 4);
    assert_eq!(sorted_names(&m), ["lejos", "lleno"]);
    // Una fila que llega o cambia respeta el filtro.
    m.add_or_update(mk("1.1.1.1", "vacio", 40, 3, "1.3"));
    m.add_or_update(mk("2.2.2.2", "lleno", 40, 0, "1.3"));
    m.add_or_update(mk("9.9.9.9", "nuevo", 40, 0, "1.3"));
    assert_eq!(m.len(), 5);
    assert_eq!(sorted_names(&m), ["lejos", "vacio"]);
    m.remove(&ServerAddress::new("1.1.1.1", 28960));
    assert_eq!(sorted_names(&m), ["lejos"]);
    assert!(!m.is_shown(&ServerAddress::new("2.2.2.2", 28960)));
    assert!(m.is_shown(&ServerAddress::new("4.4.4.4", 28960)));
}

#[derive(Debug, Clone)]
enum FOp {
    Upsert(u8, u32, i32, bool, String),
    Remove(u8),
    Filter(bool, bool, i64, String, bool),
}

fn fop() -> impl Strategy<Value = FOp> {
    prop_oneof![
        4 => (0u8..12, 0u32..300, 0i32..21, any::<bool>(), "[a-c]{0,3}")
            .prop_map(|(h, p, n, pw, s)| FOp::Upsert(h, p, n, pw, s)),
        1 => (0u8..12).prop_map(FOp::Remove),
        1 => (any::<bool>(), any::<bool>(), prop_oneof![Just(0i64), 0i64..300], "[a-c]{0,2}", any::<bool>())
            .prop_map(|(e, pw, p, t, c)| FOp::Filter(e, pw, p, t, c)),
    ]
}

proptest! {
    #[test]
    fn filter_proptest_order_is_sorted_subset(
        ops in prop::collection::vec(fop(), 0..60),
        col in 0usize..Column::ALL.len(),
        ascending: bool,
    ) {
        let column = Column::ALL[col];
        let mut m = TableModel::default();
        m.set_sort(Sort { column, ascending });
        m.set_country_lookup(Some(Arc::new(|ip: &str| {
            ["cl", "br", ""][ip.len() % 3].to_owned().into()
        })));
        for op in ops {
            match op {
                FOp::Upsert(h, ping, n, pw, s) => {
                    let mut info = mk(&format!("10.0.0.{h}"), &s, ping, n, "1.3");
                    info.password = pw;
                    info.map = s.to_uppercase();
                    m.add_or_update(info);
                }
                FOp::Remove(h) => m.remove(&ServerAddress::new(format!("10.0.0.{h}"), 28960)),
                FOp::Filter(empty, pw, ping, text, country) => {
                    let mut f = quick(QuickFilters {
                        hide_empty: empty,
                        hide_password: pw,
                        max_ping: ping,
                        ..QuickFilters::default()
                    });
                    f.set_text(&text);
                    if country {
                        f.countries = set(["cl"]);
                    }
                    m.set_filter(f);
                }
            }
            let key = |r: usize| {
                let info = &m.servers()[r];
                sort_key(info, column, &same, m.country_code(info))
            };
            let mut want: Vec<usize> = (0..m.len())
                .filter(|&r| {
                    let info = &m.servers()[r];
                    m.filter().accepts(info, m.country_code(info))
                })
                .collect();
            want.sort_by(|&a, &b| {
                let o = key(a).cmp(&key(b));
                (if ascending { o } else { o.reverse() }).then(a.cmp(&b))
            });
            prop_assert_eq!(m.order(), want.as_slice());
        }
    }
}

// ------------------------------------------------------------------ actualizar en su sitio y podar

fn addr(h: u16) -> ServerAddress {
    ServerAddress::new(format!("10.0.{}.{}", h / 256, h % 256), 28960)
}

fn row_info(h: u16, ping: u32, name: &str) -> ServerInfo {
    let a = addr(h);
    mk(&a.host, name, ping, 3, "1.3")
}

proptest! {
    #[test]
    fn update_in_place_matches_full_resort(
        ups in prop::collection::vec((0u16..300, 0u32..300, "[a-c]{0,3}"), 0..600),
        at in any::<prop::sample::Index>(),
        col in 0usize..Column::ALL.len(),
        ascending: bool,
    ) {
        let sort = Sort { column: Column::ALL[col], ascending };
        let at = at.index(ups.len() + 1);
        let mut m = TableModel::default();
        // El último estado de cada dirección, en el orden de su primera llegada.
        let mut last: indexmap::IndexMap<u16, (u32, String)> = indexmap::IndexMap::new();
        for (i, (h, ping, name)) in ups.iter().enumerate() {
            if i == at {
                m.set_sort(sort);
            }
            m.add_or_update(row_info(*h, *ping, name));
            last.insert(*h, (*ping, name.clone()));
        }
        m.set_sort(sort); // por si `at` cae al final; no cambia nada si ya estaba
        let mut fresh = TableModel::default();
        fresh.set_sort(sort);
        for (h, (ping, name)) in &last {
            fresh.add_or_update(row_info(*h, *ping, name));
        }
        prop_assert_eq!(m.order(), fresh.order());
    }

    #[test]
    fn retain_matches_removes(
        rows in prop::collection::vec((0u32..300, any::<bool>()), 0..40),
        col in 0usize..Column::ALL.len(),
        ascending: bool,
    ) {
        let sort = Sort { column: Column::ALL[col], ascending };
        let (mut a, mut b) = (TableModel::default(), TableModel::default());
        for m in [&mut a, &mut b] {
            m.set_sort(sort);
            for (h, (ping, _)) in rows.iter().enumerate() {
                m.add_or_update(row_info(h as u16, *ping, &format!("s{ping}")));
            }
        }
        let gone: HashSet<ServerAddress> = rows
            .iter()
            .enumerate()
            .filter(|(_, (_, keep))| !keep)
            .map(|(h, _)| addr(h as u16))
            .collect();
        for address in &gone {
            a.remove(address);
        }
        b.retain(|x| !gone.contains(x));
        let hosts = |m: &TableModel| -> Vec<String> {
            m.servers().iter().map(|s| s.address.host.clone()).collect()
        };
        prop_assert_eq!(hosts(&a), hosts(&b));
        prop_assert_eq!(a.order(), b.order());
        for h in 0..rows.len() {
            prop_assert_eq!(a.row_of(&addr(h as u16)), b.row_of(&addr(h as u16)));
        }
    }
}

#[test]
fn update_with_equal_keys_finds_row() {
    let mut m = TableModel::default();
    for h in 0..100 {
        m.add_or_update(row_info(h, 50, "s"));
    }
    m.add_or_update(row_info(37, 50, "s"));
    m.add_or_update(row_info(37, 10, "s"));
    let order = m.order();
    assert_eq!(order.len(), 100);
    assert_eq!(order.iter().collect::<HashSet<_>>().len(), 100);
    assert_eq!(order[0], 37);
}

#[test]
fn update_hidden_row_leaves_order() {
    let mut m = TableModel::default();
    m.set_filter(quick(QuickFilters {
        hide_empty: true,
        ..QuickFilters::default()
    }));
    m.add_or_update(mk("1.1.1.1", "A", 10, 3, "1.3"));
    m.add_or_update(mk("2.2.2.2", "B", 20, 3, "1.3"));
    m.add_or_update(mk("1.1.1.1", "A", 10, 0, "1.3"));
    assert_eq!(names(&m), ["B"]);
    m.add_or_update(mk("1.1.1.1", "A", 30, 5, "1.3"));
    assert_eq!(names(&m), ["B", "A"]);
}
