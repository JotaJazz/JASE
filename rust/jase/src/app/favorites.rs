//! Estrella, notas, recientes y grupos de favoritos (`window_favorites.py`, `window_groups.py`):
//! solo tocan la config (pequeña: se guarda en el hilo de la UI).

use std::fmt;
use std::time::Duration;

use iced::Task;
use iced::widget::operation;
use jase_core::favorites::{
    add_favorite, clean_group, game_notes, get_group, get_note, group_names, is_favorite,
    remove_favorite, set_group, set_note,
};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::recent::remove_recent;

use super::{App, Dialog, DirectView, Message, PROMPT_ID};
use crate::table;

/// Lo que muestran los mensajes de favoritos en la barra de estado (4 s, como Python).
const NOTICE: Duration = Duration::from_secs(4);

/// Una opción del combo de grupo: `None` = todos, `Some("")` = sin grupo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupOption {
    pub key: Option<String>,
    pub label: String,
}

impl fmt::Display for GroupOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

impl App {
    /// `_is_selected_favorite`.
    pub fn is_selected_favorite(&self) -> bool {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return false;
        };
        is_favorite(&self.cfg, g.id, &info.address)
    }

    /// Agrega o quita el elegido (por su dirección de consulta); en Favoritos, quitar borra la
    /// fila. Nunca consulta.
    pub fn toggle_favorite_selected(&mut self) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        let address = info.address.clone();
        let shown = [("address", &address as &dyn fmt::Display)];
        let text = if remove_favorite(&mut self.cfg, g.id, &address) {
            self.sync_notes();
            if self.direct_view() == Some(DirectView::Favorites) {
                self.table.model.remove(&address);
                table::drop_hidden_selection(&mut self.table);
            }
            tr_fmt("Quitado de favoritos: {address}", &shown)
        } else {
            add_favorite(&mut self.cfg, g.id, &address);
            tr_fmt("Agregado a favoritos: {address}", &shown)
        };
        self.save_config();
        let groups = self.sync_groups();
        Task::batch([groups, self.show_message(text, Some(NOTICE))])
    }

    /// Pasa a la tabla las notas del juego actual (`_sync_notes`).
    pub fn sync_notes(&mut self) {
        let notes = self
            .current
            .map(|g| game_notes(&self.cfg, g.id))
            .unwrap_or_default();
        self.table.model.set_notes(notes);
    }

    /// Abre la nota del favorito elegido (*Guardar* vacía la borra).
    pub fn edit_note_selected(&mut self) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        if !is_favorite(&self.cfg, g.id, &info.address) {
            return Task::none();
        }
        let address = info.address.clone();
        let text = get_note(&self.cfg, g.id, &address.to_string()).to_owned();
        self.open_dialog(Dialog::Note {
            game: g.id,
            address,
            text,
        });
        operation::focus(PROMPT_ID)
    }

    /// Quita el elegido de los recientes (solo en esa vista).
    pub fn remove_recent_selected(&mut self) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        if self.direct_view() != Some(DirectView::Recent) {
            return Task::none();
        }
        let address = info.address.clone();
        if !remove_recent(&mut self.cfg, g.id, &address) {
            return Task::none();
        }
        self.table.model.remove(&address);
        table::drop_hidden_selection(&mut self.table);
        self.save_config();
        let text = tr_fmt("Quitado de recientes: {address}", &[("address", &address)]);
        self.show_message(text, Some(NOTICE))
    }

    /// El grupo elegido en la vista Favoritos; `None` = todos, `""` = sin grupo.
    pub fn current_group(&self) -> Option<&str> {
        match self.direct_view() {
            Some(DirectView::Favorites) => self.group.as_deref(),
            _ => None,
        }
    }

    /// Las opciones del combo de grupo; vacío = oculto (solo se ve en Favoritos con grupos).
    pub fn group_options(&self) -> Vec<GroupOption> {
        let Some(g) = self.current else {
            return Vec::new();
        };
        let names = group_names(&self.cfg, g.id);
        if self.direct_view() != Some(DirectView::Favorites) || names.is_empty() {
            return Vec::new();
        }
        let all = GroupOption {
            key: None,
            label: tr("Todos los grupos"),
        };
        let none = GroupOption {
            key: Some(String::new()),
            label: tr("Sin grupo"),
        };
        let named = names.into_iter().map(|n| GroupOption {
            key: Some(n.clone()),
            label: n,
        });
        std::iter::once(all).chain(named).chain([none]).collect()
    }

    /// `_sync_group_combo`: el grupo elegido que ya no existe vuelve a «Todos» y, en
    /// Favoritos, se reconsulta.
    pub fn sync_groups(&mut self) -> Task<Message> {
        let names = self.group_names();
        match &self.group {
            Some(name) if !name.is_empty() && !names.contains(name) => {
                self.group = None;
                if self.direct_view() == Some(DirectView::Favorites) {
                    return self.query_direct_list();
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Elegido en el combo de la barra: en Favoritos vuelve a consultar.
    pub(super) fn pick_group(&mut self, group: Option<String>) -> Task<Message> {
        self.combo_open = None;
        self.group = group;
        if self.direct_view() == Some(DirectView::Favorites) {
            return self.query_direct_list();
        }
        Task::none()
    }

    /// Mueve el favorito elegido al grupo `name` (vacío = sin grupo); si se ve otro grupo, su
    /// fila sale.
    pub fn move_selected_to_group(&mut self, name: &str) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        if !is_favorite(&self.cfg, g.id, &info.address) {
            return Task::none();
        }
        let (address, server) = (info.address.clone(), info.name.clone());
        set_group(&mut self.cfg, g.id, &address.to_string(), name);
        self.save_config();
        let clean = clean_group(name);
        if self.current_group().is_some_and(|shown| shown != clean) {
            self.table.model.remove(&address);
            table::drop_hidden_selection(&mut self.table);
        }
        let groups = self.sync_groups();
        let text = if clean.is_empty() {
            tr_fmt("{server} quedó sin grupo", &[("server", &server)])
        } else {
            tr_fmt(
                "{server} pasó al grupo «{group}»",
                &[("server", &server), ("group", &clean)],
            )
        };
        Task::batch([groups, self.show_message(text, Some(NOTICE))])
    }

    /// «Nuevo grupo…» del submenú *Grupo*.
    pub(super) fn new_group_for_selected(&mut self) -> Task<Message> {
        if !self.is_selected_favorite() {
            return Task::none();
        }
        self.open_dialog(Dialog::NewGroup(String::new()));
        operation::focus(PROMPT_ID)
    }

    /// *Guardar* de la nota o del grupo nuevo (el diálogo ya cerrado).
    pub(super) fn submit_text_dialog(&mut self, dialog: Dialog) -> Task<Message> {
        match dialog {
            Dialog::Note {
                game,
                address,
                text,
            } => {
                set_note(&mut self.cfg, game, &address.to_string(), &text);
                self.save_config();
                self.sync_notes();
                Task::none()
            }
            Dialog::NewGroup(name) if !clean_group(&name).is_empty() => {
                self.move_selected_to_group(&name)
            }
            _ => Task::none(),
        }
    }

    /// El grupo del favorito elegido (`""` = sin grupo), si lo es.
    pub fn selected_group(&self) -> Option<&str> {
        let (g, info) = (self.current?, self.selected_server()?);
        is_favorite(&self.cfg, g.id, &info.address)
            .then(|| get_group(&self.cfg, g.id, &info.address.to_string()))
    }

    pub fn group_names(&self) -> Vec<String> {
        self.current
            .map(|g| group_names(&self.cfg, g.id))
            .unwrap_or_default()
    }
}
