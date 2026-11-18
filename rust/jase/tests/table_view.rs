mod support;

use iced::{Color, Size};
use jase::app::Message;
use std::sync::Arc;

use jase::table::{
    Column, Msg, Nav, ROW_HEIGHT, Sort, Table, occupancy_bar_width, occupancy_blocks,
    scroll_to_show, sort_indicator, step_selection, update, visible_range, visible_rows,
};
use jase::theme::{OCCUPANCY_ALPHA, VARIANTS, palette, ping_color, style};
use jase_core::config::Config;
use jase_core::favorites::offline_server;
use jase_core::i18n::set_language;
use jase_core::models::ServerAddress;
use proptest::prelude::*;
use support::{fake_app, mk};

fn host(i: usize) -> String {
    format!("10.{}.{}.{}", i / 65536, (i / 256) % 256, i % 256)
}

/// Tabla con `n` filas; la fila `i` tiene ping `i` (el orden por ping es el de llegada).
fn table(n: usize) -> Table {
    let mut t = Table::default();
    for i in 0..n {
        t.model
            .add_or_update(mk(&host(i), &format!("S{i}"), i as u32, 1, "1.3"));
    }
    t
}

fn addr(i: usize) -> ServerAddress {
    ServerAddress::new(host(i), 28960)
}

proptest! {
    #[test]
    fn visible_range_covers_viewport(
        rows in 0usize..10_000,
        viewport_h in 1.0f32..2_000.0,
        frac in 0.0f32..=1.0,
    ) {
        let content = rows as f32 * ROW_HEIGHT;
        let max_off = (content - viewport_h).max(0.0);
        let offset = frac * max_off;
        let r = visible_range(offset, viewport_h, rows);
        prop_assert!(r.end <= rows && r.start <= r.end);
        // Hasta h/22 + 2 filas asoman (dos partidas en los bordes), más el margen de cada lado.
        prop_assert!(r.len() as f32 <= viewport_h / ROW_HEIGHT + 4.0);
        // Toda fila que asoma en la ventana visible está en el rango.
        for i in 0..rows {
            let top = i as f32 * ROW_HEIGHT;
            if top + ROW_HEIGHT > offset && top < offset + viewport_h {
                prop_assert!(r.contains(&i), "fila {i} fuera de {r:?}");
            }
        }
    }
}

#[test]
fn visible_range_clamps_stale_offset() {
    // Tras vaciar la tabla el desplazamiento guardado puede quedar más allá del final.
    assert_eq!(visible_range(10_000.0, 400.0, 5), 0..5);
}

#[test]
fn view_builds_only_visible_rows() {
    let p = palette("dark");
    let mut t = table(5_000);
    t.viewport_h = 400.0;
    t.offset_y = 100.0 * ROW_HEIGHT;
    let rows = visible_rows(&t, p);
    assert!(rows.len() <= 22, "{} filas", rows.len());
    // Una fila de margen arriba: la primera es la 99 y la del desplazamiento, la 100.
    assert_eq!(rows[0].address, addr(99));
    assert_eq!(rows[1].address, addr(100));
    assert_eq!(rows[0].cells.len(), Column::ALL.len());
}

#[test]
fn header_click_sorts_and_toggles() {
    let mut t = table(3);
    let ping_asc = Sort {
        column: Column::Ping,
        ascending: true,
    };
    assert_eq!(t.model.sort(), ping_asc);
    let _ = update(&mut t, Msg::HeaderClicked(Column::Ping));
    assert_eq!(
        t.model.sort(),
        Sort {
            column: Column::Ping,
            ascending: false
        }
    );
    assert_eq!(t.model.servers()[t.model.order()[0]].name, "S2");
    let _ = update(&mut t, Msg::HeaderClicked(Column::Map));
    assert_eq!(
        t.model.sort(),
        Sort {
            column: Column::Map,
            ascending: true
        }
    );
}

