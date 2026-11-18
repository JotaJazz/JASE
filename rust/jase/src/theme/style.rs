//! Estilos de los widgets por tema y `WidgetStyle` (las reglas de `build_stylesheet` y de
//! `_flat_rules`, `_flavor_rules`, `_win98_rules`, `_luna_rules`, `_vgui_rules` y
//! `_terminal_rules` que tocan ventana, lista, tabla, encabezado, desplazamiento, barra de
//! estado y tooltip). Lo que Iced no dibuja (un color por lado) sale como `Edges`.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use iced::font::Weight;
use iced::gradient::Linear;
use iced::widget::container::Style;
use iced::widget::scrollable::{self, AutoScroll, Rail, Scrollbar, Scroller, Status};
use iced::{Background, Border, Color, Font, Padding, Shadow};

use super::{Edges, OCCUPANCY_ALPHA, Palette, WidgetStyle};

/// Degradado de `a` a `b`, hacia abajo (`vgrad` de Python).
fn vgrad(a: Color, b: Color) -> Background {
    Linear::new(PI).add_stop(0.0, a).add_stop(1.0, b).into()
}

/// Degradado de `a` a `b`, hacia la derecha (`hgrad`).
fn hgrad(a: Color, b: Color) -> Background {
    Linear::new(FRAC_PI_2)
        .add_stop(0.0, a)
        .add_stop(1.0, b)
        .into()
}

fn border(width: f32, color: Color, radius: f32) -> Border {
    Border {
        color,
        width,
        radius: radius.into(),
    }
}

fn base(p: &Palette, bg: Color) -> Style {
    Style {
        background: Some(Background::Color(bg)),
        text_color: Some(p.text),
        ..Style::default()
    }
}

fn colored(bg: Color, text: Color) -> Style {
    Style {
        background: Some(Background::Color(bg)),
        text_color: Some(text),
        ..Style::default()
    }
}

pub fn window(p: &Palette) -> Style {
    base(p, p.bg)
}

/// Borde de la tabla y de la lista de juegos (`QTableView, QListWidget, …`).
fn frame(p: &Palette, s: WidgetStyle) -> (Border, Option<Edges>) {
    match s {
        WidgetStyle::Flat => (border(1.0, p.border, s.radius()), None),
        WidgetStyle::Win98 => (Border::default(), Some(Edges::sunken(p))),
        WidgetStyle::Luna => (border(1.0, p.field_border, 0.0), None),
        WidgetStyle::Vgui => (Border::default(), Some(Edges::vgui(p, true))),
        WidgetStyle::Terminal => (border(1.0, p.border, 0.0), None),
    }
}

/// La lista de juegos; su bisel es el de `table_frame`.
pub fn sidebar(p: &Palette, s: WidgetStyle) -> Style {
    let bg = if s == WidgetStyle::Flat {
        p.sidebar
    } else {
        p.surface
    };
    Style {
        border: frame(p, s).0,
        ..base(p, bg)
    }
}

/// Selección; sin foco, 98 y Luna la pintan con el color de la cara (`QTableView:!focus`).
fn selection(p: &Palette, s: WidgetStyle, focused: bool) -> Style {
    match s {
        WidgetStyle::Win98 if !focused => colored(p.button_top, p.text),
        WidgetStyle::Luna if !focused => colored(p.bg, p.text),
        _ => colored(p.selection, p.selection_text),
    }
}

/// Fondo y texto de la selección (con o sin foco), para elegir el color de cada tramo.
pub fn selection_colors(p: &Palette, s: WidgetStyle, focused: bool) -> (Color, Color) {
    let st = selection(p, s, focused);
    match (st.background, st.text_color) {
        (Some(Background::Color(bg)), Some(text)) => (bg, text),
        _ => (p.selection, p.selection_text),
    }
}

/// Verde: marca de 3 px `accent` a la izquierda del juego elegido (`_flavor_rules`).
pub fn list_item_edges(p: &Palette, theme_id: &str, selected: bool) -> Option<Edges> {
    (selected && theme_id == "green").then(|| Edges {
        left: Some((3.0, p.accent)),
        ..Edges::default()
    })
}

