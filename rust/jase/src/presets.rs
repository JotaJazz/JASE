//! Filtros guardados (`window_presets.py`): botón con el menú para aplicar, guardar y borrar
//! combinaciones de filtros rápidos y texto (sus entradas, en `actions::entries`); el nombre se
//! pide en un modal dibujado.

use iced::widget::{Id, Space, column, container, mouse_area, row, text};
use iced::{Element, Fill};
use jase_core::i18n::tr;

use crate::actions::{self, Menu};
use crate::app::{App, Message};
use crate::menu::{self, Placement};
use crate::theme::{framed, palette, style, widget_style};
use crate::widgets;

/// El campo del nombre: recibe el foco al abrir el modal.
pub const NAME_ID: Id = Id::new("preset-name");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetItem {
    Apply(String),
    Save,
    Delete(String),
}

#[derive(Debug, Clone)]
pub enum Msg {
    Pick(PresetItem),
    NameChanged(String),
    Submit,
    Cancel,
}

/// «Filtros guardados ▾» (`QToolButton` con `InstantPopup`): abre su menú debajo.
pub fn button(app: &App) -> Element<'_, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let (face, edges) = style::button(p, s, app.theme_id);
    let label = text(tr("Filtros guardados") + " ▾") // abre un menú
        .font(s.font())
        .size(s.text_size())
        .line_height(widgets::FIELD_LINE);
    let face = container(label)
        .padding(widgets::field_padding(s))
        .style(move |_| face);
    let face = match edges {
        Some(e) => framed(face, e),
        None => face.into(),
    };
    menu::popup(
        mouse_area(face)
            .on_press(Message::Actions(actions::Msg::Open(Menu::Presets)))
            .into(),
        actions::open_entries(app, Menu::Presets),
        Placement::Below,
        Message::Actions(actions::Msg::Close),
        app.theme_id,
    )
}

/// La tarjeta del modal (`QInputDialog.getText`): título, etiqueta, campo y botones.
pub fn dialog<'a>(app: &'a App, name: &'a str) -> Element<'a, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let label = |t: String| text(t).font(s.font()).size(s.text_size());
    let field = widgets::field(
        NAME_ID,
        "",
        name,
        |t| Message::Presets(Msg::NameChanged(t)),
        Some(Message::Presets(Msg::Submit)),
        true, // el foco es suyo desde que abre el modal
        app.theme_id,
    );
    let buttons = row![
        Space::new().width(Fill),
        widgets::button(tr("Guardar"), Message::Presets(Msg::Submit), app.theme_id),
        widgets::button(tr("Cancelar"), Message::Presets(Msg::Cancel), app.theme_id),
    ]
    .spacing(6);
    let card = container(
        column![
            label(tr("Guardar filtros actuales…")).font(style::header_font(s)),
            label(tr("Nombre de la combinación:")),
            field,
            buttons,
        ]
        .spacing(8),
    )
    .padding(12)
    .width(320)
    .style(move |_| style::dialog(p, s).0);
    match style::dialog(p, s).1 {
        Some(e) => framed(card, e),
        None => card.into(),
    }
}
