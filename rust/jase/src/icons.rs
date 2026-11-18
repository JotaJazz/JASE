//! Iconos de la barra de herramientas, flechas del orden, insignias de juego e icono de la app.
//! Los SVG van embebidos (nada de disco en `view`); los teñidos se arman como texto.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use iced::widget::svg::Handle;
use iced::widget::{container, space, text};
use iced::{Color, Element, Font, Size, border, font, window};
use jase_core::games::GameDef;

use crate::theme::{Palette, WidgetStyle, hex, palette, widget_style};

macro_rules! asset {
    ($path:expr) => {
        include_bytes!(concat!("../../../src/jase/assets/", $path)) as &[u8]
    };
}

macro_rules! tool_icons {
    ($($name:literal),* $(,)?) => {
        pub const TOOL_ICONS: [&str; 8] = [$($name),*];
        /// (nombre, base con `currentColor`, pixel art de 98, Luna de XP).
        static TOOL_SVGS: [(&str, &[u8], &[u8], &[u8]); 8] = [$((
            $name,
            asset!(concat!("icons/", $name, ".svg")),
            asset!(concat!("icons/win98/", $name, ".svg")),
            asset!(concat!("icons/xp/", $name, ".svg")),
        )),*];
    };
}

tool_icons![
    "connect",
    "copy",
    "favorite",
    "refresh",
    "refresh_one",
    "settings",
    "stop",
    "theme"
];

pub const ICON_SIZE: f32 = 20.0;
pub const WIN98_ICON_SIZE: f32 = 16.0;

/// SVG del icono `name` en el tema: 98 y Luna con sus variantes a color (sin teñir); el resto
/// teñido con `text` (o `muted` si está deshabilitado). `None` si no existe.
pub fn tool_icon_svg(name: &str, theme_id: &str, disabled: bool) -> Option<Cow<'static, [u8]>> {
    let &(_, base, win98, xp) = TOOL_SVGS.iter().find(|i| i.0 == name)?;
    match widget_style(theme_id) {
        WidgetStyle::Win98 => Some(Cow::Borrowed(win98)),
        WidgetStyle::Luna => Some(Cow::Borrowed(xp)),
        _ => {
            let p = palette(theme_id);
            let color = if disabled { p.muted } else { p.text };
            let svg = std::str::from_utf8(base).ok()?;
            Some(Cow::Owned(tinted(svg, color).into_bytes()))
        }
    }
}

pub fn tool_icon(name: &str, theme_id: &str, disabled: bool) -> Option<Handle> {
    tool_icon_svg(name, theme_id, disabled).map(Handle::from_memory)
}

/// Lado del icono: 98 a 16 px exactos (su pixel art no se escala a medias).
pub fn tool_icon_size(theme_id: &str) -> f32 {
    if widget_style(theme_id) == WidgetStyle::Win98 {
        WIN98_ICON_SIZE
    } else {
        ICON_SIZE
    }
}

pub fn tinted(svg: &str, color: Color) -> String {
    svg.replace("currentColor", &hex(color))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    Up,
    Down,
    Right,
}

/// Flecha (9×5, o 5×9 a la derecha) en `color`, igual que `_tinted_arrow` de `theme.py`.
pub fn arrow_svg(dir: Arrow, color: Color) -> String {
    let (w, h, d) = match dir {
        Arrow::Up => (9, 5, "M0 5L4.5 0L9 5Z"),
        Arrow::Down => (9, 5, "M0 0H9L4.5 5Z"),
        Arrow::Right => (5, 9, "M0 0L5 4.5L0 9Z"),
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\"><path d=\"{d}\" fill=\"{}\"/></svg>",
        hex(color)
    )
}

/// Flecha del orden del encabezado y su tamaño en el estilo.
/// Se guarda por (estilo, color, dirección): el encabezado la pide en cada cuadro.
pub fn sort_arrow(s: WidgetStyle, p: &Palette, ascending: bool) -> (Handle, Size) {
    type Key = (u8, [u32; 4], bool);
    static CACHE: LazyLock<Mutex<HashMap<Key, (Handle, Size)>>> = LazyLock::new(Mutex::default);
    let c = match s {
        WidgetStyle::Flat => p.accent,
        _ => p.text,
    };
    let key = (s as u8, [c.r, c.g, c.b, c.a].map(f32::to_bits), ascending);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(key)
        .or_insert_with(|| make_sort_arrow(s, p, ascending))
        .clone()
}

fn make_sort_arrow(s: WidgetStyle, p: &Palette, ascending: bool) -> (Handle, Size) {
    // Como la captura de 1.3.0: ascendente dibuja la flecha hacia arriba.
    let dir = if ascending { Arrow::Up } else { Arrow::Down };
    let tinted = |c| {
        let svg = arrow_svg(dir, c).into_bytes();
        (Handle::from_memory(svg), Size::new(9.0, 5.0))
    };
    let small = |svg| (Handle::from_memory(svg), Size::new(7.0, 4.0));
    match (s, ascending) {
        (WidgetStyle::Flat, _) => tinted(p.accent),
        (WidgetStyle::Vgui | WidgetStyle::Terminal, _) => tinted(p.text),
        (WidgetStyle::Win98, true) => small(asset!("win98/arrow_up.svg")),
        (WidgetStyle::Win98, false) => small(asset!("win98/arrow_down.svg")),
        (WidgetStyle::Luna, true) => small(asset!("luna/sort_up.svg")),
        (WidgetStyle::Luna, false) => small(asset!("luna/sort_down.svg")),
    }
}

/// RGBA (sin premultiplicar) del SVG escalado a `w`×`h`; `None` si no se puede leer.
/// Rasteriza en el momento: no llamar desde `view`.
pub fn render_rgba(svg: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|c| {
                let c = c.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect(),
    )
}

/// Icono de la ventana: `app_icon.svg` a 64 px (winit toma uno solo; la variante chica y el
/// `.ico` son del empaquetado).
pub fn app_icon() -> Option<window::Icon> {
    const SIDE: u32 = 64;
    let rgba = render_rgba(asset!("app_icon.svg"), SIDE, SIDE)?;
    window::icon::from_rgba(rgba, SIDE, SIDE).ok()
}

/// Insignia del juego: cuadrado redondeado `badge_<id>` con sus iniciales en blanco y negrita
/// (`badge_icon`); vacío del mismo tamaño si el juego no tiene color.
pub fn badge<'a, M: 'a>(game: &GameDef, p: &Palette, size: f32) -> Element<'a, M> {
    let Some(color) = p.badge(game.id) else {
        return space().width(size).height(size).into();
    };
    let bold = Font {
        weight: font::Weight::Bold,
        ..Font::DEFAULT
    };
    container(
        text(game.badge)
            .font(bold)
            .size((size * 0.55).round().max(1.0))
            .color(Color::WHITE),
    )
    .center(size)
    .style(move |_| container::Style {
        background: Some(color.into()),
        border: border::rounded(size * 0.22),
        ..container::Style::default()
    })
    .into()
}
