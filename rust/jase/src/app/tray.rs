//! Minimizar a la bandeja y los avisos por la bandeja (`window_tray.py`). El icono vive en una
//! suscripción (solo con la casilla y un backend); aquí, lo que la app hace con él.

use iced::futures::{StreamExt, stream};
use iced::window::{self, Mode};
use iced::{Subscription, Task};
use jase_core::i18n::tr;

use super::notify::{Click, Prompt};
use super::{App, Message};
use crate::tray::{TrayEvent, TrayFn, TrayHandle, TrayLabels};
use crate::window_state;

#[derive(Debug, Clone)]
pub enum TrayMsg {
    Event(TrayEvent),
    /// El stream del icono terminó (no hay bandeja o se cayó).
    Gone,
    /// La principal perdió el foco o cambió de tamaño: ¿quedó minimizada?
    CheckMinimized,
    /// La respuesta (`None` = no se sabe, como en Wayland).
    Minimized(Option<bool>),
}

#[derive(Default)]
pub(super) struct Tray {
    handle: Option<TrayHandle>,
    /// La principal está oculta en la bandeja.
    hidden: bool,
    /// Lo que hace el clic en el último globo (un solo pendiente: el último reemplaza).
    click: Option<Click>,
    /// El último aviso mostrado en la bandeja: va a la barra al volver.
    notice: String,
    /// La pregunta que espera a la ventana (otros avisos no la reemplazan).
    prompt: Option<Prompt>,
}

impl App {
    pub fn want_tray(&self) -> bool {
        self.cfg.minimize_to_tray && self.deps.tray.is_some()
    }

    pub fn in_tray(&self) -> bool {
        self.tray.handle.is_some() && self.tray.hidden
    }

    /// El icono (crearlo, sus eventos y quitarlo al soltarse) y, con él, mirar si se minimizó.
    pub(super) fn tray_subscriptions(&self) -> Vec<Subscription<Message>> {
        let Some(tray_fn) = self.deps.tray.filter(|_| self.want_tray()) else {
            return Vec::new();
        };
        let labels = TrayLabels {
            show: tr("Mostrar JASE"),
            quit: tr("Salir"),
        };
        let icon = Subscription::run_with((labels, tray_fn), |(labels, f): &(_, TrayFn)| {
            f(labels)
                .map(TrayMsg::Event)
                .chain(stream::once(async { TrayMsg::Gone }))
        })
        .map(Message::Tray);
        let mut subs = vec![icon];
        if self.tray.handle.is_some() {
            // No hay evento de «minimizada»: se pregunta tras perder el foco o cambiar el tamaño.
            let check = window::events()
                .with(self.windows.main)
                .filter_map(|(main, (id, e))| {
                    let changed = matches!(e, window::Event::Unfocused | window::Event::Resized(_));
                    (id == main && changed).then_some(Message::Tray(TrayMsg::CheckMinimized))
                });
            subs.push(check);
        }
        subs
    }

    pub(super) fn on_tray(&mut self, m: TrayMsg) -> Task<Message> {
        let main = self.windows.main;
        match m {
            TrayMsg::Event(TrayEvent::Ready(handle)) => {
                if self.want_tray() {
                    self.tray.handle = Some(handle);
                } else {
                    handle.shutdown(); // llegó tras quitar la casilla
                }
            }
            TrayMsg::Event(TrayEvent::Show) => return self.restore_from_tray(),
            TrayMsg::Event(TrayEvent::Quit) => return window_state::close(main),
            TrayMsg::Event(TrayEvent::MessageClicked) => {
                let Some(click) = self.tray.click.take() else {
                    return Task::none();
                };
                let restore = self.restore_from_tray();
                return Task::batch([restore, self.run_click(click)]);
            }
            TrayMsg::Gone => return self.tray_gone(),
            TrayMsg::CheckMinimized if self.tray.handle.is_some() => {
                return window::is_minimized(main).map(|m| Message::Tray(TrayMsg::Minimized(m)));
            }
            TrayMsg::Minimized(Some(true)) if self.tray.handle.is_some() => {
                self.tray.hidden = true;
                return window::set_mode(main, Mode::Hidden);
            }
            // Sin icono, o sin saber si se minimizó (Wayland): nunca se oculta.
            TrayMsg::CheckMinimized | TrayMsg::Minimized(_) => {}
        }
        Task::none()
    }

    /// Sin icono, una ventana oculta no tendría salida: vuelve.
    fn tray_gone(&mut self) -> Task<Message> {
        self.tray.handle = None;
        self.tray.click = None;
        if self.tray.hidden {
            return self.restore_from_tray();
        }
        Task::none()
    }

    /// Tras aceptar Ajustes: sin la casilla, el icono se quita (`_apply_tray`); al ponerla, lo
    /// crea la suscripción.
    pub(super) fn sync_tray(&mut self) -> Task<Message> {
        if self.want_tray() {
            return Task::none();
        }
        if let Some(handle) = &self.tray.handle {
            handle.shutdown();
        }
        self.tray_gone()
    }

    /// Al cerrar: el icono se quita antes de salir (`_shutdown_tray`).
    pub(super) fn shutdown_tray(&mut self) {
        if let Some(handle) = self.tray.handle.take() {
            handle.shutdown();
        }
        self.tray.prompt = None;
    }

    /// Trae la principal (también desde la bandeja) sin desmaximizarla; luego el último aviso
    /// a la barra y la pregunta pendiente.
    pub fn restore_from_tray(&mut self) -> Task<Message> {
        let main = self.windows.main;
        let mut tasks = Vec::new();
        if std::mem::take(&mut self.tray.hidden) {
            tasks.push(window::set_mode(main, Mode::Windowed));
        }
        // También la saca de minimizada sin bandeja (el «mostrar» de otra instancia).
        tasks.push(window::minimize(main, false));
        tasks.push(window::gain_focus(main));
        self.tray.click = None; // de vuelta en la ventana, el globo pendiente ya no vale
        let notice = std::mem::take(&mut self.tray.notice);
        if !notice.is_empty() {
            tasks.push(self.show_message(notice, None));
        }
        if let Some(prompt) = self.tray.prompt.take() {
            tasks.push(self.run_prompt(prompt));
        }
        Task::batch(tasks)
    }

    /// `notify` con la ventana en la bandeja: globo, o sin globos, volver y avisar.
    pub(super) fn tray_notify(
        &mut self,
        title: &str,
        text: &str,
        click: Option<Click>,
    ) -> Task<Message> {
        let Some(handle) = self.tray.handle.clone() else {
            return Task::none();
        };
        if handle.messages {
            self.tray.click = click;
            self.tray.notice = format!("{title}: {text}");
            handle.notify(title, text);
            return Task::none();
        }
        // Sin globos, una alerta en una ventana oculta se perdería.
        let restore = self.restore_from_tray();
        let then = match click {
            Some(c) => self.run_click(c),
            None => self.show_message(format!("{title}: {text}"), None),
        };
        Task::batch([restore, then])
    }

    /// `notify_prompt` en la bandeja: la pregunta espera a que se vuelva a la ventana.
    pub(super) fn tray_prompt(&mut self, title: &str, text: &str, prompt: Prompt) -> Task<Message> {
        self.tray.prompt = Some(prompt);
        self.tray_notify(title, text, Some(Click::Restore))
    }
}
