//! Columnas redimensionables, divisor de la barra lateral y estado de la ventana guardado en
//! claves `rs_` de `ui_state` (los blobs de Qt quedan intactos para que 1.3.0 los siga leyendo).
mod support;

use iced::advanced::subscription::into_recipes;
use iced::window::Position;
use iced::{Point, Size, window};
use jase::app::Message;
use jase::table::{Column, Msg, Sort};
use jase::window_state::{
    Geometry, clamp_sidebar, fit, geometry_value, header_value, parse_geometry, parse_header,
    window_settings,
};
use jase_core::config::{Config, load_config};
use support::{fake_app, window_requests};

const DEFAULT_WIDTHS: [f32; 8] = [28.0, 48.0, 0.0, 70.0, 90.0, 170.0, 150.0, 70.0];
const PING_ASC: Sort = Sort {
    column: Column::Ping,
    ascending: true,
};

fn cfg_with(pairs: &[(&str, &str)]) -> Config {
    let mut cfg = Config::default();
    for (k, v) in pairs {
        cfg.ui_state.insert((*k).into(), (*v).into());
    }
    cfg
}

/// Arrastra el borde derecho de `col` `dx` px.
fn drag_column(app: &mut jase::app::App, col: Column, dx: f32) {
    let _ = app.update(Message::Table(Msg::ResizeStart(col)));
    let _ = app.update(Message::DragMoved(500.0));
    let _ = app.update(Message::DragMoved(500.0 + dx));
    let _ = app.update(Message::DragEnd);
}

#[test]
fn close_keeps_qt_blobs_and_writes_rs_keys() {
    let dir = tempfile::tempdir().unwrap();
    let qt = [
        ("geometry", "AdnQywADAAAAAAAA"),
        ("table_header", "AAAA/wAAAAAAAAAB"),
        ("table_header_cols", "8"),
        ("body_splitter", "AAAA/wAAAAEAAAAC"),
        ("right_splitter", "AAAA/wAAAAEAAAAD"),
        ("details_splitter", "AAAA/wAAAAEAAAAE"),
    ];
    let (mut app, fake) = fake_app(dir.path(), cfg_with(&qt));
    let _ = app.update(Message::SelectGame("ut99"));
    let _ = app.update(Message::WindowResized(Size::new(1300.0, 820.0)));
    let _ = app.update(Message::WindowMoved(Point::new(40.0, 60.0)));
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
    let state = load_config(&dir.path().join("config.json")).ui_state;
    for (k, v) in qt {
        assert_eq!(state.get(k).map(String::as_str), Some(v), "{k}");
    }
    let get = |k: &str| state.get(k).map(String::as_str);
    assert_eq!(get("last_game"), Some("ut99"));
    assert_eq!(get("rs_geometry"), Some("1300,820,40,60"));
    assert_eq!(get("rs_body_splitter"), Some("230"));
    assert_eq!(
        get("rs_table_header"),
        Some("8;28,48,0,70,90,170,150,70;3;asc")
    );
}

#[test]
fn new_window_restores_column_width_and_sort() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut app, _) = fake_app(dir.path(), Config::default());
        drag_column(&mut app, Column::Map, 163.0);
        assert_eq!(app.table.widths[Column::Map as usize], 333.0);
        for _ in 0..2 {
            let _ = app.update(Message::Table(Msg::HeaderClicked(Column::Players)));
        }
        let _ = app.update(Message::Close { maximized: false });
    }
    let cfg = load_config(&dir.path().join("config.json"));
    let (app, _) = fake_app(dir.path(), cfg);
    assert_eq!(app.table.widths[Column::Map as usize], 333.0);
    assert_eq!(
        app.table.model.sort(),
        Sort {
            column: Column::Players,
            ascending: false
        }
    );
}

