//! Bordes que Iced no dibuja (un color por lado, una sola línea): franjas de color sobre
//! el contenido en un `stack`. Las esquinas no van en diagonal como en Qt.

use iced::widget::{Space, Stack, column, container, row};
use iced::{Color, Element, Fill, Padding};

use super::Palette;

/// Grosor y color de cada lado; `None` = sin borde.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    pub top: Option<(f32, Color)>,
    pub right: Option<(f32, Color)>,
    pub bottom: Option<(f32, Color)>,
    pub left: Option<(f32, Color)>,
}

impl Edges {
    fn bevel(w: f32, top_left: Color, bottom_right: Color) -> Self {
        Self {
            top: Some((w, top_left)),
            right: Some((w, bottom_right)),
            bottom: Some((w, bottom_right)),
            left: Some((w, top_left)),
        }
    }

    /// `_raised` de 98: 2 px, luz arriba/izquierda.
    pub fn raised(p: &Palette) -> Self {
        Self::bevel(2.0, p.bevel_light, p.bevel_dark)
    }

    pub fn sunken(p: &Palette) -> Self {
        Self::bevel(2.0, p.bevel_dark, p.bevel_light)
    }

    /// Hundido sobre fondo blanco: abajo/derecha con la luz interior, que no se pierde.
    pub fn sunken_on_surface(p: &Palette) -> Self {
        Self::bevel(2.0, p.bevel_dark, p.bevel_inner)
    }

    /// `_thin`: 1 px de las barras planas de 98.
    pub fn thin(p: &Palette, sunken: bool) -> Self {
        let (light, dark) = (p.bevel_light, p.border);
        if sunken {
            Self::bevel(1.0, dark, light)
        } else {
            Self::bevel(1.0, light, dark)
        }
    }

    /// `_vgui_bevel`: 1 px de GoldSrc.
    pub fn vgui(p: &Palette, sunken: bool) -> Self {
        let (light, dark) = (p.bevel_light, p.bevel_dark);
        if sunken {
            Self::bevel(1.0, dark, light)
        } else {
            Self::bevel(1.0, light, dark)
        }
    }

    pub fn bottom(w: f32, c: Color) -> Self {
        Self {
            bottom: Some((w, c)),
            ..Self::default()
        }
    }

    /// Encabezado de Luna: línea fina a la derecha y gruesa abajo.
    pub fn right_bottom(right: f32, bottom: f32, c: Color) -> Self {
        Self {
            right: Some((right, c)),
            bottom: Some((bottom, c)),
            ..Self::default()
        }
    }
}

fn strip<'a, M: 'a>(side: Option<(f32, Color)>, horizontal: bool) -> Element<'a, M> {
    let Some((w, c)) = side else {
        return Space::new().into();
    };
    let (width, height) = if horizontal {
        (Fill, w.into())
    } else {
        (w.into(), Fill)
    };
    container(Space::new())
        .width(width)
        .height(height)
        .style(move |_| container::Style {
            background: Some(c.into()),
            ..container::Style::default()
        })
        .into()
}

/// `content` con relleno igual al grosor de cada lado y las franjas encima.
pub fn framed<'a, M: 'a>(content: impl Into<Element<'a, M>>, e: Edges) -> Element<'a, M> {
    let w = |side: Option<(f32, Color)>| side.map_or(0.0, |(w, _)| w);
    let content: Element<'a, M> = container(content)
        .padding(Padding {
            top: w(e.top),
            right: w(e.right),
            bottom: w(e.bottom),
            left: w(e.left),
        })
        .into();
    // El tamaño lo da el contenido: las franjas (Fill) no deben agrandar el `stack`.
    let hint = content.as_widget().size_hint();
    let overlay = column![
        strip(e.top, true),
        row![
            strip(e.left, false),
            Space::new().width(Fill),
            strip(e.right, false)
        ]
        .height(Fill),
        strip(e.bottom, true),
    ];
    Stack::from_vec(vec![content, overlay.into()])
        .width(hint.width)
        .height(hint.height)
        .into()
}
