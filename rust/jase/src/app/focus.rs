//! Quién se queda con el teclado y los eventos que llegan aunque los capture un widget.

use iced::keyboard::{self, Key, key::Named};
use iced::{Event, event, mouse, window};

use super::{App, ConnectMsg, Message};
use crate::bars::shortcut_for;
use crate::table::{self, Nav};
use crate::tree;

/// Quién se queda con el teclado: las flechas van a la tabla o al árbol, y a nadie mientras
/// se escribe en el campo (el `text_input` no captura ↑/↓), hay un diálogo o el foco está en
/// una casilla o un combo (`Sidebar`, donde Qt arranca).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Focus {
    Sidebar,
    Tree,
    Table,
    Filter,
    Dialog,
}

/// La tecla según quién tiene el foco (`subscription` y los tests).
pub fn route_key(focus: Focus, key: Key) -> Option<Message> {
    match focus {
        // Return y Enter del teclado numérico: Iced los da como la misma tecla.
        Focus::Table if key == Key::Named(Named::Enter) => {
            Some(Message::Connect(ConnectMsg::Enter))
        }
        Focus::Table => nav_for(key).map(|n| Message::Table(table::Msg::Nav(n))),
        Focus::Tree => tree_nav_for(key).map(|n| Message::Tree(tree::Msg::Nav(n))),
        Focus::Sidebar | Focus::Filter | Focus::Dialog => None,
    }
}

/// Clics, Esc y atajos de `event::listen_with` (capturados o no: el campo de texto se queda
/// con Esc, `Ctrl+F` y `Ctrl+C`).
pub fn event_message(e: Event, _: event::Status, _: window::Id) -> Option<Message> {
    match e {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            Some(Message::MousePressed)
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            ..
        }) => Some(Message::Escape),
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            shortcut_for(&key, modifiers).map(Message::Shortcut)
        }
        _ => None,
    }
}

fn tree_nav_for(key: Key) -> Option<tree::Nav> {
    match key {
        Key::Named(Named::ArrowUp) => Some(tree::Nav::Up),
        Key::Named(Named::ArrowDown) => Some(tree::Nav::Down),
        Key::Named(Named::ArrowLeft) => Some(tree::Nav::Left),
        Key::Named(Named::ArrowRight) => Some(tree::Nav::Right),
        Key::Named(Named::Space) => Some(tree::Nav::Space),
        _ => None,
    }
}

fn nav_for(key: Key) -> Option<Nav> {
    match key {
        Key::Named(Named::ArrowUp) => Some(Nav::Up),
        Key::Named(Named::ArrowDown) => Some(Nav::Down),
        Key::Named(Named::PageUp) => Some(Nav::PageUp),
        Key::Named(Named::PageDown) => Some(Nav::PageDown),
        Key::Named(Named::Home) => Some(Nav::Home),
        Key::Named(Named::End) => Some(Nav::End),
        _ => None,
    }
}

impl App {
    pub(super) fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        // Sin foco, 98 y Luna pintan la selección de la tabla con la cara (`:!focus`).
        self.table.focused = focus == Focus::Table;
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// Quién recibe las flechas: nadie con un menú abierto (como un combo desplegado).
    pub(super) fn key_focus(&self) -> Focus {
        if self.menu.is_some() {
            Focus::Dialog
        } else {
            self.focus
        }
    }

    /// La flecha o tecla de navegación como mensaje para quien tiene el foco.
    pub fn key_message(&self, key: Key) -> Option<Message> {
        route_key(self.key_focus(), key)
    }
}