pub fn list_item(p: &Palette, s: WidgetStyle, selected: bool, focused: bool) -> Style {
    let style = if selected {
        selection(p, s, focused)
    } else {
        Style {
            text_color: Some(p.text),
            ..Style::default()
        }
    };
    Style {
        border: border(0.0, Color::TRANSPARENT, s.radius()),
        ..style
    }
}

pub fn table_frame(p: &Palette, s: WidgetStyle) -> (Style, Option<Edges>) {
    let (b, edges) = frame(p, s);
    (
        Style {
            border: b,
            ..base(p, p.surface)
        },
        edges,
    )
}

/// Fondo del encabezado y el borde de cada sección (`QHeaderView::section`).
pub fn table_header(p: &Palette, s: WidgetStyle, theme_id: &str) -> (Style, Option<Edges>) {
    match s {
        WidgetStyle::Flat => {
            // `header_text` es `muted` salvo en los temas con sabor propio.
            let style = colored(p.sidebar, p.header_text);
            match theme_id {
                "synthwave" => (
                    Style {
                        background: Some(vgrad(p.bar_top, p.sidebar)),
                        ..style
                    },
                    Some(Edges::bottom(2.0, p.sunset)),
                ),
                "neon" | "green" => (style, Some(Edges::bottom(1.0, p.accent))),
                "rust" => (style, Some(Edges::bottom(2.0, p.accent))),
                _ => (style, Some(Edges::bottom(1.0, p.divider))),
            }
        }
        WidgetStyle::Win98 => (colored(p.button_top, p.text), Some(Edges::raised(p))),
        WidgetStyle::Luna => (
            colored(p.header_bg, p.text),
            Some(Edges::right_bottom(1.0, 3.0, p.header_line)),
        ),
        WidgetStyle::Vgui => (colored(p.bg, p.text), Some(Edges::vgui(p, false))),
        // `1px dashed`: Iced no tiene trazos, va continua.
        WidgetStyle::Terminal => (colored(p.bg, p.accent), Some(Edges::bottom(1.0, p.border))),
    }
}

/// Letra del encabezado: el estilo plano no pone `font-weight` y Qt deja la sección en
/// negrita; los de época ponen `font-weight: normal`.
pub fn header_font(s: WidgetStyle) -> Font {
    Font {
        weight: if s == WidgetStyle::Flat {
            Weight::Bold
        } else {
            Weight::Normal
        },
        ..s.font()
    }
}

/// `QHeaderView::section { padding }` (vertical, horizontal).
pub fn header_padding(s: WidgetStyle) -> Padding {
    match s {
        WidgetStyle::Flat => [6, 8],
        WidgetStyle::Win98 => [2, 6],
        WidgetStyle::Luna | WidgetStyle::Vgui | WidgetStyle::Terminal => [3, 6],
    }
    .into()
}

/// Barra de ocupación de los temas planos (`occupancy_color`).
pub fn occupancy_bar(p: &Palette) -> Color {
    p.occupancy.scale_alpha(OCCUPANCY_ALPHA)
}

/// Marco de la barra por bloques: línea fina en Luna/Terminal, hundido de 1 px en 98/GoldSrc
/// (`bevel_inner` abajo/derecha, que sobre `surface` no se pierde, como `_paint_blocks`).
pub fn occupancy_frame(p: &Palette, s: WidgetStyle) -> Edges {
    match s {
        WidgetStyle::Luna | WidgetStyle::Terminal => {
            Edges::bevel(1.0, p.progress_border, p.progress_border)
        }
        _ => Edges::bevel(1.0, p.border, p.bevel_inner),
    }
}

/// Relleno de cada bloque: brillo arriba hasta el 45 %.
pub fn occupancy_block(p: &Palette) -> Style {
    Style {
        background: Some(
            Linear::new(PI)
                .add_stop(0.0, p.progress_light)
                .add_stop(0.45, p.progress)
                .add_stop(1.0, p.progress)
                .into(),
        ),
        ..Style::default()
    }
}

