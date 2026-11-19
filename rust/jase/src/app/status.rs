//! La barra de estado (mensajes con plazo, como `showMessage(texto, ms)`) y los atajos de la
//! ventana.

use std::time::Duration;

use iced::Task;
use iced::widget::operation;
use jase_core::i18n::{tr, tr_n};

use super::{App, DirectView, Focus, Message};
use crate::actions::{self, Action};
use crate::bars::Shortcut;
use crate::widgets::FILTER_ID;

impl App {
    /// Un mensaje sin plazo: reemplaza al anterior y anula su plazo.
    pub(super) fn set_status(&mut self, text: String) {
        self.status = text;
        self.status_seq += 1;
    }

    /// `showMessage(texto, ms)`: con plazo, la `Task` avisa al vencer y solo borra si sigue
    /// siendo este mensaje.
    pub fn show_message(&mut self, text: String, timeout: Option<Duration>) -> Task<Message> {
        self.set_status(text);
        let seq = self.status_seq;
        match timeout {
            // `async` para que el temporizador se cree ya en el runtime, no aquí.
            Some(d) => Task::perform(async move { tokio::time::sleep(d).await }, move |()| {
                Message::StatusExpired(seq)
            }),
            None => Task::none(),
        }
    }

    pub(super) fn on_status_expired(&mut self, seq: u64) {
        if seq == self.status_seq {
            self.status.clear();
        }
    }

    pub fn status_seq(&self) -> u64 {
        self.status_seq
    }

    pub fn count_text(&self) -> String {
        let n = tr_n(
            "{n} servidor",
            "{n} servidores",
            self.table.model.order().len() as i64,
            &[],
        );
        let marked = self
            .current
            .is_some_and(|g| !self.tree.marks(g.id).is_empty());
        match self.direct_view() {
            _ if !marked => n,
            None => n + &tr(" · filtro de mod/mapa/país activo"),
            Some(DirectView::Favorites) => {
                n + &tr(" · filtro de mod/mapa/país (no aplica en Favoritos)")
            }
            Some(DirectView::Recent) => {
                n + &tr(" · filtro de mod/mapa/país (no aplica en Recientes)")
            }
        }
    }

    /// `scanning_label`: «Escaneando…» mientras dura el escaneo, o la consulta de la vista
    /// directa.
    pub fn scanning_text(&self) -> String {
        match self.direct_view() {
            _ if self.is_scanning() => tr("Escaneando…"),
            Some(DirectView::Favorites) if self.is_querying_direct() => {
                tr("Consultando favoritos…")
            }
            Some(DirectView::Recent) if self.is_querying_direct() => tr("Consultando recientes…"),
            _ => String::new(),
        }
    }

    /// Nada con un diálogo, un menú o la lista de un combo abiertos (se quedan con el teclado,
    /// como en Qt). `Ctrl+C` solo copia y `Ctrl+V` solo pega con la tabla enfocada, y las acciones apagadas ignoran
    /// su atajo en `on_action`.
    pub(super) fn on_shortcut(&mut self, s: Shortcut) -> Task<Message> {
        if self.dialog.is_some() || self.menu.is_some() || self.combo_open.is_some() {
            return Task::none();
        }
        let action = match s {
            Shortcut::F5 => Action::Refresh,
            Shortcut::Lan => Action::ScanLan,
            Shortcut::Quit => Action::Quit,
            Shortcut::Copy if self.focus == Focus::Table => Action::Copy,
            Shortcut::Copy => return Task::none(),
            Shortcut::AddServer => Action::AddServer,
            Shortcut::FindPlayer => Action::FindPlayer,
            Shortcut::NowPlaying => Action::NowPlaying,
            Shortcut::Rcon => Action::Rcon,
            // Con el campo del filtro enfocado, pega ahí (lo hace el `text_input`).
            Shortcut::Paste if self.focus == Focus::Table => return self.paste_servers(),
            Shortcut::Paste => return Task::none(),
            // `_focus_filter`: el foco y todo el texto elegido.
            Shortcut::Find => {
                self.set_focus(Focus::Filter);
                return Task::batch([
                    operation::focus(FILTER_ID),
                    operation::select_all(FILTER_ID),
                ]);
            }
        };
        self.on_action(actions::Msg::Run(action))
    }
}
