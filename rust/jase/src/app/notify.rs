//! Avisos al usuario (`notify`/`notify_prompt` de `window_tray.py`): con la ventana en la
//! bandeja, un globo (`app/tray.rs`); si no, una alerta a la principal y la barra de estado.
//! La pregunta («Hay sitio») va ya, o a la barra si hay otro diálogo abierto (no pisa lo que
//! el usuario edita).

use iced::widget::{Space, column, row, text};
use iced::window::{self, UserAttention};
use iced::{Element, Fill, Task};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::models::ServerAddress;

use super::dialogs::{PromptMsg, card};
use super::{App, Dialog, Message};
use crate::theme::{style, widget_style};
use crate::widgets;

/// Lo que hace el clic en un globo de la bandeja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    Restore,
    Reveal(&'static str, ServerAddress),
}

/// Una pregunta que espera a la ventana; sus datos viven en el `App` (`watch.free`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    SlotFree,
}

impl App {
    fn alert(&self) -> Task<Message> {
        window::request_user_attention(self.windows.main, Some(UserAttention::Informational))
    }

    /// En la bandeja, un globo cuyo clic hace `click`; si no, alerta y barra sin plazo.
    pub fn notify(&mut self, title: &str, text: &str, click: Option<Click>) -> Task<Message> {
        if self.in_tray() {
            return self.tray_notify(title, text, click);
        }
        let status = self.show_message(format!("{title}: {text}"), None);
        Task::batch([self.alert(), status])
    }

    /// En la bandeja, la pregunta espera a la ventana; si no, alerta y la pregunta ya.
    pub fn notify_prompt(&mut self, title: &str, text: &str, prompt: Prompt) -> Task<Message> {
        if self.in_tray() {
            return self.tray_prompt(title, text, prompt);
        }
        Task::batch([self.alert(), self.run_prompt(prompt)])
    }

    pub fn run_click(&mut self, c: Click) -> Task<Message> {
        match c {
            Click::Restore => Task::none(),
            Click::Reveal(game, address) => self.reveal_server(game, address),
        }
    }

    pub fn run_prompt(&mut self, p: Prompt) -> Task<Message> {
        match p {
            Prompt::SlotFree => {
                let Some((_, info)) = &self.watch.free else {
                    return Task::none();
                };
                let text = slot_free_text(info);
                if self.dialog.is_some() {
                    self.watch.free = None;
                    return self.show_message(format!("{}: {text}", tr("Hay sitio")), None);
                }
                self.open_dialog(Dialog::SlotFree { text });
                Task::none()
            }
        }
    }

    /// «Hay sitio»: Enter = *Conectar* (el `Yes` por defecto); Esc = *Cancelar*.
    pub(super) fn on_slot_prompt(&mut self, m: PromptMsg) -> Task<Message> {
        let free = self.watch.free.take();
        let close = self.close_dialog();
        match (m, free) {
            (PromptMsg::Submit, Some((game, info))) => {
                Task::batch([close, self.connect_to(game, info)])
            }
            _ => close,
        }
    }

    pub(super) fn slot_free_view<'a>(&self, body: &'a str) -> Element<'a, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: &'a str| text(t).font(s.font()).size(s.text_size());
        let prompt = Message::Prompt;
        let buttons = row![
            Space::new().width(Fill),
            widgets::button(tr("Conectar"), prompt(PromptMsg::Submit), theme_id),
            widgets::button(tr("Cancelar"), prompt(PromptMsg::Cancel), theme_id),
        ]
        .spacing(6);
        let title = text(tr("Hay sitio"))
            .font(style::header_font(s))
            .size(s.text_size());
        card(column![title, label(body), buttons], 360.0, theme_id)
    }
}

pub(super) fn slot_free_text(info: &jase_core::models::ServerInfo) -> String {
    tr_fmt(
        "Hay sitio en {name} ({players}/{max}). ¿Conectar ahora?",
        &[
            ("name", &info.name),
            ("players", &info.num_players),
            ("max", &info.max_players),
        ],
    )
}
