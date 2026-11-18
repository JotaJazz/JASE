//! Estado de la ventana (provisional en la 5a: lista de juegos, tabla y barra de estado).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::keyboard::{self, Key, key::Named};
use iced::task::Handle;
use iced::theme::Mode;
use iced::widget::{Column as Col, container, mouse_area, row, scrollable, text};
use iced::{Element, Fill, Padding, Right, Size, Subscription, Task, Theme, system, window};
use jase_core::config::{Config, default_config_path, load_config, save_config};
use jase_core::games::{GAMES, GameDef, game};
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
use crate::theme::{iced_theme, palette, resolve_theme, style};

#[allow(clippy::large_enum_variant)] // por `ScanMsg`
#[derive(Debug, Clone)]
pub enum Message {
    SelectGame(&'static str),
    Refresh,
    Stop,
    Scan { generation: u64, msg: ScanMsg },
    Table(table::Msg),
    CloseRequested,
    SystemMode(Mode),
    WindowResized(Size),
}

pub struct App {
    pub table: Table,
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
        let mut app = App {
            table: Table::default(),
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
        };
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
                    return Task::batch([top, self.start_scan()]);
                }
            }
            Message::Stop => self.stop_active_scan(),
            Message::Scan { generation, msg } if generation == self.generation => self.on_scan(msg),
            Message::Scan { .. } => {} // de un escaneo viejo
            Message::Table(m) => return table::update(&mut self.table, m),
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
        self.current = Some(g);
        let top = self.clear_table();
        self.table
            .model
            .set_version_label(move |v| g.version_label(v).to_owned());
        let Some(cached) = self.cache.get(g.id) else {
            return Task::batch([top, self.start_scan()]);
        };
        for info in cached {
            self.table.model.add_or_update(info.clone());
        }
        self.status = tr_fmt("{name}: lista guardada de la sesión", &[("name", &g.name)]);
        top
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
            self.scan_stopped = true;
            self.progress = None;
            // Un escaneo interrumpido no se guarda: se conserva el último completo.
        }
    }

    fn on_scan(&mut self, msg: ScanMsg) {
        match msg {
            ScanMsg::Event(ScanEvent::Server(info)) => {
                self.table.model.add_or_update(info);
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
                    self.status = tr_fmt(
                        "{responded} de {listed} servidores respondieron",
                        &[
                            ("responded", &result.servers_responded),
                            ("listed", &result.servers_listed),
                        ],
                    );
                }
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
        tr_n(
            "{n} servidor",
            "{n} servidores",
            self.table.model.len() as i64,
            &[],
        )
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
        let p = palette(self.theme_id);
        let games = Col::with_children(GAMES.iter().map(|g| {
            mouse_area(text(self.game_label(g.id)))
                .on_press(Message::SelectGame(g.id))
                .into()
        }))
        .spacing(4)
        .padding(6);
        let sidebar = container(scrollable(games).height(Fill))
            .width(230)
            .style(move |_| style::sidebar(p));
        let status = row![
            text(self.status()).width(Fill),
            text(self.progress_text()),
            text(self.count_text()).align_x(Right),
        ]
        .spacing(12);
        let status = container(status)
            .width(Fill)
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
        let close = window::close_requests().map(|_| Message::CloseRequested);
        if !self.table.focused {
            return Subscription::batch([resize, close]);
        }
        let keys = keyboard::listen().filter_map(|e| match e {
            keyboard::Event::KeyPressed { key, .. } => {
                nav_for(key).map(|n| Message::Table(table::Msg::Nav(n)))
            }
            _ => None,
        });
        Subscription::batch([resize, close, keys])
    }
}