pub fn table_row(p: &Palette, s: WidgetStyle, selected: bool, focused: bool) -> Style {
    if selected {
        selection(p, s, focused)
    } else {
        base(p, p.surface)
    }
}

/// Contorno de la fila seleccionada en neón/synthwave (`_outline_rules`: arriba y abajo).
pub fn row_outline(p: &Palette) -> Option<Edges> {
    let line = Some((1.0, p.selection_outline));
    (p.selection_outline != p.selection).then_some(Edges {
        top: line,
        bottom: line,
        ..Edges::default()
    })
}

/// Ancho de la barra de desplazamiento (`QScrollBar { width }`).
pub fn scrollbar_width(s: WidgetStyle) -> f32 {
    match s {
        WidgetStyle::Flat => 10.0,
        WidgetStyle::Win98 | WidgetStyle::Vgui => 16.0,
        WidgetStyle::Luna => 17.0,
        WidgetStyle::Terminal => 14.0,
    }
}

/// Medidas de la barra: la plana, pulgar de 6 px con 2 de margen (`margin: 2px`); la de
/// Luna, pulgar 1 px más adentro por lado (`handle { margin: 1px }`).
pub fn scrollbar(s: WidgetStyle) -> Scrollbar {
    let w = scrollbar_width(s);
    match s {
        WidgetStyle::Flat => Scrollbar::new()
            .width(w - 4.0)
            .scroller_width(w - 4.0)
            .margin(2),
        WidgetStyle::Luna => Scrollbar::new().width(w).scroller_width(w - 2.0),
        _ => Scrollbar::new().width(w).scroller_width(w),
    }
}

/// Riel y pulgar. Sin botones de flecha ni tramas: la trama a cuadros de 98 es `bevel_inner`
/// (su color medio) y la ▒ de Terminal, `border`; el bisel del pulgar, un borde de un color.
pub fn scrollable(p: &Palette, s: WidgetStyle, status: Status) -> scrollable::Style {
    let (hot, disabled) = match status {
        Status::Active {
            is_vertical_scrollbar_disabled,
            ..
        } => (false, is_vertical_scrollbar_disabled),
        Status::Hovered {
            is_vertical_scrollbar_hovered,
            is_vertical_scrollbar_disabled,
            ..
        } => (
            is_vertical_scrollbar_hovered,
            is_vertical_scrollbar_disabled,
        ),
        Status::Dragged {
            is_vertical_scrollbar_dragged,
            is_vertical_scrollbar_disabled,
            ..
        } => (
            is_vertical_scrollbar_dragged,
            is_vertical_scrollbar_disabled,
        ),
    };
    let thumb = |bg: Background, b: Border| Scroller {
        background: bg,
        border: b,
    };
    let (track, scroller) = match s {
        WidgetStyle::Flat => {
            let c = if hot { p.muted } else { p.border };
            (None, thumb(c.into(), border(0.0, c, 3.0)))
        }
        WidgetStyle::Win98 => (
            Some(p.bevel_inner.into()),
            thumb(p.button_top.into(), border(1.0, p.bevel_dark, 0.0)),
        ),
        WidgetStyle::Luna => {
            let bg = if hot {
                hgrad(p.surface, p.scroll_top)
            } else {
                hgrad(p.scroll_top, p.scroll_bottom)
            };
            (
                Some(hgrad(p.scroll_track, p.surface)),
                thumb(bg, border(1.0, p.scroll_border, 3.0)),
            )
        }
        WidgetStyle::Vgui => (
            Some(p.surface.into()),
            thumb(p.bg.into(), border(1.0, p.bevel_dark, 0.0)),
        ),
        WidgetStyle::Terminal => (
            Some(p.border.into()),
            thumb(p.text.into(), border(0.0, p.text, 0.0)),
        ),
    };
    let rail = Rail {
        // Qt esconde la barra si todo cabe; aquí su espacio queda vacío.
        background: if disabled { None } else { track },
        border: Border::default(),
        scroller,
    };
    scrollable::Style {
        container: Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: AutoScroll {
            background: p.surface.into(),
            border: border(1.0, p.border, 0.0),
            shadow: Shadow::default(),
            icon: p.text,
        },
    }
}