#[test]
fn garbage_rs_state_is_ignored() {
    let values = [
        "%%% no es base64 %%%",
        "AAAA",
        "",
        "NaN",
        "-5",
        "1e9",
        "100",
    ];
    let headers = [
        "8;28,48,0,70,90,-170,150,70;3;asc",
        "8;28,48,0,70,90,NaN,150,70;3;asc",
        "8;28,48,0,70,90,inf,150,70;3;asc",
        "8;28,48,0,70,90,10,150,70;3;asc", // bajo el mínimo de 20 px
        "8;28,48,0,70,90,170,150,70;3;up", // sentido desconocido
        "8;28,48,0,70,90,170,150,70;8;asc",
        "8;28,48,0,70,90,170,150,70;-1;asc",
        "8;28,48,0,70,90,170,150;3;asc",
        "8;28,48,0,70,90,170,150,70,1;3;asc",
        "8;28,48,0,70,90,170,150,70;3;asc;x",
        "x;28,48,0,70,90,170,150,70;3;asc",
    ];
    let geometries = [
        "0,700",
        "-1100,700",
        "NaN,700",
        "1100",
        "1100,700,5",
        "1100,700,max,5",
        "1100,700,NaN,0",
        "1100,700,0,0,max,max",
        "1100,700,inf,0",
        "1100,700,maximized",
    ];
    let all = ["rs_geometry", "rs_body_splitter", "rs_table_header"];
    let cases = values
        .iter()
        .flat_map(|v| all.map(|k| (k, *v)))
        .chain(headers.map(|v| ("rs_table_header", v)))
        .chain(geometries.map(|v| ("rs_geometry", v)));
    for (key, value) in cases {
        let dir = tempfile::tempdir().unwrap();
        let cfg = cfg_with(&[(key, value)]);
        let settings = window_settings(&cfg);
        assert_eq!(settings.size, Size::new(1100.0, 700.0), "{value}");
        assert!(matches!(settings.position, Position::Default), "{value}");
        assert!(!settings.maximized, "{value}");
        let (app, _) = fake_app(dir.path(), cfg);
        assert_eq!(app.table.widths, DEFAULT_WIDTHS, "{value}");
        assert_eq!(app.table.model.sort(), PING_ASC, "{value}");
        assert_eq!(app.sidebar_width, 230.0, "{value}");
    }
}

#[test]
fn header_state_with_other_column_count_is_ignored() {
    for header in [
        "9;28,48,0,70,90,333,150,70,70;4;desc",
        "7;28,48,0,70,90,333,150;4;desc",
        "8;28,48,0,70,90,333,150,70,70;4;desc",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (app, _) = fake_app(dir.path(), cfg_with(&[("rs_table_header", header)]));
        assert_eq!(app.table.widths, DEFAULT_WIDTHS, "{header}");
        assert_eq!(app.table.model.sort(), PING_ASC, "{header}");
    }
}

#[test]
fn header_state_with_matching_column_count_is_restored() {
    let dir = tempfile::tempdir().unwrap();
    let header = "8;28,48,0,70,90,333,150,70;4;desc";
    let (app, _) = fake_app(dir.path(), cfg_with(&[("rs_table_header", header)]));
    assert_eq!(app.table.widths[Column::Map as usize], 333.0);
    assert_eq!(app.table.model.sort().column, Column::Players);
    assert!(!app.table.model.sort().ascending);
}

#[test]
fn v1_header_state_with_seven_columns_is_ignored() {
    // Como lo habría guardado una versión con 7 columnas (sin País).
    let dir = tempfile::tempdir().unwrap();
    let header = "7;28,0,70,90,333,150,70;3;desc";
    let (app, _) = fake_app(dir.path(), cfg_with(&[("rs_table_header", header)]));
    assert_eq!(Column::ALL.len(), 8);
    assert_eq!(app.table.widths, DEFAULT_WIDTHS);
    assert_eq!(app.table.model.sort(), PING_ASC);
}

#[test]
fn header_round_trips() {
    let w = [28.0, 48.0, 0.0, 70.0, 90.0, 333.0, 150.0, 70.0];
    let sort = Sort {
        column: Column::Players,
        ascending: false,
    };
    let v = header_value(&w, sort);
    assert_eq!(v, "8;28,48,0,70,90,333,150,70;4;desc");
    assert_eq!(parse_header(&v), Some((w, sort)));
}

#[test]
fn map_and_mode_columns_are_wide_enough() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fake_app(dir.path(), Config::default());
    // Mapa: «CTF-Bleak-CE100», «DM-Deck16][»; Modo: «AssignedTeamGame».
    assert!(app.table.widths[Column::Map as usize] >= 160.0);
    assert!(app.table.widths[Column::Mode as usize] >= 140.0);
}

#[test]
fn country_column_layout() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fake_app(dir.path(), Config::default());
    assert_eq!(Column::Country.title(), "País");
    assert_eq!(app.table.widths[Column::Country as usize], 48.0);
    let pos = |c| Column::ALL.iter().position(|&x| x == c).unwrap();
    assert!(pos(Column::Lock) < pos(Column::Country) && pos(Column::Country) < pos(Column::Name));
}

#[test]
fn column_drag_resizes_and_keeps_minimum() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    drag_column(&mut app, Column::Ping, 30.0);
    assert_eq!(app.table.widths[Column::Ping as usize], 100.0);
    drag_column(&mut app, Column::Ping, -500.0);
    assert_eq!(app.table.widths[Column::Ping as usize], 20.0);
    // Las demás no cambian; tras soltar, mover el ratón no hace nada.
    assert_eq!(app.table.widths[Column::Map as usize], 170.0);
    let _ = app.update(Message::DragMoved(900.0));
    assert_eq!(app.table.widths[Column::Ping as usize], 20.0);
}

