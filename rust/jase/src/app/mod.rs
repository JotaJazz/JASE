//! Estado de la ventana: árbol de juegos, tabla y barra de estado. `update`, `view` y la
//! suscripción viven aquí; cada dominio, en su archivo (`focus`, `filters`, `games`, `saved`,
//! `scanning`, `dialogs`).

mod dialogs;
mod filters;
mod focus;
mod games;
mod saved;
mod scanning;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::keyboard;
use iced::theme::Mode;
use iced::widget::{Column as Col, column, container, operation, row, text};
use iced::{
    Element, Fill, Padding, Right, Size, Subscription, Task, Theme, event, system, time, window,
};
use jase_core::config::{Config, default_config_path, load_config, save_config};
use jase_core::games::GameDef;
use jase_core::geo::CountryDB;
use jase_core::i18n::{tr, tr_n};
use jase_core::masters::{
    CUSTOM_FILENAME, CustomMasters, OfficialList, load_custom, load_official,
    migrate_config_masters, save_custom,
};
use jase_core::models::ServerInfo;

pub use dialogs::Dialog;
pub use focus::{Focus, event_message, route_key};

use crate::presets;
use crate::saved_lists::SavedLists;
use crate::scan::{self, Deps, ScanMsg, ScanState, scan_stream};
use crate::sidebar;
use crate::startup::{initial_game, prepare, system_locale_name};
use crate::table::{self, Table};
use crate::theme::{framed, iced_theme, palette, resolve_theme, style, widget_style};
use crate::tree::{self, GameTree};
use crate::widgets::{self, FILTER_ID};

/// Lo que el cierre espera al escritor de listas guardadas (`CLOSE_WAIT_MS` de Python).
const CLOSE_WAIT: Duration = Duration::from_secs(2);

#[allow(clippy::large_enum_variant)] // por `ScanMsg`
#[derive(Debug, Clone)]
pub enum Message {
    SelectGame(&'static str),
    Refresh,
    Stop,
    /// Busca servidores del juego actual en la red local (`scan_lan`).
    ScanLan,
    /// Vuelve a consultar el servidor seleccionado.
    RefreshSelected,
    /// Respuesta de «Refrescar este servidor»; la de otro pedido o juego se descarta.
    Query {
        request: u64,
        game: &'static str,
        result: Result<Option<ServerInfo>, String>,
    },
    Scan {
        generation: u64,
        msg: ScanMsg,
    },
    Table(table::Msg),
    Tree(tree::Msg),
    /// Recalcula el desglose del juego en vivo (como mucho cada 300 ms, `_tree_timer`).
    TreeTick,
    Sidebar(sidebar::Msg),
    Presets(presets::Msg),
    /// Lista guardada leída del disco en un hilo; la de un pedido viejo se descarta.
    SavedList {
        request: u64,
        game: &'static str,
        result: Option<(f64, Vec<ServerInfo>)>,
    },
    /// La base de países cargada en un hilo; la de un pedido viejo se descarta.
    CountryDb {
        request: u64,
        result: Result<Arc<CountryDB>, String>,
    },
    /// ¿Está el exe de cada juego con ruta? (`ExeIconLoader`), de la búsqueda `generation`.
    Installed {
        generation: u64,
        answers: Vec<(&'static str, bool)>,
    },
    /// Clic izquierdo en cualquier parte: se pregunta si el campo de texto quedó con el foco.
    MousePressed,
    /// Esc, aunque lo capture un `text_input` (que así pierde el foco): cierra el modal o
    /// pregunta, como el clic, si el campo sigue con el foco.
    Escape,
    FilterFocused(bool),
    CloseRequested,
    SystemMode(Mode),
    WindowResized(Size),
}

pub struct App {
    pub table: Table,
    pub tree: GameTree,
    pub theme_id: &'static str,
    /// Ancho de la barra lateral (`body_splitter.setSizes([230, …])`).
    pub sidebar_width: f32,
    cfg: Config,
    cfg_path: PathBuf,
    deps: Deps,
    official: OfficialList,
    custom: CustomMasters,
    current: Option<&'static GameDef>,
    /// El escaneo (de internet o de la red local) y la consulta de un servidor.
    pub scan: ScanState,
    /// Última lista completa de cada juego en esta sesión.
    cache: HashMap<&'static str, Vec<ServerInfo>>,
    status: String,
    /// La tabla tiene una lista completa (escaneo terminado sin error o caché): se pueden
    /// podar marcas del árbol.
    tree_prune_ok: bool,
    /// Llegaron servidores que el desglose del árbol aún no cuenta.
    tree_dirty: bool,
    focus: Focus,
    filter_text: String,
    /// Versiones vistas en los servidores del juego actual, en orden de llegada.
    observed_versions: Vec<String>,
    /// Servidores por país al abrir el combo «País» (`_refresh_country_combo`).
    country_counts: HashMap<String, usize>,
    country_request: u64,
    /// Última respuesta de la búsqueda de exes («Solo instalados»); sin respuesta = listo.
    installed: HashMap<&'static str, bool>,
    installed_gen: u64,
    pub saved: SavedLists,
    /// El modal abierto y el foco de antes.
    dialog: Option<(Dialog, Focus)>,
}

/// Corre `f` en el pool bloqueante de tokio: disco y red nunca en el hilo de la UI.
pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())
}

