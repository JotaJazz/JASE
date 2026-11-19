//! Tablas de jugadores y de reglas del servidor elegido (`_show_details`): pocas decenas de
//! filas, sin virtualizar. Solo la de jugadores se ordena (`setSortingEnabled`).

use iced::font::Weight;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::scrollable::Direction;
use iced::widget::text::{Span, Wrapping};
use iced::widget::{
    Column as Col, Space, container, mouse_area, responsive, rich_text, row, scrollable, span, svg,
    text,
};
use iced::{Center, Color, Element, Fill, Font, Length};
use jase_core::i18n::tr;
use jase_core::models::{Player, ServerInfo};
use jase_core::protocols::quake3::strip_colors;

use crate::actions::{self, Menu};
use crate::app::{App, FriendsMsg, Message};
use crate::card::{self, one_line};
use crate::colored::color_runs;
use crate::icons::sort_arrow;
use crate::menu::{self, Placement};
use crate::table::{ROW_HEIGHT, bar_gap, header_height};
use crate::theme::{Palette, WidgetStyle, framed, palette, style, widget_style};
use crate::widgets::tip;
use crate::window_state::{DetailPane, SPLITTER_WIDTH};

/// Ancho de las columnas que no estiran (`sectionSize` medido offscreen).
pub(crate) const COLUMN_WIDTH: f32 = 100.0;
/// Margen del texto de una celda de Qt (el de la tabla de servidores).
pub(crate) const TEXT_MARGIN: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerCol {
    Name,
    Score,
    Ping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSort {
    pub col: PlayerCol,
    pub ascending: bool,
}

/// Qt arranca con *Nombre* descendente (con el indicador visible).
impl Default for PlayerSort {
    fn default() -> Self {
        PlayerSort {
            col: PlayerCol::Name,
            ascending: false,
        }
    }
}

/// Clic en un título: la misma columna invierte; otra, ascendente.
pub fn click(sort: PlayerSort, col: PlayerCol) -> PlayerSort {
    PlayerSort {
        col,
        ascending: sort.col != col || !sort.ascending,
    }
}

/// Jugadores en el orden de la vista: *Nombre* por unidades UTF-16 sensible a mayúsculas
/// (`QString::compare`), *Puntos* y *Ping* como enteros; los empates quedan en el orden del
/// servidor.
pub fn players_rows(info: &ServerInfo, sort: PlayerSort) -> Vec<&Player> {
    let mut rows: Vec<&Player> = info.players.iter().collect();
    rows.sort_by(|a, b| {
        let o = match sort.col {
            PlayerCol::Name => a.name.encode_utf16().cmp(b.name.encode_utf16()),
            PlayerCol::Score => a.score.cmp(&b.score),
            PlayerCol::Ping => a.ping.cmp(&b.ping),
        };
        if sort.ascending { o } else { o.reverse() }
    });
    rows
}

/// Reglas ordenadas por clave (`sorted(rules.items())`), con el valor sin códigos de color.
pub fn rules_rows(info: &ServerInfo) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = info
        .rules
        .iter()
        .map(|(k, v)| (k.clone(), strip_colors(v)))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows
}

/// Título de una columna: ancho, y si ordena, su mensaje y el indicador (`Some(ascendente)`).
pub(crate) struct Head {
    pub title: String,
    pub width: Length,
    pub sort: Option<(Message, Option<bool>)>,
    /// Margen del texto de sus celdas.
    pub margin: f32,
}

/// La fuente del nombre de un jugador: en negrita si es amigo (`_friend_font`).
pub fn player_font(app: &App, name: &str) -> Font {
    let font = widget_style(app.theme_id).font();
    if app.is_friend(name) {
        Font {
            weight: Weight::Bold,
            ..font
        }
    } else {
        font
    }
}

fn plain_cell<'a>(s: &str, p: &Palette, st: WidgetStyle) -> Element<'a, Message> {
    font_cell(s, p, st, st.font())
}

