//! Controles propios dibujados en los 5 estilos (lo que el `checkbox` de Iced no deja pintar
//! como Qt): la casilla, el combo, el campo de texto y el tooltip.

use std::sync::LazyLock;

use iced::advanced::graphics::text::Paragraph;
use iced::advanced::text::{Alignment, Paragraph as _, Shaping, Text};
use iced::widget::svg::Handle;
use iced::widget::text::LineHeight;
use iced::widget::text::Wrapping;
use iced::widget::{
    Id, Row, Space, center, column, container, mouse_area, opaque, pick_list, row, stack, svg,
    text, text_editor, text_input, tooltip,
};
use iced::{Center, Element, Fill, Font, Padding, Pixels, Size};

use crate::icons::{Arrow, arrow_svg};
use crate::theme::{Palette, WidgetStyle, framed, hex, palette, style, widget_style};

/// El campo del filtro de texto: `operation::is_focused` lo consulta tras cada clic.
pub const FILTER_ID: Id = Id::new("filter-text");

macro_rules! asset {
    ($path:expr) => {
        include_bytes!(concat!("../../../src/jase/assets/", $path)) as &[u8]
    };
}

/// ✓ de 7×7 (`_tinted_check`) y X de 7×7 (`_tinted_x`) en `color`.
const CHECK_PATH: &str = "M6 0h1v3H6v1H5v1H4v1H3v1H2V6H1V5H0V2h1v1h1v1h1V3h1V2h1V1h1z";
pub(crate) const X_PATH: &str =
    "M0 0h2v1h1v1h1V1h1V0h2v2H6v1H5v1h1v1h1v2H5V6H4V5H3v1H2v1H0V5h1V4h1V3H1V2H0z";

pub(crate) fn tinted_mark(path: &str, color: iced::Color) -> Handle {
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

/// Casilla con su etiqueta; un clic en cualquiera de las dos manda `on_toggle`. Sin mensaje
/// sale apagada (`QCheckBox:disabled`): la etiqueta toma el color apagado si no trae el suyo.
pub fn check<'a, M: Clone + 'a>(
    checked: bool,
    label: Option<Element<'a, M>>,
    on_toggle: Option<M>,
    p: &'static Palette,
    s: WidgetStyle,
) -> Element<'a, M> {
    let (side, box_style, edges) = style::check(p, s, on_toggle.is_some());
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
        // Alto de la caja, como `QCheckBox`: la línea de texto de Iced es 1 px más alta.
        Some(l) => row![boxed, l]
            .spacing(5)
            .height(side)
            .align_y(Center)
            .into(),
        None => boxed,
    };
    match on_toggle {
        Some(m) => mouse_area(content).on_press(m).into(),
        None => container(content)
            .style(move |_| container::Style {
                // `QCheckBox:disabled { color: muted }` de la hoja común, en todos los estilos.
                text_color: Some(p.muted),
                ..container::Style::default()
            })
            .into(),
    }
}

/// `content` con el tooltip del estilo (`QToolTip`) debajo.
pub fn tip<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    tip: String,
    p: &'static Palette,
    s: WidgetStyle,
) -> Element<'a, M> {
    combo_tip(content, tip, false, p, s)
}

/// Como [`tip`], sin dibujarse con la lista del combo abierta (Qt lo esconde al abrir el popup;
/// si no, tapa la primera opción). El tooltip queda vacío, no se quita: cambiar el árbol del
/// combo le haría perder su estado y cerraría la lista.
pub fn combo_tip<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    tip: String,
    open: bool,
    p: &'static Palette,
    s: WidgetStyle,
) -> Element<'a, M> {
    let body: Element<'a, M> = if open {
        Space::new().into()
    } else {
        container(text(tip).font(s.font()).size(s.text_size()))
            .padding(style::tooltip_padding(s))
            .style(move |_| style::tooltip(p, s))
            .into()
    };
    tooltip(content, body, tooltip::Position::Bottom).into()
}

