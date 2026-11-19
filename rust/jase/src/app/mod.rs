//! Estado de la ventana: árbol de juegos, tabla y barra de estado. `update`, `view` y la
//! suscripción viven aquí; cada dominio, en su archivo (`focus`, `filters`, `games`, `saved`,
//! `scanning`, `dialogs`, `connect`, `direct`, `favorites`).

mod add_server;
mod auto_refresh;
mod background;
mod connect;
pub(crate) mod dialogs;
mod direct;
mod favorites;
mod filters;
mod focus;
mod friends;
mod games;
mod history;
mod links;
mod menus;
mod notify;
mod rcon;
mod saved;
mod scanning;
mod settings;
mod status;
mod subscription;
mod tools;
mod watch;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::advanced::widget::operate;
use iced::advanced::widget::operation::focusable;
use iced::keyboard;
use iced::theme::Mode;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{Column as Col, column, container, image, mouse_area, operation, row};
use iced::{Element, Fill, Padding, Point, Size, Task, Theme, mouse, system, window};
use jase_core::config::{Config, default_config_path, load_config, save_config};
use jase_core::games::GameDef;
use jase_core::geo::CountryDB;
use jase_core::history::History;
use jase_core::masters::{
    CUSTOM_FILENAME, CustomMasters, OfficialList, load_custom, load_official,
    migrate_config_masters, save_custom,
};
use jase_core::models::ServerInfo;

pub use auto_refresh::AutoMsg;
pub use background::{BackgroundMsg, UPDATE_POLL, UpdateChoice};
pub use connect::ConnectMsg;
pub use dialogs::{Dialog, PROMPT_ID, PromptMsg, about_lines};
pub use direct::{DIRECT_CONCURRENCY, DirectMsg, DirectView};
pub use favorites::GroupOption;
pub use focus::{Focus, event_message, route_key};
pub use friends::{FriendsMsg, NOTIFY_BATCH};
pub use history::SAVE_INTERVAL;
pub use links::{LINK_RETRY, LinkMsg};
pub use notify::{Click, Prompt};
pub use rcon::RconMsg;
pub use settings::{MastersMsg, PATH_DEBOUNCE, STEAM_KEY_URL, SettingsMsg};
pub use subscription::{key_message, window_message};
pub use tools::{NowMsg, RconKey, SearchMsg, Tool, tool_settings};
pub use watch::{SLOT_INTERVAL, SLOT_MAX, WATCH_INTERVAL, WatchMsg, has_free_slot, party_value};

use crate::actions::{self, Menu};
use crate::bars::{self, Shortcut};
use crate::card;
use crate::chart::ChartVm;
use crate::details::{self, PlayerCol, PlayerSort};
use crate::menu::{self, Placement};
use crate::presets;
use crate::saved_lists::SavedLists;
use crate::scan::{Deps, ScanMsg, ScanState};
use crate::sidebar;
use crate::startup::{initial_game, prepare, system_locale_name};
use crate::table::{self, Table};
use crate::theme::{iced_theme, palette, resolve_theme, style};
use crate::tree::{self, GameTree};
use crate::widgets::{self, FILTER_ID};
use crate::window_state::{
    self, DETAILS_RATIO, DetailPane, Drag, Geometry, RIGHT_RATIO, RightPane,
};

