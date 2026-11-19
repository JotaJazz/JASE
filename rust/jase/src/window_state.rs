//! Estado de la ventana que sobrevive al cierre (`_save_ui_state`/`_restore_ui_state`): tamaño,
//! posición y maximizada, ancho de la barra lateral, anchos de columna y orden, y las divisiones
//! de la columna derecha (tabla | ficha; jugadores | reglas). Van en claves
//! `rs_` de `ui_state`; los blobs de Qt (`geometry`, `table_header`, `*_splitter`) no se tocan,
//! para que 1.3.0 los siga leyendo.

use iced::widget::pane_grid::{self, Axis, Configuration, Node, ResizeEvent};
use iced::{Event, Point, Size, Subscription, Task, event, mouse, window};
use jase_core::config::Config;

use crate::app::{App, Message};
use crate::icons;
use crate::table::{Column, Sort};

const GEOMETRY: &str = "rs_geometry";
const SPLITTER: &str = "rs_body_splitter";
const HEADER: &str = "rs_table_header";
const RIGHT_SPLIT: &str = "rs_right_splitter";
const DETAILS_SPLIT: &str = "rs_details_splitter";

/// Parte de la tabla en la columna derecha (Qt offscreen a 1280×800: `[318, 371]`; la misma
/// proporción a otros altos) y de los jugadores junto a las reglas (`[511, 511]`). El
/// `pane_grid` resta la mitad del asa a la parte: `(318 + 2) / 693`.
pub const RIGHT_RATIO: f32 = 0.4618;
pub const DETAILS_RATIO: f32 = 0.5;
/// Lo que se acepta de una proporción guardada (y de un arrastre).
const RATIO_MIN: f32 = 0.05;
const RATIO_MAX: f32 = 0.95;

/// Paneles de la columna derecha, uno arriba del otro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPane {
    Table,
    Lower,
}

/// Paneles de detalle, uno al lado del otro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailPane {
    Players,
    Rules,
}

/// `setSizes([230, …])` y los límites del divisor.
pub const SIDEBAR_WIDTH: f32 = 230.0;
const SIDEBAR_MIN: f32 = 150.0;
const TABLE_MIN: f32 = 300.0;
/// Asa del divisor (`handleWidth()` de Qt, medido offscreen).
pub const SPLITTER_WIDTH: f32 = 4.0;
/// Márgenes izquierdo y derecho del cuerpo (10 + 10) y el divisor.
const BODY_CHROME: f32 = 20.0 + SPLITTER_WIDTH;
/// `minimumSectionSize` de las columnas.
pub const COLUMN_MIN: f32 = 20.0;
/// Donde Windows deja una ventana minimizada (`WM_WINDOWPOSCHANGED`).
const MINIMIZED_POS: f32 = -32000.0;
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

/// Proporción guardada: un número finito en `[0.05, 0.95]`.
pub fn parse_ratio(s: &str) -> Option<f32> {
    let v: f32 = s.trim().parse().ok()?;
    (v.is_finite() && (RATIO_MIN..=RATIO_MAX).contains(&v)).then_some(v)
}

fn split<T>(axis: Axis, ratio: f32, a: T, b: T) -> pane_grid::State<T> {
    pane_grid::State::with_configuration(Configuration::Split {
        axis,
        ratio,
        a: Box::new(Configuration::Pane(a)),
        b: Box::new(Configuration::Pane(b)),
    })
}

/// Tabla arriba y ficha con jugadores y reglas abajo (`right_splitter`).
pub fn right_panes(ratio: f32) -> pane_grid::State<RightPane> {
    split(Axis::Horizontal, ratio, RightPane::Table, RightPane::Lower)
}

/// Jugadores a la izquierda y reglas a la derecha (`details_splitter`).
pub fn detail_panes(ratio: f32) -> pane_grid::State<DetailPane> {
    split(
        Axis::Vertical,
        ratio,
        DetailPane::Players,
        DetailPane::Rules,
    )
}

fn ratio_of<T>(state: &pane_grid::State<T>) -> f32 {
    match state.layout() {
        Node::Split { ratio, .. } => *ratio,
        Node::Pane(_) => 0.5, // no pasa: los dos `State` nacen divididos
    }
}