/// Grosor del bisel que Iced no dibuja y del borde que sí dibuja (este va dentro del relleno).
fn field_frame(p: &Palette, s: WidgetStyle) -> (f32, f32, Padding) {
    let (_, b, edges, pad) = style::field(p, s, "", false);
    let edge = edges.map_or(0.0, |e| e.top.map_or(0.0, |(w, _)| w));
    (edge, b.width, pad)
}

/// Línea de texto de combos y campos: `QComboBox` cuenta el ícono de 16 px aunque no lo tenga,
/// así que mide 16 px en todos los estilos (la de Iced, 1,3 × letra, deja bordes en medio píxel).
pub const FIELD_LINE: LineHeight = LineHeight::Absolute(Pixels(16.0));

/// Alto de combos y campos: línea de texto, relleno de Qt, borde y bisel (las etiquetas de la
/// rejilla «Ping máx.»/«País» se centran en este alto).
pub fn field_height(s: WidgetStyle) -> f32 {
    let (edge, bw, pad) = field_frame(palette("dark"), s);
    16.0 + 2.0 * (pad.top + bw + edge)
}

/// Relleno de Iced: el de Qt más el borde de Iced, que se dibuja encima del relleno.
fn iced_padding(pad: Padding, bw: f32) -> Padding {
    Padding {
        top: pad.top + bw,
        right: pad.right + bw,
        bottom: pad.bottom + bw,
        left: pad.left + bw,
    }
}

/// Relleno de Iced de combos y campos en el estilo.
pub fn field_padding(s: WidgetStyle) -> Padding {
    let (_, bw, pad) = field_frame(palette("dark"), s);
    iced_padding(pad, bw)
}

/// Flecha del combo en el estilo (`QComboBox::down-arrow`).
fn combo_arrow(p: &Palette, s: WidgetStyle) -> (Handle, f32, f32) {
    static WIN98: LazyLock<Handle> =
        LazyLock::new(|| Handle::from_memory(asset!("win98/arrow_down.svg")));
    static LUNA: LazyLock<Handle> =
        LazyLock::new(|| Handle::from_memory(asset!("luna/arrow_down.svg")));
    let tinted = |c| Handle::from_memory(arrow_svg(Arrow::Down, c).into_bytes());
    match s {
        WidgetStyle::Flat => (tinted(p.arrow), 9.0, 5.0),
        WidgetStyle::Vgui | WidgetStyle::Terminal => (tinted(p.text), 9.0, 5.0),
        WidgetStyle::Win98 => (WIN98.clone(), 7.0, 4.0),
        WidgetStyle::Luna => (LUNA.clone(), 7.0, 4.0),
    }
}

/// Combo de Qt: `pick_list` sin su flecha, con la del estilo encima (y su botón en 98, VGUI y
/// Luna). La lista que abre solo dibuja texto (sin las banderas de Qt). Apagado
/// (`QComboBox:disabled`), un campo fijo con lo elegido, que no abre.
pub fn combo<'a, T, M>(
    options: Vec<T>,
    selected: Option<T>,
    on_select: impl Fn(T) -> M + 'a,
    on_open: Option<(M, M)>,
    enabled: bool,
    theme_id: &'static str,
) -> Element<'a, M>
where
    T: ToString + PartialEq + Clone + 'a,
    M: Clone + 'a,
{
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (edge, bw, pad) = field_frame(p, s);
    let (button_w, button) = style::combo_button(p, s);
    let padding = Padding {
        right: pad.right + button_w,
        ..iced_padding(pad, bw)
    };
    let field: Element<'a, M> = if enabled {
        combo_pick(options, selected, on_select, on_open, padding, theme_id)
    } else {
        let label = selected.map(|o| o.to_string()).unwrap_or_default();
        let label = text(label)
            .font(s.font())
            .size(s.text_size())
            .line_height(FIELD_LINE)
            .wrapping(Wrapping::None);
        container(label)
            .padding(padding)
            .width(Fill)
            .clip(true)
            .style(move |_| style::combo_disabled(p, s, theme_id))
            .into()
    };
    let (handle, w, h) = combo_arrow(p, s);
    let arrow = container(svg(handle).width(w).height(h))
        .center_x(button_w)
        .center_y(Fill);
    let arrow: Element<'a, M> = match button {
        Some((face, edges)) => {
            let face = container(arrow).height(Fill).style(move |_| face);
            let face: Element<'a, M> = match edges {
                Some(e) => framed(face, e),
                None => face.into(),
            };
            // Luna deja 1 px de margen alrededor del botón.
            let margin = if s == WidgetStyle::Luna { 1.0 } else { 0.0 };
            container(face).padding(bw + margin).height(Fill).into()
        }
        None => arrow.into(),
    };
    let height = field_height(s) - 2.0 * edge;
    let combo = stack![field, container(arrow).align_right(Fill).height(height)]
        .width(Fill)
        .height(height);
    let (_, _, edges, _) = style::field(p, s, theme_id, false);
    match edges {
        Some(e) => framed(combo, e),
        None => combo.into(),
    }
}

