//! Menús abiertos y lo que hacen las acciones compartidas (`actions.rs`): cada camino (menú,
//! barra, contextual, atajo) llega a `on_action`, que no corre nada apagado.

use std::time::Duration;

use iced::{Task, clipboard};
use jase_core::config::Config;
use jase_core::games::GameDef;
use jase_core::i18n::{current_language, tr, tr_fmt};
use jase_core::jase_url;
use jase_core::models::ServerInfo;

use super::{App, Dialog, Message};
use crate::actions::{self, Action, Menu, enabled};
use crate::sidebar;
use crate::theme::resolve_theme;

impl App {
    pub(super) fn on_action(&mut self, m: actions::Msg) -> Task<Message> {
        use actions::Msg as M;
        match m {
            M::Open(menu) => self.menu = Some(menu),
            M::Hover(menu) => {
                if self.menu.is_some_and(|open| Menu::BAR.contains(&open)) {
                    self.menu = Some(menu);
                }
            }
            M::Close => self.menu = None,
            M::Group(name) => {
                self.menu = None;
                return self.move_selected_to_group(&name);
            }
            M::NewGroup => {
                self.menu = None;
                return self.new_group_for_selected();
            }
            M::AddFavorite => {
                self.menu = None;
                return self.add_server_manually(Some(true));
            }
            M::Run(a) if !enabled(self, a) => {} // apagada: nada, ni cerrar el menú
            M::Run(a) => {
                self.menu = None;
                return self.run_action(a);
            }
            M::Ping(ms) => {
                self.menu = None;
                self.quick_from_menu(sidebar::Msg::MaxPing(ms));
            }
            M::Theme(id) => {
                self.menu = None;
                self.cfg.theme = id.to_owned();
                self.save_config();
                self.apply_theme();
            }
            // `set_terminal_color`: elegirlo activa el tema Terminal.
            M::TerminalColor(color) => {
                self.menu = None;
                self.cfg.terminal_color = color.to_owned();
                self.cfg.theme = "terminal".to_owned();
                self.save_config();
                self.apply_theme();
            }
        }
        Task::none()
    }

    fn run_action(&mut self, a: Action) -> Task<Message> {
        let q = self.cfg.quick_filters.clone();
        match a {
            Action::Refresh => return self.update(Message::Refresh),
            Action::Stop => return self.update(Message::Stop),
            Action::RefreshSelected => return self.refresh_selected(),
            Action::Connect => return self.connect_selected(),
            Action::ScanLan => return self.scan_lan(),
            Action::Copy => return self.copy_selected(),
            Action::Favorite => return self.toggle_favorite_selected(),
            Action::EditNote => return self.edit_note_selected(),
            Action::RemoveRecent => return self.remove_recent_selected(),
            Action::Quit => return self.update(Message::CloseRequested),
            Action::AddServer => return self.add_server_manually(None),
            Action::CopyLink => return self.copy_selected_link(),
            Action::ExportFavorites => return self.export_favorites(),
            Action::ImportFavorites => return self.import_favorites(),
            Action::CopyFavorites => return self.copy_favorites_as_text(),
            Action::Settings => return self.open_settings(),
            Action::FindGames => return self.find_games(),
            Action::CheckUpdates => return self.check_app_update(true),
            Action::FindPlayer => return self.on_search(super::SearchMsg::Open),
            Action::NowPlaying => return self.on_now(super::NowMsg::Open),
            Action::Rcon => return self.on_rcon(super::RconMsg::Open),
            Action::WatchSlot => return self.on_watch(super::WatchMsg::ToggleSlot),
            Action::PartyAlert => return self.edit_party_alert(),
            Action::Friends => return self.on_friends(super::FriendsMsg::Open),
            Action::ClearTreeMarks => self.clear_tree_marks(),
            Action::About => {
                self.open_dialog(Dialog::About);
            }
            Action::License => self.open_dialog(Dialog::License),
            Action::Help => return self.open_web(help_url(current_language())),
            Action::ReportIssue => return self.open_web(ISSUES_URL),
            Action::Website => return self.open_web(REPO_URL),
            Action::ThirdParty => return self.open_third_party(),
            Action::HideEmpty => self.quick_from_menu(sidebar::Msg::HideEmpty(!q.hide_empty)),
            Action::HideFull => self.quick_from_menu(sidebar::Msg::HideFull(!q.hide_full)),
            Action::HidePassword => {
                self.quick_from_menu(sidebar::Msg::HidePassword(!q.hide_password));
            }
            Action::HideBots => self.quick_from_menu(sidebar::Msg::HideBots(!q.hide_bots)),
            // El resto, sin portar: `enabled` ya las dejó afuera.
            _ => {}
        }
        Task::none()
    }

