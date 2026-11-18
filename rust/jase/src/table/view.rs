//! Vista de la tabla: solo arma las filas visibles (alto fijo), encabezado fijo y teclado.

use std::ops::Range;

use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::scrollable::{Direction, Scrollbar, Viewport};
use iced::widget::text::{Span, Wrapping};
use iced::widget::{
    Column as Col, Id, Space, container, mouse_area, rich_text, row, scrollable, span, text,
    tooltip,
};
use iced::{Color, Element, Fill, Length, Padding, Task};
use jase_core::i18n::tr_n;
use jase_core::models::{ServerAddress, ServerInfo};

use super::{Column, Sort, TableModel, cell_text};
use crate::app::Message;
use crate::colored::{cell_text_color, color_runs};
use crate::theme::{Palette, WidgetStyle, framed, palette, ping_color, style, widget_style};

pub const ROW_HEIGHT: f32 = 22.0;
const TABLE_ID: Id = Id::new("server-table");

/// Posiciones (en el orden de la vista) a dibujar, con una fila de margen a cada lado.
/// El desplazamiento se acota al contenido: tras vaciar la tabla puede quedar viejo.
pub fn visible_range(offset_y: f32, viewport_h: f32, rows: usize) -> Range<usize> {
    let max_off = (rows as f32 * ROW_HEIGHT - viewport_h).max(0.0);
    let off = offset_y.clamp(0.0, max_off);
    let first = (off / ROW_HEIGHT) as usize;
    let last = ((off + viewport_h) / ROW_HEIGHT).ceil() as usize;
    first.saturating_sub(1).min(rows)..(last + 1).min(rows)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Nueva posición seleccionada; sin selección, cualquier tecla va a la primera (Fin, a la última).
pub fn step_selection(
    order_pos: Option<usize>,
    rows: usize,
    nav: Nav,
    page_rows: usize,
) -> Option<usize> {
    let last = rows.checked_sub(1)?;
    let Some(pos) = order_pos.map(|p| p.min(last)) else {
        return Some(if nav == Nav::End { last } else { 0 });
    };
    Some(match nav {
        Nav::Up => pos.saturating_sub(1),
        Nav::Down => (pos + 1).min(last),
        Nav::PageUp => pos.saturating_sub(page_rows),
        Nav::PageDown => (pos + page_rows).min(last),
        Nav::Home => 0,
        Nav::End => last,
    })
}

/// Desplazamiento que deja visible la fila `pos`, o `None` si ya se ve entera.
pub fn scroll_to_show(pos: usize, offset_y: f32, viewport_h: f32) -> Option<f32> {
    let top = pos as f32 * ROW_HEIGHT;
    if top < offset_y {
        Some(top)
    } else if top + ROW_HEIGHT > offset_y + viewport_h {
        Some((top + ROW_HEIGHT - viewport_h).max(0.0))
    } else {
        None
    }
}

pub struct Table {
    pub model: TableModel,
    pub selected: Option<ServerAddress>,
    pub offset_y: f32,
    pub viewport_h: f32,
    /// Alto de la ventana: solo agranda lo que se dibuja (si la tabla cabe, Iced no informa
    /// su alto); la navegación usa `viewport_h`, que solo da el `scrollable`.
    pub window_h: f32,
    pub focused: bool,
}

impl Table {
    fn draw_h(&self) -> f32 {
        self.viewport_h.max(self.window_h)
    }
}

impl Default for Table {
    fn default() -> Self {
        Self {
            model: TableModel::default(),
            selected: None,
            offset_y: 0.0,
            // El alto de la ventana hasta que el `scrollable` informe el suyo.
            viewport_h: 700.0,
            window_h: 0.0,
            focused: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Scrolled(Viewport),
    HeaderClicked(Column),
    RowClicked(ServerAddress),
    Nav(Nav),
}

pub fn update(t: &mut Table, msg: Msg) -> Task<Message> {
    match msg {
        Msg::Scrolled(v) => {
            t.offset_y = v.absolute_offset().y;
            t.viewport_h = v.bounds().height;
        }
        Msg::HeaderClicked(column) => {
            let s = t.model.sort();
            // Misma columna: invierte; otra: ascendente.
            let ascending = s.column != column || !s.ascending;
            t.model.set_sort(Sort { column, ascending });
        }
        Msg::RowClicked(address) => {
            t.selected = Some(address);
            t.focused = true;
        }
        Msg::Nav(nav) => return navigate(t, nav),
    }
    Task::none()
}

fn navigate(t: &mut Table, nav: Nav) -> Task<Message> {
    let order = t.model.order();
    let pos = t
        .selected
        .as_ref()
        .and_then(|a| t.model.row_of(a))
        .and_then(|row| order.iter().position(|&r| r == row));
    let page = ((t.viewport_h / ROW_HEIGHT) as usize).max(1);
    let Some(new) = step_selection(pos, order.len(), nav, page) else {
        return Task::none();
    };
    t.selected = Some(t.model.servers()[order[new]].address.clone());
    match scroll_to_show(new, t.offset_y, t.viewport_h) {
        Some(y) => {
            t.offset_y = y;
            operation::scroll_to(
                TABLE_ID,
                AbsoluteOffset {
                    x: None,
                    y: Some(y),
                },
            )
        }
        None => Task::none(),
    }
}

pub fn scroll_to_top(t: &mut Table) -> Task<Message> {
    t.offset_y = 0.0;
    operation::scroll_to(
        TABLE_ID,
        AbsoluteOffset {
            x: None,
            y: Some(0.0),
        },
    )
}

/// Celda lista para dibujar: tramos con su color final y el tooltip, si hay.
#[derive(Debug, Clone, PartialEq)]
pub struct CellVm {
    pub runs: Vec<(String, Color)>,
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RowVm {
    pub address: ServerAddress,
    pub cells: Vec<(Column, CellVm)>,
    pub selected: bool,
    pub online: bool,
}

/// Las filas que dibuja `view`: solo las de `visible_range`, en el orden de la vista.
pub fn visible_rows(t: &Table, p: &Palette) -> Vec<RowVm> {
    let order = t.model.order();
    visible_range(t.offset_y, t.draw_h(), order.len())
        .map(|pos| row_vm(t, &t.model.servers()[order[pos]], p))
        .collect()
}

fn row_vm(t: &Table, info: &ServerInfo, p: &Palette) -> RowVm {
    let selected = t.selected.as_ref() == Some(&info.address);
    let cells = Column::ALL
        .iter()
        .map(|&col| (col, cell_vm(&t.model, info, col, selected, p)))
        .collect();
    RowVm {
        address: info.address.clone(),
        cells,
        selected,
        online: info.online,
    }
}

fn cell_vm(m: &TableModel, info: &ServerInfo, col: Column, selected: bool, p: &Palette) -> CellVm {
    let color = |c| cell_text_color(c, selected, p);
    let plain = |c| vec![(cell_text(info, col, &|v| m.version_label(v)), color(c))];
    // Un favorito caído va atenuado en todas las columnas (`ForegroundRole` de Python).
    let runs = if !info.online {
        plain(Some(p.muted))
    } else if col == Column::Name && !info.raw_name.is_empty() {
        color_runs(&info.raw_name, p)
            .into_iter()
            .map(|(s, c)| (s, color(c)))
            .collect()
    } else if col == Column::Ping {
        plain(Some(ping_color(info.ping_ms, p)))
    } else {
        plain(None)
    };
    let tooltip = (col == Column::Players && info.bots != 0).then(|| {
        tr_n("{n} jugador", "{n} jugadores", info.humans().into(), &[])
            + " · "
            + &tr_n("{n} bot", "{n} bots", info.bots.into(), &[])
    });
    CellVm { runs, tooltip }
}

/// Caja de ancho fijo (o el resto, para Servidor) que recorta lo que no cabe.
fn cell_box<'a>(content: impl Into<Element<'a, Message>>, col: Column) -> Element<'a, Message> {
    container(content)
        .width(col.initial_width().map_or(Fill, Length::Fixed))
        .height(Fill)
        .padding([0, 4])
        .center_y(Fill)
        .clip(true)
        .into()
}

fn row_view<'a>(
    r: RowVm,
    p: &'static Palette,
    s: WidgetStyle,
    focused: bool,
) -> Element<'a, Message> {
    let cells = r.cells.into_iter().map(|(col, cell)| {
        let spans: Vec<Span<'static>> = cell
            .runs
            .into_iter()
            .map(|(s, c)| span(s).color(c))
            .collect();
        let content = cell_box(
            rich_text(spans)
                .font(s.font())
                .size(s.text_size())
                .wrapping(Wrapping::None),
            col,
        );
        match cell.tooltip {
            Some(tip) => tooltip(
                content,
                container(text(tip).font(s.font()).size(s.text_size()))
                    .padding(4)
                    .style(move |_| style::tooltip(p, s)),
                tooltip::Position::Bottom,
            )
            .into(),
            None => content,
        }
    });
    let selected = r.selected;
    mouse_area(
        container(row(cells))
            .height(ROW_HEIGHT)
            .style(move |_| style::table_row(p, s, selected, focused)),
    )
    .on_press(Message::Table(Msg::RowClicked(r.address)))
    .into()
}

