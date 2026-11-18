//! App de escritorio de JASE (Iced).

pub mod app;
pub mod colored;
pub mod scan;
pub mod smoke;
pub mod startup;
pub mod table;
pub mod theme;

use app::App;
use iced::{Size, window};

pub(crate) const WINDOW_SIZE: Size = Size::new(1100.0, 700.0);

/// Los mismos ajustes en la app y en `--smoke`.
pub(crate) fn settings() -> iced::Settings {
    iced::Settings {
        default_text_size: 13.into(),
        ..iced::Settings::default()
    }
}

/// El cierre pasa por `CloseRequested` (guarda la config y sale sin esperas).
pub(crate) fn window_settings() -> window::Settings {
    window::Settings {
        size: WINDOW_SIZE,
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}

pub fn run() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(startup::APP_NAME)
        .settings(settings())
        .window(window_settings())
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}
