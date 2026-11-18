//! Vista de la tabla: solo arma las filas visibles (alto fijo), encabezado fijo y teclado.

use std::ops::Range;

use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::scrollable::{Direction, Viewport};
use iced::widget::text::{Span, Wrapping};
use iced::widget::{
    Column as Col, Id, Space, container, mouse_area, rich_text, row, scrollable, span, stack, svg,
    text, tooltip,
};
use iced::{Color, Element, Fill, Length, Padding, Task, border, mouse};
use jase_core::geo::country_name;
use jase_core::i18n::tr_n;
use jase_core::models::{ServerAddress, ServerInfo};

use super::{Column, ServerFilter, Sort, TableModel, cell_text, occupancy_ratio};
use crate::app::Message;
use crate::colored::{cell_text_color, color_runs};
use crate::flags::{FLAG_SIZE, flag};
use crate::icons::sort_arrow;
use crate::theme::{Palette, WidgetStyle, framed, palette, ping_color, style, widget_style};
use crate::widgets::text_width;

/// `setDefaultSectionSize(28)` de `_no_wrap`.
pub const ROW_HEIGHT: f32 = 28.0;
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
    /// Ancho de cada columna en el orden de `Column::ALL`; el de *Servidor* no cuenta (estira).
    pub widths: [f32; 8],
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
            widths: Column::ALL.map(|c| c.initial_width().unwrap_or(0.0)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Scrolled(Viewport),
    HeaderClicked(Column),
    RowClicked(ServerAddress),
    /// Clic en la zona sin filas: la tabla toma el foco (como Qt), sin cambiar la selección.
    BlankClicked,
    /// Clic derecho en una fila (la elige) o en la zona vacía (suelta la selección): abre el
    /// menú contextual.
    RightClicked(Option<ServerAddress>),
    Nav(Nav),
    /// Se aprieta el borde derecho de una columna (el arrastre lo lleva `window_state`).
    ResizeStart(Column),
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
        Msg::BlankClicked => t.focused = true,
        Msg::RightClicked(address) => {
            t.selected = address;
            t.focused = true;
        }
        Msg::Nav(nav) => return navigate(t, nav),
        Msg::ResizeStart(_) => {}
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

/// Cambia el filtro y suelta la selección que deja de verse: todo cambio de filtro pasa por aquí.
pub fn set_filter(t: &mut Table, filter: ServerFilter) {
    t.model.set_filter(filter);
    drop_hidden_selection(t);
}

/// Suelta la selección si su fila ya no se ve (filtro, `remove` o `clear`), como el proxy de Qt.
pub fn drop_hidden_selection(t: &mut Table) {
    if t.selected.as_ref().is_some_and(|a| !t.model.is_shown(a)) {
        t.selected = None;
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

/// `Some(ascendente)` solo en la columna por la que se ordena.
pub fn sort_indicator(t: &Table, col: Column) -> Option<bool> {
    let s = t.model.sort();
    (s.column == col).then_some(s.ascending)
}

/// Ancho de la barra plana: `max(1, round(cell_w × ratio))`, nunca más que la celda.
pub fn occupancy_bar_width(ratio: f32, cell_w: f32) -> f32 {
    if ratio <= 0.0 {
        return 0.0;
    }
    (cell_w * ratio.min(1.0)).round().max(1.0).min(cell_w)
}

const BLOCK: f32 = 6.0;
const BLOCK_GAP: f32 = 2.0;

/// (x, ancho) de los bloques dentro del interior de la barra (`_paint_blocks`).
pub fn occupancy_blocks(ratio: f32, inner_w: f32) -> Vec<(f32, f32)> {
    let end = (inner_w * ratio.clamp(0.0, 1.0)).round();
    let mut blocks = Vec::new();
    let mut x = 0.0;
    while x < end {
        blocks.push((x, BLOCK.min(end - x)));
        x += BLOCK + BLOCK_GAP;
    }
    blocks
}

/// Celda lista para dibujar: tramos con su color final, el tooltip, la ocupación (Jugadores,
/// solo con gente) y el código de la bandera (País, solo si hay bandera).
#[derive(Debug, Clone, PartialEq)]
pub struct CellVm {
    pub runs: Vec<(String, Color)>,
    pub tooltip: Option<String>,
    pub occupancy: Option<f32>,
    pub flag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RowVm {
    pub address: ServerAddress,
    pub cells: Vec<(Column, CellVm)>,
    pub selected: bool,
    pub online: bool,
}

/// Las filas que dibuja `view`: solo las de `visible_range`, en el orden de la vista.
pub fn visible_rows(t: &Table, p: &Palette, s: WidgetStyle) -> Vec<RowVm> {
    let order = t.model.order();
    let sel = style::selection_colors(p, s, t.focused);
    visible_range(t.offset_y, t.draw_h(), order.len())
        .map(|pos| row_vm(t, &t.model.servers()[order[pos]], p, sel))
        .collect()
}

fn row_vm(t: &Table, info: &ServerInfo, p: &Palette, sel: (Color, Color)) -> RowVm {
    let selected = t.selected.as_ref() == Some(&info.address);
    let sel = selected.then_some(sel);
    let cells = Column::ALL
        .iter()
        .map(|&col| (col, cell_vm(&t.model, info, col, sel, p)))
        .collect();
    RowVm {
        address: info.address.clone(),
        cells,
        selected,
        online: info.online,
    }
}

fn cell_vm(
    m: &TableModel,
    info: &ServerInfo,
    col: Column,
    sel: Option<(Color, Color)>,
    p: &Palette,
) -> CellVm {
    let color = |c| cell_text_color(c, sel, p);
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
    let country = (col == Column::Country)
        .then(|| m.country_code(info))
        .flatten();
    let tooltip = match col {
        Column::Players if info.bots != 0 => Some(
            tr_n("{n} jugador", "{n} jugadores", info.humans().into(), &[])
                + " · "
                + &tr_n("{n} bot", "{n} bots", info.bots.into(), &[]),
        ),
        Column::Country => country.map(|c| country_name(Some(c))),
        _ => None,
    };
    let occupancy = (col == Column::Players && info.online)
        .then(|| occupancy_ratio(info.num_players, info.max_players))
        .filter(|&r| r > 0.0);
    let flag = country
        .filter(|code| flag(code).is_some())
        .map(str::to_owned);
    CellVm {
        runs,
        tooltip,
        occupancy,
        flag,
    }
}

/// Ancho de la columna en encabezado y celdas: el guardado, o el resto para *Servidor*.
fn column_width(widths: &[f32; 8], col: Column) -> Length {
    if col == Column::Name {
        Fill
    } else {
        Length::Fixed(widths[col as usize])
    }
}

/// Caja de ancho fijo (o el resto, para Servidor) que recorta lo que no cabe.
fn cell_box<'a>(
    content: impl Into<Element<'a, Message>>,
    width: Length,
    pad: f32,
) -> Element<'a, Message> {
    container(content)
        .width(width)
        .height(Fill)
        .padding([0.0, pad])
        .center_y(Fill)
        .clip(true)
        .into()
}

/// Ancho de «n/max» (como mínimo el de «00/00», `_paint_blocks`).
/// shortcut: avances de DejaVu Sans (la que cae sin Tahoma: dígito 0,636 em, barra 0,337 em) y
/// de DejaVu Sans Mono (0,602 em) en vez de medir con `Paragraph`; medir si cambia la letra.
pub fn players_text_width(chars: usize, s: WidgetStyle) -> f32 {
    let (n, em) = (chars.max("00/00".len()) as f32, s.text_size());
    if s == WidgetStyle::Terminal {
        n * 0.602 * em
    } else {
        ((n - 1.0) * 0.636 + 0.337) * em
    }
}

/// El título que cabe en `avail` px, cortado con «…» como `QHeaderView` (elide a la derecha).
/// Mide con el mismo sistema de letras que dibuja; solo corre para los títulos de ancho fijo.
pub fn elide_title(title: &str, avail: f32, s: WidgetStyle) -> String {
    let fits = |t: &str| text_width(t, style::header_font(s), s.text_size()) <= avail;
    if fits(title) {
        return title.to_owned();
    }
    let chars: Vec<char> = title.chars().collect();
    (0..chars.len())
        .rev()
        .map(|n| chars[..n].iter().chain(['…'].iter()).collect::<String>())
        .find(|t| fits(t))
        .unwrap_or_default()
}

/// Jugadores en los temas de época: «n/max» y a su derecha la barra por bloques.
fn blocks_cell<'a>(
    label: Element<'a, Message>,
    chars: usize,
    ratio: f32,
    cell_w: f32,
    p: &'static Palette,
    s: WidgetStyle,
) -> Element<'a, Message> {
    // Qt deja el texto a 2 px del borde de la celda y el marco a 4 px del otro lado.
    let pad = Padding {
        left: 2.0,
        right: 4.0,
        ..Padding::ZERO
    };
    let text_w = players_text_width(chars, s);
    let left = pad.left + text_w + 6.0;
    let (frame_w, frame_h) = (cell_w - pad.right - left, ROW_HEIGHT - 8.0);
    let label = container(label).width(text_w).clip(true);
    if frame_w < 12.0 || frame_h < 6.0 {
        return row![label].padding(pad).height(Fill).into();
    }
    let blocks = occupancy_blocks(ratio, frame_w - 4.0)
        .into_iter()
        .map(|(_, w)| {
            container(Space::new())
                .width(w)
                .height(Fill)
                .style(move |_| style::occupancy_block(p))
                .into()
        });
    let edges = style::occupancy_frame(p, s);
    // El marco de 1 px lo pone `framed`; el otro px de margen interior, el `padding`.
    let well = container(row(blocks).spacing(BLOCK_GAP))
        .width(frame_w - 2.0)
        .height(frame_h - 2.0)
        .padding(1)
        .style(move |_| container::Style {
            background: Some(p.surface.into()),
            ..container::Style::default()
        });
    row![label, Space::new().width(6), framed(well, edges)]
        .padding(pad)
        .height(Fill)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

fn row_view<'a>(
    r: RowVm,
    p: &'static Palette,
    s: WidgetStyle,
    focused: bool,
    widths: &[f32; 8],
) -> Element<'a, Message> {
    let cells = r.cells.into_iter().map(|(col, cell)| {
        let chars: usize = cell.runs.iter().map(|(t, _)| t.chars().count()).sum();
        let spans: Vec<Span<'static>> = cell
            .runs
            .into_iter()
            .map(|(s, c)| span(s).color(c))
            .collect();
        let label = rich_text(spans)
            .font(s.font())
            .size(s.text_size())
            .wrapping(Wrapping::None);
        let width = column_width(widths, col);
        let cell_w = widths[col as usize];
        let content = match (col, s) {
            // La bandera es la decoración de Qt: a la izquierda de la celda, no centrada.
            (Column::Country, _) => cell_box(
                container(
                    cell.flag
                        .and_then(|code| flag(&code))
                        .map(|h| svg(h).width(FLAG_SIZE.width).height(FLAG_SIZE.height)),
                ),
                width,
                TEXT_MARGIN,
            ),
            (Column::Players, WidgetStyle::Flat) => {
                let bar_w = occupancy_bar_width(cell.occupancy.unwrap_or(0.0), cell_w);
                let bar = container(Space::new())
                    .width(bar_w)
                    .height(Fill)
                    .style(move |_| container::Style {
                        background: Some(style::occupancy_bar(p).into()),
                        border: border::rounded(3),
                        ..container::Style::default()
                    });
                stack![
                    container(bar).width(cell_w).height(Fill),
                    cell_box(label, width, 4.0)
                ]
                .into()
            }
            (Column::Players, _) => container(blocks_cell(
                label.into(),
                chars,
                cell.occupancy.unwrap_or(0.0),
                cell_w,
                p,
                s,
            ))
            .width(cell_w)
            .height(Fill)
            .clip(true)
            .into(),
            // Los delegados de Python (nombre con colores, ping) dibujan en el rectángulo del
            // texto sin el margen que Qt deja a las demás celdas.
            // shortcut: un nombre sin `raw_name` en Qt sí lleva el margen; distinguirlo si algún
            // protocolo deja `raw_name` vacío.
            (Column::Name | Column::Ping, _) => cell_box(label, width, 0.0),
            _ => cell_box(label, width, TEXT_MARGIN),
        };
        match cell.tooltip {
            Some(tip) => tooltip(
                content,
                container(text(tip).font(s.font()).size(s.text_size()))
                    .padding(style::tooltip_padding(s))
                    .style(move |_| style::tooltip(p, s)),
                tooltip::Position::Bottom,
            )
            .into(),
            None => content,
        }
    });
    let selected = r.selected;
    let outline = style::row_outline(p).filter(|_| selected);
    // `framed` suma el grosor del contorno: la fila sigue midiendo `ROW_HEIGHT`.
    let line = |side: Option<(f32, Color)>| side.map_or(0.0, |(w, _)| w);
    let inner_h = ROW_HEIGHT - outline.map_or(0.0, |e| line(e.top) + line(e.bottom));
    let cells = container(row(cells))
        .height(inner_h)
        .style(move |_| style::table_row(p, s, selected, focused));
    let cells = match outline {
        Some(e) => framed(cells, e),
        None => cells.into(),
    };
    mouse_area(cells)
        .on_press(Message::Table(Msg::RowClicked(r.address.clone())))
        .on_right_press(Message::Table(Msg::RightClicked(Some(r.address))))
        .into()
}