#[test]
fn selection_survives_update_and_resort() {
    let p = palette("dark");
    let mut t = table(5);
    let _ = update(&mut t, Msg::RowClicked(addr(1)));
    assert!(t.focused);
    let selected_at = |t: &Table| {
        visible_rows(t, p)
            .iter()
            .filter(|r| r.selected)
            .map(|r| r.address.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(selected_at(&t), [addr(1)]);
    // La fila 1 pasa a tener el peor ping: baja al final.
    t.model.add_or_update(mk(&host(1), "S1", 999, 1, "1.3"));
    let rows = visible_rows(&t, p);
    assert_eq!(rows[4].address, addr(1));
    assert!(rows[4].selected);
    assert_eq!(selected_at(&t), [addr(1)]);
    let _ = update(&mut t, Msg::HeaderClicked(Column::Ping)); // descendente: queda primera
    let rows = visible_rows(&t, p);
    assert_eq!(rows[0].address, addr(1));
    assert!(rows[0].selected);
    assert_eq!(t.selected, Some(addr(1)));
}

#[test]
fn arrows_move_selection_and_scroll() {
    assert_eq!(step_selection(None, 0, Nav::Down, 10), None);
    assert_eq!(step_selection(Some(3), 0, Nav::End, 10), None);
    assert_eq!(step_selection(None, 5, Nav::Down, 10), Some(0));
    assert_eq!(step_selection(Some(0), 5, Nav::Up, 10), Some(0));
    assert_eq!(step_selection(Some(4), 5, Nav::Down, 10), Some(4));
    assert_eq!(step_selection(Some(2), 50, Nav::PageDown, 10), Some(12));
    assert_eq!(step_selection(Some(2), 50, Nav::PageUp, 10), Some(0));
    assert_eq!(step_selection(Some(2), 50, Nav::Home, 10), Some(0));
    assert_eq!(step_selection(Some(2), 50, Nav::End, 10), Some(49));

    // Dentro de la ventana visible no hace falta desplazar.
    assert_eq!(scroll_to_show(5, 0.0, 220.0), None);
    // Más abajo: se desplaza lo justo para que la fila quede al pie.
    assert_eq!(
        scroll_to_show(10, 0.0, 220.0),
        Some(11.0 * ROW_HEIGHT - 220.0)
    );
    // Más arriba: queda arriba.
    assert_eq!(scroll_to_show(2, 220.0, 220.0), Some(2.0 * ROW_HEIGHT));

    let mut t = table(100);
    t.viewport_h = 220.0; // 10 filas
    let _ = update(&mut t, Msg::RowClicked(addr(9)));
    let _ = update(&mut t, Msg::Nav(Nav::Down));
    assert_eq!(t.selected, Some(addr(10)));
    assert_eq!(t.offset_y, 11.0 * ROW_HEIGHT - 220.0);
    let _ = update(&mut t, Msg::Nav(Nav::End));
    assert_eq!(t.selected, Some(addr(99)));
    assert_eq!(t.offset_y, 100.0 * ROW_HEIGHT - 220.0);
    let _ = update(&mut t, Msg::Nav(Nav::Home));
    assert_eq!(t.selected, Some(addr(0)));
    assert_eq!(t.offset_y, 0.0);

    let mut empty = Table::default();
    let _ = update(&mut empty, Msg::Nav(Nav::Down));
    assert_eq!(empty.selected, None);
}

#[test]
fn window_resize_does_not_change_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    app.table = table(100);
    app.table.viewport_h = 220.0; // lo que informó el `scrollable` (10 filas)
    let _ = app.update(Message::WindowResized(Size::new(1100.0, 700.0)));
    assert_eq!(app.table.viewport_h, 220.0);
    let _ = update(&mut app.table, Msg::RowClicked(addr(9)));
    let _ = update(&mut app.table, Msg::Nav(Nav::Down));
    assert_eq!(app.table.offset_y, 11.0 * ROW_HEIGHT - 220.0);
    let _ = update(&mut app.table, Msg::Nav(Nav::PageDown)); // página de 10, no de 31
    assert_eq!(app.table.selected, Some(addr(20)));
    // Lo dibujado sí cubre la ventana entera.
    assert!(visible_rows(&app.table, palette("dark")).len() >= 31);
}

#[test]
fn view_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let mut t = Table::default();
    for i in 0..50 {
        let mut info = mk(&host(i), &format!("S{i}"), i as u32, (i % 25) as i32, "1.3");
        info.bots = (i % 3) as i32;
        t.model.add_or_update(info);
    }
    t.model.set_country_lookup(Some(Arc::new(|ip: &str| {
        ip.ends_with('1').then(|| "CL".to_owned())
    })));
    app.table = t;
    let _ = update(&mut app.table, Msg::RowClicked(addr(3)));
    for (id, _, _) in VARIANTS.iter() {
        app.theme_id = id;
        let _ = app.view();
        let _ = app.theme();
    }
}

