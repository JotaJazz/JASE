//! Conectar a un servidor (`connect_selected`/`connect_to` de `window_connect.py`): ruta que
//! falta, servidor sin respuesta, contraseña con el llavero, lanzamiento y recientes. El
//! llavero y el lanzador solo corren en `blocking`, dentro de una `Task`.

use iced::Task;
use iced::widget::operation;
use jase_core::games::{GameDef, LaunchStyle};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::recent::record_recent;

use super::dialogs::{PROMPT_ID, PromptMsg};
use super::{App, Dialog, Message, blocking};
use crate::launch::{has_path, launch_args};
use crate::passwords::server_user;

#[derive(Debug, Clone)]
pub enum ConnectMsg {
    /// Enter en la tabla: conecta si hay una fila elegida (`_connect_if_selected`).
    Enter,
    /// La contraseña guardada del pedido `request`, leída del llavero en un hilo.
    Stored {
        request: u64,
        password: Option<String>,
    },
    /// El juego se lanzó (o no) tras guardar (o no) la contraseña; nunca se descarta: el
    /// juego ya se abrió.
    Launched {
        game: &'static GameDef,
        /// La dirección de consulta (la de recientes).
        address: ServerAddress,
        /// La dirección para conectar (la del mensaje).
        connect: ServerAddress,
        saved: Result<(), String>,
        result: Result<(), String>,
    },
}

/// El pedido de contraseña vigente y su servidor: las respuestas de otro se descartan.
#[derive(Default)]
pub(crate) struct Connect {
    request: u64,
    pending: Option<(&'static GameDef, ServerInfo)>,
}

impl App {
    /// *Conectar*, el doble clic y Enter: sin selección solo avisa si falta la ruta.
    pub fn connect_selected(&mut self) -> Task<Message> {
        let Some(game) = self.current else {
            return Task::none();
        };
        match self.selected_server().cloned() {
            Some(info) => self.connect_to(game, info),
            None => {
                if !has_path(&self.cfg, game) {
                    self.prompt_missing_path(game);
                }
                Task::none()
            }
        }
    }

    /// Lanza `game` hacia `info`, pidiendo antes la contraseña si la tiene.
    pub fn connect_to(&mut self, game: &'static GameDef, info: ServerInfo) -> Task<Message> {
        if self.dialog.is_some() {
            return Task::none(); // modal: como en Qt, nada conecta por debajo
        }
        if !has_path(&self.cfg, game) {
            self.prompt_missing_path(game);
            return Task::none();
        }
        if !info.online {
            self.set_status(tr("El servidor no responde; refresca para conectar"));
            return Task::none();
        }
        if !info.password {
            return self.launch(game, info, None, None);
        }
        self.connect.request += 1;
        let request = self.connect.request;
        let user = server_user(game.id, &info.address);
        self.connect.pending = Some((game, info));
        let keyring = self.deps.keyring.clone();
        Task::perform(blocking(move || keyring.get(&user)), move |r| {
            Message::Connect(ConnectMsg::Stored {
                request,
                password: r.ok().flatten(),
            })
        })
    }

    pub fn connect_request(&self) -> u64 {
        self.connect.request
    }

    pub(super) fn on_connect(&mut self, m: ConnectMsg) -> Task<Message> {
        match m {
            ConnectMsg::Enter if self.selected_server().is_some() => self.connect_selected(),
            ConnectMsg::Enter => Task::none(),
            ConnectMsg::Stored { request, password } => {
                if request != self.connect.request || self.dialog.is_some() {
                    return Task::none();
                }
                let Some((_, info)) = &self.connect.pending else {
                    return Task::none();
                };
                let password = password.filter(|p| !p.is_empty());
                self.open_dialog(Dialog::Password {
                    name: info.name.clone(),
                    saved: password.is_some(),
                    typed: password.unwrap_or_default(),
                });
                operation::focus(PROMPT_ID)
            }
            ConnectMsg::Launched {
                game,
                address,
                connect,
                saved,
                result,
            } => {
                self.on_launched(game, &address, &connect, saved, result);
                Task::none()
            }
        }
    }

