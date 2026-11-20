//! Menú propio (`QMenu`): una lista con ítems (atajo, casilla o radio), separadores y
//! submenús que se abre pegada a su ancla en la capa de arriba (`overlay`). El widget envuelve
//! al ancla (un título de la barra, un botón o la tabla) y, mientras le pasen entradas, dibuja
//! la lista: debajo del ancla, a la derecha de un ítem (submenú) o donde fue el último clic
//! derecho, siempre dentro de la ventana. Los submenús se abren al pasar el ratón (estado del
//! widget, sin mensajes); el clic afuera manda `on_dismiss` y no llega a lo de abajo.

use std::sync::LazyLock;

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::svg::{Handle, Svg};
use iced::advanced::text::{Alignment, LineHeight, Renderer as _, Shaping, Text};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, svg as core_svg};
use iced::alignment::Vertical;
use iced::widget::text::Wrapping;
use iced::{
    Background, Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    mouse, window,
};

use crate::icons::{self, Arrow, arrow_svg};
use crate::theme::{Edges, WidgetStyle, palette, style, widget_style};
use crate::widgets::{self, text_width};

macro_rules! asset {
    ($path:expr) => {
        include_bytes!(concat!("../assets/", $path)) as &[u8]
    };
}

/// Casilla del ítem: ninguna, de las que se marcan solas o exclusiva (radio).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    None,
    Check(bool),
    Radio(bool),
}

/// Entrada de un menú; `on: None` = ítem apagado y `Sub` sin entradas = submenú apagado.
#[derive(Debug, Clone)]
pub enum Entry<M> {
    Item {
        label: String,
        shortcut: Option<String>,
        check: Check,
        /// Icono de la barra (`icons::tool_icon`), como el `QIcon` de la acción.
        icon: Option<&'static str>,
        on: Option<M>,
    },
    Separator,
    Sub {
        label: String,
        icon: Option<&'static str>,
        entries: Vec<Entry<M>>,
    },
}

impl<M> Entry<M> {
    fn icon(&self) -> Option<&'static str> {
        match self {
            Entry::Item { icon, .. } | Entry::Sub { icon, .. } => *icon,
            Entry::Separator => None,
        }
    }

    fn enabled(&self) -> bool {
        match self {
            Entry::Item { on, .. } => on.is_some(),
            Entry::Sub { entries, .. } => !entries.is_empty(),
            Entry::Separator => false,
        }
    }
}

/// Dónde se abre: debajo del ancla, a su derecha o donde se hizo clic derecho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Below,
    Right,
    Cursor,
}

/// Esquina de una lista de tamaño `size` junto a `anchor`; si no cabe se corre como `QMenu`:
/// debajo, a la izquierda hasta caber o arriba del ancla; a la derecha (submenú), a la
/// izquierda del ancla; en el cursor, a su izquierda o arriba. Siempre dentro de `window`
/// si cabe en ella.
pub fn place(anchor: Rectangle, size: Size, window: Size, placement: Placement) -> Point {
    let (mut x, mut y) = match placement {
        Placement::Below => (anchor.x, anchor.y + anchor.height),
        Placement::Right => (anchor.x + anchor.width, anchor.y),
        Placement::Cursor => (anchor.x, anchor.y),
    };
    if x + size.width > window.width {
        x = match placement {
            Placement::Below => window.width - size.width,
            Placement::Right | Placement::Cursor => anchor.x - size.width,
        };
    }
    if y + size.height > window.height {
        y = match placement {
            Placement::Right => window.height - size.height,
            Placement::Below | Placement::Cursor => anchor.y - size.height,
        };
    }
    // En píxeles enteros (el ancla puede quedar en medio píxel): bordes y texto nítidos.
    Point::new(
        x.min(window.width - size.width).max(0.0).floor(),
        y.min(window.height - size.height).max(0.0).floor(),
    )
}

/// Espacio entre el texto de un ítem y su atajo.
const SHORTCUT_GAP: f32 = 24.0;

/// Lado de los iconos de los menús (`PM_SmallIconSize`).
const ICON: f32 = 16.0;

/// Alto de un ítem: la línea de Qt (o el icono, si es más alto) más el relleno.
fn item_height(s: WidgetStyle, icon: bool) -> f32 {
    let line = style::qt_line_height(s);
    let line = if icon { line.max(ICON) } else { line };
    line + 2.0 * style::menu_item_padding(s).0
}

