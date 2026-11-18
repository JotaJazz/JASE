//! Estado de la ventana (provisional en la 5a: lista de juegos, tabla y barra de estado).

use iced::keyboard::{self, Key, key::Named};
use iced::theme::Mode;
use iced::widget::{Column as Col, container, row, scrollable, text};
use iced::{Element, Fill, Padding, Right, Size, Subscription, Task, Theme, system, window};
use jase_core::config::{Config, default_config_path, load_config};
use jase_core::games::GAMES;
use jase_core::i18n::tr_n;

use crate::startup::{prepare, system_locale_name};
use crate::table::{self, Nav, Table};
use crate::theme::{iced_theme, palette, resolve_theme, style};

#[derive(Debug, Clone)]
pub enum Message {
    Table(table::Msg),
    SystemMode(Mode),
    WindowResized(Size),
}

pub struct App {
    pub table: Table,
    pub theme_id: &'static str,
    cfg: Config,
}

impl Default for App {
    fn default() -> Self {
        Self {
            table: Table::default(),
            theme_id: "dark",
            cfg: Config::default(),
        }
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
    /// Lee la config y fija el idioma antes de abrir la ventana; el modo del sistema llega después.
    pub fn boot() -> (App, Task<Message>) {
        let cfg = load_config(&default_config_path());
        prepare(&cfg, &system_locale_name());
        let app = App {
            theme_id: resolve_theme(&cfg.theme, Mode::None, &cfg.terminal_color),
            cfg,
            ..App::default()
        };
        (app, system::theme().map(Message::SystemMode))
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Table(m) => return table::update(&mut self.table, m),
            Message::SystemMode(mode) => {
                self.theme_id = resolve_theme(&self.cfg.theme, mode, &self.cfg.terminal_color);
            }
            // Si al agrandar la ventana la tabla ya cabe, el `scrollable` no informa su alto.
            Message::WindowResized(size) => self.table.viewport_h = size.height,
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let p = palette(self.theme_id);
        let games = Col::with_children(GAMES.iter().map(|g| text(g.short_name()).into()))
            .spacing(4)
            .padding(6);
        let sidebar = container(scrollable(games).height(Fill))
            .width(230)
            .style(move |_| style::sidebar(p));
        let n = self.table.model.len();
        let status = container(text(tr_n("{n} servidor", "{n} servidores", n as i64, &[])))
            .width(Fill)
            .align_x(Right)
            .padding([2, 10])
            .style(move |_| style::status_bar(p));
        let body = row![sidebar, table::view(&self.table, p)]
            .spacing(6)
            .height(Fill);
        let body = container(body).padding(Padding {
            top: 10.0,
            right: 10.0,
            bottom: 6.0,
            left: 10.0,
        });
        container(Col::new().push(body).push(status))
            .style(move |_| style::window(p))
            .into()
    }

    pub fn theme(&self) -> Theme {
        iced_theme(self.theme_id)
    }

    /// Teclado solo con la tabla enfocada (como el foco de `QTableView`).
    pub fn subscription(&self) -> Subscription<Message> {
        let resize = window::resize_events().map(|(_, size)| Message::WindowResized(size));
        if !self.table.focused {
            return resize;
        }
        let keys = keyboard::listen().filter_map(|e| match e {
            keyboard::Event::KeyPressed { key, .. } => {
                nav_for(key).map(|n| Message::Table(table::Msg::Nav(n)))
            }
            _ => None,
        });
        Subscription::batch([resize, keys])
    }
}