/// El `pick_list` del combo encendido.
fn combo_pick<'a, T, M>(
    options: Vec<T>,
    selected: Option<T>,
    on_select: impl Fn(T) -> M + 'a,
    on_open: Option<(M, M)>,
    padding: Padding,
    theme_id: &'static str,
) -> Element<'a, M>
where
    T: ToString + PartialEq + Clone + 'a,
    M: Clone + 'a,
{
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let mut pick = pick_list(options, selected, on_select)
        .handle(pick_list::Handle::None)
        .font(s.font())
        .text_size(s.text_size())
        .text_line_height(FIELD_LINE)
        .padding(padding)
        .width(Fill)
        .style(move |_, status| style::pick_list(p, s, theme_id, status))
        .menu_style(move |_| style::menu(p, s));
    if let Some((open, close)) = on_open {
        pick = pick.on_open(open).on_close(close);
    }
    pick.into()
}

/// Ancho de un combo con `setMinimumContentsLength(chars)`: el mayor entre `chars` veces la «X»
/// y la opción más larga de `labels` (`sizeHint` de `QComboBox`), relleno, botón de la flecha,
/// borde y bisel.
pub fn combo_width(chars: usize, labels: &[&str], theme_id: &'static str) -> f32 {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (edge, bw, pad) = field_frame(p, s);
    let x = text_width("X", s.font(), s.text_size());
    let text = labels
        .iter()
        .map(|l| text_width(l, s.font(), s.text_size()))
        .fold(chars as f32 * x, f32::max);
    text + pad.left + pad.right + style::combo_button(p, s).0 + 2.0 * (bw + edge)
}

/// Campo de texto (`QLineEdit`); `focused` elige el bisel con foco (98/VGUI, que Iced no
/// dibuja); el borde de Iced sigue al estado del propio `text_input`.
pub fn field<'a, M: Clone + 'a>(
    id: Id,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: Option<M>,
    focused: bool,
    theme_id: &'static str,
) -> Element<'a, M> {
    line_edit(
        id,
        placeholder,
        value,
        Some(on_input),
        on_submit,
        focused,
        false,
        theme_id,
    )
}

/// Campo de contraseña (`EchoMode.Password`): como [`field`], con el texto oculto; solo vive en
/// el diálogo de contraseña, que le da el foco al abrir.
pub fn password_field<'a, M: Clone + 'a>(
    id: Id,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: Option<M>,
    theme_id: &'static str,
) -> Element<'a, M> {
    line_edit(
        id,
        placeholder,
        value,
        Some(on_input),
        on_submit,
        true,
        true,
        theme_id,
    )
}