/// Agrega la versión a las vistas si es nueva (`_add_observed_versions`).
fn observe_version(observed: &mut Vec<String>, version: &str) {
    if !version.is_empty() && !observed.iter().any(|v| v == version) {
        observed.push(version.to_owned());
    }
}

impl App {
    /// Lee la config y fija el idioma antes de abrir la ventana; el modo del sistema llega después.
    pub fn boot() -> (App, Task<Message>) {
        let path = default_config_path();
        let cfg = load_config(&path);
        prepare(&cfg, &system_locale_name());
        let deps = Deps {
            scan: Arc::new(scan_stream),
            lan: Arc::new(scan::lan_stream),
            query: Arc::new(scan::query_one),
            // Sin esperar a las tareas bloqueantes que sigan vivas (ver «Cierre sin esperas»).
            exit: Arc::new(|code| std::process::exit(code)),
            steam_key: String::new(), // el llavero llega en la 5f
            country_db: Some(Arc::new(scan::country_db)),
            game_installed: Arc::new(scan::game_installed),
        };
        let (app, scan) = App::new(cfg, path, deps);
        (
            app,
            Task::batch([scan, system::theme().map(Message::SystemMode)]),
        )
    }

    /// Masters oficiales y personales (migra `Config.masters`, como Python) y el juego inicial.
    pub fn new(mut cfg: Config, cfg_path: PathBuf, deps: Deps) -> (App, Task<Message>) {
        let dir = cfg_path.parent().map(PathBuf::from).unwrap_or_default();
        let official = load_official(&dir.join("cache"));
        let custom_path = dir.join(CUSTOM_FILENAME);
        let mut custom = load_custom(&custom_path);
        let saved_masters = cfg.masters.clone();
        if migrate_config_masters(&mut cfg, &mut custom, &official) {
            if save_custom(&custom, &custom_path).is_err() {
                // Sin archivo personal no se vacía la config: se migra de nuevo al reiniciar.
                cfg.masters = saved_masters;
            } else {
                let _ = save_config(&cfg, &cfg_path); // la migración es idempotente
            }
        }
        let saved = SavedLists::open(&dir);
        let first = initial_game(&cfg);
        let mut tree = GameTree::default();
        tree.set_games(App::sorted_game_ids(&cfg));
        let mut app = App {
            table: Table::default(),
            tree,
            theme_id: resolve_theme(&cfg.theme, Mode::None, &cfg.terminal_color),
            sidebar_width: 230.0,
            cfg,
            cfg_path,
            deps,
            official,
            custom,
            current: None,
            scan: ScanState::default(),
            cache: HashMap::new(),
            status: String::new(),
            tree_prune_ok: false,
            tree_dirty: false,
            // Qt arranca con el foco en una casilla de la barra lateral (visto offscreen).
            focus: Focus::Sidebar,
            filter_text: String::new(),
            observed_versions: Vec::new(),
            country_counts: HashMap::new(),
            country_request: 0,
            installed: HashMap::new(),
            installed_gen: 0,
            saved,
            dialog: None,
        };
        let quick = app.cfg.quick_filters.clone();
        app.edit_filter(|f| f.quick = quick);
        app.tree.set_games(app.tree_game_ids());
        app.update_sidebar();
        let task = app.select_game(first);
        let country = app.start_country_db();
        let installed = app.request_installed();
        (app, Task::batch([task, country, installed]))
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SelectGame(id) => return self.select_game(id),
            Message::Refresh => {
                if self.current.is_some() {
                    self.stop_active_scan();
                    self.scan.cancel_query();
                    self.saved.cancel_load(); // F5 manda sobre la lista del disco en camino
                    let top = self.clear_table();
                    let scan = self.start_scan();
                    self.update_sidebar();
                    return Task::batch([top, scan]);
                }
            }
            Message::Stop => {
                self.stop_active_scan();
                self.update_sidebar();
            }
            Message::ScanLan => return self.scan_lan(),
            Message::RefreshSelected => return self.refresh_selected(),
            Message::Query {
                request,
                game,
                result,
            } => self.on_query(request, game, result),
            Message::Scan { generation, msg } if generation == self.scan.generation() => {
                self.on_scan(msg);
            }
            Message::Scan { .. } => {} // de un escaneo viejo
            Message::Table(m) => {
                if matches!(m, table::Msg::RowClicked(_) | table::Msg::BlankClicked) {
                    self.set_focus(Focus::Table);
                }
                return table::update(&mut self.table, m);
            }
            Message::Tree(m) => {
                self.set_focus(Focus::Tree);
                match tree::update(&mut self.tree, m) {
                    Some(tree::Event::GameActivated(id)) => return self.select_game(id),
                    Some(tree::Event::MarksChanged(id))
                        if self.current.is_some_and(|c| c.id == id) =>
                    {
                        self.apply_tree_filter();
                    }
                    _ => {}
                }
            }
            Message::TreeTick => {
                self.tree_dirty = false;
                if let Some(g) = self.current {
                    self.set_tree_breakdown(g.id);
                }
            }
            Message::CloseRequested => {
                if let Some(g) = self.current {
                    self.cfg.ui_state.insert("last_game".into(), g.id.into());
                }
                let _ = save_config(&self.cfg, &self.cfg_path);
                self.stop_active_scan();
                self.scan.cancel_query();
                // La última lista se escribe, sin colgar el cierre con un disco trabado.
                self.saved.flush_within(CLOSE_WAIT);
                (self.deps.exit)(0);
            }
            Message::SystemMode(mode) => {
                self.theme_id = resolve_theme(&self.cfg.theme, mode, &self.cfg.terminal_color);
            }
            Message::WindowResized(size) => self.table.window_h = size.height,
            Message::Sidebar(m) => self.on_sidebar(m),
            Message::Presets(m) => return self.on_presets(m),
            Message::SavedList {
                request,
                game,
                result,
            } if request == self.saved.request => return self.on_saved_list(game, result),
            Message::SavedList { .. } => {} // reemplazada o cancelada (F5)
            Message::CountryDb { request, result } if request == self.country_request => {
                self.on_country_db(result);
            }
            Message::CountryDb { .. } => {} // de un pedido viejo
            Message::Installed {
                generation,
                answers,
            } if generation == self.installed_gen => {
                self.installed.extend(answers);
                if self.cfg.only_installed_games {
                    self.refresh_tree_games();
                }
            }
            Message::Installed { .. } => {}
            Message::Escape if self.dialog.is_some() => return self.cancel_dialog(),
            Message::MousePressed | Message::Escape => {
                return operation::is_focused(FILTER_ID).map(Message::FilterFocused);
            }
            Message::FilterFocused(true) if self.focus != Focus::Dialog => {
                self.set_focus(Focus::Filter);
            }
            Message::FilterFocused(false) if self.focus == Focus::Filter => {
                self.set_focus(Focus::Tree);
            }
            Message::FilterFocused(_) => {}
        }
        Task::none()
    }

    fn save_config(&self) {
        let _ = save_config(&self.cfg, &self.cfg_path);
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn count_text(&self) -> String {
        let n = tr_n(
            "{n} servidor",
            "{n} servidores",
            self.table.model.order().len() as i64,
            &[],
        );
        match self.current {
            Some(g) if !self.tree.marks(g.id).is_empty() => {
                n + &tr(" · filtro de mod/mapa/país activo")
            }
            _ => n,
        }
    }

    pub fn servers(&self) -> &[ServerInfo] {
        self.table.model.servers()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let (p, s) = (palette(self.theme_id), widget_style(self.theme_id));
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let count = label(self.count_text()).align_x(Right);
        let count = match style::status_label(p, s) {
            Some(e) => framed(container(count).padding(style::status_label_padding(s)), e),
            None => container(count)
                .padding(style::status_label_padding(s))
                .into(),
        };
        let status = row![
            label(self.status().to_owned()).width(Fill),
            label(self.progress_text()),
            count,
        ]
        .spacing(12);
        let (bar_style, bar_edges) = style::status_bar(p, s);
        // 22 px como el `QStatusBar` de 1.3.0: con el alto de la línea de Iced (16,9) todo lo de
        // arriba quedaba en medio píxel y los bordes salían borrosos.
        let status = container(status)
            .width(Fill)
            .height(22)
            .center_y(22)
            .padding([0, 10])
            .style(move |_| bar_style);
        let status = match bar_edges {
            Some(e) => framed(status, e),
            None => status.into(),
        };
        let right = column![
            sidebar::filter_row(self),
            table::view(&self.table, self.theme_id)
        ]
        .spacing(6);
        let body = row![sidebar::view(self), right].spacing(6).height(Fill);
        let body = container(body).padding(Padding {
            top: 10.0,
            right: 10.0,
            bottom: 6.0,
            left: 10.0,
        });
        let window = container(Col::new().push(body).push(status)).style(move |_| style::window(p));
        match &self.dialog {
            Some((Dialog::SavePreset(name), _)) => {
                widgets::modal(window, presets::dialog(self, name))
            }
            None => window.into(),
        }
    }

    pub fn theme(&self) -> Theme {
        iced_theme(self.theme_id)
    }

    /// El teclado va a quien tenga el foco (`route_key`); cada clic pregunta por el campo.
    pub fn subscription(&self) -> Subscription<Message> {
        let resize = window::resize_events().map(|(_, size)| Message::WindowResized(size));
        let close = window::close_requests().map(|_| Message::CloseRequested);
        // `colorSchemeChanged` de Qt: `SystemMode` solo cambia algo con el tema «system».
        let mode = system::theme_changes().map(Message::SystemMode);
        let keys = keyboard::listen()
            .with(self.focus)
            .filter_map(|(focus, e)| match e {
                keyboard::Event::KeyPressed { key, .. } => route_key(focus, key),
                _ => None,
            });
        let clicks = event::listen_with(event_message);
        let mut subs = vec![resize, close, mode, keys, clicks];
        // Solo mientras haya servidores que el desglose aún no cuenta.
        if self.tree_dirty {
            subs.push(time::every(Duration::from_millis(300)).map(|_| Message::TreeTick));
        }
        Subscription::batch(subs)
    }
}
