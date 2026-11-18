//! Temas de la app: paletas generadas desde `theme.py` y funciones de color.

mod edges;
#[rustfmt::skip]
mod palette;
pub mod style;

use iced::theme::Mode;
use iced::{Color, Font};
use jase_core::i18n::tr;

pub use edges::{Edges, framed};
pub use palette::{Palette, VARIANTS};

/// Estilo de los widgets de cada tema (`STYLE` de `theme.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetStyle {
    Flat,
    Win98,
    Luna,
    Vgui,
    Terminal,
}

impl WidgetStyle {
    /// `FONTS` de Python: Tahoma en los de época (sin ella, cosmic-text cae en la sans del
    /// sistema, como fontconfig con Qt) y monoespaciada en Terminal.
    pub fn font(self) -> Font {
        match self {
            Self::Win98 | Self::Luna | Self::Vgui => Font::with_name("Tahoma"),
            Self::Terminal => Font::MONOSPACE,
            Self::Flat => Font::DEFAULT,
        }
    }

    /// 8 pt a 96 ppp en los de época; el resto, el tamaño de siempre.
    pub fn text_size(self) -> f32 {
        match self {
            Self::Win98 | Self::Luna | Self::Vgui => 8.0 * 96.0 / 72.0,
            Self::Flat | Self::Terminal => 13.0,
        }
    }

    /// `radius` de `build_stylesheet`.
    pub fn radius(self) -> f32 {
        match self {
            Self::Flat => 6.0,
            Self::Luna => 3.0,
            Self::Win98 | Self::Vgui | Self::Terminal => 0.0,
        }
    }
}

/// Temas elegibles; Terminal se desdobla por color en `terminal_<color>`.
pub const THEME_IDS: [&str; 11] = [
    "dark",
    "light",
    "neon",
    "brown",
    "green",
    "classic",
    "xp",
    "terminal",
    "synthwave",
    "rust",
    "goldsrc",
];
pub const TERMINAL_COLORS: [&str; 4] = ["green", "white", "sepia", "amber"];
pub const PING_GOOD_MAX: u32 = 80;
pub const PING_MID_MAX: u32 = 150;
/// Alfa de la barra de ocupación de los temas planos.
pub const OCCUPANCY_ALPHA: f32 = 0.35;
/// Contraste mínimo del texto de color (ping, ^N) sobre la fila seleccionada.
pub const SELECTION_COLOR_MIN_CONTRAST: f32 = 3.5;

fn variant(id: &str) -> &'static (&'static str, Palette, WidgetStyle) {
    VARIANTS.iter().find(|v| v.0 == id).unwrap_or(&VARIANTS[0])
}

/// Paleta del id de variante; uno desconocido da `dark`.
pub fn palette(id: &str) -> &'static Palette {
    &variant(id).1
}

pub fn widget_style(id: &str) -> WidgetStyle {
    variant(id).2
}

/// Id concreto de tema: "system" (o un id inválido) sigue el modo del sistema y "terminal"
/// se vuelve "terminal_<color>" (verde si el color no es válido).
pub fn resolve_theme(pref: &str, mode: Mode, terminal_color: &str) -> &'static str {
    if pref == "terminal" {
        let color = TERMINAL_COLORS
            .iter()
            .find(|c| **c == terminal_color)
            .unwrap_or(&"green");
        return VARIANTS
            .iter()
            .find(|v| v.0.strip_prefix("terminal_") == Some(color))
            .map_or("dark", |v| v.0);
    }
    match VARIANTS.iter().find(|v| v.0 == pref) {
        Some(v) => v.0,
        None if mode == Mode::Light => "light",
        None => "dark",
    }
}

pub fn theme_display_name(id: &str) -> String {
    let id = if id.starts_with("terminal_") {
        "terminal"
    } else {
        id
    };
    match id {
        "system" => tr("Sistema"),
        "dark" => tr("Oscuro"),
        "light" => tr("Claro"),
        "neon" => tr("Neón"),
        "brown" => tr("Marrón"),
        "green" => tr("Verde"),
        "classic" => tr("Clásico"),
        "xp" => tr("Windows XP"),
        "terminal" => tr("Terminal"),
        "synthwave" => tr("Synthwave"),
        "rust" => tr("Óxido"),
        "goldsrc" => tr("GoldSrc"),
        other => other.to_owned(),
    }
}

pub fn terminal_color_name(color: &str) -> String {
    match color {
        "green" => tr("Verde"),
        "white" => tr("Blanco"),
        "sepia" => tr("Sepia"),
        "amber" => tr("Ámbar"),
        other => other.to_owned(),
    }
}

pub fn ping_color(ping_ms: u32, p: &Palette) -> Color {
    if ping_ms < PING_GOOD_MAX {
        p.ping_good
    } else if ping_ms < PING_MID_MAX {
        p.ping_mid
    } else {
        p.ping_bad
    }
}

fn channel8(v: f32) -> u8 {
    (v * 255.0).round() as u8
}

fn luminance(c: Color) -> f32 {
    let lin = |v: f32| {
        let v = f32::from(channel8(v)) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// Contraste WCAG entre dos colores (1.0 a 21.0).
pub fn contrast_ratio(fg: Color, bg: Color) -> f32 {
    let (a, b) = (luminance(fg), luminance(bg));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `color` si se lee sobre `bg` (contraste >= `SELECTION_COLOR_MIN_CONTRAST`); si no, `fallback`.
pub fn legible(color: Color, bg: Color, fallback: Color) -> Color {
    if contrast_ratio(color, bg) >= SELECTION_COLOR_MIN_CONTRAST {
        color
    } else {
        fallback
    }
}

/// `Theme` de Iced de una variante: fondo, texto, acento y los tres colores de ping.
pub fn iced_theme(id: &str) -> iced::Theme {
    let (name, p, _) = variant(id);
    iced::Theme::custom(
        *name,
        iced::theme::Palette {
            background: p.bg,
            text: p.text,
            primary: p.accent,
            success: p.ping_good,
            warning: p.ping_mid,
            danger: p.ping_bad,
        },
    )
}

/// "#rrggbb" del color.
pub fn hex(c: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        channel8(c.r),
        channel8(c.g),
        channel8(c.b)
    )
}
