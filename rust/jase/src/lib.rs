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
pub mod files;
pub mod flags;
pub mod icons;
pub mod keyring_store;
pub mod launch;
pub mod localtime;
pub mod masters_tab;
pub mod menu;
pub mod passwords;
pub mod presets;
pub mod saved_lists;
pub mod scan;
pub mod screenshot;
pub mod settings;
pub mod sidebar;
pub mod single_instance;
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

/// Con otra JASE abierta, ella se encarga (del enlace o de mostrarse) y esta sale: nunca dos
/// ventanas, que se pisarían la config. Si no, lee la config una vez: la ventana abre como
/// quedó y la app arranca con la misma.
pub fn run() -> iced::Result {
    let args = startup::utf8_args(std::env::args_os().skip(1));
    let link = startup::link_from_argv(&args);
    let endpoint = single_instance::endpoint();
    if single_instance::send_to_running(&endpoint, startup::running_message(link.as_deref())) {
        std::process::exit(0);
    }
    let cfg = load_config(&default_config_path());
    let window = window_settings(&cfg);
    let boot = move || App::boot_with(cfg.clone(), link.clone(), Some(endpoint.clone()));
    iced::application(boot, App::update, App::view)
        .title(startup::APP_NAME)
        .settings(settings())
        .window(window)
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}
