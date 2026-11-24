//! Bloques de la ficha a la derecha de los datos: jugadores por hora (`occupancy_chart.py`,
//! 24 barras de `container`, sin `canvas`) y el mapa en la lista cargada.

use std::collections::HashMap;

use iced::border::Radius;
use iced::font::Weight;
use iced::widget::{Column as Col, Row, Space, column, container, row};
use iced::{Bottom, Center, Color, Element, Fill, Font, Length, Padding, Right};
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::models::ServerInfo;

use crate::app::Message;
use crate::card::label;
use crate::theme::{
    Edges, OCCUPANCY_ALPHA, Palette, WidgetStyle, framed, palette, style, widget_style,
};
use crate::widgets::tip;

pub const CHART_HEIGHT: f32 = 48.0;
/// Horas rotuladas bajo las barras: cada una abre un grupo de 6.
const AXIS_HOURS: [usize; 4] = [0, 6, 12, 18];
/// Mapas en la lista del bloque del mapa.
pub const MAP_TOP: usize = 4;
/// Alto de una fila de la lista de mapas.
const MAP_ROW: f32 = 16.0;

/// Lo que dibuja el gráfico: promedio de cada hora local, máximo de jugadores y la hora actual.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartVm {
    pub values: [Option<f64>; 24],
    pub max_players: i64,
    pub current_hour: usize,
}

pub fn has_data(values: &[Option<f64>; 24]) -> bool {
    values.iter().any(Option::is_some)
}

/// La hora con el promedio más alto (la primera si empatan) y ese promedio.
pub fn peak(values: &[Option<f64>; 24]) -> Option<(usize, f64)> {
    values
        .iter()
        .enumerate()
        .filter_map(|(h, v)| v.map(|v| (h, v)))
        .fold(None, |best: Option<(usize, f64)>, (h, v)| match best {
            Some((_, b)) if b >= v => best,
            _ => Some((h, v)),
        })
}

/// Altura de cada barra (0 sin datos), proporcional a promedio / tope, con tope = máx(máximo
/// de jugadores, mayor promedio).
pub fn bar_heights(values: &[Option<f64>; 24], max_players: i64, full: f32) -> [f32; 24] {
    let top = values
        .iter()
        .flatten()
        .fold(max_players as f64, |a, &v| a.max(v));
    if top <= 0.0 {
        return [0.0; 24];
    }
    values.map(|v| v.map_or(0.0, |v| ((v / top).min(1.0) * f64::from(full)) as f32))
}

/// Color de cada barra: la hora actual `accent`, las demás `occupancy` translúcido, las horas
/// sin datos (una línea de 1 px) `border`.
pub fn bar_colors(chart: &ChartVm, p: &Palette) -> [Color; 24] {
    std::array::from_fn(|hour| match chart.values[hour] {
        None => p.border,
        Some(_) if hour == chart.current_hour => p.accent,
        Some(_) => p.occupancy.scale_alpha(OCCUPANCY_ALPHA),
    })
}

/// El mapa del servidor elegido entre los servidores que respondieron de la lista cargada.
#[derive(Debug, Clone, PartialEq)]
pub struct MapStatsVm {
    pub map: String,
    pub servers: usize,
    pub players: i64,
    /// Puesto por jugadores (1 = el más jugado) y cuántos mapas distintos hay.
    pub rank: usize,
    pub total: usize,
    /// (mapa, jugadores, es el elegido): los más jugados; si el elegido no entra, ocupa el
    /// último lugar.
    pub top: Vec<(String, i64, bool)>,
}

/// Agrupa por nombre exacto del mapa; ordena por jugadores, luego servidores, luego nombre.
/// `None` sin mapa o si ningún servidor que respondió lo tiene.
pub fn map_stats(servers: &[ServerInfo], map: &str) -> Option<MapStatsVm> {
    if map.is_empty() {
        return None;
    }
    let mut by_map: HashMap<&str, (usize, i64)> = HashMap::new();
    for s in servers.iter().filter(|s| s.online && !s.map.is_empty()) {
        let e = by_map.entry(s.map.as_str()).or_default();
        e.0 += 1;
        e.1 += i64::from(s.num_players.max(0));
    }
    let mut ranked: Vec<(&str, usize, i64)> =
        by_map.into_iter().map(|(m, (n, p))| (m, n, p)).collect();
    ranked.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)).then(a.0.cmp(b.0)));
    let pos = ranked.iter().position(|r| r.0 == map)?;
    let mut top: Vec<usize> = (0..ranked.len().min(MAP_TOP)).collect();
    if pos >= MAP_TOP {
        top[MAP_TOP - 1] = pos;
    }
    let (_, servers, players) = ranked[pos];
    Some(MapStatsVm {
        map: map.to_owned(),
        servers,
        players,
        rank: pos + 1,
        total: ranked.len(),
        top: top
            .into_iter()
            .map(|i| (ranked[i].0.to_owned(), ranked[i].2, i == pos))
            .collect(),
    })
}

fn bold(font: Font) -> Font {
    Font {
        weight: Weight::Bold,
        ..font
    }
}

/// Recuadro de un bloque: el del mapa sin imagen, con título a la izquierda y dato a la derecha.
fn block<'a>(
    title: Element<'a, Message>,
    aside: Option<Element<'a, Message>>,
    body: Element<'a, Message>,
    p: &'static Palette,
    s: WidgetStyle,
) -> Element<'a, Message> {
    let head = row![container(title).width(Fill).clip(true)]
        .push(aside)
        .spacing(6)
        .align_y(Center);
    let (frame, edges): (_, Option<Edges>) = style::map_frame(p, s);
    let inner = container(column![head, body].spacing(4))
        .padding(6.0 + frame.border.width)
        .width(Fill)
        .height(Fill)
        .style(move |_| frame);
    match edges {
        Some(e) => framed(inner, e),
        None => inner.into(),
    }
}

