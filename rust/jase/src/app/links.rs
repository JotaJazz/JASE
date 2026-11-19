//! Abrir servidores desde enlaces `jase://` (`window_links.py`): siempre con confirmación, en
//! texto plano; uno a la vez (solo el último espera) y nunca debajo ni encima de otro diálogo.

use std::time::Duration;

use iced::widget::{Space, column, row, text};
use iced::{Element, Fill, Task, window};
use jase_core::games::{GameDef, game};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::jase_url;
use jase_core::models::ServerInfo;
use jase_core::protocols::quake3::strip_colors;

use super::dialogs::card;
use super::{App, Dialog, Message};
use crate::single_instance::SHOW_COMMAND;
use crate::theme::{style, widget_style};
use crate::widgets;

/// Con un diálogo abierto, el enlace espera y se reintenta.
pub const LINK_RETRY: Duration = Duration::from_millis(500);
/// El aviso de un enlace inválido que llegó por el socket.
const INVALID: Duration = Duration::from_secs(6);

#[allow(clippy::large_enum_variant)] // por `ServerInfo`, como `Message::Query`
#[derive(Debug, Clone)]
pub enum LinkMsg {
    /// Una línea del socket: `show` o un enlace.
    Received(String),
    /// `interactive`: el enlace inválido se avisa con un diálogo (el de `argv`).
    Open {
        url: String,
        interactive: bool,
    },
    Retry,
    /// La respuesta del pedido `request`; la de otro se descarta.
    Found {
        request: u64,
        game: &'static str,
        result: Result<Option<ServerInfo>, String>,
    },
    /// *Conectar* (`true`) o *Cancelar*, Enter y Esc.
    Confirm(bool),
}

#[derive(Default)]
pub(super) struct Links {
    /// Consulta o confirmación de un enlace en curso.
    busy: bool,
    /// El enlace en curso (vuelve a esperar si su respuesta llega con otro diálogo abierto).
    current: Option<(String, bool)>,
    /// Solo el último enlace espera.
    pending: Option<(String, bool)>,
    /// Un reintento en marcha (uno solo, como el `QTimer` de Python).
    retrying: bool,
    request: u64,
    confirm: Option<(&'static GameDef, ServerInfo)>,
}

/// Trae la principal al frente (sin bandeja hasta la 5i).
fn raise_window(main: window::Id) -> Task<Message> {
    Task::batch([window::minimize(main, false), window::gain_focus(main)])
}

impl App {
    /// El enlace de `argv`, abierto tras el juego inicial (también uno inválido, que avisa).
    pub fn link_at_start(link: Option<String>) -> Task<Message> {
        link.map_or_else(Task::none, |url| {
            Task::done(Message::Link(LinkMsg::Open {
                url,
                interactive: true,
            }))
        })
    }

    pub fn link_request(&self) -> u64 {
        self.links.request
    }

    pub fn link_busy(&self) -> bool {
        self.links.busy
    }

    pub(super) fn on_link(&mut self, m: LinkMsg) -> Task<Message> {
        match m {
            LinkMsg::Received(line) if line == SHOW_COMMAND => raise_window(self.windows.main),
            LinkMsg::Received(url) => self.open_link(url, false),
            LinkMsg::Open { url, interactive } => self.open_link(url, interactive),
            LinkMsg::Retry => {
                self.links.retrying = false;
                if self.links.busy {
                    return Task::none(); // al terminar se abre el pendiente
                }
                match self.links.pending.take() {
                    Some((url, interactive)) => self.open_link(url, interactive),
                    None => Task::none(),
                }
            }
            LinkMsg::Found {
                request,
                game,
                result,
            } if request == self.links.request && self.links.busy => {
                self.on_link_found(game, result)
            }
            LinkMsg::Found { .. } => Task::none(),
            LinkMsg::Confirm(yes) => {
                let Some((game, info)) = self.links.confirm.take() else {
                    return Task::none();
                };
                let close = self.close_dialog();
                let connect = if yes {
                    self.connect_to(game, info)
                } else {
                    Task::none()
                };
                Task::batch([close, connect, self.link_done()])
            }
        }
    }

    /// `open_link`: uno a la vez; con un diálogo propio abierto espera (no con el selector de
    /// archivos, que no es modal).
    fn open_link(&mut self, url: String, interactive: bool) -> Task<Message> {
        if self.links.busy {
            self.links.pending = Some((url, interactive));
            return Task::none();
        }
        if self.dialog.is_some() {
            return self.wait_link(url, interactive);
        }
        let Some((id, address)) = jase_url::parse(&url) else {
            let text = tr("El enlace no es válido.");
            if !interactive {
                return self.show_message(text, Some(INVALID));
            }
            self.open_dialog(Dialog::Message {
                title: tr("Enlace jase://"),
                text,
                informative: String::new(),
                settings: false,
            });
            return Task::none();
        };
        let Some(g) = game(id) else {
            return Task::none();
        };
        self.links.busy = true;
        self.links.current = Some((url, interactive));
        self.links.request += 1;
        let select = self.select_game(id);
        self.set_status(tr_fmt("Consultando {address}…", &[("address", &address)]));
        let request = self.links.request;
        // Ni la `QueryFn` se llama aquí: todo corre al ejecutar la `Task`.
        let query = self.deps.query.clone();
        let found = Task::perform(async move { query(g, address).await }, move |result| {
            Message::Link(LinkMsg::Found {
                request,
                game: g.id,
                result,
            })
        });
        Task::batch([raise_window(self.windows.main), select, found])
    }

