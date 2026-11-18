//! Estilos de los widgets por tema y `WidgetStyle` (las reglas de `build_stylesheet` y de
//! `_flat_rules`, `_flavor_rules`, `_win98_rules`, `_luna_rules`, `_vgui_rules` y
//! `_terminal_rules` que tocan ventana, lista, tabla, encabezado, desplazamiento, barra de
//! estado y tooltip). Lo que Iced no dibuja (un color por lado) sale como `Edges`.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use iced::font::Weight;
use iced::gradient::Linear;
use iced::widget::container::Style;
use iced::widget::overlay::menu;
use iced::widget::scrollable::{self, AutoScroll, Rail, Scrollbar, Scroller, Status};
use iced::widget::{pick_list as pick, text_input as input};
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

/// Alto de la barra de estado (medido offscreen en 1.3.0): Fusion le da 27 px; las hojas de
/// época, 22.
pub fn status_bar_height(s: WidgetStyle) -> f32 {
    if s == WidgetStyle::Flat { 27.0 } else { 22.0 }
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
/// bisel que Iced no dibuja. Plana de 16 px; las de época, 9 px de caja + relleno + borde.
/// Apagada, la de `::indicator:disabled`.
pub fn check(p: &Palette, s: WidgetStyle, enabled: bool) -> (f32, Style, Option<Edges>) {
    let (side, mut look, edges) = check_enabled(p, s);
    if !enabled {
        match s {
            WidgetStyle::Flat => look.border.color = p.muted,
            WidgetStyle::Win98 | WidgetStyle::Vgui => look.background = Some(p.bg.into()),
            WidgetStyle::Luna => {
                look.border.color = p.disabled_border;
                look.background = Some(p.disabled_bg.into());
            }
            WidgetStyle::Terminal => look.border.color = p.border,
        }
    }
    (side, look, edges)
}

fn check_enabled(p: &Palette, s: WidgetStyle) -> (f32, Style, Option<Edges>) {
    let boxed = |b: Border, bg: Background| Style {
        border: b,
        background: Some(bg),
        ..Style::default()
    };
    match s {
        WidgetStyle::Flat => (
            16.0,
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

/// Campo de texto y combo (`QLineEdit, QComboBox`): fondo, borde de Iced, bisel que Iced no
/// dibuja y relleno de Qt (vertical, horizontal). `focused` = `:focus` (el combo, abierto).
pub fn field(
    p: &Palette,
    s: WidgetStyle,
    theme_id: &str,
    focused: bool,
) -> (Background, Border, Option<Edges>, Padding) {
    let pad: Padding = if s == WidgetStyle::Flat {
        [5, 8]
    } else {
        [2, 4]
    }
    .into();
    // Neón ilumina el campo con foco.
    let bg = if focused && theme_id == "neon" {
        p.selection
    } else {
        p.surface
    };
    let (b, edges) = match s {
        WidgetStyle::Flat => {
            let c = if focused { p.accent } else { p.border };
            (border(1.0, c, s.radius()), None)
        }
        // El foco de 98 repite los colores del hundido.
        WidgetStyle::Win98 => (Border::default(), Some(Edges::sunken(p))),
        WidgetStyle::Luna => (border(1.0, p.field_border, 0.0), None),
        WidgetStyle::Vgui if focused => (
            Border::default(),
            Some(Edges {
                top: Some((1.0, p.bevel_dark)),
                left: Some((1.0, p.bevel_dark)),
                right: Some((1.0, p.accent)),
                bottom: Some((1.0, p.accent)),
            }),
        ),
        WidgetStyle::Vgui => (Border::default(), Some(Edges::vgui(p, true))),
        WidgetStyle::Terminal => {
            let c = if focused { p.text } else { p.border };
            (border(1.0, c, 0.0), None)
        }
    };
    (bg.into(), b, edges, pad)
}

pub fn text_input(
    p: &Palette,
    s: WidgetStyle,
    theme_id: &str,
    status: input::Status,
) -> input::Style {
    let focused = matches!(status, input::Status::Focused { .. });
    let (background, border, _, _) = field(p, s, theme_id, focused);
    input::Style {
        background,
        border,
        icon: p.text,
        placeholder: p.muted,
        value: p.text,
        selection: p.selection,
    }
}

pub fn pick_list(p: &Palette, s: WidgetStyle, theme_id: &str, status: pick::Status) -> pick::Style {
    let open = matches!(status, pick::Status::Opened { .. });
    let (background, border, _, _) = field(p, s, theme_id, open);
    pick::Style {
        text_color: p.text,
        placeholder_color: p.muted,
        handle_color: p.arrow,
        background,
        border,
    }
}

/// `QComboBox:disabled`: fondo y borde del combo apagado (el texto, `disabled_text`).
pub fn combo_disabled(p: &Palette, s: WidgetStyle, theme_id: &str) -> Style {
    let (background, mut b, _, _) = field(p, s, theme_id, false);
    let background = match s {
        WidgetStyle::Win98 | WidgetStyle::Vgui => p.bg.into(),
        WidgetStyle::Luna => {
            b.color = p.disabled_border;
            p.disabled_bg.into()
        }
        WidgetStyle::Flat | WidgetStyle::Terminal => background,
    };
    Style {
        background: Some(background),
        border: b,
        text_color: Some(disabled_text(p, s)),
        ..Style::default()
    }
}

/// La lista que abre el combo (`QComboBox QAbstractItemView`).
pub fn menu(p: &Palette, s: WidgetStyle) -> menu::Style {
    let edge = match s {
        WidgetStyle::Flat => p.border,
        WidgetStyle::Vgui => p.bevel_light,
        WidgetStyle::Win98 | WidgetStyle::Luna | WidgetStyle::Terminal => p.text,
    };
    menu::Style {
        background: p.surface.into(),
        border: border(1.0, edge, 0.0),
        text_color: p.text,
        selected_text_color: p.selection_text,
        selected_background: p.selection.into(),
        shadow: Shadow::default(),
    }
}

/// Botón de la flecha del combo (`QComboBox::drop-down`): ancho, y en 98/VGUI/Luna su cara y
/// bisel; en los planos y Terminal solo hay flecha.
pub fn combo_button(p: &Palette, s: WidgetStyle) -> (f32, Option<(Style, Option<Edges>)>) {
    let face = |bg: Color| Style {
        background: Some(bg.into()),
        ..Style::default()
    };
    match s {
        WidgetStyle::Flat => (20.0, None),
        WidgetStyle::Terminal => (18.0, None),
        WidgetStyle::Win98 => (16.0, Some((face(p.button_top), Some(Edges::raised(p))))),
        WidgetStyle::Vgui => (16.0, Some((face(p.bg), Some(Edges::vgui(p, false))))),
        WidgetStyle::Luna => (
            15.0,
            Some((
                Style {
                    background: Some(vgrad(p.scroll_top, p.scroll_bottom)),
                    border: border(1.0, p.scroll_border, 2.0),
                    ..Style::default()
                },
                None,
            )),
        ),
    }
}

/// Tarjeta de un diálogo dibujado (`QDialog`): fondo de la ventana con el marco del estilo
/// (en relieve en 98 y VGUI).
pub fn dialog(p: &Palette, s: WidgetStyle) -> (Style, Option<Edges>) {
    let (b, edges) = match s {
        WidgetStyle::Win98 => (Border::default(), Some(Edges::raised(p))),
        WidgetStyle::Vgui => (Border::default(), Some(Edges::vgui(p, false))),
        _ => (frame(p, s).0, None),
    };
    (
        Style {
            border: b,
            ..base(p, p.bg)
        },
        edges,
    )
}

/// Botón (`QPushButton`): la cara del botón del combo en 98, VGUI y Luna; en los planos y
/// Terminal, la del campo.
pub fn button(p: &Palette, s: WidgetStyle, theme_id: &str) -> (Style, Option<Edges>) {
    match combo_button(p, s).1 {
        Some((face, edges)) => (
            Style {
                text_color: Some(p.text),
                ..face
            },
            edges,
        ),
        None => {
            let (bg, b, _, _) = field(p, s, theme_id, false);
            (
                Style {
                    background: Some(bg),
                    border: b,
                    ..base(p, p.surface)
                },
                None,
            )
        }
    }
}

/// `QPushButton` (*Copiar*, la estrella, los de los diálogos): en Luna, el degradado blanco
/// con borde azul oscuro de 3 px de radio; en los demás, como `button`.
pub fn push_button(p: &Palette, s: WidgetStyle, theme_id: &str) -> (Style, Option<Edges>) {
    if s == WidgetStyle::Luna {
        return (
            Style {
                background: Some(luna_button(p)),
                border: border(1.0, p.button_border, 3.0),
                ..base(p, p.surface)
            },
            None,
        );
    }
    button(p, s, theme_id)
}

/// Texto de los controles apagados (`QPushButton:disabled`, `QCheckBox:disabled`,
/// `QComboBox:disabled` de cada estilo).
pub fn disabled_text(p: &Palette, s: WidgetStyle) -> Color {
    match s {
        WidgetStyle::Flat | WidgetStyle::Vgui => p.muted,
        WidgetStyle::Win98 | WidgetStyle::Terminal => p.border,
        WidgetStyle::Luna => p.disabled_text,
    }
}

/// `QPushButton:disabled`: la cara de `push_button` con el texto apagado y, según el estilo,
/// otro fondo o borde (98 solo cambia el texto).
pub fn push_button_disabled(p: &Palette, s: WidgetStyle, theme_id: &str) -> (Style, Option<Edges>) {
    let (mut face, mut edges) = push_button(p, s, theme_id);
    face.text_color = Some(disabled_text(p, s));
    match s {
        WidgetStyle::Flat => {
            face.background = Some(p.bg.into());
            face.border.color = p.divider;
        }
        WidgetStyle::Terminal => {
            face.background = Some(p.bg.into());
            face.border.color = p.border;
        }
        WidgetStyle::Vgui => edges = Some(Edges::bevel(1.0, p.bevel_dark, p.bevel_dark)),
        WidgetStyle::Luna => {
            face.background = Some(p.disabled_bg.into());
            face.border.color = p.disabled_border;
        }
        WidgetStyle::Win98 => {}
    }
    (face, edges)
}

/// `QFontMetrics::height()` de la letra de cada estilo en 1.3.0 (medido offscreen): 14 px con
/// 9 pt (planos y Terminal), 13 con 8 pt. Iced mide la línea más alta (letra × 1,3).
pub fn qt_line_height(s: WidgetStyle) -> f32 {
    match s {
        WidgetStyle::Flat | WidgetStyle::Terminal => 14.0,
        _ => 13.0,
    }
}

/// Barra de menús (`QMenuBar`): fondo, línea de abajo (98, VGUI y Terminal), alto sin esa
/// línea (medido en las capturas de 1.3.0), relleno horizontal de cada título, margen
/// izquierdo y separación entre títulos.
pub fn menu_bar(p: &Palette, s: WidgetStyle, theme_id: &str) -> (Style, Option<Edges>, f32, f32) {
    let bg = match theme_id {
        "synthwave" | "brown" => p.bar_top,
        _ => p.bg,
    };
    let (line, height) = match s {
        WidgetStyle::Flat => (None, 19.0),
        WidgetStyle::Win98 => (Some(p.bevel_light), 18.0),
        WidgetStyle::Luna => (None, 17.0),
        WidgetStyle::Vgui => (Some(p.bevel_dark), 16.0),
        WidgetStyle::Terminal => (Some(p.border), 17.0),
    };
    // Qt deja 6 px antes del primer título en todos los estilos (medido offscreen).
    (
        base(p, bg),
        line.map(|c| Edges::bottom(1.0, c)),
        height,
        6.0,
    )
}

/// Relleno horizontal de un título y separación entre títulos (`QMenuBar::item`).
pub fn menu_bar_item_spacing(s: WidgetStyle) -> (f32, f32) {
    match s {
        WidgetStyle::Flat => (4.0, 6.0),  // Fusion, sin hoja propia
        WidgetStyle::Win98 => (8.0, 6.0), // 7 px más el borde transparente
        WidgetStyle::Terminal => (8.0, 6.0),
        WidgetStyle::Luna | WidgetStyle::Vgui => (7.0, 6.0),
    }
}

/// Título de la barra resaltado (`:selected` al pasar, `:pressed` con su menú abierto): 98
/// pinta la cara con un bisel fino (hacia afuera o hundido); Terminal, video inverso.
pub fn menu_bar_item(p: &Palette, s: WidgetStyle, open: bool) -> (Style, Option<Edges>) {
    match s {
        WidgetStyle::Win98 => (colored(p.button_top, p.text), Some(Edges::thin(p, open))),
        WidgetStyle::Terminal => (colored(p.text, p.bg), None),
        _ => (colored(p.selection, p.selection_text), None),
    }
}

/// Lista de un menú (`QMenu`): fondo, bordes de afuera hacia adentro (con su margen desde el
/// borde exterior) y lo que ocupan borde más `padding` por lado. El doble de Terminal son dos
/// líneas de 1 px.
pub fn menu_frame(p: &Palette, s: WidgetStyle) -> (Color, Vec<(f32, Edges)>, f32) {
    let line = |c| Edges::bevel(1.0, c, c);
    match s {
        WidgetStyle::Flat => (p.surface, vec![(0.0, line(p.border))], 1.0),
        WidgetStyle::Win98 => (p.button_top, vec![(0.0, Edges::raised(p))], 3.0),
        WidgetStyle::Vgui => (p.bg, vec![(0.0, Edges::vgui(p, false))], 2.0),
        WidgetStyle::Luna => (p.surface, vec![(0.0, line(p.disabled_text))], 3.0),
        WidgetStyle::Terminal => (p.bg, vec![(0.0, line(p.text)), (2.0, line(p.text))], 4.0),
    }
}

/// Relleno de cada ítem (`QMenu::item { padding }`): vertical, izquierda y derecha.
pub fn menu_item_padding(s: WidgetStyle) -> (f32, f32, f32) {
    match s {
        WidgetStyle::Flat => (5.0, 24.0, 24.0),
        _ => (3.0, 22.0, 20.0),
    }
}

/// Fondo (solo resaltado) y texto de un ítem; el apagado no se resalta.
pub fn menu_item(
    p: &Palette,
    s: WidgetStyle,
    selected: bool,
    enabled: bool,
) -> (Option<Color>, Color) {
    if !enabled {
        let c = match s {
            WidgetStyle::Win98 | WidgetStyle::Terminal => p.border,
            WidgetStyle::Luna => p.disabled_text,
            WidgetStyle::Flat | WidgetStyle::Vgui => p.muted,
        };
        return (None, c);
    }
    match (selected, s) {
        (false, _) => (None, p.text),
        (true, WidgetStyle::Terminal) => (Some(p.text), p.bg),
        (true, _) => (Some(p.selection), p.selection_text),
    }
}

/// Separador (`QMenu::separator`): margen vertical y horizontal, y sus líneas de 1 px de
/// arriba abajo (98 y VGUI, hundido; Terminal, la de trazos, continua).
pub fn menu_separator(p: &Palette, s: WidgetStyle) -> (f32, f32, Vec<Color>) {
    match s {
        WidgetStyle::Flat => (4.0, 8.0, vec![p.divider]),
        WidgetStyle::Win98 => (3.0, 1.0, vec![p.border, p.bevel_light]),
        WidgetStyle::Vgui => (3.0, 1.0, vec![p.bevel_dark, p.bevel_light]),
        WidgetStyle::Luna => (3.0, 1.0, vec![p.disabled_text]),
        WidgetStyle::Terminal => (3.0, 4.0, vec![p.border]),
    }
}

/// Casilla o radio del ítem en los temas planos (`QMenu::indicator`): caja de 16 px con borde
/// `muted` (redonda si es exclusiva) y, marcada, el cuadro `accent` de 10 px. Los de época solo
/// dibujan la marca (imagen de 13 px, sin caja).
pub fn menu_indicator(p: &Palette, radio: bool) -> (f32, Border, Color) {
    let radius = if radio { 8.0 } else { 0.0 };
    (16.0, border(1.0, p.muted, radius), p.accent)
}

/// Barra de herramientas (`QToolBar#toolbar`): fondo y línea de abajo (los sabores de
/// Atardecer, Industrial y Madera; 98, VGUI, Luna y Terminal, la suya).
pub fn toolbar(p: &Palette, s: WidgetStyle, theme_id: &str) -> (Style, Option<Edges>) {
    let (bg, line) = match s {
        WidgetStyle::Flat => match theme_id {
            "synthwave" => (vgrad(p.bar_top, p.bg), Some((2.0, p.accent))),
            "rust" => (p.bg.into(), Some((3.0, p.accent))),
            "brown" => (vgrad(p.bar_top, p.bg), Some((1.0, p.border))),
            _ => (p.bg.into(), None),
        },
        WidgetStyle::Win98 => (p.bg.into(), Some((1.0, p.border))),
        WidgetStyle::Vgui => (p.bg.into(), Some((1.0, p.bevel_dark))),
        WidgetStyle::Luna => (vgrad(p.button_top, p.bg), Some((1.0, p.border))),
        WidgetStyle::Terminal => (p.bg.into(), Some((1.0, p.border))),
    };
    (
        Style {
            background: Some(bg),
            text_color: Some(p.text),
            ..Style::default()
        },
        line.map(|(w, c)| Edges::bottom(w, c)),
    )
}

/// Separador de la barra (`::separator`, margen 4×6): sus líneas de 1 px, de izquierda a
/// derecha (98 y VGUI, hundido).
pub fn toolbar_separator(p: &Palette, s: WidgetStyle) -> Vec<Color> {
    match s {
        WidgetStyle::Flat | WidgetStyle::Luna => vec![p.divider],
        WidgetStyle::Win98 => vec![p.border, p.bevel_light],
        WidgetStyle::Vgui => vec![p.bevel_dark, p.bevel_light],
        WidgetStyle::Terminal => vec![p.border],
    }
}

/// `QToolBar#toolbar QToolButton { padding }` (dentro del borde transparente de 1 px);
/// Terminal no lo fija y hereda los 4 px de la hoja base.
pub fn tool_button_padding(s: WidgetStyle) -> f32 {
    match s {
        WidgetStyle::Flat | WidgetStyle::Terminal => 4.0,
        _ => 3.0,
    }
}

/// Botón de la barra: fondo y borde según `hovered`/`pressed` (`pressed` también vale para
/// `:open`, el de tema con su menú), y el bisel de 98 y VGUI, que Iced no dibuja.
pub fn tool_button(
    p: &Palette,
    s: WidgetStyle,
    hovered: bool,
    pressed: bool,
) -> (iced::widget::button::Style, Option<Edges>) {
    let clear = Color::TRANSPARENT;
    let (bg, edge, radius, edges): (Background, Color, f32, Option<Edges>) = match s {
        WidgetStyle::Flat => match (hovered, pressed) {
            (_, true) => (p.selection.into(), p.border, s.radius(), None),
            (true, false) => (p.surface.into(), p.border, s.radius(), None),
            _ => (clear.into(), clear, s.radius(), None),
        },
        WidgetStyle::Win98 => {
            let edges = (hovered || pressed).then(|| Edges::thin(p, pressed));
            (p.button_top.into(), clear, 0.0, edges)
        }
        WidgetStyle::Vgui => {
            let edges = (hovered || pressed).then(|| Edges::vgui(p, pressed));
            (clear.into(), clear, 0.0, edges)
        }
        WidgetStyle::Luna => match (hovered, pressed) {
            (_, true) => (vgrad(p.pressed_top, p.pressed_bottom), p.border, 3.0, None),
            (true, false) => (luna_button(p), p.border, 3.0, None),
            _ => (clear.into(), clear, 3.0, None),
        },
        // `:hover`/`:open` solo marcan el borde; `:pressed`, también el fondo.
        WidgetStyle::Terminal => {
            let bg = if hovered && pressed { p.surface } else { clear };
            let edge = if hovered || pressed { p.text } else { clear };
            (bg.into(), edge, 0.0, None)
        }
    };
    (
        iced::widget::button::Style {
            background: Some(bg),
            text_color: p.text,
            border: border(1.0, edge, radius),
            ..iced::widget::button::Style::default()
        },
        edges,
    )
}

/// Botón de Luna: `vgrad(surface, button_bottom, button_top)` con la parada al 85 %.
fn luna_button(p: &Palette) -> Background {
    Linear::new(PI)
        .add_stop(0.0, p.surface)
        .add_stop(0.85, p.button_top)
        .add_stop(1.0, p.button_bottom)
        .into()
}

/// Barra de progreso de la barra de estado (`QProgressBar` y su `::chunk`): marco (fondo y
/// borde), bisel de 98 y VGUI, relleno y color del texto. Terminal va invertida, como su barra
/// de estado; su texto, en `bg` para que se lea sobre el fondo `text`.
pub fn progress_bar(p: &Palette, s: WidgetStyle) -> (Style, Option<Edges>, Background, Color) {
    let frame = |bg: Color, b: Border| Style {
        background: Some(bg.into()),
        border: b,
        ..Style::default()
    };
    match s {
        // Sin hoja propia, Qt la dibuja con Fusion (azul de selección del sistema): `accent`.
        WidgetStyle::Flat => (
            frame(p.bg, border(1.0, p.progress_border, s.radius())),
            None,
            p.accent.into(),
            p.text,
        ),
        WidgetStyle::Win98 => (
            frame(p.button_top, Border::default()),
            Some(Edges::thin(p, true)),
            p.selection.into(),
            p.text,
        ),
        WidgetStyle::Vgui => (
            frame(p.surface, Border::default()),
            Some(Edges::vgui(p, true)),
            p.selection.into(),
            p.text,
        ),
        WidgetStyle::Luna => (
            frame(p.surface, border(1.0, p.progress_border, 3.0)),
            None,
            vgrad(p.progress_light, p.progress),
            p.text,
        ),
        WidgetStyle::Terminal => (
            frame(p.text, border(1.0, p.bg, 0.0)),
            None,
            p.bg.into(),
            p.bg,
        ),
    }
}

/// Ficha del servidor (`QFrame#server_card`): fondo `surface` y el borde de cada estilo; los
/// cantos de acento de Industrial (arriba, 3 px) y Madera (izquierda, 4 px) van como `Edges`
/// con el resto en 1 px, sin el radio. El doble de 3 px de Terminal va simple (Iced no tiene
/// líneas dobles).
pub fn card(p: &Palette, s: WidgetStyle, theme_id: &str) -> (Style, Option<Edges>) {
    let plain = base(p, p.surface);
    let accent = |top: f32, left: f32| {
        let side = |w: f32| {
            Some(if w > 1.0 {
                (w, p.accent)
            } else {
                (1.0, p.border)
            })
        };
        Edges {
            top: side(top),
            right: side(1.0),
            bottom: side(1.0),
            left: side(left),
        }
    };
    let bordered = |b: Border| Style { border: b, ..plain };
    match (s, theme_id) {
        (WidgetStyle::Flat, "rust") => (plain, Some(accent(3.0, 1.0))),
        (WidgetStyle::Flat, "brown") => (plain, Some(accent(1.0, 4.0))),
        (WidgetStyle::Flat, _) => (bordered(border(1.0, p.border, s.radius())), None),
        (WidgetStyle::Win98, _) => (plain, Some(Edges::sunken(p))),
        (WidgetStyle::Vgui, _) => (plain, Some(Edges::vgui(p, true))),
        (WidgetStyle::Luna, _) => (bordered(border(1.0, p.field_border, 3.0)), None),
        (WidgetStyle::Terminal, _) => (bordered(border(3.0, p.text, 0.0)), None),
    }
}

/// Títulos de la ficha, su estado vacío y «🔒 Contraseña» (`role="caption"`).
pub fn card_caption(p: &Palette) -> Color {
    p.muted
}

/// Recuadro del mapa sin imagen (`QLabel#map_image[placeholder]`): `small_radius` en los
/// planos y Luna, hundido en 98 y VGUI; el discontinuo de Terminal va continuo.
pub fn map_frame(p: &Palette, s: WidgetStyle) -> (Style, Option<Edges>) {
    let st = |b: Border| Style {
        border: b,
        ..colored(p.surface, p.muted)
    };
    match s {
        WidgetStyle::Flat => (st(border(1.0, p.border, 4.0)), None),
        WidgetStyle::Luna => (st(border(1.0, p.border, 3.0)), None),
        WidgetStyle::Win98 => (st(Border::default()), Some(Edges::sunken_on_surface(p))),
        WidgetStyle::Vgui => (st(Border::default()), Some(Edges::vgui(p, true))),
        WidgetStyle::Terminal => (st(border(1.0, p.border, 0.0)), None),
    }
}

/// Estrella del favorito (`QPushButton#favorite_toggle[favorite]`).
pub fn favorite_star(p: &Palette, favorite: bool) -> Color {
    if favorite { p.favorite } else { p.muted }
}

/// `QPushButton#copy_address { padding: 2px 10px }`, en todos los estilos (gana al de época).
pub fn copy_button() -> Padding {
    [2, 10].into()
}

/// Divisores de la columna derecha: el asa de Qt es solo el fondo (sin línea ni resaltado).
pub fn splitter() -> iced::widget::pane_grid::Style {
    use iced::widget::pane_grid::{Highlight, Line};
    let line = Line {
        color: Color::TRANSPARENT,
        width: 0.0,
    };
    iced::widget::pane_grid::Style {
        hovered_region: Highlight {
            background: Color::TRANSPARENT.into(),
            border: Border::default(),
        },
        picked_split: line,
        hovered_split: line,
    }
}