/// Aviso gris centrado en el cuerpo de un bloque.
fn empty<'a>(msg: String, p: &'static Palette, s: WidgetStyle) -> Element<'a, Message> {
    let msg = label(&msg, p.muted, s.font(), s)
        .wrapping(iced::widget::text::Wrapping::Word)
        .align_x(Center);
    container(msg).center_x(Fill).center_y(Fill).into()
}

/// Bloque de jugadores por hora; `None` = el servidor no guarda historial.
pub fn view<'a>(chart: Option<&ChartVm>, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let font = s.font();
    let title = label(&tr("Jugadores por hora"), p.text, bold(font), s).into();
    let Some(chart) = chart else {
        let body = empty(tr("Historial solo de favoritos y recientes"), p, s);
        return block(title, None, body, p, s);
    };
    if !has_data(&chart.values) {
        return block(title, None, empty(tr("Sin historial todavía"), p, s), p, s);
    }
    let aside = peak(&chart.values).map(|(hour, v)| {
        let n = format!("{v:.0}");
        let text = tr_fmt("Pico {hour} h · {n}", &[("hour", &hour), ("n", &n)]);
        label(&text, p.muted, font, s).into()
    });
    // Base a 2 px del fondo y barra máxima 2 px por debajo de la guía de arriba, como Qt.
    let heights = bar_heights(&chart.values, chart.max_players, CHART_HEIGHT - 4.0);
    let colors = bar_colors(chart, p);
    let radius = if s == WidgetStyle::Flat { 2.0 } else { 0.0 };
    let bars = Row::with_children((0..24).map(|hour| {
        // Sin datos o con promedio 0: una línea de 1 px `border`, como en Qt.
        let (height, color, r) = match heights[hour] {
            h if h > 0.0 => (h, colors[hour], radius),
            _ => (1.0, p.border, 0.0),
        };
        let bar = container(Space::new())
            .width(Fill)
            .height(height)
            .style(move |_| container::Style {
                background: Some(color.into()),
                border: iced::Border {
                    radius: Radius::default().top(r),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
        container(bar)
            .padding(Padding {
                top: 0.0,
                right: 1.0,
                bottom: 2.0,
                left: 1.0,
            })
            .width(Fill)
            .height(CHART_HEIGHT)
            .align_y(Bottom)
            .into()
    }));
    // Guía de 1 px en el tope (máximo de jugadores o mayor promedio).
    let guide = container(Space::new())
        .width(Fill)
        .height(1)
        .style(move |_| container::Style::default().background(p.border));
    let axis = Row::with_children(AXIS_HOURS.map(|h| {
        container(label(&h.to_string(), p.muted, font, s))
            .width(Length::FillPortion(6))
            .padding([0, 1])
            .into()
    }));
    let plot = tip(
        column![guide, bars],
        tr("Jugadores promedio por hora"),
        p,
        s,
    );
    block(title, aside, column![plot, axis].into(), p, s)
}

/// Bloque del mapa en la lista cargada; `None` = sin mapa o sin respuesta.
pub fn map_view<'a>(stats: Option<&MapStatsVm>, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let font = s.font();
    let Some(m) = stats else {
        let title = label(&tr("Mapa"), p.text, bold(font), s).into();
        return block(title, None, empty(tr("Sin datos del mapa"), p, s), p, s);
    };
    let title = label(&m.map, p.text, bold(font), s).into();
    let rank = tr_fmt(
        "#{rank} de {total}",
        &[("rank", &m.rank), ("total", &m.total)],
    );
    let aside = tip(
        label(&rank, p.muted, font, s),
        tr("Puesto del mapa por jugadores en la lista cargada"),
        p,
        s,
    );
    let summary = tr_n("{n} servidor", "{n} servidores", m.servers as i64, &[])
        + " · "
        + &tr_n("{n} jugador", "{n} jugadores", m.players, &[]);
    let top = m.top.iter().map(|r| r.1).max().unwrap_or(0).max(1);
    let rows = m.top.iter().map(|(name, players, current)| {
        let (color, f) = if *current {
            (p.accent, bold(font))
        } else {
            (p.text, font)
        };
        let bar_color = if *current {
            p.accent
        } else {
            p.occupancy.scale_alpha(OCCUPANCY_ALPHA)
        };
        // Porciones de 100: la barra y el resto del ancho.
        let fill = ((*players * 100 + top / 2) / top) as u16;
        let bar = container(Space::new())
            .width(Length::FillPortion(fill))
            .height(8)
            .style(move |_| container::Style::default().background(bar_color));
        let track = row![]
            .push((fill > 0).then_some(bar))
            .push((fill < 100).then(|| Space::new().width(Length::FillPortion(100 - fill))))
            .width(Length::FillPortion(4))
            .align_y(Center);
        row![
            container(label(name, color, f, s))
                .width(Length::FillPortion(5))
                .clip(true),
            track,
            container(label(&players.to_string(), color, f, s))
                .width(32)
                .align_x(Right),
        ]
        .spacing(6)
        .height(MAP_ROW)
        .align_y(Center)
        .into()
    });
    let body = Col::new()
        .push(label(&summary, p.muted, font, s))
        .extend(rows)
        .spacing(1);
    block(title, Some(aside), body.into(), p, s)
}