#[test]
fn sidebar_drag_is_clamped() {
    assert_eq!(clamp_sidebar(230.0, 1100.0), 230.0);
    assert_eq!(clamp_sidebar(10.0, 1100.0), 150.0);
    // La tabla deja al menos 300 px (más los márgenes del cuerpo y el divisor).
    assert_eq!(clamp_sidebar(5000.0, 1100.0), 1100.0 - 324.0);
    assert_eq!(clamp_sidebar(5000.0, 300.0), 150.0);
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::WindowResized(Size::new(1100.0, 700.0)));
    let _ = app.update(Message::SplitterStart);
    let _ = app.update(Message::DragMoved(240.0));
    let _ = app.update(Message::DragMoved(290.0));
    assert_eq!(app.sidebar_width, 280.0);
    let _ = app.update(Message::DragMoved(-1000.0));
    assert_eq!(app.sidebar_width, 150.0);
    let _ = app.update(Message::DragMoved(3000.0));
    assert_eq!(app.sidebar_width, 776.0);
    let _ = app.update(Message::DragEnd);
    let _ = app.update(Message::Close { maximized: false });
    let cfg = load_config(&dir.path().join("config.json"));
    assert_eq!(
        cfg.ui_state.get("rs_body_splitter").map(String::as_str),
        Some("776")
    );
    let (app, _) = fake_app(dir.path(), cfg);
    assert_eq!(app.sidebar_width, 776.0);
}

#[test]
fn maximized_close_keeps_previous_size() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "1200,800,10,20")]);
    let settings = window_settings(&cfg);
    assert_eq!(settings.size, Size::new(1200.0, 800.0));
    assert!(matches!(settings.position, Position::Specific(p) if p == Point::new(10.0, 20.0)));
    let (mut app, _) = fake_app(dir.path(), cfg);
    // Maximizada: el tamaño y la posición que informa el sistema no pisan los guardados.
    let _ = app.update(Message::WindowResized(Size::new(1920.0, 1040.0)));
    let _ = app.update(Message::WindowMoved(Point::new(0.0, 0.0)));
    let _ = app.update(Message::Close { maximized: true });
    let cfg = load_config(&dir.path().join("config.json"));
    assert_eq!(
        cfg.ui_state.get("rs_geometry").map(String::as_str),
        Some("1200,800,10,20,max")
    );
    let settings = window_settings(&cfg);
    assert!(settings.maximized);
    assert_eq!(settings.size, Size::new(1200.0, 800.0));
}

#[test]
fn geometry_without_position_on_wayland_round_trips() {
    let g = Geometry {
        size: Size::new(1280.0, 720.0),
        position: None,
        maximized: true,
    };
    assert_eq!(geometry_value(&g), "1280,720,max");
    assert_eq!(parse_geometry("1280,720,max"), Some(g));
    let plain = parse_geometry("1280,720").unwrap();
    assert_eq!((plain.position, plain.maximized), (None, false));
    assert_eq!(geometry_value(&plain), "1280,720");
    // Sin `WindowMoved` (Wayland no informa la posición) no se guarda ninguna.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::WindowResized(Size::new(1280.0, 720.0)));
    let _ = app.update(Message::Close { maximized: false });
    let cfg = load_config(&dir.path().join("config.json"));
    assert_eq!(
        cfg.ui_state.get("rs_geometry").map(String::as_str),
        Some("1280,720")
    );
    assert!(matches!(window_settings(&cfg).position, Position::Default));
}

#[test]
fn drag_subscription_only_while_dragging() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let idle = into_recipes(app.subscription()).len();
    let _ = app.update(Message::Table(Msg::ResizeStart(Column::Map)));
    assert_eq!(into_recipes(app.subscription()).len(), idle + 1);
    let _ = app.update(Message::DragEnd);
    assert_eq!(into_recipes(app.subscription()).len(), idle);
    let _ = app.update(Message::SplitterStart);
    assert_eq!(into_recipes(app.subscription()).len(), idle + 1);
    let _ = app.update(Message::DragEnd);
    assert_eq!(into_recipes(app.subscription()).len(), idle);
}

fn saved_geometry(dir: &std::path::Path) -> Option<String> {
    load_config(&dir.join("config.json"))
        .ui_state
        .get("rs_geometry")
        .cloned()
}