/// El campo de [`field`] y [`password_field`]; sin `on_input`, de solo lectura (Iced lo dibuja
/// igual: el estilo apagado es el normal).
#[allow(clippy::too_many_arguments)]
pub fn line_edit<'a, M: Clone + 'a>(
    id: Id,
    placeholder: &str,
    value: &str,
    on_input: Option<impl Fn(String) -> M + 'a>,
    on_submit: Option<M>,
    focused: bool,
    secure: bool,
    theme_id: &'static str,
) -> Element<'a, M> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (_, bw, pad) = field_frame(p, s);
    let input = text_input(placeholder, value)
        .id(id)
        .secure(secure)
        .on_input_maybe(on_input)
        .on_submit_maybe(on_submit)
        .font(s.font())
        .size(s.text_size())
        .line_height(FIELD_LINE)
        .padding(iced_padding(pad, bw))
        .width(Fill)
        .style(move |_, status| style::text_input(p, s, theme_id, status));
    let (_, _, edges, _) = style::field(p, s, theme_id, focused);
    match edges {
        Some(e) => framed(input, e),
        None => input.into(),
    }
}

/// Texto de varias líneas (`QPlainTextEdit`) de `height` px de alto, con el marco de [`field`].
pub fn text_area<'a, M: Clone + 'a>(
    id: Id,
    placeholder: &str,
    content: &'a text_editor::Content,
    on_action: impl Fn(text_editor::Action) -> M + 'a,
    height: f32,
    focused: bool,
    theme_id: &'static str,
) -> Element<'a, M> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (_, bw, pad) = field_frame(p, s);
    let padding = iced_padding(pad, bw);
    let (_, _, edges, _) = style::field(p, s, theme_id, focused);
    // Las franjas de 98 y VGUI van por fuera: el alto total queda en `height`, como en Qt.
    let w = |side: Option<(f32, iced::Color)>| side.map_or(0.0, |(w, _)| w);
    let height = height - edges.map_or(0.0, |e| w(e.top) + w(e.bottom));
    let editor = text_editor(content)
        .id(id)
        .placeholder(placeholder.to_owned())
        .on_action(on_action)
        .font(s.font())
        .size(s.text_size())
        .line_height(FIELD_LINE)
        .padding(padding)
        .height(height)
        .style(move |_, status| {
            let focused = matches!(status, text_editor::Status::Focused { .. });
            style::text_editor(p, s, theme_id, focused)
        });
    match edges {
        Some(e) => framed(editor, e),
        None => editor.into(),
    }
}

/// Botón de texto (`QPushButton`) en el estilo.
pub fn button<'a, M: Clone + 'a>(
    label: String,
    on_press: M,
    theme_id: &'static str,
) -> Element<'a, M> {
    let s = widget_style(theme_id);
    button_with(
        button_label(label, s),
        button_padding(s),
        on_press,
        theme_id,
    )
}

/// [`button`] a todo el ancho y con el texto al centro (`QPushButton` en un `QVBoxLayout`).
pub fn wide_button<'a, M: Clone + 'a>(
    label: String,
    on_press: M,
    theme_id: &'static str,
) -> Element<'a, M> {
    let s = widget_style(theme_id);
    let label = container(button_label(label, s)).center_x(Fill);
    button_with(label, button_padding(s), on_press, theme_id)
}

/// Relleno de [`button`] para medir lo que Qt: el plano 28 px de alto (`padding: 6px 14px` con
/// borde de 1 sobre una línea de 14; Iced mide la línea a 1,3 × 13 = 16,9), 98, VGUI y Luna
/// con `padding: 4px 12px` y Terminal con el suyo.
fn button_padding(s: WidgetStyle) -> Padding {
    match s {
        WidgetStyle::Flat => [5.5, 14.0].into(),
        // + 1: Iced dibuja el borde de 1 adentro del relleno.
        WidgetStyle::Luna => [4.0, 13.0].into(),
        WidgetStyle::Win98 | WidgetStyle::Vgui => [3.0, 12.0].into(),
        WidgetStyle::Terminal => [3.0, 14.0].into(),
    }
}

/// El texto de [`button`]; en 98, VGUI y Luna, de 60 px como mínimo (`min-width: 60px`).
fn button_label<'a>(label: String, s: WidgetStyle) -> iced::widget::Text<'a> {
    let min = match s {
        WidgetStyle::Win98 | WidgetStyle::Vgui | WidgetStyle::Luna => 60.0,
        WidgetStyle::Flat | WidgetStyle::Terminal => 0.0,
    };
    let width = text_width(&label, s.font(), s.text_size()).max(min);
    text(label)
        .font(s.font())
        .size(s.text_size())
        .width(width)
        .wrapping(Wrapping::None)
        .align_x(Center)
}