/// Lo que el cierre espera al escritor de listas guardadas (`CLOSE_WAIT_MS` de Python), y
/// aparte al del historial.
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
    /// Menús y acciones compartidas.
    Actions(actions::Msg),
    /// El botón que cierra un diálogo («Cerrar» de *Acerca de*).
    CloseDialog,
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
    /// ¿Está el exe de cada juego con ruta?, y su icono (`ExeIconLoader`), de la búsqueda
    /// `generation`.
    Installed {
        generation: u64,
        answers: Vec<(&'static str, bool, Option<image::Handle>)>,
    },
    /// Clic izquierdo en cualquier parte: se pregunta si el campo de texto quedó con el foco.
    MousePressed,
    /// Esc, aunque lo capture un `text_input` (que así pierde el foco): cierra el modal o
    /// pregunta, como el clic, si el campo sigue con el foco.
    Escape,
    FilterFocused(bool),
    /// Pide si la ventana está maximizada y sigue en `Close`.
    CloseRequested,
    /// Guarda el estado de la ventana y la config, y sale.
    Close {
        maximized: bool,
    },
    SystemMode(Mode),
    WindowResized(Size),
    /// La ventana se movió a este punto (Wayland no lo informa).
    WindowMoved(Point),
    /// La ventana se abrió (con su posición, salvo en Wayland): se ajusta al monitor.
    WindowOpened(window::Id, Option<Point>),
    /// Tamaño del monitor de la ventana (`window::monitor_size`).
    MonitorSize(window::Id, Option<Size>),
    /// Respuesta de `is_maximized` tras mover o cambiar el tamaño.
    WindowMaximized {
        request: u64,
        maximized: bool,
    },
    /// El divisor entre la barra lateral y la tabla se empieza a arrastrar.
    SplitterStart,
    /// La `x` del ratón mientras dura un arrastre (columna o divisor).
    DragMoved(f32),
    DragEnd,
    /// Un atajo de teclado (`event_message`); `update` decide según el foco.
    Shortcut(Shortcut),
    /// Venció el plazo del mensaje `seq` de la barra de estado.
    StatusExpired(u64),
    /// Se arrastró el divisor entre la tabla y la ficha, o entre jugadores y reglas.
    RightResized(pane_grid::ResizeEvent),
    DetailsResized(pane_grid::ResizeEvent),
    /// Clic en un título de la tabla de jugadores.
    PlayerSort(PlayerCol),
    /// Imagen del mapa leída en un hilo; la de un pedido viejo se descarta.
    MapImage {
        request: u64,
        image: Option<iced::widget::image::Handle>,
    },
    /// El historial de ocupación leído en un hilo al arrancar.
    HistoryLoaded(Arc<History>),
    /// Venció `SAVE_INTERVAL`: se guarda el historial.
    HistorySave,
    /// La clave de la API web de Steam leída del llavero al arrancar ("" = no hay).
    SteamKey(String),
    Connect(ConnectMsg),
    /// Lo que llega del diálogo de contraseña y de los avisos.
    Prompt(PromptMsg),
    /// Las vistas Favoritos y Recientes.
    Direct(DirectMsg),
    /// «Agregar servidor…»: el diálogo, *Detectar* y la consulta de lo agregado.
    AddServer(crate::add_server::Msg),
    /// Exportar e importar favoritos: los selectores y el archivo.
    Files(crate::files::FilesMsg),
    /// Enlaces `jase://` (de `argv` o de otra instancia) y su confirmación.
    Link(LinkMsg),
    /// El diálogo de Ajustes: sus pestañas, rutas, masters y la clave de Steam.
    Settings(SettingsMsg),
    /// Tareas de fondo: la lista oficial de masters y la versión nueva de JASE.
    Background(BackgroundMsg),
    AutoRefresh(AutoMsg),
    Search(SearchMsg),
    Now(NowMsg),
    Rcon(RconMsg),
    Watch(WatchMsg),
    Friends(FriendsMsg),
    /// Se cerró una ventana: su herramienta suelta lo suyo.
    WindowClosed(window::Id),
    /// Esc en una herramienta: se pide cerrarla.
    CloseTool(window::Id),
    /// Una tecla no capturada en una herramienta.
    ToolKey(window::Id, keyboard::Key),
}