/// Columna que Qt reserva antes del texto si algún ítem del menú tiene casilla (lo que mide
/// `QMenu::indicator`) o icono (16 px más 4), medida en las capturas de 1.3.0.
pub fn check_column<M>(entries: &[Entry<M>], s: WidgetStyle) -> f32 {
    let indicator = if s == WidgetStyle::Flat { 16.0 } else { 13.0 };
    entries
        .iter()
        .map(|e| match e {
            Entry::Item {
                check: Check::Check(_) | Check::Radio(_),
                ..
            } => indicator,
            _ if e.icon().is_some() => ICON + 4.0,
            _ => 0.0,
        })
        .fold(0.0, f32::max)
}

fn entry_height<M>(e: &Entry<M>, s: WidgetStyle) -> f32 {
    match e {
        Entry::Separator => {
            let (margin, _, lines) = style::menu_separator(palette("dark"), s);
            2.0 * margin + lines.len() as f32
        }
        _ => item_height(s, e.icon().is_some()),
    }
}

/// Tamaño de la lista: el texto más largo con su atajo, el relleno y el marco.
pub fn menu_size<M>(entries: &[Entry<M>], s: WidgetStyle) -> Size {
    let (_, pad_l, pad_r) = style::menu_item_padding(s);
    let frame = style::menu_frame(palette("dark"), s).2;
    let width = |t: &str| text_width(t, s.font(), s.text_size());
    let text = entries
        .iter()
        .map(|e| match e {
            Entry::Item {
                label, shortcut, ..
            } => width(label) + shortcut.as_deref().map_or(0.0, |k| SHORTCUT_GAP + width(k)),
            Entry::Sub { label, .. } => width(label),
            Entry::Separator => 0.0,
        })
        .fold(0.0, f32::max);
    let height: f32 = entries.iter().map(|e| entry_height(e, s)).sum();
    let column = check_column(entries, s);
    Size::new(
        (pad_l + column + text + pad_r + 2.0 * frame).ceil(),
        height + 2.0 * frame,
    )
}

/// Filas de la lista, relativas a su esquina.
fn row_nodes<M>(entries: &[Entry<M>], s: WidgetStyle, width: f32) -> Vec<Node> {
    let frame = style::menu_frame(palette("dark"), s).2;
    let mut y = frame;
    entries
        .iter()
        .map(|e| {
            let h = entry_height(e, s);
            let node = Node::new(Size::new(width - 2.0 * frame, h)).move_to(Point::new(frame, y));
            y += h;
            node
        })
        .collect()
}

/// El ancla con su menú, abierto mientras `open` traiga entradas.
pub fn popup<'a, M: Clone + 'a>(
    anchor: Element<'a, M>,
    open: Option<Vec<Entry<M>>>,
    placement: Placement,
    on_dismiss: M,
    theme_id: &'static str,
) -> Element<'a, M> {
    Element::new(Popup {
        anchor,
        entries: open,
        placement,
        on_dismiss,
        theme_id,
    })
}

struct Popup<'a, M> {
    anchor: Element<'a, M>,
    entries: Option<Vec<Entry<M>>>,
    placement: Placement,
    on_dismiss: M,
    theme_id: &'static str,
}

#[derive(Debug, Default)]
struct State {
    /// Último clic derecho sobre el ancla (`Placement::Cursor`); sin él (las capturas, que
    /// abren el menú con un mensaje), la esquina del ancla.
    cursor: Option<Point>,
    /// Submenús abiertos: el índice del ítem en cada nivel.
    path: Vec<usize>,
    /// Ítem bajo el ratón: (nivel, índice).
    hover: Option<(usize, usize)>,
}