#[test]
fn restored_window_is_fitted_to_monitor() {
    let monitor = Size::new(1920.0, 1080.0);
    // Monitor desenchufado o cambio de escala: más grande que la pantalla y fuera de ella.
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "5000,4000,-9000,-9000")]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    let id = window::Id::unique();
    let task = app.update(Message::WindowOpened(
        id,
        Some(Point::new(-9000.0, -9000.0)),
    ));
    assert_eq!(window_requests(task), 1); // monitor_size
    let task = app.update(Message::MonitorSize(id, Some(monitor)));
    assert_eq!(window_requests(task), 2); // resize y move_to
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(saved_geometry(dir.path()).as_deref(), Some("1920,1080,0,0"));
    // Una que entra no se toca.
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "1000,600,100,100")]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    let _ = app.update(Message::WindowOpened(id, Some(Point::new(100.0, 100.0))));
    let task = app.update(Message::MonitorSize(id, Some(monitor)));
    assert_eq!(window_requests(task), 0);
    // Maximizada no se mueve: se ajusta (solo el tamaño, x = 3000 puede ser otro monitor) lo
    // que se guardará al desmaximizar.
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "5000,700,3000,0,max")]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    let task = app.update(Message::MonitorSize(id, Some(monitor)));
    assert_eq!(window_requests(task), 0);
    let _ = app.update(Message::Close { maximized: true });
    assert_eq!(
        saved_geometry(dir.path()).as_deref(),
        Some("1920,700,3000,0,max")
    );
}

#[test]
fn fit_keeps_wayland_geometry_without_position() {
    let g = parse_geometry("3000,700").unwrap();
    let f = fit(g, Size::new(1920.0, 1080.0));
    assert_eq!(f.size, Size::new(1920.0, 700.0));
    assert_eq!(f.position, None);
    let inside = parse_geometry("800,600,1120,480").unwrap();
    assert_eq!(fit(inside, Size::new(1920.0, 1080.0)), inside);
}

#[test]
fn maximized_close_keeps_last_normal_geometry_of_session() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "1200,800,10,20")]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    // Cada `WindowResized`/`WindowMoved` pide `is_maximized` con el número siguiente.
    let _ = app.update(Message::WindowResized(Size::new(1300.0, 900.0)));
    let _ = app.update(Message::WindowMoved(Point::new(30.0, 40.0)));
    let _ = app.update(maximized(2, false));
    let _ = app.update(Message::WindowResized(Size::new(1920.0, 1040.0)));
    let _ = app.update(Message::WindowMoved(Point::new(0.0, 0.0)));
    let _ = app.update(maximized(4, true));
    let _ = app.update(Message::Close { maximized: true });
    assert_eq!(
        saved_geometry(dir.path()).as_deref(),
        Some("1300,900,30,40,max")
    );
}

#[test]
fn huge_column_drag_is_capped_and_still_restores() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut app, _) = fake_app(dir.path(), Config::default());
        drag_column(&mut app, Column::Map, 1.0e7);
        assert_eq!(app.table.widths[Column::Map as usize], 16384.0);
        let _ = app.update(Message::Table(Msg::HeaderClicked(Column::Players)));
        let _ = app.update(Message::Close { maximized: false });
    }
    let (app, _) = fake_app(dir.path(), load_config(&dir.path().join("config.json")));
    assert_eq!(app.table.widths[Column::Map as usize], 16384.0);
    assert_eq!(app.table.model.sort().column, Column::Players);
}

#[test]
fn shrinking_window_keeps_table_minimum() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::WindowResized(Size::new(1100.0, 700.0)));
    let _ = app.update(Message::SplitterStart);
    let _ = app.update(Message::DragMoved(0.0));
    let _ = app.update(Message::DragMoved(470.0));
    let _ = app.update(Message::DragEnd);
    assert_eq!(app.sidebar_width, 700.0);
    let _ = app.update(Message::WindowResized(Size::new(800.0, 600.0)));
    assert_eq!(app.sidebar_width, 800.0 - 324.0);
}

