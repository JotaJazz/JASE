//! Controles propios dibujados en los 5 estilos (lo que el `checkbox` de Iced no deja pintar
//! como Qt): por ahora, la casilla.

use std::sync::LazyLock;

use iced::widget::svg::Handle;
use iced::widget::{Space, container, mouse_area, row, svg};
use iced::{Center, Element};

use crate::theme::{Palette, WidgetStyle, framed, hex, style};

macro_rules! asset {
    ($path:expr) => {
        include_bytes!(concat!("../../../src/jase/assets/", $path)) as &[u8]
    };
}

/// ✓ de 7×7 (`_tinted_check`) y X de 7×7 (`_tinted_x`) en `color`.
const CHECK_PATH: &str = "M6 0h1v3H6v1H5v1H4v1H3v1H2V6H1V5H0V2h1v1h1v1h1V3h1V2h1V1h1z";
const X_PATH: &str = "M0 0h2v1h1v1h1V1h1V0h2v2H6v1H5v1h1v1h1v2H5V6H4V5H3v1H2v1H0V5h1V4h1V3H1V2H0z";

fn tinted_mark(path: &str, color: iced::Color) -> Handle {
    Handle::from_memory(
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"7\" height=\"7\" \
             viewBox=\"0 0 7 7\" shape-rendering=\"crispEdges\"><path d=\"{path}\" \
             fill=\"{}\"/></svg>",
            hex(color)
        )
        .into_bytes(),
    )
}

/// Marca de la casilla marcada en el estilo.
fn mark<'a, M: 'a>(p: &'static Palette, s: WidgetStyle, inner: f32) -> Element<'a, M> {
    static WIN98: LazyLock<Handle> =
        LazyLock::new(|| Handle::from_memory(asset!("win98/check.svg")));
    static LUNA: LazyLock<Handle> = LazyLock::new(|| Handle::from_memory(asset!("luna/check.svg")));
    let handle = match s {
        // Cuadro `accent` con 2 px de margen dentro del borde.
        WidgetStyle::Flat => {
            return container(Space::new())
                .width(inner - 4.0)
                .height(inner - 4.0)
                .style(move |_| container::Style {
                    background: Some(p.accent.into()),
                    ..container::Style::default()
                })
                .into();
        }
        WidgetStyle::Win98 => WIN98.clone(),
        WidgetStyle::Luna => LUNA.clone(),
        WidgetStyle::Vgui => tinted_mark(CHECK_PATH, p.text),
        WidgetStyle::Terminal => tinted_mark(X_PATH, p.text),
    };
    svg(handle).width(7).height(7).into()
}

/// Casilla con su etiqueta; un clic en cualquiera de las dos manda `on_toggle`.
pub fn check<'a, M: Clone + 'a>(
    checked: bool,
    label: Option<Element<'a, M>>,
    on_toggle: Option<M>,
    p: &'static Palette,
    s: WidgetStyle,
) -> Element<'a, M> {
    let (side, box_style, edges) = style::check(p, s);
    let bevel = edges.map_or(0.0, |e| e.left.map_or(0.0, |(w, _)| w));
    let inner = side - 2.0 * bevel;
    // Con borde de Iced (plana, Luna, Terminal), la marca va dentro de ese borde.
    let border_w = box_style.border.width;
    let content: Option<Element<'a, M>> = checked.then(|| mark(p, s, inner - 2.0 * border_w));
    let boxed = container(content).center(inner).style(move |_| box_style);
    let boxed: Element<'a, M> = match edges {
        Some(e) => framed(boxed, e),
        None => boxed.into(),
    };
    let content: Element<'a, M> = match label {
        Some(l) => row![boxed, l].spacing(5).align_y(Center).into(),
        None => boxed,
    };
    match on_toggle {
        Some(m) => mouse_area(content).on_press(m).into(),
        None => content,
    }
}