impl<'a, M: Clone + 'a> Widget<M, Theme, Renderer> for Popup<'a, M> {
    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.anchor.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.anchor)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.anchor));
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> Node {
        self.anchor
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.anchor
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) = event
            && let Some(at) = cursor.position_over(layout.bounds())
        {
            tree.state.downcast_mut::<State>().cursor = Some(at);
        }
        self.anchor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.anchor.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        let anchor = self.anchor.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        );
        let menu = match &self.entries {
            Some(entries) => {
                let bounds = match self.placement {
                    Placement::Cursor => Rectangle::new(
                        state.cursor.unwrap_or(layout.bounds().position()),
                        Size::ZERO,
                    ),
                    _ => layout.bounds(),
                } + translation;
                Some(overlay::Element::new(Box::new(Menu {
                    entries,
                    state,
                    anchor: bounds,
                    placement: self.placement,
                    on_dismiss: &self.on_dismiss,
                    theme_id: self.theme_id,
                })))
            }
            None => {
                // Cerrado: la próxima vez abre sin submenús.
                state.path.clear();
                state.hover = None;
                None
            }
        };
        match (anchor, menu) {
            (None, None) => None,
            (Some(o), None) | (None, Some(o)) => Some(o),
            (Some(a), Some(m)) => Some(overlay::Group::with_children(vec![a, m]).overlay()),
        }
    }
}

/// La lista abierta y sus submenús: un nodo por nivel con una fila por entrada.
struct Menu<'b, M> {
    entries: &'b [Entry<M>],
    state: &'b mut State,
    anchor: Rectangle,
    placement: Placement,
    on_dismiss: &'b M,
    theme_id: &'static str,
}

impl<M> Menu<'_, M> {
    /// Entradas del nivel `level` (el menú o uno de los submenús abiertos).
    fn level(&self, level: usize) -> &[Entry<M>] {
        self.state.path[..level]
            .iter()
            .fold(self.entries, |es, &i| match es.get(i) {
                Some(Entry::Sub { entries, .. }) => entries,
                _ => &[],
            })
    }

    /// (nivel, fila) bajo el punto; la fila es `None` sobre el marco.
    fn hit(layout: Layout<'_>, at: Point) -> Option<(usize, Option<usize>)> {
        layout
            .children()
            .enumerate()
            .rev()
            .find(|(_, l)| l.bounds().contains(at))
            .map(|(k, l)| (k, l.children().position(|r| r.bounds().contains(at))))
    }
}

/// ✓ y • de 98 (Luna los usa también en sus menús), ✓ y X teñibles y la flecha de 98.
static CHECK: LazyLock<Handle> = LazyLock::new(|| Handle::from_memory(asset!("win98/check.svg")));
static DOT: LazyLock<Handle> = LazyLock::new(|| Handle::from_memory(asset!("win98/dot.svg")));
static ARROW_98: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_memory(asset!("win98/arrow_right.svg")));
static X_MARK: LazyLock<Handle> =
    LazyLock::new(|| widgets::tinted_mark(widgets::X_PATH, Color::BLACK));
static ARROW: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_memory(arrow_svg(Arrow::Right, Color::BLACK).into_bytes()));

fn fill(renderer: &mut Renderer, bounds: Rectangle, color: Color) {
    renderer::Renderer::fill_quad(
        renderer,
        Quad {
            bounds,
            ..Quad::default()
        },
        Background::Color(color),
    );
}

/// Los lados de `e` por dentro de `r`.
fn draw_edges(renderer: &mut Renderer, r: Rectangle, e: Edges) {
    let sides = [
        e.top
            .map(|(w, c)| (Rectangle::new(r.position(), Size::new(r.width, w)), c)),
        e.left
            .map(|(w, c)| (Rectangle::new(r.position(), Size::new(w, r.height)), c)),
        e.bottom.map(|(w, c)| {
            let at = Point::new(r.x, r.y + r.height - w);
            (Rectangle::new(at, Size::new(r.width, w)), c)
        }),
        e.right.map(|(w, c)| {
            let at = Point::new(r.x + r.width - w, r.y);
            (Rectangle::new(at, Size::new(w, r.height)), c)
        }),
    ];
    for (bounds, c) in sides.into_iter().flatten() {
        fill(renderer, bounds, c);
    }
}

/// Imagen de `size` centrada verticalmente en `row`, con su borde izquierdo en `x`.
fn draw_svg(
    renderer: &mut Renderer,
    handle: &Handle,
    color: Option<Color>,
    x: f32,
    row: Rectangle,
    size: Size,
) {
    let at = Point::new(x, (row.center_y() - size.height / 2.0).round());
    let mut svg = Svg::new(handle.clone());
    svg.color = color;
    core_svg::Renderer::draw_svg(renderer, svg, Rectangle::new(at, size), row);
}