#[test]
fn window_on_another_monitor_is_not_moved() {
    let monitor = Size::new(1920.0, 1080.0);
    // Un segundo monitor a la derecha, a la izquierda o arriba del principal: queda igual.
    for saved in [
        "1100,700,2500,100",
        "1100,700,-1500,100",
        "1100,700,300,-900",
        "1100,700,4000,1500",
        "1100,700,-3800,100", // un 4K a la izquierda
    ] {
        let g = parse_geometry(saved).unwrap();
        assert_eq!(fit(g, monitor), g, "{saved}");
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = fake_app(dir.path(), cfg_with(&[("rs_geometry", saved)]));
        let id = window::Id::unique();
        let _ = app.update(Message::WindowOpened(id, g.position));
        let task = app.update(Message::MonitorSize(id, Some(monitor)));
        assert_eq!(window_requests(task), 0, "{saved}");
    }
    // Más grande que el monitor: se achica sin moverla.
    let g = parse_geometry("2500,1400,2500,100").unwrap();
    let f = fit(g, monitor);
    assert_eq!(f.size, monitor);
    assert_eq!(f.position, g.position);
    // Imposible en cualquier disposición razonable: se centra.
    for saved in [
        "1100,700,-9000,100",
        "1100,700,100,-9000",
        "1100,700,9000,100",
    ] {
        let g = parse_geometry(saved).unwrap();
        assert_eq!(
            fit(g, monitor).position,
            Some(Point::new(410.0, 190.0)),
            "{saved}"
        );
    }
}

fn maximized(request: u64, maximized: bool) -> Message {
    Message::WindowMaximized { request, maximized }
}

#[test]
fn bigger_neighbour_monitor_keeps_position() {
    // Portátil de 1280 con un 4K a la derecha o a la izquierda.
    let laptop = Size::new(1280.0, 800.0);
    for saved in [
        "1100,700,4280,100",
        "1100,700,-3800,100",
        "1100,700,2500,100",
    ] {
        let g = parse_geometry(saved).unwrap();
        assert_eq!(fit(g, laptop), g, "{saved}");
    }
    // Tras achicarla a 1920×1080, -9000 + 1920 queda más allá de dos monitores de 2560.
    let g = parse_geometry("5000,4000,-9000,-9000").unwrap();
    let f = fit(g, Size::new(1920.0, 1080.0));
    assert_eq!(f.position, Some(Point::new(0.0, 0.0)));
}

#[test]
fn late_maximized_reply_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "1200,800,10,20")]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    let _ = app.update(Message::WindowResized(Size::new(1300.0, 900.0))); // pedido 1
    let _ = app.update(Message::WindowResized(Size::new(1910.0, 1000.0))); // pedido 2
    // Llega tarde la respuesta del pedido 1 (ya maximizada): no cuenta.
    let _ = app.update(maximized(1, false));
    let _ = app.update(Message::Close { maximized: true });
    assert_eq!(
        saved_geometry(dir.path()).as_deref(),
        Some("1200,800,10,20,max")
    );
}

#[test]
fn monitor_sized_window_is_not_normal_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "1200,800,10,20")]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    let id = window::Id::unique();
    let _ = app.update(Message::MonitorSize(id, Some(Size::new(1920.0, 1080.0))));
    // Maximizada, pero el sistema todavía dice que no (X11): cubre el monitor, no se toma.
    let _ = app.update(Message::WindowResized(Size::new(1920.0, 1040.0))); // pedido 1
    let _ = app.update(maximized(1, false));
    let _ = app.update(Message::Close { maximized: true });
    assert_eq!(
        saved_geometry(dir.path()).as_deref(),
        Some("1200,800,10,20,max")
    );
    // Una normal más chica sí se toma.
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[("rs_geometry", "1200,800,10,20")]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    let _ = app.update(Message::MonitorSize(id, Some(Size::new(1920.0, 1080.0))));
    let _ = app.update(Message::WindowResized(Size::new(1500.0, 900.0)));
    let _ = app.update(maximized(1, false));
    let _ = app.update(Message::Close { maximized: true });
    assert_eq!(
        saved_geometry(dir.path()).as_deref(),
        Some("1500,900,10,20,max")
    );
}

/// Windows minimiza con `Resized(0×0)` y `Moved(-32000, -32000)`: ni la barra lateral ni la
/// geometría normal cambian.
#[test]
fn minimize_keeps_sidebar_and_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_with(&[
        ("rs_geometry", "1200,800,10,20"),
        ("rs_body_splitter", "400"),
    ]);
    let (mut app, _) = fake_app(dir.path(), cfg);
    assert_eq!(app.sidebar_width, 400.0);
    let resized = app.update(Message::WindowResized(Size::ZERO));
    let moved = app.update(Message::WindowMoved(Point::new(-32000.0, -32000.0)));
    assert_eq!(window_requests(resized) + window_requests(moved), 0);
    assert_eq!(app.sidebar_width, 400.0);
    let _ = app.update(maximized(0, false));
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(
        saved_geometry(dir.path()).as_deref(),
        Some("1200,800,10,20")
    );
    let cfg = load_config(&dir.path().join("config.json"));
    assert_eq!(
        cfg.ui_state.get("rs_body_splitter").map(String::as_str),
        Some("400")
    );
}