pub struct App {
    pub table: Table,
    pub tree: GameTree,
    pub theme_id: &'static str,
    /// Ancho de la barra lateral (`body_splitter.setSizes([230, …])`).
    pub sidebar_width: f32,
    /// Tabla | ficha con jugadores y reglas (`right_splitter`).
    pub right: pane_grid::State<RightPane>,
    /// Jugadores | reglas (`details_splitter`).
    pub details: pane_grid::State<DetailPane>,
    /// Orden de la tabla de jugadores: dura la sesión, como en Qt.
    pub player_sort: PlayerSort,
    /// La imagen del mapa de la ficha (`sync_card`).
    pub(crate) map: card::MapImage,
    /// El gráfico de ocupación de la ficha (`sync_card`); `None` = oculto.
    pub(crate) chart: Option<ChartVm>,
    /// Historial de ocupación; hasta que se lee (`history_loaded`) no se registra ni guarda.
    pub history: History,
    pub history_loaded: bool,
    pub(crate) history_writes: history::Writes,
    /// Tamaño y posición que informa el sistema (lo que se guarda al cerrar).
    pub(crate) window: Geometry,
    /// El último tamaño y posición sin maximizar (lo que se guarda si cierra maximizada).
    pub(crate) normal: Geometry,
    /// Tamaño del monitor de la ventana, si ya se supo.
    pub(crate) monitor: Option<Size>,
    /// Número del último `is_maximized` pedido: las respuestas viejas se descartan.
    pub(crate) maximized_request: u64,
    /// El borde de columna o el divisor que se arrastra.
    pub(crate) drag: Option<Drag>,
    pub(crate) cfg: Config,
    cfg_path: PathBuf,
    pub(crate) deps: Deps,
    official: OfficialList,
    custom: CustomMasters,
    current: Option<&'static GameDef>,
    /// El escaneo (de internet o de la red local) y la consulta de un servidor.
    pub scan: ScanState,
    /// Última lista completa de cada juego en esta sesión.
    cache: HashMap<&'static str, Vec<ServerInfo>>,
    status: String,
    /// Número del mensaje vigente: el plazo de uno reemplazado no borra el nuevo.
    status_seq: u64,
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
    /// Un pedido de la base de países en curso (`true` = la vuelve a bajar).
    country_busy: Option<bool>,
    /// Última respuesta de la búsqueda de exes («Solo instalados»); sin respuesta = listo.
    installed: HashMap<&'static str, bool>,
    installed_gen: u64,
    pub saved: SavedLists,
    /// El modal abierto y el foco de antes.
    dialog: Option<(Dialog, Focus)>,
    /// El menú abierto (sus submenús los lleva el widget).
    menu: Option<Menu>,
    /// La lista de un combo de la barra lateral o de herramientas está abierta.
    combo_open: Option<sidebar::Combo>,
    /// Lo último que informó el sistema (para el tema «Sistema»).
    system_mode: Mode,
    /// `None` = el llavero aún no respondió.
    steam_key: Option<String>,
    /// El juego de Steam que espera la clave para escanear.
    steam_wait: Option<&'static str>,
    connect: connect::Connect,
    direct: direct::Direct,
    /// El grupo del combo de la barra: `None` = todos, `Some("")` = sin grupo (solo cuenta en
    /// la vista Favoritos).
    group: Option<String>,
    add_server: add_server::AddServer,
    /// Un selector de archivos abierto (no es modal): *Exportar*/*Importar* apagadas.
    files_open: bool,
    links: links::Links,
    /// Lo editable de Ajustes mientras está abierto (`Dialog::Settings`).
    settings: Option<crate::settings::Form>,
    /// La consulta vigente del exe de cada ruta de Ajustes.
    settings_paths: settings::PathRequests,
    /// *Buscar juegos* (del menú o de Ajustes) y su cancelación.
    find: settings::Find,
    /// La fila por elegir cuando llegue (refresco automático de una vista que se vacía).
    pending_select: Option<jase_core::models::ServerAddress>,
    masters_request: background::Request,
    updates: background::Updates,
    /// La principal y las herramientas abiertas.
    pub(crate) windows: tools::Windows,
    search: Option<crate::player_search::PlayerSearch>,
    now: tools::Now,
    /// Las consolas RCON abiertas, por ventana.
    rcon: HashMap<window::Id, crate::rcon_console::Console>,
    watch: watch::Watch,
    friends_form: Option<crate::friends_dialog::FriendsForm>,
    /// El jugador del clic derecho (`Menu::Players`).
    player_menu: Option<String>,
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
        App::boot_with(load_config(&default_config_path()), None, None)
    }

    /// Como [`App::boot`], con la config ya leída, el enlace de `argv` y dónde escuchar a
    /// otras instancias. Abre la principal como quedó y, ya abierta, sigue con el arranque.
    pub fn boot_with(
        cfg: Config,
        link: Option<String>,
        endpoint: Option<PathBuf>,
    ) -> (App, Task<Message>) {
        let path = default_config_path();
        prepare(&cfg, &system_locale_name());
        let deps = Deps::real(&path, endpoint);
        let settings = window_state::window_settings(&cfg);
        let (mut app, scan) = App::new(cfg, path, deps);
        let offer = app.offer_find_games();
        let mode = system::theme().map(Message::SystemMode);
        let link = App::link_at_start(link);
        let mut boot = Some(Task::batch([scan, mode, link, offer]));
        let open = app.open_main(settings);
        (
            app,
            open.then(move |_| boot.take().unwrap_or_else(Task::none)),
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
            sidebar_width: window_state::SIDEBAR_WIDTH,
            right: window_state::right_panes(RIGHT_RATIO),
            details: window_state::detail_panes(DETAILS_RATIO),
            player_sort: PlayerSort::default(),
            map: card::MapImage::default(),
            chart: None,
            history: History::new(history::history_path(&cfg_path)),
            history_loaded: false,
            history_writes: history::Writes::default(),
            window: Geometry::default(),
            normal: Geometry::default(),
            monitor: None,
            maximized_request: 0,
            drag: None,
            cfg,
            cfg_path,
            deps,
            official,
            custom,
            current: None,
            scan: ScanState::default(),
            cache: HashMap::new(),
            status: String::new(),
            status_seq: 0,
            tree_prune_ok: false,
            tree_dirty: false,
            // Qt arranca con el foco en una casilla de la barra lateral (visto offscreen).
            focus: Focus::Sidebar,
            filter_text: String::new(),
            observed_versions: Vec::new(),
            country_counts: HashMap::new(),
            country_request: 0,
            country_busy: None,
            installed: HashMap::new(),
            installed_gen: 0,
            saved,
            dialog: None,
            menu: None,
            combo_open: None,
            system_mode: Mode::None,
            steam_key: None,
            steam_wait: None,
            connect: connect::Connect::default(),
            direct: direct::Direct::default(),
            group: None,
            add_server: add_server::AddServer::default(),
            files_open: false,
            links: links::Links::default(),
            settings: None,
            settings_paths: settings::PathRequests::default(),
            find: settings::Find::default(),
            pending_select: None,
            masters_request: background::Request::default(),
            updates: background::Updates::default(),
            windows: tools::Windows::default(),
            search: None,
            now: tools::Now::default(),
            rcon: HashMap::new(),
            watch: watch::Watch::default(),
            friends_form: None,
            player_menu: None,
        };
        app.watch.friends = friends::Friends::new(&app.cfg.friends);
        window_state::restore(&mut app);
        let quick = app.cfg.quick_filters.clone();
        app.edit_filter(|f| f.quick = quick);
        app.tree.set_games(app.tree_game_ids());
        app.update_sidebar();
        let task = app.select_game(first);
        let country = app.start_country_db(false).unwrap_or_else(Task::none);
        let installed = app.request_installed();
        let history = app.load_history();
        let key = Task::batch([app.load_steam_key(), app.start_background()]);
        (app, Task::batch([task, country, installed, history, key]))
    }

    /// Reparte el mensaje y pone al día la ficha (la imagen del mapa sigue a la selección).
    pub fn update(&mut self, msg: Message) -> Task<Message> {
        let task = self.dispatch(msg);
        let sync = card::sync_card(self);
        if sync.units() == 0 {
            task
        } else {
            Task::batch([task, sync])
        }
    }

    fn dispatch(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SelectGame(id) => return self.select_game(id),
            Message::Refresh => return self.refresh(false),
            Message::Stop => {
                self.steam_wait = None; // *Detener* también corta la espera de la clave
                self.stop_active_scan();
                self.cancel_direct_query();
                self.update_sidebar();
            }
            Message::ScanLan => return self.scan_lan(),
            Message::RefreshSelected => return self.refresh_selected(),
            Message::Query {
                request,
                game,
                result,
            } => return self.on_query(request, game, result),
            Message::Scan { generation, msg } if generation == self.scan.generation() => {
                return self.on_scan(msg);
            }
            Message::Scan { .. } => {} // de un escaneo viejo
            Message::Table(m) => {
                if matches!(
                    m,
                    table::Msg::RowClicked(_)
                        | table::Msg::RowDoubleClicked(_)
                        | table::Msg::BlankClicked
                        | table::Msg::RightClicked(_)
                ) {
                    self.set_focus(Focus::Table);
                }
                // El clic derecho no le quita el foco al `text_input` (solo el izquierdo); Qt sí.
                let unfocus = if matches!(m, table::Msg::RightClicked(_)) {
                    self.menu = Some(Menu::Context);
                    operate(focusable::unfocus::<()>()).discard()
                } else {
                    Task::none()
                };
                if let table::Msg::ResizeStart(col) = m {
                    window_state::start_column(self, col);
                }
                let double = matches!(m, table::Msg::RowDoubleClicked(_));
                let task = Task::batch([unfocus, table::update(&mut self.table, m)]);
                self.clear_stale_pending();
                if double {
                    return Task::batch([task, self.connect_selected()]);
                }
                return task;
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
            Message::CloseRequested => return window_state::close(self.windows.main),
            Message::Close { maximized } => {
                window_state::save(self, maximized);
                if let Some(g) = self.current {
                    self.cfg.ui_state.insert("last_game".into(), g.id.into());
                }
                let _ = save_config(&self.cfg, &self.cfg_path);
                self.stop_active_scan();
                self.scan.cancel_query();
                self.cancel_direct_query();
                self.stop_download();
                self.stop_friend_batch();
                // La última lista se escribe, sin colgar el cierre con un disco trabado.
                self.saved.flush_within(CLOSE_WAIT);
                self.save_history_within(CLOSE_WAIT);
                (self.deps.exit)(0);
            }
            Message::SystemMode(mode) => {
                self.system_mode = mode;
                self.apply_theme();
            }
            Message::WindowResized(size) => {
                self.table.window_h = size.height;
                return window_state::resized(self, size);
            }
            Message::WindowMoved(p) => return window_state::moved(self, p),
            Message::WindowOpened(id, p) => return window_state::opened(self, id, p),
            Message::MonitorSize(id, size) => return window_state::on_monitor(self, id, size),
            Message::WindowMaximized { request, maximized } => {
                window_state::on_maximized(self, request, maximized);
            }
            Message::SplitterStart => window_state::start_sidebar(self),
            Message::DragMoved(x) => window_state::drag_moved(self, x),
            Message::DragEnd => self.drag = None,
            // Con un menú abierto lo escrito no entra en el campo (`QMenu` toma el teclado).
            Message::Sidebar(sidebar::Msg::Text(_)) if self.menu.is_some() => {}
            Message::Sidebar(m) => self.on_sidebar(m),
            Message::Presets(m) => {
                self.menu = None; // también lo elegido en «Filtros guardados»
                return self.on_presets(m);
            }
            Message::Actions(m) => return self.on_action(m),
            Message::CloseDialog => return self.close_dialog(),
            Message::SavedList {
                request,
                game,
                result,
            } if request == self.saved.request => return self.on_saved_list(game, result),
            Message::SavedList { .. } => {} // reemplazada o cancelada (F5)
            Message::CountryDb { request, result } if request == self.country_request => {
                return self.on_country_db(result);
            }
            Message::CountryDb { .. } => {} // de un pedido viejo
            Message::Installed {
                generation,
                answers,
            } if generation == self.installed_gen => self.on_installed(answers),
            Message::Installed { .. } => {}
            // Esc cierra primero el menú, luego el diálogo.
            Message::Escape if self.menu.is_some() => self.menu = None,
            Message::Escape if self.dialog.is_some() => return self.cancel_dialog(),
            Message::MousePressed | Message::Escape => {
                return operation::is_focused(FILTER_ID).map(Message::FilterFocused);
            }
            Message::FilterFocused(true) if self.focus != Focus::Dialog => {
                self.set_focus(Focus::Filter);
            }
            // Qt deja el foco donde estaba (el campo no toma flechas): no van a nadie.
            Message::FilterFocused(false) if self.focus == Focus::Filter => {
                self.set_focus(Focus::Sidebar);
            }
            Message::FilterFocused(_) => {}
            Message::Shortcut(s) => return self.on_shortcut(s),
            Message::StatusExpired(seq) => self.on_status_expired(seq),
            Message::RightResized(e) => window_state::resize(&mut self.right, e),
            Message::DetailsResized(e) => window_state::resize(&mut self.details, e),
            Message::PlayerSort(col) => self.player_sort = details::click(self.player_sort, col),
            Message::MapImage { request, image } => card::on_map_image(self, request, image),
            Message::HistoryLoaded(h) => self.on_history_loaded(h),
            Message::HistorySave => return self.save_history(),
            Message::SteamKey(key) => return self.on_steam_key(key),
            Message::Connect(m) => return self.on_connect(m),
            Message::Prompt(m) => return self.on_prompt(m),
            Message::Direct(m) => return self.on_direct(m),
            Message::AddServer(m) => return self.on_add_server(m),
            Message::Files(m) => return self.on_files(m),
            Message::Link(m) => return self.on_link(m),
            Message::Settings(m) => return self.on_settings(m),
            Message::Background(m) => return self.on_background(m),
            Message::AutoRefresh(m) => return self.on_auto_refresh(m),
            Message::Search(m) => return self.on_search(m),
            Message::Now(m) => return self.on_now(m),
            Message::Rcon(m) => return self.on_rcon(m),
            Message::Watch(m) => return self.on_watch(m),
            Message::Friends(m) => return self.on_friends(m),
            Message::WindowClosed(id) => self.on_window_closed(id),
            Message::CloseTool(id) => return window::close(id),
            Message::ToolKey(id, key) => return self.on_tool_key(id, key),
        }
        Task::none()
    }

    fn save_config(&self) {
        let _ = save_config(&self.cfg, &self.cfg_path);
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn servers(&self) -> &[ServerInfo] {
        self.table.model.servers()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let p = palette(self.theme_id);
        // La tabla mide su alto visible con su propio `scrollable`, dentro del panel.
        let right = PaneGrid::new(&self.right, |_, pane, _| {
            pane_grid::Content::new(match pane {
                RightPane::Table => menu::popup(
                    table::view(&self.table, self.theme_id),
                    actions::open_entries(self, Menu::Context),
                    Placement::Cursor,
                    Message::Actions(actions::Msg::Close),
                    self.theme_id,
                ),
                RightPane::Lower => column![card::view(self), details::view(self)]
                    .spacing(card::LOWER_SPACING)
                    .into(),
            })
        })
        .spacing(window_state::SPLITTER_WIDTH)
        .min_size(card::lower_min_height(self))
        .on_resize(0, Message::RightResized)
        .style(move |_| style::splitter());
        let body = row![sidebar::view(self), sidebar::splitter(), right].height(Fill);
        let body = container(body).padding(Padding {
            top: 10.0,
            right: 10.0,
            bottom: 6.0,
            left: 10.0,
        });
        let window = container(
            Col::new()
                .push(actions::menu_bar(self))
                .push(bars::toolbar(self))
                .push(body)
                .push(bars::status_bar(self)),
        )
        .style(move |_| style::window(p));
        // Durante un arrastre el cursor sigue siendo el de redimensionar, como en Qt.
        let window = mouse_area(window);
        let window = if self.drag.is_some() {
            window.interaction(mouse::Interaction::ResizingHorizontally)
        } else {
            window
        };
        match self.dialog_view() {
            Some(card) => widgets::modal(window, card),
            None => window.into(),
        }
    }

    pub fn theme(&self) -> Theme {
        iced_theme(self.theme_id)
    }
}