fn font_cell<'a>(s: &str, p: &Palette, st: WidgetStyle, font: Font) -> Element<'a, Message> {
    text(one_line(s))
        .color(p.text)
        .font(font)
        .size(st.text_size())
        .wrapping(Wrapping::None)
        .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    PaneGrid::new(&app.details, |_, pane, _| {
        pane_grid::Content::new(match pane {
            DetailPane::Players => players(app),
            DetailPane::Rules => rules(app),
        })
    })
    .spacing(SPLITTER_WIDTH)
    .on_resize(0, Message::DetailsResized)
    .style(move |_| style::splitter())
    .into()
}

/// La tabla según el alto que le toca: con él se sabe si las filas piden barra.
fn sized<'a>(table: impl Fn(f32) -> Element<'a, Message> + 'a) -> Element<'a, Message> {
    responsive(move |size| table(size.height)).into()
}

/// Con el menú del clic derecho en un jugador (`_on_players_context_menu`).
fn players(app: &App) -> Element<'_, Message> {
    menu::popup(
        sized(move |h| players_table(app, h)),
        actions::open_entries(app, Menu::Players),
        Placement::Cursor,
        Message::Actions(actions::Msg::Close),
        app.theme_id,
    )
}

fn players_table(app: &App, height: f32) -> Element<'_, Message> {
    let id = app.theme_id;
    let (p, s) = (palette(id), widget_style(id));
    let sort = app.player_sort;
    let head = |title: String, col: PlayerCol, width: Length| Head {
        title,
        width,
        sort: Some((
            Message::PlayerSort(col),
            (sort.col == col).then_some(sort.ascending),
        )),
        // El delegado de colores del nombre dibuja sin margen (como *Servidor*).
        margin: if col == PlayerCol::Name {
            0.0
        } else {
            TEXT_MARGIN
        },
    };
    let heads = vec![
        head(tr("Nombre"), PlayerCol::Name, Fill),
        head(tr("Puntos"), PlayerCol::Score, COLUMN_WIDTH.into()),
        head(tr("Ping"), PlayerCol::Ping, COLUMN_WIDTH.into()),
    ];
    let shown = card::server(app).map_or_else(Vec::new, |info| players_rows(info, sort));
    let right = shown
        .iter()
        .map(|pl| Some(Message::Friends(FriendsMsg::PlayerMenu(pl.name.clone()))))
        .collect();
    let rows = shown
        .into_iter()
        .map(|pl| {
            let font = player_font(app, &pl.name);
            let name: Element<'_, Message> = if pl.raw_name.is_empty() {
                font_cell(&pl.name, p, s, font)
            } else {
                let spans: Vec<Span<'static>> = color_runs(&pl.raw_name, p)
                    .into_iter()
                    .map(|(t, c)| span(one_line(&t)).color(c.unwrap_or(p.text)))
                    .collect();
                rich_text(spans)
                    .font(font)
                    .size(s.text_size())
                    .wrapping(Wrapping::None)
                    .into()
            };
            vec![
                name,
                plain_cell(&pl.score.to_string(), p, s),
                plain_cell(&pl.ping.to_string(), p, s),
            ]
        })
        .collect();
    detail_table(heads, rows, id, height, None, None, right)
}

fn rules(app: &App) -> Element<'_, Message> {
    sized(move |h| rules_table(app, h))
}

fn rules_table(app: &App, height: f32) -> Element<'_, Message> {
    let id = app.theme_id;
    let (p, s) = (palette(id), widget_style(id));
    let heads = vec![
        Head {
            title: tr("Variable"),
            width: Fill,
            sort: None,
            margin: TEXT_MARGIN,
        },
        Head {
            title: tr("Valor"),
            width: COLUMN_WIDTH.into(),
            sort: None,
            margin: TEXT_MARGIN,
        },
    ];
    let rows = card::server(app).map_or_else(Vec::new, |info| {
        rules_rows(info)
            .into_iter()
            .map(|(k, v)| {
                let value = plain_cell(&v, p, s);
                let value = if v.is_empty() {
                    value
                } else {
                    tip(value, v, p, s)
                };
                vec![plain_cell(&k, p, s), value]
            })
            .collect()
    });
    detail_table(heads, rows, id, height, None, None, Vec::new())
}

