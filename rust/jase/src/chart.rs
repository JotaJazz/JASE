//! Gráfico de ocupación por hora de la ficha (`occupancy_chart.py`): 24 barras hechas de
//! `container`, sin `canvas`.

use iced::widget::{Row, Space, container, text};
use iced::{Bottom, Color, Element, Fill, Padding};
use jase_core::i18n::tr;

use crate::app::Message;
use crate::theme::{OCCUPANCY_ALPHA, Palette, palette, widget_style};
use crate::widgets::tip;

pub const CHART_HEIGHT: f32 = 48.0;

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

pub fn view<'a>(chart: &ChartVm, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    if !has_data(&chart.values) {
        let empty = text(tr("Sin historial todavía"))
            .color(p.muted)
            .font(s.font())
            .size(s.text_size());
        return container(empty)
            .center_x(Fill)
            .center_y(CHART_HEIGHT)
            .into();
    }
    // Base a 2 px del fondo y barra máxima 2 px por debajo del borde de arriba, como Qt.
    let heights = bar_heights(&chart.values, chart.max_players, CHART_HEIGHT - 4.0);
    let colors = bar_colors(chart, p);
    let bars = Row::with_children((0..24).map(|hour| {
        // Sin datos o con promedio 0: una línea de 1 px `border`, como en Qt.
        let (height, color) = match heights[hour] {
            h if h > 0.0 => (h, colors[hour]),
            _ => (1.0, p.border),
        };
        let bar = container(Space::new())
            .width(Fill)
            .height(height)
            .style(move |_| container::Style::default().background(color));
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
    tip(bars, tr("Jugadores promedio por hora"), p, s)
}
