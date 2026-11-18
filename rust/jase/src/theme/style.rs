//! Estilos de contenedor de la app (estilo plano en todos los temas, por ahora).

use iced::widget::container::Style;
use iced::{Background, Border};

use super::Palette;

fn base(p: &Palette, bg: iced::Color) -> Style {
    Style {
        background: Some(Background::Color(bg)),
        text_color: Some(p.text),
        ..Style::default()
    }
}

pub fn window(p: &Palette) -> Style {
    base(p, p.bg)
}

pub fn sidebar(p: &Palette) -> Style {
    base(p, p.sidebar)
}

pub fn table_header(p: &Palette) -> Style {
    Style {
        text_color: Some(p.muted),
        border: Border {
            color: p.divider,
            width: 1.0,
            ..Border::default()
        },
        ..base(p, p.surface)
    }
}

pub fn table_row(p: &Palette, selected: bool) -> Style {
    if selected {
        Style {
            text_color: Some(p.selection_text),
            border: Border {
                color: p.selection_outline,
                width: 1.0,
                ..Border::default()
            },
            ..base(p, p.selection)
        }
    } else {
        base(p, p.surface)
    }
}

pub fn status_bar(p: &Palette) -> Style {
    Style {
        text_color: Some(p.muted),
        ..base(p, p.sidebar)
    }
}

pub fn tooltip(p: &Palette) -> Style {
    Style {
        border: Border {
            color: p.border,
            width: 1.0,
            ..Border::default()
        },
        ..base(p, p.tooltip)
    }
}
