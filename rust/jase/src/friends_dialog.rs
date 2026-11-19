//! El diálogo *Amigos* (`friends_dialog.py`): la lista, el campo para agregar y la casilla de
//! la revisión de fondo, sin `App`. Guardar y avisar viven en `app/friends.rs`.

use iced::widget::scrollable::Direction;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Id, Space, column, container, mouse_area, row, scrollable, text};
use iced::{Element, Fill};
use jase_core::friends;
use jase_core::i18n::tr;
use jase_core::pystr::py_strip;
use serde_json::Value;

use crate::app::dialogs::card;
use crate::app::{FriendsMsg, Message};
use crate::card::one_line;
use crate::table::ROW_HEIGHT;
use crate::theme::{framed, palette, style, widget_style};
use crate::widgets;

/// El campo «Nombre del jugador» (toma el foco al abrir).
pub const FIELD_ID: Id = Id::new("friends-field");
/// `resize(360, 420)`.
const SIZE: (f32, f32) = (360.0, 420.0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendsForm {
    pub names: Vec<String>,
    pub selected: Option<usize>,
    pub text: String,
    pub watch: bool,
}

impl FriendsForm {
    pub fn new(current: &[String], watch: bool) -> Self {
        FriendsForm {
            names: current.to_vec(),
            selected: None,
            text: String::new(),
            watch,
        }
    }

    /// `_add`: `parse` deduplica y recorta, así que la lista se rehace con su resultado.
    pub fn add(&mut self) {
        let name = py_strip(&self.text).to_owned();
        self.text.clear();
        if !name.is_empty() {
            self.names.push(name);
            self.names = self.names();
            self.selected = None;
        }
    }

    pub fn remove_selected(&mut self) {
        if let Some(i) = self.selected.take().filter(|i| *i < self.names.len()) {
            self.names.remove(i);
        }
    }

    pub fn names(&self) -> Vec<String> {
        friends::parse(&Value::from(self.names.clone()))
    }
}

/// Lista elegible con clic, campo + *Agregar*, *Quitar*, la casilla y *Guardar*/*Cancelar*
/// (Qt pone *OK*, que no está en los catálogos).
pub fn view<'a>(form: &'a FriendsForm, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let m = Message::Friends;
    let label = |t: String| text(t).font(s.font()).size(s.text_size());
    let (_, chosen) = style::selection_colors(p, s, true);
    // Texto plano: los nombres pueden venir de un servidor (clic derecho en un jugador).
    let items = Column::with_children(form.names.iter().enumerate().map(|(i, n)| {
        let selected = form.selected == Some(i);
        let name = text(one_line(n))
            .color(if selected { chosen } else { p.text })
            .font(s.font())
            .size(s.text_size())
            .wrapping(Wrapping::None);
        let line = container(name)
            .height(ROW_HEIGHT)
            .width(Fill)
            .padding([0, 4])
            .center_y(ROW_HEIGHT)
            .clip(true)
            .style(move |_| style::table_row(p, s, selected, true));
        mouse_area(line).on_press(m(FriendsMsg::Select(i))).into()
    }));
    let list = scrollable(items)
        .direction(Direction::Vertical(style::scrollbar(s)))
        .style(move |_, status| style::scrollable(p, s, status))
        .height(Fill);
    let (frame, edges) = style::table_frame(p, s);
    let list = container(list)
        .padding(frame.border.width)
        .width(Fill)
        .height(Fill)
        .style(move |_| frame);
    let list: Element<'a, Message> = match edges {
        Some(e) => framed(list, e),
        None => list.into(),
    };
    let field = widgets::field(
        FIELD_ID,
        &tr("Nombre del jugador"),
        &form.text,
        move |t| m(FriendsMsg::Text(t)),
        Some(m(FriendsMsg::Add)),
        true,
        theme_id,
    );
    let add = row![
        container(field).width(Fill),
        widgets::button(tr("Agregar"), m(FriendsMsg::Add), theme_id),
    ]
    .spacing(6);
    let remove = row![widgets::button(
        tr("Quitar"),
        m(FriendsMsg::Remove),
        theme_id
    )];
    let watch_text =
        tr("Revisar favoritos y recientes cada 2 min, también con la ventana en la bandeja");
    // `QCheckBox` no corta su texto: el diálogo se ensancha hasta que entre.
    let width = SIZE
        .0
        .max(widgets::text_width(&watch_text, s.font(), s.text_size()) + 48.0);
    let watch = widgets::check(
        form.watch,
        Some(label(watch_text).wrapping(Wrapping::None).into()),
        Some(m(FriendsMsg::Watch(!form.watch))),
        p,
        s,
    );
    let buttons = row![
        Space::new().width(Fill),
        widgets::button(tr("Guardar"), m(FriendsMsg::Accept), theme_id),
        widgets::button(tr("Cancelar"), m(FriendsMsg::Cancel), theme_id),
    ]
    .spacing(6);
    let content = column![
        label(tr("Amigos")).font(style::header_font(s)),
        list,
        add,
        remove,
        watch,
        buttons,
    ]
    .height(SIZE.1 - 24.0); // menos el relleno de la tarjeta
    card(content, width, theme_id)
}