/// Margen del texto de una celda de Qt (`PM_FocusFrameHMargin` + 1), medido en las capturas.
const TEXT_MARGIN: f32 = 3.0;

/// Alto del encabezado como `QHeaderView`: la línea de Qt más 4 (medido offscreen en 1.3.0),
/// el relleno y el borde de la sección.
pub fn header_height(theme_id: &str) -> f32 {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let section = style::table_header(p, s, theme_id).1.unwrap_or_default();
    let pad = style::header_padding(s);
    let edge = |side: Option<(f32, Color)>| side.map_or(0.0, |(w, _)| w);
    style::qt_line_height(s) + 4.0 + pad.top + pad.bottom + edge(section.top) + edge(section.bottom)
}

pub fn view<'a>(t: &'a Table, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (header_style, section) = style::table_header(p, s, theme_id);
    let section = section.unwrap_or_default();
    let pad = style::header_padding(s);
    let edge = |side: Option<(f32, Color)>| side.map_or(0.0, |(w, _)| w);
    let header_h = header_height(theme_id);
    let header = row(Column::ALL.map(|col| {
        let arrow_w = sort_indicator(t, col).map_or(0.0, |asc| {
            let margin = if s == WidgetStyle::Flat { 6.0 } else { 4.0 };
            sort_arrow(s, p, asc).1.width + margin
        });
        let fixed = (col != Column::Name).then(|| t.widths[col as usize]);
        let title = match fixed {
            // shortcut: la última (Versión) no se corta: con la letra plana de 13 px «Version»
            // mide ~2 px más que en Qt (12 px) y saldría «Versi…» donde Qt lo muestra entero;
            // quitar al igualar el tamaño de letra de los temas planos con Qt.
            Some(w) if col != Column::Version => {
                let avail = w - 2.0 * pad.left - edge(section.left) - edge(section.right) - arrow_w;
                elide_title(&col.title(), avail, s)
            }
            _ => col.title(),
        };
        // Centrado: la alineación por defecto de `QHeaderView`.
        let title = container(
            text(title)
                .font(style::header_font(s))
                .size(s.text_size())
                .wrapping(Wrapping::None),
        )
        .center_x(Fill)
        .clip(true);
        // `QHeaderView::{up,down}-arrow`: al borde derecho, con su `padding-right`.
        let arrow = sort_indicator(t, col).map(|asc| {
            let (h, size) = sort_arrow(s, p, asc);
            let margin = if s == WidgetStyle::Flat { 6.0 } else { 4.0 };
            row![
                svg(h).width(size.width).height(size.height),
                Space::new().width(margin)
            ]
        });
        // El borde es de cada sección, dentro de su ancho, como en `QHeaderView`.
        let section = framed(
            container(
                row![title]
                    .push(arrow)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(Fill)
            .height(Fill)
            .padding([0.0, pad.left])
            .center_y(Fill)
            .clip(true),
            section,
        );
        // `QHeaderView` sin línea propia: el asa (4 px al borde derecho) solo cambia el cursor.
        let handle = fixed.map(|_| {
            container(
                mouse_area(Space::new().width(4).height(Fill))
                    .interaction(mouse::Interaction::ResizingHorizontally)
                    .on_press(Message::Table(Msg::ResizeStart(col))),
            )
            .align_right(Fill)
        });
        let section: Element<'a, Message> = mouse_area(
            container(stack![section].push(handle).width(Fill).height(Fill))
                .width(column_width(&t.widths, col))
                .height(Fill),
        )
        .on_press(Message::Table(Msg::HeaderClicked(col)))
        .into();
        match col.header_tooltip() {
            Some(tip) => tooltip(
                section,
                container(text(tip).font(s.font()).size(s.text_size()))
                    .padding(style::tooltip_padding(s))
                    .style(move |_| style::tooltip(p, s)),
                tooltip::Position::Bottom,
            )
            .into(),
            None => section,
        }
    }));
    // Las barras clásicas ocupan su lugar (la plana flota): encabezado y filas dejan su
    // hueco siempre, para que las columnas coincidan aunque la barra no se vea.
    let embedded = s != WidgetStyle::Flat;
    let bar_w = style::scrollbar_width(s);
    let gap = Padding {
        right: if embedded { bar_w } else { 0.0 },
        ..Padding::ZERO
    };
    let header = container(header)
        .height(header_h)
        .padding(gap)
        .style(move |_| header_style);
    let n = t.model.order().len();
    let range = visible_range(t.offset_y, t.draw_h(), n);
    let body = Col::new()
        .width(Fill)
        .padding(gap)
        .push(Space::new().height(range.start as f32 * ROW_HEIGHT))
        .extend(
            visible_rows(t, p, s)
                .into_iter()
                .map(|r| row_view(r, p, s, t.focused, &t.widths)),
        )
        .push(Space::new().height((n - range.end) as f32 * ROW_HEIGHT));
    let body = scrollable(body)
        .id(TABLE_ID)
        .direction(Direction::Vertical(style::scrollbar(s)))
        .on_scroll(|v| Message::Table(Msg::Scrolled(v)))
        .style(move |_, status| style::scrollable(p, s, status))
        .height(Fill);
    // Las filas capturan su clic: aquí solo llega el de la zona vacía.
    let body = mouse_area(body)
        .on_press(Message::Table(Msg::BlankClicked))
        .on_right_press(Message::Table(Msg::RightClicked(None)));
    let (frame_style, edges) = style::table_frame(p, s);
    let table = container(Col::new().push(header).push(body))
        .padding(frame_style.border.width) // que el encabezado no tape el borde
        .style(move |_| frame_style);
    match edges {
        Some(e) => framed(table, e),
        None => table.into(),
    }
}