#[test]
fn offline_rows_use_muted_color() {
    let p = palette("dark");
    let mut t = Table::default();
    t.model
        .add_or_update(offline_server(&ServerAddress::new("9.9.9.9", 1)));
    t.model.add_or_update(mk("1.1.1.1", "A", 900, 1, "1.3"));
    let rows = visible_rows(&t, p); // el caído va al final por ping
    assert!(rows[0].online && !rows[1].online);
    for (col, cell) in &rows[1].cells {
        if [Column::Name, Column::Ping, Column::Players, Column::Map].contains(col) {
            assert!(cell.runs.iter().all(|(_, c)| *c == p.muted), "{col:?}");
        }
    }
    let name = |row: usize| {
        rows[row]
            .cells
            .iter()
            .find(|(c, _)| *c == Column::Name)
            .map(|(_, cell)| cell.runs.clone())
            .unwrap()
    };
    assert_eq!(name(0), [("A".to_owned(), p.text)]);
    assert!(name(1)[0].0.starts_with("9.9.9.9:1"));
}

fn cell(t: &Table, p: &jase::theme::Palette, row: usize, col: Column) -> jase::table::CellVm {
    visible_rows(t, p)[row]
        .cells
        .iter()
        .find(|(c, _)| *c == col)
        .map(|(_, cell)| cell.clone())
        .unwrap()
}

#[test]
fn ping_foreground_follows_mode() {
    let mut t = Table::default();
    t.model.add_or_update(mk("1.1.1.1", "A", 50, 3, "1.3"));
    for id in ["dark", "light"] {
        let p = palette(id);
        let runs = cell(&t, p, 0, Column::Ping).runs;
        assert_eq!(runs, [("50".to_owned(), ping_color(50, p))]);
    }
}

#[test]
fn name_cell_uses_color_runs() {
    let p = palette("dark");
    let mut t = Table::default();
    let mut info = mk("1.1.1.1", "Jimmy TDM", 50, 1, "1.3");
    info.raw_name = "^1Jimmy^7 TDM".to_owned();
    t.model.add_or_update(info);
    let runs = cell(&t, p, 0, Column::Name).runs;
    assert_eq!(
        runs,
        [
            ("Jimmy".to_owned(), p.q3[1].unwrap()),
            (" TDM".to_owned(), p.text)
        ]
    );
}

#[test]
fn players_column_shows_humans_and_bots() {
    set_language("es").unwrap();
    let p = palette("dark");
    let mut t = Table::default();
    let mut mixed = mk("2.2.2.2", "mixto", 50, 8, "1.3");
    mixed.bots = 6;
    t.model.add_or_update(mixed);
    t.model.add_or_update(mk("3.3.3.3", "gente", 60, 3, "1.3"));
    let c = cell(&t, p, 0, Column::Players);
    assert_eq!(c.runs[0].0, "2+6/20");
    assert_eq!(c.tooltip.as_deref(), Some("2 jugadores · 6 bots"));
    let c = cell(&t, p, 1, Column::Players);
    assert_eq!(c.runs[0].0, "3/20");
    assert_eq!(c.tooltip, None);
}

#[test]
fn occupancy_cell_has_bar_only_with_players() {
    let p = palette("dark");
    let mut t = Table::default();
    t.model.add_or_update(mk("1.1.1.1", "lleno", 10, 50, "1.3"));
    t.model.add_or_update(mk("2.2.2.2", "vacio", 20, 0, "1.3"));
    let mut no_max = mk("3.3.3.3", "sin maximo", 30, 5, "1.3");
    no_max.max_players = 0;
    t.model.add_or_update(no_max);
    let mut offline = mk("4.4.4.4", "caido", 40, 10, "1.3");
    offline.online = false;
    t.model.add_or_update(offline);
    let occ = |row| cell(&t, p, row, Column::Players).occupancy;
    assert_eq!(occ(0), Some(1.0)); // 50/20: nunca más que la celda
    assert_eq!(occ(1), None);
    assert_eq!(occ(2), None);
    assert_eq!(occ(3), None);
    assert_eq!(cell(&t, p, 0, Column::Ping).occupancy, None);
}