impl<M: Clone> overlay::Overlay<M, Theme, Renderer> for Menu<'_, M> {
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> Node {
        let s = widget_style(self.theme_id);
        let frame = style::menu_frame(palette(self.theme_id), s).2;
        let (mut anchor, mut placement) = (self.anchor, self.placement);
        let mut levels = Vec::new();
        loop {
            let entries = self.level(levels.len());
            let size = menu_size(entries, s);
            let origin = place(anchor, size, bounds, placement);
            let rows = row_nodes(entries, s, size.width);
            let k = levels.len();
            let open = self.state.path.get(k).copied();
            let next = open.and_then(|i| match entries.get(i) {
                Some(e @ Entry::Sub { .. }) if e.enabled() => Some(i),
                _ => None,
            });
            // El submenú se alinea con su ítem y se abre al borde de la lista.
            let sub_anchor = next.map(|i| {
                let row = rows[i].bounds();
                Rectangle::new(
                    Point::new(origin.x, origin.y + row.y - frame),
                    Size::new(size.width, row.height),
                )
            });
            levels.push(Node::with_children(size, rows).move_to(origin));
            match sub_anchor {
                Some(a) => (anchor, placement) = (a, Placement::Right),
                None => {
                    self.state.path.truncate(k);
                    break;
                }
            }
        }
        Node::with_children(bounds, levels)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        let (p, s) = (palette(self.theme_id), widget_style(self.theme_id));
        let (bg, frames, _) = style::menu_frame(p, s);
        let (_, pad_l, pad_r) = style::menu_item_padding(s);
        let (sep_v, sep_h, sep_lines) = style::menu_separator(p, s);
        for (k, level) in layout.children().enumerate() {
            let r = level.bounds();
            let column = check_column(self.level(k), s);
            fill(renderer, r, bg);
            for &(inset, e) in &frames {
                draw_edges(renderer, r.shrink(inset), e);
            }
            for (i, (e, row)) in self.level(k).iter().zip(level.children()).enumerate() {
                let row = row.bounds();
                let label = match e {
                    Entry::Separator => {
                        for (j, &c) in sep_lines.iter().enumerate() {
                            let at = Point::new(row.x + sep_h, row.y + sep_v + j as f32);
                            fill(
                                renderer,
                                Rectangle::new(at, Size::new(row.width - 2.0 * sep_h, 1.0)),
                                c,
                            );
                        }
                        continue;
                    }
                    Entry::Item { label, .. } | Entry::Sub { label, .. } => label,
                };
                let on = self.state.hover == Some((k, i)) || self.state.path.get(k) == Some(&i);
                let selected = on && e.enabled();
                let (highlight, color) = style::menu_item(p, s, selected, e.enabled());
                if let Some(c) = highlight {
                    fill(renderer, row, c);
                }
                let text = |content: &str| Text {
                    content: content.to_owned(),
                    bounds: Size::new(f32::INFINITY, row.height),
                    size: s.text_size().into(),
                    line_height: LineHeight::default(),
                    font: s.font(),
                    align_x: Alignment::Default,
                    align_y: Vertical::Center,
                    shaping: Shaping::default(),
                    wrapping: Wrapping::None,
                };
                let at = Point::new(row.x + pad_l + column, row.center_y());
                renderer.fill_text(text(label), at, color, r);
                if let Some(h) = e
                    .icon()
                    .and_then(|n| icons::tool_icon(n, self.theme_id, !e.enabled()))
                {
                    draw_svg(renderer, &h, None, row.x + 4.0, row, Size::new(ICON, ICON));
                }
                match e {
                    Entry::Item {
                        shortcut, check, ..
                    } => {
                        // El atajo, alineado a la derecha antes del relleno.
                        if let Some(k) = shortcut {
                            let w = text_width(k, s.font(), s.text_size());
                            let at = Point::new(row.x + row.width - pad_r - w, row.center_y());
                            renderer.fill_text(text(k), at, color, r);
                        }
                        self.draw_check(renderer, *check, row, selected, color);
                    }
                    Entry::Sub { .. } => {
                        // `QMenu::right-arrow` con su `padding-right: 6px`.
                        let (handle, size, tint) = match s {
                            WidgetStyle::Win98 | WidgetStyle::Luna => (
                                &*ARROW_98,
                                Size::new(4.0, 7.0),
                                selected.then_some(Color::WHITE),
                            ),
                            _ => (&*ARROW, Size::new(5.0, 9.0), Some(color)),
                        };
                        let x = row.x + row.width - 6.0 - size.width;
                        draw_svg(renderer, handle, tint, x, row, size);
                    }
                    Entry::Separator => {}
                }
            }
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
    ) {
        let hit = cursor.position().and_then(|at| Self::hit(layout, at));
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let hover = hit.and_then(|(k, i)| i.map(|i| (k, i)));
                if hover != self.state.hover {
                    self.state.hover = hover;
                    shell.request_redraw();
                }
                // Pasar por un ítem cierra los submenús de más adentro; por un submenú, lo abre.
                if let Some((k, i)) = hover
                    && let Some(e) = self
                        .level(k)
                        .get(i)
                        .filter(|e| !matches!(e, Entry::Separator))
                {
                    let mut path = self.state.path[..k.min(self.state.path.len())].to_vec();
                    if matches!(e, Entry::Sub { .. }) && e.enabled() {
                        path.push(i);
                    }
                    if path != self.state.path {
                        self.state.path = path;
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                }
            }
            // Qt traga el clic que cierra el menú.
            Event::Mouse(mouse::Event::ButtonPressed(_)) => {
                if hit.is_none() {
                    shell.publish(self.on_dismiss.clone());
                }
                shell.capture_event();
            }
            // Como `QMenu`, el ítem se elige al soltar (también tras arrastrar desde el título).
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some((k, Some(i))) = hit
                    && let Some(Entry::Item { on: Some(m), .. }) = self.level(k).get(i)
                {
                    shell.publish(m.clone());
                    shell.capture_event();
                }
            }
            // Qt captura el ratón con el menú abierto: la rueda no desplaza lo de abajo.
            Event::Mouse(mouse::Event::WheelScrolled { .. }) => shell.capture_event(),
            // Como `QMenu`, se cierra si la ventana pierde el foco.
            Event::Window(window::Event::Unfocused) => shell.publish(self.on_dismiss.clone()),
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        // Sobre la lista, lo de abajo no ve el ratón; afuera sí (títulos de la barra).
        match cursor.position().and_then(|at| Self::hit(layout, at)) {
            Some(_) => mouse::Interaction::Idle,
            None => mouse::Interaction::None,
        }
    }
}

