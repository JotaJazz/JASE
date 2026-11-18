//! La barra de estado (mensajes con plazo, como `showMessage(texto, ms)`) y los atajos de la
//! ventana.

use std::time::Duration;

use iced::Task;
use iced::widget::operation;
use jase_core::i18n::tr;

use super::{App, Focus, Message};
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

    /// `scanning_label`: «Escaneando…» mientras dura el escaneo.
    pub fn scanning_text(&self) -> String {
        if self.is_scanning() {
            tr("Escaneando…")
        } else {
            String::new()
        }
    }

    /// Nada con un diálogo, un menú o la lista de un combo abiertos (se quedan con el teclado,
    /// como en Qt); `Ctrl+C` solo con la
    /// tabla enfocada; las acciones apagadas los ignoran en `on_action`.
    pub(super) fn on_shortcut(&mut self, s: Shortcut) -> Task<Message> {
        if self.dialog.is_some() || self.menu.is_some() || self.combo_open {
            return Task::none();
        }
        let action = match s {
            Shortcut::F5 => Action::Refresh,
            Shortcut::Lan => Action::ScanLan,
            Shortcut::Quit => Action::Quit,
            Shortcut::Copy if self.focus == Focus::Table => Action::Copy,
            Shortcut::Copy => return Task::none(),
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