#[test]
fn occupancy_tokens_in_both_themes() {
    for (id, p, _) in &VARIANTS {
        let c = style::occupancy_bar(p);
        assert!(c.a == OCCUPANCY_ALPHA && c.a < 1.0, "{id}");
        assert_eq!(Color { a: 1.0, ..c }, p.occupancy, "{id}");
    }
}

#[test]
fn country_column_shows_flag() {
    let p = palette("dark");
    let mut t = Table::default();
    for h in ["1.1.1.1", "2.2.2.2", "5.5.5.5", "6.6.6.6", "9.9.9.9"] {
        t.model.add_or_update(mk(h, h, 10, 1, "1.3"));
    }
    let flag = |t: &Table, row| cell(t, p, row, Column::Country).flag;
    assert_eq!(flag(&t, 0), None); // sin base: vacío
    t.model.set_country_lookup(Some(Arc::new(|ip: &str| {
        match ip {
            "1.1.1.1" => Some("cl"),
            "2.2.2.2" => Some("BR"),
            "5.5.5.5" => Some("zz"),  // sin bandera
            "6.6.6.6" => Some("../"), // inválido
            _ => None,
        }
        .map(str::to_owned)
    })));
    assert_eq!(
        t.model.country_code(&t.model.servers()[1]).as_deref(),
        Some("br")
    );
    assert_eq!(flag(&t, 0).as_deref(), Some("cl"));
    assert_eq!(flag(&t, 1).as_deref(), Some("br"));
    assert_eq!(flag(&t, 2), None);
    assert_eq!(flag(&t, 3), None);
    assert_eq!(flag(&t, 4), None);
    assert_eq!(cell(&t, p, 0, Column::Name).flag, None); // solo en País
    t.model.set_country_lookup(None);
    assert_eq!(flag(&t, 0), None);
}

proptest! {
    #[test]
    fn occupancy_blocks_fill_ratio(ratio in 0.0f32..=1.0, inner_w in 0.0f32..200.0) {
        let end = (inner_w * ratio).round();
        let blocks = occupancy_blocks(ratio, inner_w);
        let mut gaps = 0.0;
        for (i, &(x, w)) in blocks.iter().enumerate() {
            prop_assert_eq!(x, i as f32 * 8.0);
            prop_assert!(w > 0.0 && w <= 6.0);
            prop_assert!(x + w <= end);
            if i + 1 < blocks.len() {
                gaps += 2.0;
            }
        }
        let sum: f32 = blocks.iter().map(|b| b.1).sum();
        // Sin huecos de más: lo que no cubren los bloques son los huecos de 2 px y,
        // como mucho, el hueco tras el último bloque completo.
        prop_assert!(end - sum - gaps >= 0.0 && end - sum - gaps < 2.0 + 1e-3);
    }

    #[test]
    fn occupancy_bar_never_exceeds_cell(ratio in 0.0f32..=1.0, cell_w in 0.0f32..500.0) {
        let w = occupancy_bar_width(ratio, cell_w);
        prop_assert!(w >= 0.0 && w <= cell_w);
        if ratio > 0.0 && cell_w >= 1.0 {
            prop_assert!(w >= 1.0);
        }
    }
}

#[test]
fn occupancy_bar_is_proportional() {
    assert_eq!(occupancy_bar_width(0.0, 90.0), 0.0);
    assert_eq!(occupancy_bar_width(0.5, 90.0), 45.0);
    assert_eq!(occupancy_bar_width(0.001, 90.0), 1.0);
    assert_eq!(occupancy_bar_width(1.0, 90.0), 90.0);
    assert_eq!(
        occupancy_blocks(0.5, 40.0),
        [(0.0, 6.0), (8.0, 6.0), (16.0, 4.0)]
    );
}

#[test]
fn header_shows_sort_indicator_only_on_sorted_column() {
    let mut t = table(3);
    for col in Column::ALL {
        let want = (col == Column::Ping).then_some(true);
        assert_eq!(sort_indicator(&t, col), want, "{col:?}");
    }
    let _ = update(&mut t, Msg::HeaderClicked(Column::Map));
    assert_eq!(sort_indicator(&t, Column::Map), Some(true));
    assert_eq!(sort_indicator(&t, Column::Ping), None);
    let _ = update(&mut t, Msg::HeaderClicked(Column::Map));
    assert_eq!(sort_indicator(&t, Column::Map), Some(false));
}