/// Barra de estado y la línea de arriba (98, VGUI); Terminal va en video inverso.
pub fn status_bar(p: &Palette, s: WidgetStyle) -> (Style, Option<Edges>) {
    let top = Edges {
        top: Some((1.0, p.bevel_light)),
        ..Edges::default()
    };
    match s {
        WidgetStyle::Win98 | WidgetStyle::Vgui => (window(p), Some(top)),
        WidgetStyle::Terminal => (colored(p.text, p.bg), None),
        WidgetStyle::Flat | WidgetStyle::Luna => (window(p), None),
    }
}

/// Bisel de las etiquetas fijas de la barra de estado (`QStatusBar QLabel`).
pub fn status_label(p: &Palette, s: WidgetStyle) -> Option<Edges> {
    match s {
        WidgetStyle::Win98 => Some(Edges::thin(p, true)),
        WidgetStyle::Vgui => Some(Edges::vgui(p, true)),
        WidgetStyle::Luna => Some(Edges {
            left: Some((1.0, p.disabled_text)),
            ..Edges::default()
        }),
        WidgetStyle::Flat | WidgetStyle::Terminal => None,
    }
}

/// `QStatusBar QLabel { padding }` (horizontal).
pub fn status_label_padding(s: WidgetStyle) -> Padding {
    match s {
        WidgetStyle::Win98 | WidgetStyle::Vgui => [0, 4],
        WidgetStyle::Luna | WidgetStyle::Terminal => [0, 6],
        WidgetStyle::Flat => [0, 0],
    }
    .into()
}

/// `QToolTip { padding }`.
pub fn tooltip_padding(s: WidgetStyle) -> Padding {
    match s {
        WidgetStyle::Flat => [4, 6],
        WidgetStyle::Win98 | WidgetStyle::Luna => [1, 2],
        WidgetStyle::Vgui => [2, 4],
        WidgetStyle::Terminal => [2, 6],
    }
    .into()
}

pub fn tooltip(p: &Palette, s: WidgetStyle) -> Style {
    let (bg, edge) = match s {
        WidgetStyle::Flat => (p.tooltip, p.border),
        WidgetStyle::Win98 | WidgetStyle::Luna => (p.tooltip, p.text),
        WidgetStyle::Vgui => (p.surface, p.bevel_light),
        WidgetStyle::Terminal => return colored(p.text, p.bg),
    };
    Style {
        border: border(1.0, edge, 0.0),
        ..base(p, bg)
    }
}

/// Caja de la casilla (`_indicator_rules` de cada estilo): lado total, fondo y borde, y el
/// bisel que Iced no dibuja. Plana de 14 px; las de época, 9 px de caja + relleno + borde.
pub fn check(p: &Palette, s: WidgetStyle) -> (f32, Style, Option<Edges>) {
    let boxed = |b: Border, bg: Background| Style {
        border: b,
        background: Some(bg),
        ..Style::default()
    };
    match s {
        WidgetStyle::Flat => (
            14.0,
            boxed(border(1.0, p.check_border, 2.0), p.surface.into()),
            None,
        ),
        // Hundido sobre fondo blanco (`_sunken_on_surface`).
        WidgetStyle::Win98 => (
            15.0,
            boxed(Border::default(), p.surface.into()),
            Some(Edges::sunken_on_surface(p)),
        ),
        WidgetStyle::Luna => (
            15.0,
            boxed(
                border(1.0, p.check_border, 0.0),
                Linear::new(3.0 * FRAC_PI_4)
                    .add_stop(0.0, p.check_top)
                    .add_stop(1.0, p.surface)
                    .into(),
            ),
            None,
        ),
        WidgetStyle::Vgui => (
            15.0,
            boxed(Border::default(), p.surface.into()),
            Some(Edges::vgui(p, true)),
        ),
        WidgetStyle::Terminal => (
            15.0,
            boxed(border(1.0, p.text, 0.0), p.surface.into()),
            None,
        ),
    }
}
