//! Estado de la ventana: árbol de juegos, tabla y barra de estado.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::keyboard::{self, Key, key::Named};
use iced::task::Handle;
use iced::theme::Mode;
use iced::widget::{Column as Col, column, container, operation, row, text};
use iced::{
    Element, Event, Fill, Padding, Right, Size, Subscription, Task, Theme, event, mouse, system,
    time, window,
};
use jase_core::browse::breakdown;
use jase_core::config::{Config, default_config_path, load_config, save_config, version_for};
use jase_core::games::{GAMES, GameDef, LaunchStyle, game};
use jase_core::geo::{CountryDB, DB_FILENAME};
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::masters::{
    CUSTOM_FILENAME, CustomMasters, OfficialList, effective_masters, load_custom, load_official,
    migrate_config_masters, save_custom,
};
use jase_core::models::{QuickFilters, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanOptions};

use crate::scan::{self, Deps, ScanMsg, scan_stream};
use crate::sidebar::{self, CountryOption, VersionOption};
use crate::startup::{initial_game, prepare, system_locale_name};
use crate::table::{self, Nav, ServerFilter, Table};
use crate::theme::{framed, iced_theme, palette, resolve_theme, style, widget_style};
use crate::tree::{self, GameTree};
use crate::widgets::FILTER_ID;

/// Quién se queda con el teclado: las flechas van a la tabla o al árbol, y a nadie mientras
/// se escribe en el campo (el `text_input` no captura ↑/↓) o hay un diálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Focus {
    Tree,
    Table,
    Filter,
    Dialog,
}

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
    Sidebar(sidebar::Msg),
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
    FilterFocused(bool),
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
}

/// Corre `f` en el pool bloqueante de tokio: disco y red nunca en el hilo de la UI.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())
}

/// La tecla según quién tiene el foco (`subscription` y los tests).
pub fn route_key(focus: Focus, key: Key) -> Option<Message> {
    match focus {
        Focus::Table => nav_for(key).map(|n| Message::Table(table::Msg::Nav(n))),
        Focus::Tree => tree_nav_for(key).map(|n| Message::Tree(tree::Msg::Nav(n))),
        Focus::Filter | Focus::Dialog => None,
    }
}