/// Mueve un divisor sin salirse del rango que se guarda.
pub fn resize<T>(state: &mut pane_grid::State<T>, e: ResizeEvent) {
    state.resize(e.split, e.ratio.clamp(RATIO_MIN, RATIO_MAX));
}

impl App {
    /// Proporciones de la tabla en la columna derecha y de los jugadores en el detalle.
    pub fn split_ratios(&self) -> (f32, f32) {
        (ratio_of(&self.right), ratio_of(&self.details))
    }
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
    let ratio = |key: &str, default: f32| {
        state
            .get(key)
            .and_then(|v| parse_ratio(v))
            .unwrap_or(default)
    };
    app.right = right_panes(ratio(RIGHT_SPLIT, RIGHT_RATIO));
    app.details = detail_panes(ratio(DETAILS_SPLIT, DETAILS_RATIO));
    app.window = geometry.unwrap_or_default();
    app.normal = app.window;
    if let Some(w) = sidebar {
        app.sidebar_width = clamp_sidebar(w, app.window.size.width);
    }
    if let Some((widths, sort)) = header {
        app.table.widths = widths;
        app.table.model.set_sort(sort);
    }
}

/// Escribe las claves `rs_` en la config (la guarda quien cierra). Maximizada se guarda el
/// último tamaño y posición sin maximizar (`normalGeometry` de Qt), no los de la pantalla.
pub fn save(app: &mut App, maximized: bool) {
    let geometry = if maximized {
        Geometry {
            maximized: true,
            ..app.normal
        }
    } else {
        Geometry {
            maximized: false,
            ..app.window
        }
    };
    let (right, details) = app.split_ratios();
    let state = &mut app.cfg.ui_state;
    state.insert(GEOMETRY.into(), geometry_value(&geometry));
    state.insert(RIGHT_SPLIT.into(), format!("{right:.4}"));
    state.insert(DETAILS_SPLIT.into(), format!("{details:.4}"));
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
        Drag::Column(col, ..) => app.table.widths[col as usize] = w.clamp(COLUMN_MIN, MAX_PX),
        Drag::Sidebar(..) => app.sidebar_width = clamp_sidebar(w, app.window.size.width),
    }
}

pub fn start_column(app: &mut App, col: Column) {
    app.drag = Some(Drag::Column(col, f32::NAN, app.table.widths[col as usize]));
}

pub fn start_sidebar(app: &mut App) {
    app.drag = Some(Drag::Sidebar(f32::NAN, app.sidebar_width));
}

/// El ratón en toda la ventana, solo mientras dura un arrastre (con la ventana: cuenta la
/// principal).
pub fn drag_subscription(app: &App) -> Option<Subscription<(window::Id, Message)>> {
    app.drag.is_some().then(|| {
        event::listen_with(|e, _, id| match e {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                Some((id, Message::DragMoved(position.x)))
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some((id, Message::DragEnd))
            }
            _ => None,
        })
    })
}

/// La apertura y la posición al mover (Wayland no la da), con la ventana: cuenta la principal.
pub fn moves() -> Subscription<(window::Id, Message)> {
    window::events().filter_map(|(id, e)| match e {
        window::Event::Opened { position, .. } => Some((id, Message::WindowOpened(id, position))),
        window::Event::Moved(p) => Some((id, Message::WindowMoved(p))),
        _ => None,
    })
}

/// Monitores que puede haber a cada lado del actual (ver `fit`).
const MONITORS_AROUND: f32 = 2.0;
/// Los vecinos pueden ser más grandes que el actual (un portátil junto a un 4K): el alcance
/// cuenta al menos monitores de 2560×1440.
const NEIGHBOUR_MIN: Size = Size::new(2560.0, 1440.0);
/// Lo que puede faltarle a una ventana maximizada para cubrir el monitor (bordes, barra de
/// título y de tareas).
const MAXIMIZED_SLACK: Size = Size::new(16.0, 100.0);

