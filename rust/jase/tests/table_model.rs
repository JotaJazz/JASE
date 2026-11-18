mod support;

use jase::colored::color_runs;
use jase::table::{
    Column, OFFLINE_PING_SORT, Sort, SortKey, TableModel, cell_text, occupancy_ratio, players_text,
    sort_key,
};
use jase::theme::{palette, ping_color};
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
        .map(|&r| sort_key(&m.servers()[r], Column::Ping, &same))
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
        sort_key(&off, Column::Ping, &same),
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
                    sort_key(&m.servers()[a], column, &same),
                    sort_key(&m.servers()[b], column, &same),
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
