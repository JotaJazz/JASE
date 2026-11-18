//! Estado de la ventana que sobrevive al cierre (`_save_ui_state`/`_restore_ui_state`): tamaño,
//! posición y maximizada, ancho de la barra lateral, anchos de columna y orden. Van en claves
//! `rs_` de `ui_state`; los blobs de Qt (`geometry`, `table_header`, `*_splitter`) no se tocan,
//! para que 1.3.0 los siga leyendo.

use iced::{Event, Point, Size, Subscription, Task, event, mouse, window};
use jase_core::config::Config;

use crate::app::{App, Message};
use crate::icons;
use crate::table::{Column, Sort};

const GEOMETRY: &str = "rs_geometry";
const SPLITTER: &str = "rs_body_splitter";
const HEADER: &str = "rs_table_header";

/// `setSizes([230, …])` y los límites del divisor.
pub const SIDEBAR_WIDTH: f32 = 230.0;
const SIDEBAR_MIN: f32 = 150.0;
const TABLE_MIN: f32 = 300.0;
/// Márgenes izquierdo y derecho del cuerpo (10 + 10) y el divisor (6).
const BODY_CHROME: f32 = 26.0;
/// `minimumSectionSize` de las columnas.
pub const COLUMN_MIN: f32 = 20.0;
/// Más que esto no lo da ninguna pantalla: es basura.
const MAX_PX: f32 = 16384.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub size: Size,
    /// `None` donde el sistema no la informa (Wayland).
    pub position: Option<Point>,
    pub maximized: bool,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            size: crate::WINDOW_SIZE,
            position: None,
            maximized: false,
        }
    }
}

/// Arrastre en curso: el borde de una columna o el divisor, con la `x` del ratón al empezar
/// (`NaN` hasta el primer movimiento: el clic no trae posición) y el ancho de entonces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drag {
    Column(Column, f32, f32),
    Sidebar(f32, f32),
}

fn number(s: &str, min: f32) -> Option<f32> {
    let v: f32 = s.trim().parse().ok()?;
    (v.is_finite() && (min..=MAX_PX).contains(&v)).then_some(v)
}

fn coord(s: &str) -> Option<f32> {
    number(s, -MAX_PX)
}

/// `"w,h"`, más `",x,y"` si hay posición y `",max"` si estaba maximizada.
pub fn parse_geometry(s: &str) -> Option<Geometry> {
    let mut parts: Vec<&str> = s.split(',').collect();
    let maximized = parts.last() == Some(&"max");
    if maximized {
        parts.pop();
    }
    let (size, position) = match parts[..] {
        [w, h] => ((w, h), None),
        [w, h, x, y] => ((w, h), Some(Point::new(coord(x)?, coord(y)?))),
        _ => return None,
    };
    Some(Geometry {
        size: Size::new(number(size.0, 1.0)?, number(size.1, 1.0)?),
        position,
        maximized,
    })
}

pub fn geometry_value(g: &Geometry) -> String {
    let mut v = format!("{},{}", g.size.width.round(), g.size.height.round());
    if let Some(p) = g.position {
        v += &format!(",{},{}", p.x.round(), p.y.round());
    }
    if g.maximized {
        v += ",max";
    }
    v
}

/// `"8;28,48,0,70,90,170,150,70;3;asc"`: columnas, anchos (*Servidor* estira: su valor no
/// cuenta), columna de orden y sentido. Otro número de columnas o un campo inválido
/// descarta todo (`table_header_cols` de Python).
pub fn parse_header(s: &str) -> Option<([f32; 8], Sort)> {
    let [cols, widths, column, order] = s.split(';').collect::<Vec<_>>()[..] else {
        return None;
    };
    if cols != Column::ALL.len().to_string() {
        return None;
    }
    let widths: [&str; 8] = widths.split(',').collect::<Vec<_>>().try_into().ok()?;
    let mut out = [0.0; 8];
    for ((w, col), out) in widths.into_iter().zip(Column::ALL).zip(&mut out) {
        *out = number(w, if col == Column::Name { 0.0 } else { COLUMN_MIN })?;
    }
    let column = *Column::ALL.get(column.parse::<usize>().ok()?)?;
    let ascending = match order {
        "asc" => true,
        "desc" => false,
        _ => return None,
    };
    Some((out, Sort { column, ascending }))
}

pub fn header_value(widths: &[f32; 8], sort: Sort) -> String {
    let widths: Vec<String> = widths.iter().map(|w| w.round().to_string()).collect();
    let order = if sort.ascending { "asc" } else { "desc" };
    format!(
        "{};{};{};{order}",
        widths.len(),
        widths.join(","),
        sort.column as usize
    )
}