    /// Un filtro rápido desde el menú: el mismo cambio que la casilla o el combo, sin
    /// moverle el foco a la barra lateral (el menú de Qt no lo toma).
    fn quick_from_menu(&mut self, m: sidebar::Msg) {
        let focus = self.focus;
        self.on_sidebar(m);
        self.set_focus(focus);
    }

    /// `copy_selected_address`: la dirección para conectar, al portapapeles.
    fn copy_selected(&mut self) -> Task<Message> {
        let Some(text) = self
            .selected_server()
            .map(|i| i.connect_address.to_string())
        else {
            return Task::none();
        };
        let shown = tr_fmt("Dirección copiada: {address}", &[("address", &text)]);
        let timer = self.show_message(shown, Some(Duration::from_secs(4)));
        Task::batch([clipboard::write(text), timer])
    }

    /// `copy_selected_link`: el enlace con la dirección de consulta, sin plazo.
    fn copy_selected_link(&mut self) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        let link = jase_url::build(g.id, &info.address);
        Task::batch([
            clipboard::write(link),
            self.show_message(tr("Enlace copiado"), None),
        ])
    }

    /// `apply_theme`: el tema elegido con el modo que informó el sistema.
    pub(super) fn apply_theme(&mut self) {
        self.theme_id = resolve_theme(&self.cfg.theme, self.system_mode, &self.cfg.terminal_color);
    }

    pub fn open_menu(&self) -> Option<Menu> {
        self.menu
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// La config para cambiarla desde afuera (los tests, como `w.cfg` de Python).
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.cfg
    }

    pub fn current_game(&self) -> Option<&'static GameDef> {
        self.current
    }

    pub fn selected_server(&self) -> Option<&ServerInfo> {
        let address = self.table.selected.as_ref()?;
        let row = self.table.model.row_of(address)?;
        self.table.model.servers().get(row)
    }
}

pub const REPO_URL: &str = "https://github.com/JotaJazz/JASE";
pub const ISSUES_URL: &str = "https://github.com/JotaJazz/JASE/issues";

/// La guía de uso en el idioma de la interfaz: en español la original, si no la inglesa.
pub fn help_url(language: &str) -> &'static str {
    if language == "es" {
        "https://github.com/JotaJazz/JASE/blob/master/docs/guia/guia-usuario.md"
    } else {
        "https://github.com/JotaJazz/JASE/blob/master/docs/guia/user-guide.md"
    }
}

/// `licenses/THIRD-PARTY.txt` junto al ejecutable (lo ponen los paquetes; no existe al compilar).
fn third_party_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.parent()?.join("licenses").join("THIRD-PARTY.txt");
    path.is_file().then_some(path)
}

impl App {
    /// Abre `url` en el navegador fuera del hilo de la UI.
    fn open_web(&self, url: &'static str) -> Task<Message> {
        let open = self.deps.open_url.clone();
        Task::perform(super::blocking(move || open(url)), |_| ()).discard()
    }

    /// Abre la lista de licencias de terceros; si no está (compilado a mano), lo dice en la barra.
    fn open_third_party(&self) -> Task<Message> {
        let open = self.deps.open_url.clone();
        Task::perform(
            super::blocking(move || match third_party_path() {
                Some(path) => open(&path.to_string_lossy()).is_ok(),
                None => false,
            }),
            |opened| Message::ThirdPartyOpened(matches!(opened, Ok(true))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::help_url;

    #[test]
    fn help_url_follows_language() {
        assert!(help_url("es").ends_with("/guia-usuario.md"));
        assert!(help_url("en").ends_with("/user-guide.md"));
        assert!(help_url("de").ends_with("/user-guide.md"));
    }
}
