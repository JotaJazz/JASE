//! Estado de la ventana: árbol de juegos, tabla y barra de estado.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::keyboard::{self, Key, key::Named};
use iced::task::Handle;
use iced::theme::Mode;
use iced::widget::{Column as Col, container, row, text};
use iced::{Element, Fill, Padding, Right, Size, Subscription, Task, Theme, system, time, window};
use jase_core::browse::breakdown;
use jase_core::config::{Config, default_config_path, load_config, save_config};
use jase_core::games::{GAMES, GameDef, LaunchStyle, game};
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::masters::{
    CUSTOM_FILENAME, CustomMasters, OfficialList, effective_masters, load_custom, load_official,
    migrate_config_masters, save_custom,
};
use jase_core::models::ServerInfo;
use jase_core::scanner::scan::{ScanEvent, ScanOptions};

use crate::scan::{Deps, ScanMsg, scan_stream};
use crate::startup::{initial_game, prepare, system_locale_name};
use crate::table::{self, Nav, Table};
use crate::theme::{framed, iced_theme, palette, resolve_theme, style, widget_style};
use crate::tree::{self, Event, GameTree};

#[allow(clippy::large_enum_variant)] // por `ScanMsg`
#[derive(Debug, Clone)]
pub enum Message {
    SelectGame(&'static str),
    Refresh,
    Stop,
    Scan {
        generation: u64,
        msg: ScanMsg,
    },
    Table(table::Msg),
    Tree(tree::Msg),
    /// Recalcula el desglose del juego en vivo (como mucho cada 300 ms, `_tree_timer`).
    TreeTick,
    CloseRequested,
    SystemMode(Mode),
    WindowResized(Size),
}

pub struct App {
    pub table: Table,
    pub tree: GameTree,
    pub theme_id: &'static str,
    cfg: Config,
    cfg_path: PathBuf,
    deps: Deps,
    official: OfficialList,
    custom: CustomMasters,
    current: Option<&'static GameDef>,
    /// Sube con cada escaneo nuevo o detenido: los mensajes de otra generación se descartan.
    generation: u64,
    scan: Option<Handle>,
    /// Última lista completa de cada juego en esta sesión.
    cache: HashMap<&'static str, Vec<ServerInfo>>,
    /// Escaneo cancelado: la barra lateral cuenta las filas que quedaron.
    scan_stopped: bool,
    status: String,
    progress: Option<(usize, usize)>,
    /// La tabla tiene una lista completa (escaneo terminado sin error o caché): se pueden
    /// podar marcas del árbol.
    tree_prune_ok: bool,
    /// Llegaron servidores que el desglose del árbol aún no cuenta.
    tree_dirty: bool,
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
    /// Lee la config y fija el idioma antes de abrir la ventana; el modo del sistema llega después.
    pub fn boot() -> (App, Task<Message>) {
        let path = default_config_path();
        let cfg = load_config(&path);
        prepare(&cfg, &system_locale_name());
        let deps = Deps {
            scan: Arc::new(scan_stream),
            // Sin esperar a las tareas bloqueantes que sigan vivas (ver «Cierre sin esperas»).
            exit: Arc::new(|code| std::process::exit(code)),
            steam_key: String::new(), // el llavero llega en la 5f
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
        let first = initial_game(&cfg);
        let mut tree = GameTree::default();
        tree.set_games(App::sorted_game_ids(&cfg));
        let mut app = App {
            table: Table::default(),
            tree,
            theme_id: resolve_theme(&cfg.theme, Mode::None, &cfg.terminal_color),
            cfg,
            cfg_path,
            deps,
            official,
            custom,
            current: None,
            generation: 0,
            scan: None,
            cache: HashMap::new(),
            scan_stopped: false,
            status: String::new(),
            progress: None,
            tree_prune_ok: false,
            tree_dirty: false,
        };
        app.update_sidebar();
        let task = app.select_game(first);
        (app, task)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SelectGame(id) => return self.select_game(id),
            Message::Refresh => {
                if self.current.is_some() {
                    self.stop_active_scan();
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
            Message::Scan { generation, msg } if generation == self.generation => self.on_scan(msg),
            Message::Scan { .. } => {} // de un escaneo viejo
            Message::Table(m) => return table::update(&mut self.table, m),
            Message::Tree(m) => {
                // shortcut: el foco del árbol es «la tabla no lo tiene» hasta el `Focus` de la Task 3.
                self.table.focused = false;
                match tree::update(&mut self.tree, m) {
                    Some(Event::GameActivated(id)) => return self.select_game(id),
                    Some(Event::MarksChanged(id)) if self.current.is_some_and(|c| c.id == id) => {
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
                (self.deps.exit)(0);
            }
            Message::SystemMode(mode) => {
                self.theme_id = resolve_theme(&self.cfg.theme, mode, &self.cfg.terminal_color);
            }
            Message::WindowResized(size) => self.table.window_h = size.height,
        }
        Task::none()
    }

    fn select_game(&mut self, id: &str) -> Task<Message> {
        let Some(g) = game(id) else {
            return Task::none();
        };
        if self.current.is_some_and(|c| c.id == g.id) {
            return Task::none();
        }
        self.stop_active_scan();
        let previous = self.current.replace(g);
        self.tree_prune_ok = false;
        self.tree.set_current(Some(g.id));
        let top = self.clear_table();
        self.apply_tree_filter();
        self.table
            .model
            .set_version_label(move |v| g.version_label(v).to_owned());
        let task = match self.cache.get(g.id) {
            Some(cached) => {
                for info in cached {
                    self.table.model.add_or_update(info.clone());
                }
                self.tree_prune_ok = true;
                self.status = tr_fmt("{name}: lista guardada de la sesión", &[("name", &g.name)]);
                top
            }
            None => Task::batch([top, self.start_scan()]),
        };
        // El desglose del árbol (con caché, poda marcas); el juego que se deja pasa a su caché.
        self.update_sidebar();
        if let Some(p) = previous.filter(|p| p.id != g.id) {
            self.set_tree_breakdown(p.id);
        }
        task
    }

    /// Juegos listos primero (con ruta o que conectan por Steam, sin ruta); dentro de cada
    /// grupo, el orden de `GAMES` (`_sorted_game_ids`).
    pub fn sorted_game_ids(cfg: &Config) -> Vec<&'static str> {
        let ready = |g: &GameDef| {
            cfg.game_paths.get(g.id).is_some_and(|p| !p.is_empty())
                || g.launch_style == LaunchStyle::Steam
        };
        let mut ids: Vec<&'static str> = GAMES.iter().map(|g| g.id).collect();
        ids.sort_by_key(|id| game(id).is_none_or(|g| !ready(g)));
        ids
    }

    /// Textos de los juegos del árbol y el desglose del actual (`_update_sidebar`).
    fn update_sidebar(&mut self) {
        for id in self.tree.game_ids() {
            if let Some(g) = game(id) {
                self.tree
                    .set_label(id, self.game_label(id), g.name.to_owned());
            }
        }
        self.tree_dirty = false;
        if let Some(g) = self.current {
            self.set_tree_breakdown(g.id);
        }
    }

    /// Desglose del juego: el actual sale de la tabla y solo poda con lista completa; otro, de
    /// su lista guardada (siempre poda). Una marca podada del actual cambia el filtro.
    fn set_tree_breakdown(&mut self, id: &'static str) {
        let Some(g) = game(id) else {
            return;
        };
        let model = &self.table.model;
        let (b, prune) = if self.current.is_some_and(|c| c.id == id) {
            let country = |info: &ServerInfo| model.country_code(info).map(str::to_owned);
            let b = breakdown(g, model.servers(), Some(&country));
            (Some(b), self.tree_prune_ok)
        } else {
            let country = |info: &ServerInfo| model.lookup(info);
            let b = self.cache.get(id).map(|s| breakdown(g, s, Some(&country)));
            (b, true)
        };
        if self.tree.set_breakdown(id, b, prune) && self.current.is_some_and(|c| c.id == id) {
            self.apply_tree_filter();
        }
    }

    /// Marcas del juego actual → filtro de la tabla (mods y mapas solo cuentan con juego).
    fn apply_tree_filter(&mut self) {
        let mut f = self.table.model.filter().clone();
        let marks = self
            .current
            .map(|g| self.tree.marks(g.id).clone())
            .unwrap_or_default();
        f.tree_game = self
            .current
            .filter(|_| !marks.mods.is_empty() || !marks.maps.is_empty());
        f.mods = marks.mods;
        f.maps = marks.maps;
        f.countries = marks.countries;
        table::set_filter(&mut self.table, f);
    }

    pub fn clear_tree_marks(&mut self) {
        if let Some(g) = self.current {
            self.tree.clear_marks(g.id);
            self.apply_tree_filter();
        }
    }

    /// Como `model.clear()` de Qt, que también suelta la selección y vuelve arriba; la tarea
    /// mueve el `scrollable`, que si no seguiría desplazado y dibujaría la tabla en blanco.
    fn clear_table(&mut self) -> Task<Message> {
        self.table.model.clear();
        self.table.selected = None;
        table::scroll_to_top(&mut self.table)
    }

    fn start_scan(&mut self) -> Task<Message> {
        let Some(g) = self.current else {
            return Task::none();
        };
        let active = effective_masters(g.id, &self.official, &self.custom);
        let name: [(&str, &dyn std::fmt::Display); 1] = [("game", &g.name)];
        if active.is_empty() {
            self.status = tr_fmt(
                "{game}: no hay masters activos; revísalos en Ajustes → Masters",
                &name,
            );
            return Task::none();
        }
        if g.steam_webapi && self.deps.steam_key.is_empty() {
            self.status = tr_fmt(
                "{game}: falta la clave de API de Steam; ponla en Ajustes → General",
                &name,
            );
            return Task::none();
        }
        self.generation += 1;
        self.tree_prune_ok = false;
        self.scan_stopped = false;
        self.progress = None;
        let opts = ScanOptions {
            steam_key: if g.uses_steam_api() {
                self.deps.steam_key.clone()
            } else {
                String::new()
            },
            steam_include_empty: self.cfg.steam_include_empty,
            ..ScanOptions::default()
        };
        let generation = self.generation;
        let stream = (self.deps.scan)(g, active, opts);
        let (task, handle) =
            Task::run(stream, move |msg| Message::Scan { generation, msg }).abortable();
        self.scan = Some(handle);
        self.status = tr_fmt("Consultando masters de {name}…", &[("name", &g.name)]);
        task
    }

    /// Cancela sin esperar: soltar el future lo corta y sus mensajes quedan viejos.
    fn stop_active_scan(&mut self) {
        if let Some(handle) = self.scan.take() {
            handle.abort();
            self.generation += 1;
            self.tree_prune_ok = false;
            self.tree_dirty = false;
            self.scan_stopped = true;
            self.progress = None;
            // Un escaneo interrumpido no se guarda: se conserva el último completo.
        }
    }

    fn on_scan(&mut self, msg: ScanMsg) {
        match msg {
            ScanMsg::Event(ScanEvent::Server(info)) => {
                self.table.model.add_or_update(info);
                // La fila nueva puede ya no pasar el filtro.
                table::drop_hidden_selection(&mut self.table);
                if let Some(g) = self.current {
                    self.tree
                        .set_label(g.id, self.game_label(g.id), g.name.to_owned());
                }
                self.tree_dirty = true;
            }
            ScanMsg::Event(ScanEvent::Status(s)) => self.status = s,
            ScanMsg::Event(ScanEvent::Progress { done, total }) => {
                self.progress = Some((done, total));
            }
            ScanMsg::Finished(result) => {
                self.scan = None;
                self.progress = None;
                // Solo un escaneo con algún master que respondió reemplaza la lista guardada.
                if let (Some(g), true) = (self.current, result.ok()) {
                    self.cache.insert(g.id, self.table.model.servers().to_vec());
                    self.tree_prune_ok = true;
                    self.status = tr_fmt(
                        "{responded} de {listed} servidores respondieron",
                        &[
                            ("responded", &result.servers_responded),
                            ("listed", &result.servers_listed),
                        ],
                    );
                }
                self.update_sidebar();
            }
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.scan.is_some()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn progress(&self) -> Option<(usize, usize)> {
        self.progress
    }

    /// Texto de la barra de progreso (el `%v / %m` de `QProgressBar`).
    pub fn progress_text(&self) -> String {
        let Some((done, total)) = self.progress else {
            return String::new();
        };
        tr("Consultando %v / %m")
            .replace("%v", &done.to_string())
            .replace("%m", &total.to_string())
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

    /// «CoD2 · 12»: en vivo mientras escanea; si no, la lista guardada; si no, lo que quedó
    /// de un escaneo detenido; si no, «—».
    pub fn game_label(&self, id: &str) -> String {
        let Some(g) = game(id) else {
            return String::new();
        };
        let current = self.current.is_some_and(|c| c.id == id);
        let rows = self.table.model.len();
        let count = if current && self.is_scanning() {
            rows.to_string()
        } else if let Some(cached) = self.cache.get(id) {
            cached.len().to_string()
        } else if current && self.scan_stopped && rows > 0 {
            rows.to_string()
        } else {
            "—".to_owned()
        };
        format!("{} · {count}", g.short_name())
    }

    pub fn servers(&self) -> &[ServerInfo] {
        self.table.model.servers()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let (p, s) = (palette(self.theme_id), widget_style(self.theme_id));
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let list_style = style::sidebar(p, s);
        let games = tree::view(&self.tree, self.theme_id, !self.table.focused);
        let sidebar = container(games)
            .width(230)
            .padding(list_style.border.width)
            .style(move |_| list_style);
        let sidebar = match style::table_frame(p, s).1 {
            Some(e) => framed(sidebar, e),
            None => sidebar.into(),
        };
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
        let status = container(status)
            .width(Fill)
            .padding([2, 10])
            .style(move |_| bar_style);
        let status = match bar_edges {
            Some(e) => framed(status, e),
            None => status.into(),
        };
        let body = row![sidebar, table::view(&self.table, self.theme_id)]
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

    /// El teclado va a la tabla o al árbol, según quién tenga el foco.
    pub fn subscription(&self) -> Subscription<Message> {
        let resize = window::resize_events().map(|(_, size)| Message::WindowResized(size));
        let close = window::close_requests().map(|_| Message::CloseRequested);
        // `colorSchemeChanged` de Qt: `SystemMode` solo cambia algo con el tema «system».
        let mode = system::theme_changes().map(Message::SystemMode);
        let keys = if self.table.focused {
            keyboard::listen().filter_map(|e| match e {
                keyboard::Event::KeyPressed { key, .. } => {
                    nav_for(key).map(|n| Message::Table(table::Msg::Nav(n)))
                }
                _ => None,
            })
        } else {
            keyboard::listen().filter_map(|e| match e {
                keyboard::Event::KeyPressed { key, .. } => {
                    tree_nav_for(key).map(|n| Message::Tree(tree::Msg::Nav(n)))
                }
                _ => None,
            })
        };
        let mut subs = vec![resize, close, mode, keys];
        // Solo mientras haya servidores que el desglose aún no cuenta.
        if self.tree_dirty {
            subs.push(time::every(Duration::from_millis(300)).map(|_| Message::TreeTick));
        }
        Subscription::batch(subs)
    }
}
