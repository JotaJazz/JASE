//! Filtros guardados (`window_presets.py`): botón para aplicar, guardar y borrar combinaciones
//! de filtros rápidos y texto. Sin el menú de la 5d: un `pick_list` plano (combinaciones,
//! «Guardar filtros actuales…» y «Borrar › nombre») y el nombre se pide en un modal dibujado.

use std::fmt;

use iced::widget::{Id, Space, column, container, pick_list, row, text};
use iced::{Element, Fill};
use jase_core::filter_presets::FilterPreset;
use jase_core::i18n::tr;

use crate::app::{App, Message};
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

impl fmt::Display for PresetItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetItem::Apply(name) => f.write_str(name),
            PresetItem::Save => f.write_str(&tr("Guardar filtros actuales…")),
            PresetItem::Delete(name) => write!(f, "{} › {name}", tr("Borrar")),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Pick(PresetItem),
    NameChanged(String),
    Submit,
    Cancel,
}

/// Las combinaciones, «Guardar…» y un «Borrar › nombre» por cada una (`_rebuild_presets_menu`).
pub fn preset_items(presets: &[FilterPreset]) -> Vec<PresetItem> {
    let names = || presets.iter().map(|p| p.name.clone());
    names()
        .map(PresetItem::Apply)
        .chain([PresetItem::Save])
        .chain(names().map(PresetItem::Delete))
        .collect()
}

/// «Filtros guardados ▾»: un `pick_list` sin selección que nunca muestra lo elegido.
pub fn button(app: &App) -> Element<'_, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let theme_id = app.theme_id;
    // Un botón (`QToolButton`), no un combo: su cara y su texto sin atenuar.
    let (face, edges) = style::button(p, s, theme_id);
    let pick = pick_list(
        preset_items(app.filter_presets()),
        None::<PresetItem>,
        |i| Message::Presets(Msg::Pick(i)),
    )
    .placeholder(tr("Filtros guardados") + " ▾") // abre un menú
    .handle(pick_list::Handle::None)
    .font(s.font())
    .text_size(s.text_size())
    .text_line_height(widgets::FIELD_LINE)
    .padding(widgets::field_padding(s))
    .style(move |_, status| pick_list::Style {
        placeholder_color: p.text,
        background: face.background.unwrap_or(p.surface.into()),
        border: face.border,
        ..style::pick_list(p, s, theme_id, status)
    })
    .menu_style(move |_| style::menu(p, s));
    match edges {
        Some(e) => framed(pick, e),
        None => pick.into(),
    }
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
        false,
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