/// Botón de texto que, sin mensaje, sale apagado (`QPushButton:disabled`) y no hace nada.
pub fn button_maybe<'a, M: Clone + 'a>(
    label: String,
    on_press: Option<M>,
    theme_id: &'static str,
) -> Element<'a, M> {
    if let Some(m) = on_press {
        return button(label, m, theme_id);
    }
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (face, edges) = style::push_button_disabled(p, s, theme_id);
    let face = container(button_label(label, s))
        .padding(button_padding(s))
        .style(move |_| face);
    match edges {
        Some(e) => framed(face, e),
        None => face.into(),
    }
}

/// Botón del estilo con otro contenido y relleno (*Copiar* y la estrella de la ficha).
pub fn button_with<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M>>,
    padding: Padding,
    on_press: M,
    theme_id: &'static str,
) -> Element<'a, M> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (face, edges) = style::push_button(p, s, theme_id);
    let face = container(content).padding(padding).style(move |_| face);
    let face: Element<'a, M> = match edges {
        Some(e) => framed(face, e),
        None => face.into(),
    };
    mouse_area(face).on_press(on_press).into()
}

/// Pestañas (`QTabWidget`; Iced no trae): una fila de botones de texto sobre el panel con
/// `page`. La elegida, 2 px más alta (`QTabBar::tab:!selected { margin-top: 2px }`). El teclado
/// no cambia de pestaña (en Qt, Ctrl+Tab).
pub fn tabs<'a, M: Clone + 'a>(
    labels: Vec<String>,
    active: usize,
    on_select: impl Fn(usize) -> M + 'a,
    page: Element<'a, M>,
    theme_id: &'static str,
) -> Element<'a, M> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let (pad_y, gap) = if s == WidgetStyle::Terminal {
        (3.0, 2.0)
    } else {
        (4.0, 1.0)
    };
    let bar = Row::with_children(labels.into_iter().enumerate().map(|(i, label)| {
        let selected = i == active;
        let lift = if selected { 2.0 } else { 0.0 };
        let (face, edges) = style::tab(p, s, selected, theme_id);
        let tab = container(text(label).font(s.font()).size(s.text_size()))
            .padding(Padding {
                top: pad_y,
                bottom: pad_y + lift,
                left: 12.0,
                right: 12.0,
            })
            .style(move |_| face);
        let tab: Element<'a, M> = match edges {
            Some(e) => framed(tab, e),
            None => tab.into(),
        };
        let tab = container(tab).padding(Padding {
            top: 2.0 - lift,
            ..Padding::ZERO
        });
        mouse_area(tab).on_press(on_select(i)).into()
    }))
    .spacing(gap)
    .align_y(iced::Bottom);
    let (pane, edges) = style::tab_pane(p, s, theme_id);
    let pane = container(page).padding(8).width(Fill).style(move |_| pane);
    let pane: Element<'a, M> = match edges {
        Some(e) => framed(pane, e),
        None => pane.into(),
    };
    column![container(bar).padding([0, 2]), pane].into()
}

/// `card` centrada sobre `base`. El fondo traga los clics sin hacer nada (como el diálogo modal
/// de Qt) y, al ser `opaque`, deja el cursor «levitando» para `base`: ni hover ni rueda abajo.
pub fn modal<'a, M: Clone + 'a>(
    base: impl Into<Element<'a, M>>,
    card: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    stack![base.into(), opaque(center(opaque(card)))].into()
}

/// Ancho de `text` en una línea, medido con el mismo sistema de letras que dibuja.
pub fn text_width(text: &str, font: Font, size: f32) -> f32 {
    Paragraph::with_text(Text {
        content: text,
        bounds: Size::INFINITE,
        size: size.into(),
        line_height: LineHeight::default(),
        font,
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::default(),
        wrapping: Wrapping::None,
    })
    .min_width()
}