/// Lo guardado ajustado al monitor actual, de tamaño `monitor`: más chica que él y, solo si
/// la posición es imposible, centrada. Iced 0.14 no da la lista de monitores ni el origen del
/// actual (`window::run` solo expone los handles de `raw-window-handle`), así que una
/// posición que podría estar en otro monitor no se toca: se corrige solo si la ventana no
/// toca la zona del monitor actual más dos monitores a cada lado, cada uno del tamaño del
/// actual y al menos de 2560×1440 (deuda.md).
pub fn fit(g: Geometry, monitor: Size) -> Geometry {
    let size = Size::new(
        g.size.width.min(monitor.width),
        g.size.height.min(monitor.height),
    );
    let reach = |p: f32, len: f32, screen: f32| {
        p + len > -MONITORS_AROUND * screen && p < (MONITORS_AROUND + 1.0) * screen
    };
    let possible = |p: Point| {
        reach(p.x, size.width, monitor.width.max(NEIGHBOUR_MIN.width))
            && reach(p.y, size.height, monitor.height.max(NEIGHBOUR_MIN.height))
    };
    let centered = Point::new(
        ((monitor.width - size.width) / 2.0).round(),
        ((monitor.height - size.height) / 2.0).round(),
    );
    Geometry {
        size,
        position: g.position.map(|p| if possible(p) { p } else { centered }),
        ..g
    }
}

/// Al abrir: anota la posición y pide el monitor para ajustar lo restaurado.
pub fn opened(app: &mut App, id: window::Id, position: Option<Point>) -> Task<Message> {
    if position.is_some() {
        app.window.position = position;
    }
    window::monitor_size(id).map(move |size| Message::MonitorSize(id, size))
}

/// Achica o centra la ventana restaurada si no entra en el monitor. Maximizada solo se
/// ajusta lo que se guardará al desmaximizar.
pub fn on_monitor(app: &mut App, id: window::Id, monitor: Option<Size>) -> Task<Message> {
    let Some(monitor) = monitor else {
        return Task::none();
    };
    app.monitor = Some(monitor);
    let fitted = fit(app.normal, monitor);
    if fitted == app.normal {
        return Task::none();
    }
    app.normal = fitted;
    if app.window.maximized {
        return Task::none();
    }
    let mut tasks = Vec::new();
    if fitted.size != app.window.size {
        tasks.push(window::resize(id, fitted.size));
    }
    if let Some(p) = fitted.position.filter(|&p| Some(p) != app.window.position) {
        tasks.push(window::move_to(id, p));
    }
    app.window = fitted;
    app.sidebar_width = clamp_sidebar(app.sidebar_width, fitted.size.width);
    Task::batch(tasks)
}

/// Nuevo tamaño: la barra lateral deja a la tabla sus 300 px; se pregunta si está maximizada.
/// Minimizada, Windows informa 0×0: no es un tamaño de la ventana.
pub fn resized(app: &mut App, size: Size) -> Task<Message> {
    if size.width == 0.0 || size.height == 0.0 {
        return Task::none();
    }
    app.window.size = size;
    app.sidebar_width = clamp_sidebar(app.sidebar_width, size.width);
    ask_maximized(app)
}

/// Windows lleva la ventana minimizada a (-32000, -32000): no es una posición de verdad.
pub fn moved(app: &mut App, p: Point) -> Task<Message> {
    if p.x <= MINIMIZED_POS || p.y <= MINIMIZED_POS {
        return Task::none();
    }
    app.window.position = Some(p);
    ask_maximized(app)
}

/// Las respuestas llegan sin orden: solo cuenta la del último evento (`maximized_request`).
fn ask_maximized(app: &mut App) -> Task<Message> {
    app.maximized_request += 1;
    let request = app.maximized_request;
    window::is_maximized(app.main_window())
        .map(move |maximized| Message::WindowMaximized { request, maximized })
}

/// Sin maximizar, el tamaño y la posición actuales son los que se guardan maximizada.
/// Un tamaño que cubre el monitor no se toma como normal: el estado maximizado puede llegar
/// con retraso (X11).
pub fn on_maximized(app: &mut App, request: u64, maximized: bool) {
    if request != app.maximized_request {
        return;
    }
    app.window.maximized = maximized;
    let covers = app.monitor.is_some_and(|m| {
        app.window.size.width >= m.width - MAXIMIZED_SLACK.width
            && app.window.size.height >= m.height - MAXIMIZED_SLACK.height
    });
    if !maximized && !covers {
        app.normal = app.window;
    }
}

/// Pregunta si la principal está maximizada y sigue en `Message::Close`.
pub fn close(main: window::Id) -> Task<Message> {
    window::is_maximized(main).map(|maximized| Message::Close { maximized })
}