pub fn view<'a>(t: &'a Table, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (header_style, section) = style::table_header(p, s, theme_id);
    let header = row(Column::ALL.map(|col| {
        let title = text(col.title())
            .font(s.font())
            .size(s.text_size())
            .wrapping(Wrapping::None);
        // El borde es de cada sección, dentro de su ancho, como en `QHeaderView`.
        let section = framed(
            container(title)
                .width(Fill)
                .height(Fill)
                .padding([0, 4])
                .center_y(Fill)
                .clip(true),
            section.unwrap_or_default(),
        );
        mouse_area(
            container(section)
                .width(col.initial_width().map_or(Fill, Length::Fixed))
                .height(Fill),
        )
        .on_press(Message::Table(Msg::HeaderClicked(col)))
        .into()
    }));
    // Las barras clásicas ocupan su lugar (la plana flota): el encabezado termina antes.
    let embedded = s != WidgetStyle::Flat;
    let bar_w = style::scrollbar_width(s);
    let header = container(header)
        .height(ROW_HEIGHT)
        .padding(Padding {
            right: if embedded { bar_w } else { 0.0 },
            ..Padding::ZERO
        })
        .style(move |_| header_style);
    let n = t.model.len();
    let range = visible_range(t.offset_y, t.draw_h(), n);
    let body = Col::new()
        .width(Fill)
        .push(Space::new().height(range.start as f32 * ROW_HEIGHT))
        .extend(
            visible_rows(t, p)
                .into_iter()
                .map(|r| row_view(r, p, s, t.focused)),
        )
        .push(Space::new().height((n - range.end) as f32 * ROW_HEIGHT));
    let mut bar = Scrollbar::new().width(bar_w).scroller_width(bar_w);
    if embedded {
        bar = bar.spacing(0);
    }
    let body = scrollable(body)
        .id(TABLE_ID)
        .direction(Direction::Vertical(bar))
        .on_scroll(|v| Message::Table(Msg::Scrolled(v)))
        .style(move |_, status| style::scrollable(p, s, status))
        .height(Fill);
    let (frame_style, edges) = style::table_frame(p, s);
    let table = container(Col::new().push(header).push(body))
        .padding(frame_style.border.width) // que el encabezado no tape el borde
        .style(move |_| frame_style);
    match edges {
        Some(e) => framed(table, e),
        None => table.into(),
    }
}