/// Ancho de la barra lateral: al menos 150 px y que la tabla conserve 300.
pub fn clamp_sidebar(w: f32, window_w: f32) -> f32 {
    w.min(window_w - BODY_CHROME - TABLE_MIN).max(SIDEBAR_MIN)
}

/// Lo guardado que sea válido; lo demás queda por defecto.
pub fn restore(app: &mut App) {
    let state = &app.cfg.ui_state;
    let geometry = state.get(GEOMETRY).and_then(|v| parse_geometry(v));
    let sidebar = state.get(SPLITTER).and_then(|v| number(v, SIDEBAR_MIN));
    let header = state.get(HEADER).and_then(|v| parse_header(v));
    app.window = geometry.unwrap_or_default();
    if let Some(w) = sidebar {
        app.sidebar_width = clamp_sidebar(w, app.window.size.width);
    }
    if let Some((widths, sort)) = header {
        app.table.widths = widths;
        app.table.model.set_sort(sort);
    }
}

/// Escribe las claves `rs_` en la config (la guarda quien cierra). Maximizada, el tamaño y la
/// posición guardados quedan: los que informa el sistema son los de la pantalla entera.
/// shortcut: lo cambiado antes de maximizar en la misma sesión se pierde (Qt guarda
/// `normalGeometry`); pedir tamaño y posición al desmaximizar si molesta.
pub fn save(app: &mut App, maximized: bool) {
    let geometry = if maximized {
        let saved = app
            .cfg
            .ui_state
            .get(GEOMETRY)
            .and_then(|v| parse_geometry(v));
        Geometry {
            maximized: true,
            ..saved.unwrap_or_default()
        }
    } else {
        Geometry {
            maximized: false,
            ..app.window
        }
    };
    let state = &mut app.cfg.ui_state;
    state.insert(GEOMETRY.into(), geometry_value(&geometry));
    state.insert(SPLITTER.into(), app.sidebar_width.round().to_string());
    state.insert(
        HEADER.into(),
        header_value(&app.table.widths, app.table.model.sort()),
    );
}

/// La ventana como quedó al cerrar; el cierre pasa por `CloseRequested` (guarda la config y
/// sale sin esperas).
pub fn window_settings(cfg: &Config) -> window::Settings {
    let g = cfg
        .ui_state
        .get(GEOMETRY)
        .and_then(|v| parse_geometry(v))
        .unwrap_or_default();
    window::Settings {
        size: g.size,
        position: g
            .position
            .map_or(window::Position::Default, window::Position::Specific),
        maximized: g.maximized,
        exit_on_close_request: false,
        icon: icons::app_icon(),
        ..window::Settings::default()
    }
}

/// Mueve el borde que se arrastra; `x` es la del ratón en la ventana.
pub fn drag_moved(app: &mut App, x: f32) {
    let Some(drag) = &mut app.drag else {
        return;
    };
    let (x0, w0) = match drag {
        Drag::Column(_, x0, w0) | Drag::Sidebar(x0, w0) => (x0, *w0),
    };
    if x0.is_nan() {
        *x0 = x;
    }
    let w = w0 + x - *x0;
    match *drag {
        Drag::Column(col, ..) => app.table.widths[col as usize] = w.max(COLUMN_MIN),
        Drag::Sidebar(..) => app.sidebar_width = clamp_sidebar(w, app.window.size.width),
    }
}

pub fn start_column(app: &mut App, col: Column) {
    app.drag = Some(Drag::Column(col, f32::NAN, app.table.widths[col as usize]));
}

pub fn start_sidebar(app: &mut App) {
    app.drag = Some(Drag::Sidebar(f32::NAN, app.sidebar_width));
}

/// El ratón en toda la ventana, solo mientras dura un arrastre.
pub fn drag_subscription(app: &App) -> Option<Subscription<Message>> {
    app.drag.is_some().then(|| {
        event::listen_with(|e, _, _| match e {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                Some(Message::DragMoved(position.x))
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Message::DragEnd)
            }
            _ => None,
        })
    })
}

/// La posición al abrir y al mover (Wayland no la da).
pub fn moves() -> Subscription<Message> {
    window::events().filter_map(|(_, e)| match e {
        window::Event::Opened {
            position: Some(p), ..
        }
        | window::Event::Moved(p) => Some(Message::WindowMoved(p)),
        _ => None,
    })
}

/// Pregunta si está maximizada y sigue en `Message::Close`.
pub fn close() -> Task<Message> {
    window::latest()
        .then(|id| id.map_or_else(|| Task::done(false), window::is_maximized))
        .map(|maximized| Message::Close { maximized })
}
