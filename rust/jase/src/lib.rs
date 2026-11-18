//! App de escritorio de JASE (Iced).

pub mod actions;
pub mod add_server;
pub mod app;
pub mod bars;
pub mod card;
pub mod chart;
pub mod colored;
pub mod details;
pub mod exe_icon;
pub mod flags;
pub mod icons;
pub mod keyring_store;
pub mod launch;
pub mod localtime;
pub mod menu;
pub mod passwords;
pub mod presets;
pub mod saved_lists;
pub mod scan;
pub mod screenshot;
pub mod sidebar;
pub mod smoke;
pub mod startup;
pub mod table;
pub mod theme;
pub mod tree;
pub mod widgets;
pub mod window_state;

use app::App;
use iced::Size;
use jase_core::config::{default_config_path, load_config};

pub use window_state::window_settings;

/// Tamaño de la ventana sin estado guardado.
pub const WINDOW_SIZE: Size = Size::new(1100.0, 700.0);

/// Los mismos ajustes en la app y en `--smoke`.
pub(crate) fn settings() -> iced::Settings {
    iced::Settings {
        default_text_size: 13.into(),
        ..iced::Settings::default()
    }
}

/// Lee la config una vez: la ventana abre como quedó y la app arranca con la misma.
pub fn run() -> iced::Result {
    let cfg = load_config(&default_config_path());
    let window = window_settings(&cfg);
    iced::application(move || App::boot_with(cfg.clone()), App::update, App::view)
        .title(startup::APP_NAME)
        .settings(settings())
        .window(window)
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}