/// Encabezado fijo (como el de la tabla de servidores) y filas de 28 px sin cuadrícula; con
/// `on_row`, cada fila da (clic, doble clic) y `selected` se pinta elegida; `right[i]`, el
/// clic derecho en la fila `i`.
pub(crate) fn detail_table<'a>(
    heads: Vec<Head>,
    rows: Vec<Vec<Element<'a, Message>>>,
    theme_id: &'static str,
    height: f32,
    selected: Option<usize>,
    on_row: Option<fn(usize) -> (Message, Message)>,
    right: Vec<Option<Message>>,
) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (frame_style, edges) = style::table_frame(p, s);
    let frame = |side: Option<(f32, Color)>| side.map_or(0.0, |(w, _)| w);
    let body_h = height
        - header_height(theme_id)
        - 2.0 * frame_style.border.width
        - edges.map_or(0.0, |e| frame(e.top) + frame(e.bottom));
    let gap = bar_gap(s, rows.len(), body_h);
    let (header_style, section) = style::table_header(p, s, theme_id);
    let section = section.unwrap_or_default();
    let pad = style::header_padding(s);
    let widths: Vec<(Length, f32)> = heads.iter().map(|h| (h.width, h.margin)).collect();
    let header = row(heads.into_iter().map(|h| {
        let title = container(
            // Normal: Qt solo los pone en negrita con una fila elegida y aquí no se elige.
            text(h.title)
                .font(s.font())
                .size(s.text_size())
                .wrapping(Wrapping::None),
        )
        .center_x(Fill)
        .clip(true);
        let arrow = h.sort.as_ref().and_then(|(_, asc)| *asc).map(|asc| {
            let (handle, size) = sort_arrow(s, p, asc);
            let margin = if s == WidgetStyle::Flat { 6.0 } else { 4.0 };
            row![
                svg(handle).width(size.width).height(size.height),
                Space::new().width(margin)
            ]
        });
        let cell = framed(
            container(row![title].push(arrow).align_y(Center))
                .width(Fill)
                .height(Fill)
                .padding([0.0, pad.left])
                .center_y(Fill)
                .clip(true),
            section,
        );
        let cell = container(cell).width(h.width).height(Fill);
        match h.sort {
            Some((msg, _)) => mouse_area(cell).on_press(msg).into(),
            None => Element::from(cell),
        }
    }));
    let header = container(header)
        .height(header_height(theme_id))
        .padding(gap)
        .style(move |_| header_style);
    let body = Col::with_children(rows.into_iter().enumerate().map(|(i, cells)| {
        let cells = cells.into_iter().zip(&widths).map(|(c, &(w, margin))| {
            container(c)
                .width(w)
                .height(Fill)
                .padding([0.0, margin])
                .center_y(Fill)
                .clip(true)
                .into()
        });
        let chosen = selected == Some(i);
        let line = container(row(cells))
            .height(ROW_HEIGHT)
            .style(move |_| style::table_row(p, s, chosen, false));
        let right = right.get(i).cloned().flatten();
        if on_row.is_none() && right.is_none() {
            return line.into();
        }
        let mut area = mouse_area(line);
        if let Some((press, double)) = on_row.map(|f| f(i)) {
            area = area.on_press(press).on_double_click(double);
        }
        if let Some(m) = right {
            area = area.on_right_press(m);
        }
        area.into()
    }))
    .width(Fill)
    .padding(gap);
    let body = scrollable(body)
        .direction(Direction::Vertical(style::scrollbar(s)))
        .style(move |_, status| style::scrollable(p, s, status))
        .height(Fill);
    let table = container(Col::new().push(header).push(body))
        .padding(frame_style.border.width)
        .width(Fill)
        .height(Fill)
        .style(move |_| frame_style);
    match edges {
        Some(e) => framed(table, e),
        None => table.into(),
    }
}