    /// Deja el enlace pendiente (el último gana) y programa un reintento si no hay uno.
    fn wait_link(&mut self, url: String, interactive: bool) -> Task<Message> {
        self.links.pending = Some((url, interactive));
        if std::mem::replace(&mut self.links.retrying, true) {
            return Task::none();
        }
        // `async` para que el temporizador se cree ya en el runtime, no aquí.
        Task::perform(async { tokio::time::sleep(LINK_RETRY).await }, |()| {
            Message::Link(LinkMsg::Retry)
        })
    }

    /// `_on_link_found` + `_on_link_finished`.
    fn on_link_found(
        &mut self,
        game_id: &'static str,
        result: Result<Option<ServerInfo>, String>,
    ) -> Task<Message> {
        let Some(g) = self.current.filter(|c| c.id == game_id) else {
            return self.link_done(); // se cambió de juego mientras tanto
        };
        let info = match result {
            Ok(Some(info)) => info,
            Ok(None) => {
                self.set_status(tr("El servidor no respondió"));
                return self.link_done();
            }
            Err(e) => {
                self.set_status(tr_fmt("Error al consultar: {error}", &[("error", &e)]));
                return self.link_done();
            }
        };
        if self.dialog.is_some() {
            // No se pregunta encima de otro diálogo: el enlace vuelve a esperar (si no llegó
            // uno más nuevo).
            self.links.busy = false;
            let current = self.links.current.take();
            let Some((url, interactive)) = self.links.pending.take().or(current) else {
                return Task::none();
            };
            return self.wait_link(url, interactive);
        }
        let hook = self.on_server_response(g.id, &info);
        self.table.model.add_or_update(info.clone());
        let select = self.select_address(&info.address);
        let name = strip_colors(&info.name);
        let name = if name.is_empty() {
            info.address.to_string()
        } else {
            name
        };
        let text = tr_fmt(
            "¿Conectar a {name}?\n\nJuego: {game}\nDirección: {address}",
            &[
                ("name", &name),
                ("game", &g.name),
                ("address", &info.address),
            ],
        );
        self.links.confirm = Some((g, info));
        self.open_dialog(Dialog::Link { text });
        Task::batch([hook, select.unwrap_or_else(Task::none)])
    }

    /// `_link_done`: libre, y abre el pendiente.
    fn link_done(&mut self) -> Task<Message> {
        self.links.busy = false;
        self.links.current = None;
        match self.links.pending.take() {
            Some((url, interactive)) => self.open_link(url, interactive),
            None => Task::none(),
        }
    }

    /// Reprograma el enlace pendiente que no tiene reintento en marcha (al cerrar un diálogo).
    pub(super) fn resume_pending_link(&mut self) -> Task<Message> {
        if self.links.busy || self.links.retrying {
            return Task::none();
        }
        match self.links.pending.take() {
            Some((url, interactive)) => self.wait_link(url, interactive),
            None => Task::none(),
        }
    }

    /// Otro diálogo reemplazó la confirmación: el enlace (o uno más nuevo) queda pendiente y se
    /// reprograma al cerrarse ese diálogo.
    pub(super) fn drop_link_confirm(&mut self) {
        if self.links.confirm.take().is_some() {
            // Si no llegó otro más nuevo, se vuelve a preguntar al cerrar el aviso.
            if self.links.pending.is_none() {
                self.links.pending = self.links.current.take();
            }
            self.links.busy = false;
            self.links.current = None;
        }
    }

    /// `confirm_plain`: título, la pregunta en texto plano (el nombre es remoto) y
    /// *Conectar*/*Cancelar* (los catálogos no tienen «Sí» ni «No»).
    pub(super) fn link_view(&self, body: &str) -> Element<'static, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let confirm = |yes| Message::Link(LinkMsg::Confirm(yes));
        let buttons = row![
            Space::new().width(Fill),
            widgets::button(tr("Conectar"), confirm(true), theme_id),
            widgets::button(tr("Cancelar"), confirm(false), theme_id),
        ]
        .spacing(6);
        let content = column![
            label(tr("Enlace jase://")).font(style::header_font(s)),
            label(body.to_owned()),
            buttons,
        ];
        card(content, 360.0, theme_id)
    }
}