impl<M> Menu<'_, M> {
    /// Casilla o radio: caja de 16 px en los planos; en los de época solo la marca de 13 px.
    fn draw_check(
        &self,
        renderer: &mut Renderer,
        check: Check,
        row: Rectangle,
        selected: bool,
        color: Color,
    ) {
        let (radio, on) = match check {
            Check::None => return,
            Check::Check(on) => (false, on),
            Check::Radio(on) => (true, on),
        };
        let p = palette(self.theme_id);
        let s = widget_style(self.theme_id);
        // Qt pone el indicador al borde izquierdo del ítem.
        let x = row.x;
        if s == WidgetStyle::Flat {
            let (side, border, accent) = style::menu_indicator(p, radio);
            let at = Point::new(x, (row.center_y() - side / 2.0).round());
            let quad = |bounds, border| Quad {
                bounds,
                border,
                ..Quad::default()
            };
            renderer::Renderer::fill_quad(
                renderer,
                quad(Rectangle::new(at, Size::new(side, side)), border),
                Background::Color(Color::TRANSPARENT),
            );
            if on {
                let inner = Rectangle::new(at, Size::new(side, side)).shrink(3.0);
                let radius = if radio { 5.0 } else { 0.0 };
                renderer::Renderer::fill_quad(
                    renderer,
                    quad(inner, iced::border::rounded(radius)),
                    Background::Color(accent),
                );
            }
            return;
        }
        if !on {
            return;
        }
        // Qt escala la imagen SVG (`image:`) al indicador entero de 13 px (visto en 1.3.0).
        let (handle, tint) = match s {
            WidgetStyle::Win98 | WidgetStyle::Luna if radio => (&*DOT, None),
            WidgetStyle::Win98 | WidgetStyle::Luna => (&*CHECK, None),
            WidgetStyle::Terminal => (&*X_MARK, Some(if selected { color } else { p.text })),
            _ => (&*CHECK, Some(p.text)),
        };
        draw_svg(renderer, handle, tint, x, row, Size::new(13.0, 13.0));
    }
}