/// Agrega la versión a las vistas si es nueva (`_add_observed_versions`).
fn observe_version(observed: &mut Vec<String>, version: &str) {
    if !version.is_empty() && !observed.iter().any(|v| v == version) {
        observed.push(version.to_owned());
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
            focus: Focus::Tree,
            filter_text: String::new(),
            observed_versions: Vec::new(),
            country_counts: HashMap::new(),
            country_request: 0,
            installed: HashMap::new(),
            installed_gen: 0,
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
            Message::Table(m) => {
                if matches!(m, table::Msg::RowClicked(_)) {
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
                (self.deps.exit)(0);
            }
            Message::SystemMode(mode) => {
                self.theme_id = resolve_theme(&self.cfg.theme, mode, &self.cfg.terminal_color);
            }
            Message::WindowResized(size) => self.table.window_h = size.height,
            Message::Sidebar(m) => self.on_sidebar(m),
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
            Message::MousePressed => {
                return operation::is_focused(FILTER_ID).map(Message::FilterFocused);
            }
            Message::FilterFocused(true) => self.set_focus(Focus::Filter),
            Message::FilterFocused(false) if self.focus == Focus::Filter => {
                self.set_focus(Focus::Tree);
            }
            Message::FilterFocused(false) => {}
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
        self.refresh_tree_games(); // con «Solo instalados», el actual siempre tiene fila
        self.tree.set_current(Some(g.id));
        let top = self.clear_table();
        self.apply_tree_filter();
        // `_populate_version_combo`: la versión guardada del juego (o la por defecto) filtra.
        self.observed_versions.clear();
        let version = version_for(g, &self.cfg).to_owned();
        self.edit_filter(|f| f.version = version);
        self.table
            .model
            .set_version_label(move |v| g.version_label(v).to_owned());
        let task = match self.cache.get(g.id) {
            Some(cached) => {
                for info in cached {
                    observe_version(&mut self.observed_versions, &info.version);
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

    /// Cambia el filtro de la tabla (y suelta la selección que deja de verse).
    fn edit_filter(&mut self, edit: impl FnOnce(&mut ServerFilter)) {
        let mut f = self.table.model.filter().clone();
        edit(&mut f);
        table::set_filter(&mut self.table, f);
    }

    /// Marcas del juego actual → filtro de la tabla (mods y mapas solo cuentan con juego).
    fn apply_tree_filter(&mut self) {
        let marks = self
            .current
            .map(|g| self.tree.marks(g.id).clone())
            .unwrap_or_default();
        let game = self
            .current
            .filter(|_| !marks.mods.is_empty() || !marks.maps.is_empty());
        self.edit_filter(|f| {
            f.tree_game = game;
            f.mods = marks.mods;
            f.maps = marks.maps;
            f.countries = marks.countries;
        });
    }

    fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        // Sin foco, 98 y Luna pintan la selección de la tabla con la cara (`:!focus`).
        self.table.focused = focus == Focus::Table;
    }

    fn save_config(&self) {
        let _ = save_config(&self.cfg, &self.cfg_path);
    }

    /// Filtros rápidos, texto y versión (`_on_quick_filters_changed`, `_on_version_changed`);
    /// cada cambio se guarda en la config.
    fn on_sidebar(&mut self, m: sidebar::Msg) {
        use sidebar::Msg as S;
        if let S::Text(t) = m {
            self.set_focus(Focus::Filter);
            self.edit_filter(|f| f.set_text(&t));
            self.filter_text = t;
            return;
        }
        // Un clic en un filtro rápido devuelve las flechas al árbol.
        self.set_focus(Focus::Tree);
        let q = &mut self.cfg.quick_filters;
        match m {
            S::HideEmpty(b) => q.hide_empty = b,
            S::HideFull(b) => q.hide_full = b,
            S::HidePassword(b) => q.hide_password = b,
            S::HideBots(b) => q.hide_bots = b,
            S::MaxPing(ms) => q.max_ping = ms,
            S::Country(code) => q.country = code,
            S::CountryOpened => {
                let model = &self.table.model;
                let mut counts = HashMap::new();
                for code in model.servers().iter().filter_map(|i| model.country_code(i)) {
                    *counts.entry(code.to_owned()).or_insert(0) += 1;
                }
                self.country_counts = counts;
                return;
            }
            S::OnlyInstalled(on) => {
                self.cfg.only_installed_games = on;
                self.save_config();
                self.refresh_tree_games();
                return;
            }
            S::Version(v) => {
                let Some(g) = self.current else {
                    return;
                };
                self.cfg.version_filter.insert(g.id.to_owned(), v.clone());
                self.edit_filter(|f| f.version = v);
                self.save_config();
                return;
            }
            S::Text(_) => unreachable!("atendido arriba"),
        }
        let quick = self.cfg.quick_filters.clone();
        self.edit_filter(|f| f.quick = quick);
        self.save_config();
    }

    /// Pide la base de países (`_start_country_db`); llega en `Message::CountryDb`.
    fn start_country_db(&mut self) -> Task<Message> {
        let Some(load) = self.deps.country_db.clone() else {
            return Task::none();
        };
        self.country_request += 1;
        let request = self.country_request;
        let dir = self
            .cfg_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default();
        let path = dir.join("cache").join(DB_FILENAME);
        Task::perform(
            blocking(move || load(&path, false).map(Arc::new)),
            move |r| Message::CountryDb {
                request,
                result: r.and_then(|db| db),
            },
        )
    }

    /// `_on_country_db_loaded`: el error solo va a la barra de estado.
    fn on_country_db(&mut self, result: Result<Arc<CountryDB>, String>) {
        match result {
            Ok(db) => {
                let lookup = move |ip: &str| db.lookup(ip).map(str::to_owned);
                self.table.model.set_country_lookup(Some(Arc::new(lookup)));
                table::drop_hidden_selection(&mut self.table);
                // La carpeta «Países» necesita la base.
                if let Some(g) = self.current {
                    self.set_tree_breakdown(g.id);
                }
            }
            Err(e) if e.is_empty() => self.status = tr("Base de países no disponible"),
            Err(e) => self.status = e,
        }
    }

    /// Busca el exe de cada juego con ruta en un hilo (`_refresh_game_icons`); un cambio de
    /// tema no la repite (sin icono de exe hasta la 5e), así que sus respuestas se conservan.
    fn request_installed(&mut self) -> Task<Message> {
        let paths: Vec<(&'static GameDef, String)> = GAMES
            .iter()
            .filter_map(|g| {
                let dir = self.cfg.game_paths.get(g.id).filter(|d| !d.is_empty())?;
                Some((g, dir.clone()))
            })
            .collect();
        if paths.is_empty() {
            return Task::none();
        }
        self.installed_gen += 1;
        let generation = self.installed_gen;
        let installed = self.deps.game_installed.clone();
        Task::perform(
            blocking(move || {
                paths
                    .into_iter()
                    .map(|(g, dir)| (g.id, installed(g, &dir)))
                    .collect::<Vec<_>>()
            }),
            move |r| Message::Installed {
                generation,
                answers: r.unwrap_or_default(),
            },
        )
    }

    /// Filas del árbol: con «Solo instalados», los juegos con exe encontrado o que conectan
    /// por Steam, más el actual (`_tree_game_ids`). Sin respuesta, una ruta cuenta como lista.
    fn tree_game_ids(&self) -> Vec<&'static str> {
        let ids = App::sorted_game_ids(&self.cfg);
        if !self.cfg.only_installed_games {
            return ids;
        }
        let installed = |id: &str| {
            self.cfg.game_paths.get(id).is_some_and(|p| !p.is_empty())
                && self.installed.get(id).copied().unwrap_or(true)
        };
        ids.into_iter()
            .filter(|&id| {
                self.current.is_some_and(|c| c.id == id)
                    || game(id).is_some_and(|g| g.launch_style == LaunchStyle::Steam)
                    || installed(id)
            })
            .collect()
    }

    fn refresh_tree_games(&mut self) {
        let ids = self.tree_game_ids();
        if ids != self.tree.game_ids() {
            self.tree.set_games(ids);
            self.update_sidebar();
        }
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// La flecha o tecla de navegación como mensaje para quien tiene el foco.
    pub fn key_message(&self, key: Key) -> Option<Message> {
        route_key(self.focus, key)
    }

    pub fn quick_filters(&self) -> &QuickFilters {
        &self.cfg.quick_filters
    }

    pub fn cfg_only_installed(&self) -> bool {
        self.cfg.only_installed_games
    }

    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    pub fn country_request(&self) -> u64 {
        self.country_request
    }

    pub fn installed_generation(&self) -> u64 {
        self.installed_gen
    }

    pub fn country_options(&self) -> Vec<CountryOption> {
        sidebar::country_options(&self.country_counts, &self.cfg.quick_filters.country)
    }

    /// Opciones del combo de versión del juego actual (vacío sin juego).
    pub fn version_options(&self) -> Vec<VersionOption> {
        self.current.map_or_else(Vec::new, |g| {
            sidebar::version_options(g, version_for(g, &self.cfg), &self.observed_versions)
        })
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
                observe_version(&mut self.observed_versions, &info.version);
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
        container(Col::new().push(body).push(status))
            .style(move |_| style::window(p))
            .into()
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
        let clicks = event::listen_with(|e, _, _| match e {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                Some(Message::MousePressed)
            }
            _ => None,
        });
        let mut subs = vec![resize, close, mode, keys, clicks];
        // Solo mientras haya servidores que el desglose aún no cuenta.
        if self.tree_dirty {
            subs.push(time::every(Duration::from_millis(300)).map(|_| Message::TreeTick));
        }
        Subscription::batch(subs)
    }
}
