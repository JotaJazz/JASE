//! La consola RCON del servidor elegido (`window_rcon.py`): una ventana por servidor, la clave
//! leída y guardada en el llavero y el comando, todo en una `Task` (nunca en `update`).

use iced::keyboard::{Key, key::Named};
use iced::widget::{operation, text_editor};
use iced::{Task, window};
use jase_core::games::game;
use jase_core::rcon::{RconError, kind_of};

use super::{App, Focus, Message, Tool, blocking};
use crate::passwords::{Secret, rcon_user};
use crate::rcon_console::Console;

#[derive(Debug, Clone)]
pub enum RconMsg {
    /// *Consola RCON…*: la del servidor elegido, nueva o traída al frente.
    Open,
    /// La clave guardada, leída del llavero en un hilo.
    Saved {
        window: window::Id,
        saved: Option<Secret>,
    },
    Password(window::Id, Secret),
    Remember(window::Id, bool),
    Command(window::Id, String),
    Send(window::Id),
    /// La respuesta del pedido `request`, o el error como `(código, detalle)`.
    Answered {
        window: window::Id,
        request: u64,
        result: Result<String, (String, String)>,
    },
    Output(window::Id, text_editor::Action),
}

/// El detalle de `Connect`/`Network` (el texto del sistema); los demás no tienen.
fn detail(e: &RconError) -> String {
    match e {
        RconError::Connect(d) | RconError::Network(d) => d.clone(),
        _ => String::new(),
    }
}

impl App {
    /// Juego con RCON y un servidor elegido que responde.
    pub fn can_rcon(&self) -> bool {
        self.current.is_some_and(|g| kind_of(g).is_some())
            && self.selected_server().is_some_and(|i| i.online)
    }

    pub fn console(&self, id: window::Id) -> Option<&Console> {
        self.rcon.get(&id)
    }

    pub(super) fn on_rcon(&mut self, m: RconMsg) -> Task<Message> {
        let id = match &m {
            RconMsg::Open => return self.open_rcon(),
            RconMsg::Saved { window, .. } | RconMsg::Answered { window, .. } => *window,
            RconMsg::Password(w, _)
            | RconMsg::Remember(w, _)
            | RconMsg::Command(w, _)
            | RconMsg::Send(w)
            | RconMsg::Output(w, _) => *w,
        };
        // De una consola cerrada (o reabierta, con otra id): se descarta.
        let Some(c) = self.rcon.get_mut(&id) else {
            return Task::none();
        };
        match m {
            RconMsg::Open => {}
            RconMsg::Saved { saved, .. } => {
                let field = c.on_saved(saved);
                // El caret deja el filtro de la principal (ver `tools.rs`).
                if self.focus == Focus::Filter {
                    self.set_focus(Focus::Sidebar);
                }
                return operation::focus(field);
            }
            RconMsg::Password(_, pw) => c.password = pw,
            RconMsg::Remember(_, on) => c.remember = on,
            RconMsg::Command(_, text) => c.command = text,
            RconMsg::Send(_) => return self.rcon_send(id),
            RconMsg::Answered {
                request, result, ..
            } if request == c.request => match result {
                Ok(text) => c.show_answer(&text),
                Err((code, detail)) => c.show_error(&code, &detail),
            },
            RconMsg::Answered { .. } => {}
            RconMsg::Output(_, action) if !action.is_edit() => c.output.perform(action),
            RconMsg::Output(..) => {}
        }
        Task::none()
    }

    fn open_rcon(&mut self) -> Task<Message> {
        if !self.can_rcon() {
            return Task::none();
        }
        let (Some(g), Some(info)) = (self.current, self.selected_server().cloned()) else {
            return Task::none();
        };
        let key = (g.id, info.connect_address.clone());
        let fresh = self.tool_window(&Tool::Rcon(key.clone())).is_none();
        let (id, open) = self.open_tool(Tool::Rcon(key));
        if !fresh {
            return open;
        }
        let address = info.connect_address;
        self.rcon
            .insert(id, Console::new(info.name, address.to_string()));
        let (keyring, user) = (self.deps.keyring.clone(), rcon_user(g.id, &address));
        let read = Task::perform(blocking(move || keyring.get(&user)), move |r| {
            Message::Rcon(RconMsg::Saved {
                window: id,
                saved: r.ok().flatten().map(Secret),
            })
        });
        // Tras abrirse: el foco solo llega a ventanas que ya existen.
        open.chain(read)
    }

    /// En un hilo, primero guarda (o borra) la clave y luego manda el comando. Sin llavero
    /// la clave vale solo para esta sesión: el error no corta el envío.
    fn rcon_send(&mut self, id: window::Id) -> Task<Message> {
        let Some(Tool::Rcon((game_id, address))) = self.tool_of(id).cloned() else {
            return Task::none();
        };
        let (Some(g), Some(c)) = (game(game_id), self.rcon.get_mut(&id)) else {
            return Task::none();
        };
        let Some((pw, command, remember)) = c.send() else {
            return Task::none();
        };
        c.request += 1;
        let request = c.request;
        let (keyring, rcon) = (self.deps.keyring.clone(), self.deps.rcon.clone());
        let user = rcon_user(g.id, &address);
        let work = async move {
            let send = blocking(move || {
                let _ = keyring.set(&user, if remember { &pw.0 } else { "" });
                rcon(g, address, pw, command)
            });
            let send = send.await.map_err(|e| ("network".to_owned(), e))?;
            send.await.map_err(|e| (e.code().to_owned(), detail(&e)))
        };
        Task::perform(work, move |result| {
            Message::Rcon(RconMsg::Answered {
                window: id,
                request,
                result,
            })
        })
    }

    /// ↑/↓ de una consola: el historial del comando.
    pub(super) fn rcon_key(&mut self, id: window::Id, key: &Key) {
        if let Some(c) = self.rcon.get_mut(&id) {
            match key {
                Key::Named(Named::ArrowUp) => c.history_step(true),
                Key::Named(Named::ArrowDown) => c.history_step(false),
                _ => {}
            }
        }
    }
}
