//! Las suscripciones de la app. Con varias ventanas los eventos llegan con su `window::Id`:
//! los de la principal pasan como siempre y los de una herramienta solo la afectan a ella.

use std::time::Duration;

use iced::keyboard::{self, Key};
use iced::{Event, Subscription, event, system, time, window};

use super::{App, Focus, LinkMsg, Message, SAVE_INTERVAL, event_message, route_key};
use crate::window_state;

/// Un evento de ventana, clic o Esc de la ventana `id`: de la principal pasa igual; de una
/// herramienta, Esc la cierra y lo demás se descarta (ni foco del filtro, menús, diálogos,
/// tamaño ni cierre de la principal).
pub fn window_message(main: window::Id, id: window::Id, m: Message) -> Option<Message> {
    if id == main {
        Some(m)
    } else if let Message::Escape = m {
        Some(Message::CloseTool(id))
    } else {
        None
    }
}

/// Una tecla no capturada: en la principal, para quien tiene el foco (`route_key`); en una
/// herramienta, para ella.
pub fn key_message(main: window::Id, focus: Focus, id: window::Id, key: Key) -> Option<Message> {
    if id == main {
        route_key(focus, key)
    } else {
        Some(Message::ToolKey(id, key))
    }
}

impl App {
    /// El teclado va a quien tenga el foco (`route_key`); cada clic pregunta por el campo.
    pub fn subscription(&self) -> Subscription<Message> {
        let main = self.windows.main;
        let resize = window::resize_events().map(|(id, size)| (id, Message::WindowResized(size)));
        let close = window::close_requests().map(|id| (id, Message::CloseRequested));
        let clicks = event::listen_with(|e, s, id| event_message(e, s, id).map(|m| (id, m)));
        let mut per_window = vec![resize, close, clicks, window_state::moves()];
        per_window.extend(window_state::drag_subscription(self));
        let per_window = Subscription::batch(per_window)
            .with(main)
            .filter_map(|(main, (id, m))| window_message(main, id, m));
        // `colorSchemeChanged` de Qt: `SystemMode` solo cambia algo con el tema «system».
        let mode = system::theme_changes().map(Message::SystemMode);
        // Como `keyboard::listen`: solo lo que ningún widget capturó, con su ventana.
        let keys = event::listen_with(|e, status, id| match (e, status) {
            (Event::Keyboard(keyboard::Event::KeyPressed { key, .. }), event::Status::Ignored) => {
                Some((id, key))
            }
            _ => None,
        })
        .with((main, self.key_focus()))
        .filter_map(|((main, focus), (id, key))| key_message(main, focus, id, key));
        let closed = window::close_events().map(Message::WindowClosed);
        let save = time::every(SAVE_INTERVAL).map(|_| Message::HistorySave);
        let mut subs = vec![per_window, mode, keys, closed, save];
        subs.extend(self.auto_refresh_subscription());
        subs.extend(self.update_subscription());
        subs.extend(self.watch_subscriptions());
        subs.extend(self.tray_subscriptions());
        if let Some(endpoint) = &self.deps.link_endpoint {
            let lines = Subscription::run_with(endpoint.clone(), crate::single_instance::listen);
            subs.push(lines.map(|l| Message::Link(LinkMsg::Received(l))));
        }
        // Solo mientras haya servidores que el desglose aún no cuenta.
        if self.tree_dirty {
            subs.push(time::every(Duration::from_millis(300)).map(|_| Message::TreeTick));
        }
        Subscription::batch(subs)
    }
}