    /// El diálogo de contraseña: *Conectar*, *Olvidar contraseña* o *Cancelar*.
    pub(super) fn on_password(&mut self, m: PromptMsg) -> Task<Message> {
        let Some((Dialog::Password { typed, saved, .. }, _)) = &self.dialog else {
            return Task::none();
        };
        let (typed, saved) = (typed.clone(), *saved);
        match m {
            PromptMsg::Input(_) => Task::none(), // lo escribe `on_prompt`
            PromptMsg::Forget if !saved => Task::none(), // sin el botón
            PromptMsg::Cancel => {
                self.connect.pending = None;
                self.close_dialog()
            }
            PromptMsg::Forget => {
                let close = self.close_dialog();
                let Some((game, info)) = self.connect.pending.take() else {
                    return close;
                };
                let (keyring, user) = (
                    self.deps.keyring.clone(),
                    server_user(game.id, &info.address),
                );
                let forget =
                    Task::perform(blocking(move || keyring.set(&user, "")), |_| ()).discard();
                self.set_status(tr("Contraseña olvidada"));
                Task::batch([close, forget])
            }
            PromptMsg::Submit => {
                let close = self.close_dialog();
                let Some((game, info)) = self.connect.pending.take() else {
                    return close;
                };
                // Vacía = olvidar la guardada y conectar sin contraseña.
                let user = server_user(game.id, &info.address);
                let password = (!typed.is_empty()).then(|| typed.clone());
                let launch = self.launch(game, info, password, Some((user, typed)));
                Task::batch([close, launch])
            }
        }
    }

    /// Una sola `Task` en un hilo: guarda (o borra) en el llavero y lanza.
    fn launch(
        &self,
        game: &'static GameDef,
        info: ServerInfo,
        password: Option<String>,
        save: Option<(String, String)>,
    ) -> Task<Message> {
        let args = launch_args(game, &info, &self.cfg, password);
        let (keyring, launch) = (self.deps.keyring.clone(), self.deps.launch.clone());
        let (address, connect) = (info.address, info.connect_address);
        let work = blocking(move || {
            let saved = save.map_or(Ok(()), |(user, value)| keyring.set(&user, &value));
            (saved, launch(&args))
        });
        Task::perform(work, move |r| {
            let (saved, result) = r.unwrap_or_else(|e| (Ok(()), Err(e)));
            Message::Connect(ConnectMsg::Launched {
                game,
                address,
                connect,
                saved,
                result,
            })
        })
    }

    /// El resultado del lanzamiento: el estado y los recientes, o un solo aviso a la vez (el
    /// del lanzamiento antes que el del llavero). Python avisa del llavero antes de lanzar.
    fn on_launched(
        &mut self,
        game: &'static GameDef,
        address: &ServerAddress,
        connect: &ServerAddress,
        saved: Result<(), String>,
        result: Result<(), String>,
    ) {
        let dialog = match result {
            Err(e) => {
                let steam = game.launch_style == LaunchStyle::Steam;
                Some(Dialog::Message {
                    title: tr("No se pudo conectar"),
                    text: e,
                    // Steam no usa la ruta del juego.
                    informative: if steam {
                        String::new()
                    } else {
                        tr("Revisa la ruta del juego en Ajustes.")
                    },
                    settings: !steam,
                })
            }
            Ok(()) => {
                let name: [(&str, &dyn std::fmt::Display); 2] =
                    [("name", &game.name), ("address", connect)];
                self.set_status(tr_fmt("Lanzando {name} → {address}", &name));
                record_recent(&mut self.cfg, game.id, address);
                self.save_config();
                self.update_sidebar();
                saved.err().map(|e| Dialog::Message {
                    title: tr("Contraseña"),
                    text: tr_fmt(
                        "No se pudo guardar la contraseña: {error}\nSe usará solo para esta conexión.",
                        &[("error", &e)],
                    ),
                    informative: String::new(),
                    settings: false,
                })
            }
        };
        match dialog {
            // Llega tarde con otro diálogo abierto (p. ej. la contraseña de otro servidor): no
            // lo pisa; el aviso va a la barra de estado.
            Some(Dialog::Message { title, text, .. }) if self.dialog.is_some() => {
                self.set_status(format!("{title}: {text}"));
            }
            Some(d) => self.open_dialog(d),
            None => {}
        }
    }

    /// `_prompt_missing_path`: el aviso con *Abrir ajustes*.
    fn prompt_missing_path(&mut self, game: &GameDef) {
        self.open_dialog(Dialog::Message {
            title: tr("Ruta no configurada"),
            text: tr_fmt(
                "No hay ruta configurada para {name}.",
                &[("name", &game.name)],
            ),
            informative: tr("Configúrala en Ajustes para poder conectarte."),
            settings: true,
        });
    }
}
