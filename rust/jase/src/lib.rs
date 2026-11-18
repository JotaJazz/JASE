//! App de escritorio de JASE (Iced).

pub mod app;
pub mod colored;
pub mod startup;
pub mod table;
pub mod theme;

use app::App;

pub fn run() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(startup::APP_NAME)
        .settings(iced::Settings {
            default_text_size: 13.into(),
            ..iced::Settings::default()
        })
        .window_size((1100.0, 700.0))
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}
