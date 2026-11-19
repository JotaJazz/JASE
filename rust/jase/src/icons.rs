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

/// SVG del icono `name` en el tema: 98 y Luna con sus variantes a color (sin teñir; apagadas,
/// en gris como las genera Qt); el resto teñido con `text` (o `muted` si está deshabilitado).
/// `None` si no existe.
pub fn tool_icon_svg(name: &str, theme_id: &str, disabled: bool) -> Option<Cow<'static, [u8]>> {
    let &(_, base, win98, xp) = TOOL_SVGS.iter().find(|i| i.0 == name)?;
    match widget_style(theme_id) {
        WidgetStyle::Win98 | WidgetStyle::Luna => {
            let color = if widget_style(theme_id) == WidgetStyle::Win98 {
                win98
            } else {
                xp
            };
            if disabled {
                grayed(color).map(|s| Cow::Owned(s.into_bytes()))
            } else {
                Some(Cow::Borrowed(color))
            }
        }
        _ => {
            let p = palette(theme_id);
            let color = if disabled { p.muted } else { p.text };
            let svg = std::str::from_utf8(base).ok()?;
            Some(Cow::Owned(tinted(svg, color).into_bytes()))
        }
    }
}

/// El icono apagado que genera Qt (`QCommonStyle::generatedIconPixmap`, `QIcon::Disabled`):
/// el gris de cada píxel (`qGray`) elige un color de una escala negro → fondo → blanco, corrida
/// según la intensidad del fondo. Va como filtro SVG (gris y tabla por canal). El fondo es el
/// `Window` de la paleta de la app, que JASE no tiñe con el tema (medido offscreen: `#efefef`).
fn grayed(svg: &[u8]) -> Option<String> {
    let svg = std::str::from_utf8(svg).ok()?;
    let open = svg.find("<svg")?;
    let body = open + svg[open..].find('>')? + 1;
    let end = svg.rfind("</svg>")?;
    let [r, g, b] = [0xef; 3];
    let mut intensity = (77 * r + 150 * g + 28 * b) / 255; // `qt_intensity`
    let f = 191;
    if (r - f > g && r - f > b) || (g - f > r && g - f > b) || (b - f > r && b - f > g) {
        intensity = (intensity + 91).min(255);
    } else if intensity <= 128 {
        intensity -= 51;
    }
    // Tabla de 16 tramos sobre el gris 0..=255 (el filtro interpola entre puntos).
    let table = |c: i32| {
        (0..=255)
            .step_by(17)
            .map(|gray| {
                let ci = (gray / 3 + 130 - intensity / 3).clamp(0, 255);
                let v = if ci < 128 {
                    (c * (ci << 1)) >> 8
                } else {
                    (c + ((ci - 128) << 1)).min(255)
                };
                format!("{:.4}", v as f32 / 255.0)
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    // `qGray` = (11 r + 16 g + 5 b) / 32, en sRGB como Qt (no en el lineal por defecto del SVG).
    let filter = format!(
        "<filter id=\"qt-disabled\" color-interpolation-filters=\"sRGB\">\
         <feColorMatrix type=\"matrix\" values=\"{m} {m} {m} 0 0 0 1 0\"/>\
         <feComponentTransfer><feFuncR type=\"table\" tableValues=\"{}\"/>\
         <feFuncG type=\"table\" tableValues=\"{}\"/>\
         <feFuncB type=\"table\" tableValues=\"{}\"/></feComponentTransfer></filter>",
        table(r),
        table(g),
        table(b),
        m = "0.34375 0.5 0.15625 0 0",
    );
    Some(format!(
        "{}{filter}<g filter=\"url(#qt-disabled)\">{}</g>{}",
        &svg[..body],
        &svg[body..end],
        &svg[end..]
    ))
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
    window::icon::from_rgba(app_icon_rgba(SIDE)?, SIDE, SIDE).ok()
}

/// `app_icon.svg` en RGBA de `side` × `side` (la ventana y el icono de la bandeja).
pub fn app_icon_rgba(side: u32) -> Option<Vec<u8>> {
    render_rgba(asset!("app_icon.svg"), side, side)
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
