//! Ventanas aparte (`iced::daemon`): la principal y las herramientas no modales de Qt (*Buscar
//! jugador…*, *Ahora se juega*, cada consola RCON), y revelar en la principal el servidor que
//! una de ellas elige (`window_player_search.py`, `window_now_playing.py`).
//!
//! Foco: en Iced `operation::focus` quita el caret de los campos de todas las ventanas. Regla:
//! cuando una herramienta enfoca su campo, la principal pasa de `Focus::Filter` a `Sidebar`
//! (sin caret), como al perderlo con un clic; `Dialog` no dice nada del caret y sigue. Lo que
//! la principal enfoca sí le quita el caret a la herramienta, que no guarda foco propio (sus
//! teclas llegan como `ToolKey`). *Ahora se juega* no enfoca nada.

use iced::keyboard::{Key, key::Named};
use iced::task::Handle;
use iced::widget::{Space, operation};
use iced::{Element, Size, Task, window};
use indexmap::IndexMap;
use jase_core::games::{GameDef, LaunchStyle, game};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::masters::effective_masters;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::ScanEvent;

use super::{App, Focus, Message};
use crate::icons;
use crate::now_playing::{self, NowRow, PARALLEL_SCANS};
use crate::player_search::{DEBOUNCE, PlayerSearch};
use crate::scan::ScanMsg;
use crate::startup::APP_NAME;

/// Juego y dirección para conectar (`connect_address`): una consola por servidor.
pub type RconKey = (&'static str, ServerAddress);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tool {
    PlayerSearch,
    NowPlaying,
    Rcon(RconKey),
}

impl Tool {
    /// El tamaño de Qt (`resize`).
    fn size(&self) -> Size {
        match self {
            Tool::PlayerSearch => Size::new(640.0, 420.0),
            Tool::NowPlaying => Size::new(760.0, 480.0),
            Tool::Rcon(_) => Size::new(640.0, 440.0),
        }
    }
}

pub(crate) struct Windows {
    pub main: window::Id,
    tools: IndexMap<window::Id, Tool>,
}

impl Default for Windows {
    /// Una id que nunca abre (los tests no abren ventanas); `open_main` pone la de verdad.
    fn default() -> Self {
        Windows {
            main: window::Id::unique(),
            tools: IndexMap::new(),
        }
    }
}

/// Ventana de herramienta centrada, con el icono de JASE; Iced la cierra solo.
pub fn tool_settings(size: Size) -> window::Settings {
    window::Settings {
        size,
        position: window::Position::Centered,
        min_size: Some(Size::new(size.width / 2.0, size.height / 2.0)),
        icon: icons::app_icon(),
        exit_on_close_request: true,
        ..window::Settings::default()
    }
}

#[derive(Debug, Clone)]
pub enum SearchMsg {
    /// *Buscar jugador…*: abre la ventana o la trae al frente.
    Open,
    Query(String),
    /// Venció el plazo del texto `seq`.
    Debounced(u64),
    Select(usize),
    /// Doble clic o Enter: revela el servidor; la ventana sigue abierta.
    Choose(usize),
    /// Enter en el campo.
    Submit,
}

#[allow(clippy::large_enum_variant)] // por `ScanMsg`
#[derive(Debug, Clone)]
pub enum NowMsg {
    /// *Ahora se juega…*: abre la ventana o la trae al frente.
    Open,
    /// *Actualizar*: vuelve a escanear todos, también los de la sesión.
    Refresh,
    /// Lo que trae el escaneo de `game` del pedido `request`.
    Scan {
        request: u64,
        game: &'static str,
        msg: ScanMsg,
    },
    Select(usize),
    /// Doble clic o Enter: revela el servidor; la ventana sigue abierta.
    Choose(usize),
}

/// Los escaneos de *Ahora se juega* y lo que muestra.
#[derive(Default)]
pub(crate) struct Now {
    /// Sube al cancelar: lo que llegue de un pedido anterior se descarta.
    request: u64,
    lists: IndexMap<String, Vec<ServerInfo>>,
    queue: Vec<&'static GameDef>,
    running: Vec<(&'static str, Handle)>,
    /// La fila elegida por (juego, dirección): sigue elegida al rearmarse la tabla.
    selected: Option<(String, ServerAddress)>,
    rows: Vec<NowRow>,
    status: String,
}

impl Now {
    fn selected_row(&self) -> Option<usize> {
        let (g, a) = self.selected.as_ref()?;
        self.rows
            .iter()
            .position(|r| r.game == *g && r.address == *a)
    }
}

impl App {
    /// Abre la ventana principal (el `daemon` no abre ninguna solo).
    pub fn open_main(&mut self, settings: window::Settings) -> Task<window::Id> {
        let (id, open) = window::open(settings);
        self.windows.main = id;
        open
    }

    pub fn main_window(&self) -> window::Id {
        self.windows.main
    }

    /// La ventana de `tool`: la ya abierta pasa al frente (`show/raise_/activateWindow`).
    pub fn open_tool(&mut self, tool: Tool) -> (window::Id, Task<Message>) {
        if let Some(id) = self.tool_window(&tool) {
            return (
                id,
                Task::batch([window::minimize(id, false), window::gain_focus(id)]),
            );
        }
        let (id, open) = window::open(tool_settings(tool.size()));
        self.windows.tools.insert(id, tool);
        (id, open.discard())
    }

    pub fn tool_windows(&self) -> Vec<(window::Id, Tool)> {
        let tools = &self.windows.tools;
        tools.iter().map(|(id, t)| (*id, t.clone())).collect()
    }

    pub fn tool_window(&self, tool: &Tool) -> Option<window::Id> {
        let tools = &self.windows.tools;
        tools.iter().find(|(_, t)| *t == tool).map(|(id, _)| *id)
    }

    /// Se cerró: la herramienta suelta su estado (`finished` de Qt).
    pub(super) fn on_window_closed(&mut self, id: window::Id) {
        match self.windows.tools.shift_remove(&id) {
            Some(Tool::PlayerSearch) => self.search = None,
            Some(Tool::NowPlaying) => self.now_cancel(),
            _ => {}
        }
    }

    pub fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if id == self.windows.main {
            return self.view();
        }
        match (self.windows.tools.get(&id), &self.search) {
            (Some(Tool::PlayerSearch), Some(s)) => s.view(self.theme_id),
            (Some(Tool::NowPlaying), _) => {
                let now = &self.now;
                let (busy, theme) = (self.now_busy(), self.theme_id);
                now_playing::view(&now.rows, now.selected_row(), &now.status, busy, id, theme)
            }
            // Cerrándose, o sin portar aún.
            _ => Space::new().into(),
        }
    }

    pub fn title(&self, id: window::Id) -> String {
        match self.windows.tools.get(&id) {
            Some(Tool::PlayerSearch) => tr("Buscar jugador…"),
            Some(Tool::NowPlaying) => tr("Ahora se juega"),
            Some(Tool::Rcon((_, address))) => tr_fmt(
                "Consola RCON: {server}",
                &[("server", &address.to_string())],
            ),
            None => APP_NAME.to_owned(),
        }
    }

    /// Teclas no capturadas de una herramienta: ↑/↓ mueven la fila (en Qt los toma la tabla
    /// con el foco; en la búsqueda llegan desde el campo) y Enter elige la de *Ahora se juega*.
    pub(super) fn on_tool_key(&mut self, id: window::Id, key: Key) -> Task<Message> {
        // shortcut: la tabla no se desplaza hasta la fila elegida; con muchas filas, si se pide.
        let step = |sel: Option<usize>, len: usize, key: &Key| match (key, sel) {
            (_, None) => None,
            (Key::Named(Named::ArrowDown), Some(i)) => Some((i + 1).min(len.saturating_sub(1))),
            (Key::Named(Named::ArrowUp), Some(i)) => Some(i.saturating_sub(1)),
            (_, sel) => sel,
        };
        match (self.windows.tools.get(&id), &mut self.search) {
            (Some(Tool::PlayerSearch), Some(s)) => {
                s.selected = step(s.selected, s.hits.len(), &key)
            }
            (Some(Tool::NowPlaying), _) => {
                let sel = self.now.selected_row();
                if key == Key::Named(Named::Enter) {
                    return sel.map_or_else(Task::none, |i| self.now_choose(i));
                }
                if let Some(i) = step(sel, self.now.rows.len(), &key) {
                    self.now_select(i);
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// `reveal_server`: el juego en vista normal y la fila elegida; si el escaneo aún no la
    /// trajo queda pendiente, y si los filtros la ocultan se avisa. No trae la principal al
    /// frente (Qt tampoco).
    pub fn reveal_server(&mut self, game: &'static str, address: ServerAddress) -> Task<Message> {
        let mut task = Task::none();
        if self.current.is_none_or(|g| g.id != game) || self.direct_view().is_some() {
            task = self.select_game(game);
            if self.saved.loading() == Some(game) {
                // Solo hay lista del disco: se escanea para encontrarlo.
                task = Task::batch([task, self.refresh(false)]);
            }
        }
        if let Some(select) = self.select_address(&address) {
            return Task::batch([task, select]);
        }
        if self.table.model.row_of(&address).is_some() {
            self.set_status(tr("El servidor está oculto por los filtros activos"));
        } else if self.is_scanning() {
            self.pending_select = Some(address);
        }
        task
    }

    /// Listas de la sesión donde buscar: la caché más la tabla visible, sin las leídas del
    /// disco (avisarían de gente que ya no está).
    pub fn searchable_lists(&self) -> IndexMap<String, Vec<ServerInfo>> {
        // shortcut: copia las listas en cada búsqueda; si con 20.000 servidores se nota, pasar a `Arc`.
        let mut lists: IndexMap<String, Vec<ServerInfo>> = self
            .cache
            .iter()
            .filter(|(g, _)| self.saved.from_disk(g).is_none())
            .map(|(g, servers)| ((*g).to_owned(), servers.clone()))
            .collect();
        if let Some(g) = self.current
            && self.direct_view().is_none()
            && self.saved.from_disk(g.id).is_none()
        {
            lists.insert(g.id.to_owned(), self.servers().to_vec());
        }
        lists
    }

    pub fn player_search(&self) -> Option<&PlayerSearch> {
        self.search.as_ref()
    }

    pub(super) fn on_search(&mut self, m: SearchMsg) -> Task<Message> {
        if let SearchMsg::Open = m {
            let (_, open) = self.open_tool(Tool::PlayerSearch);
            let field = self
                .search
                .get_or_insert_with(PlayerSearch::default)
                .field
                .clone();
            // `showEvent`: las listas pueden haber cambiado; el campo con todo elegido.
            self.refresh_search();
            let focus = Task::batch([
                operation::focus(field.clone()),
                operation::select_all(field),
            ]);
            // El caret deja el filtro de la principal (ver el comentario del módulo).
            if self.focus == Focus::Filter {
                self.set_focus(Focus::Sidebar);
            }
            // Tras abrirse: las operaciones solo llegan a ventanas que ya existen.
            return open.chain(focus);
        }
        let Some(s) = &mut self.search else {
            return Task::none();
        };
        match m {
            SearchMsg::Open => {}
            SearchMsg::Query(text) => {
                let seq = s.edit(text);
                // `async` para que el temporizador se cree ya en el runtime, no aquí.
                let wait = async { tokio::time::sleep(DEBOUNCE).await };
                return Task::perform(wait, move |()| Message::Search(SearchMsg::Debounced(seq)));
            }
            SearchMsg::Debounced(seq) if s.is_latest(seq) && s.stale() => self.refresh_search(),
            SearchMsg::Debounced(_) => {}
            SearchMsg::Select(i) if i < s.hits.len() => s.selected = Some(i),
            SearchMsg::Select(_) => {}
            SearchMsg::Choose(i) => return self.choose_hit(i),
            SearchMsg::Submit => {
                // Enter antes del plazo: no se elige sobre resultados viejos.
                if s.stale() {
                    self.refresh_search();
                }
                let selected = self.search.as_ref().and_then(|s| s.selected);
                return selected.map_or_else(Task::none, |i| self.choose_hit(i));
            }
        }
        Task::none()
    }

    fn refresh_search(&mut self) {
        let lists = self.searchable_lists();
        if let Some(s) = &mut self.search {
            s.refresh(&lists);
        }
    }

    fn choose_hit(&mut self, i: usize) -> Task<Message> {
        let Some(s) = &mut self.search else {
            return Task::none();
        };
        let Some(hit) = s.hits.get(i) else {
            return Task::none();
        };
        s.selected = Some(i);
        let (game, address) = (hit.game, hit.address.clone());
        self.reveal_server(game, address)
    }
}

impl App {
    /// Juegos listos (con carpeta, o que conectan por Steam) que se pueden escanear, en el
    /// orden del árbol.
    pub fn now_playing_games(&self) -> Vec<&'static GameDef> {
        self.tree_game_ids()
            .into_iter()
            .filter_map(game)
            .filter(|g| {
                self.cfg.game_paths.get(g.id).is_some_and(|p| !p.is_empty())
                    || g.launch_style == LaunchStyle::Steam
            })
            .filter(|g| self.can_scan(g))
            .collect()
    }

    pub fn now_rows(&self) -> Vec<NowRow> {
        self.now.rows.clone()
    }

    pub fn now_status(&self) -> String {
        self.now.status.clone()
    }

    /// Hay escaneos en curso o en cola (*Actualizar* apagado).
    pub fn now_busy(&self) -> bool {
        !self.now.running.is_empty() || !self.now.queue.is_empty()
    }

    pub(super) fn on_now(&mut self, m: NowMsg) -> Task<Message> {
        match m {
            NowMsg::Open => {
                let (_, open) = self.open_tool(Tool::NowPlaying);
                if self.now.running.is_empty() {
                    return Task::batch([open, self.now_refresh(false)]);
                }
                open
            }
            NowMsg::Refresh => self.now_refresh(true),
            NowMsg::Scan { request, game, msg } if request == self.now.request => {
                self.on_now_scan(game, msg)
            }
            NowMsg::Scan { .. } => Task::none(), // cancelado o de antes de *Actualizar*
            NowMsg::Select(i) => {
                self.now_select(i);
                Task::none()
            }
            NowMsg::Choose(i) => self.now_choose(i),
        }
    }

    /// Junta las listas: sin `force` valen las de la sesión; el resto se escanea, de a
    /// `PARALLEL_SCANS`.
    fn now_refresh(&mut self, force: bool) -> Task<Message> {
        self.now_cancel();
        let games = self.now_playing_games();
        let mut session = if force {
            IndexMap::new()
        } else {
            self.searchable_lists()
        };
        self.now.lists = games
            .iter()
            .filter_map(|g| Some((g.id.to_owned(), session.swap_remove(g.id)?)))
            .collect();
        let lists = &self.now.lists;
        self.now.queue = games
            .iter()
            .copied()
            .filter(|g| !lists.contains_key(g.id))
            .collect();
        if games.is_empty() {
            self.now_show(Some(tr(
                "No hay juegos listos: configura sus carpetas en Ajustes o con Archivo → Buscar juegos…",
            )));
            return Task::none();
        }
        let scans: Vec<_> = (0..PARALLEL_SCANS).map(|_| self.now_next()).collect();
        self.now_show(None);
        Task::batch(scans)
    }

    /// Arranca el próximo de la cola, si hay.
    fn now_next(&mut self) -> Task<Message> {
        if self.now.queue.is_empty() {
            return Task::none();
        }
        let g = self.now.queue.remove(0);
        let active = effective_masters(g.id, &self.official, &self.custom);
        let stream = (self.deps.scan)(g, active, self.scan_options(g));
        let request = self.now.request;
        let (task, handle) = Task::run(stream, move |msg| {
            Message::Now(NowMsg::Scan {
                request,
                game: g.id,
                msg,
            })
        })
        .abortable();
        self.now.running.push((g.id, handle));
        self.now.lists.insert(g.id.to_owned(), Vec::new());
        task
    }

    /// Corta los escaneos en curso y vacía la cola (*Actualizar*, cerrar la ventana).
    pub(super) fn now_cancel(&mut self) {
        for (_, handle) in self.now.running.drain(..) {
            handle.abort();
        }
        self.now.queue.clear();
        self.now.request += 1;
    }

    fn on_now_scan(&mut self, game: &'static str, msg: ScanMsg) -> Task<Message> {
        match msg {
            ScanMsg::Event(ScanEvent::Server(info)) => {
                self.now
                    .lists
                    .entry(game.to_owned())
                    .or_default()
                    .push(info);
                Task::none()
            }
            ScanMsg::Event(_) => Task::none(),
            ScanMsg::Finished(result) => {
                self.now.running.retain(|(g, _)| *g != game);
                // Una lista completa sirve también a la ventana, salvo el juego que muestra.
                // Python cachea cualquier lista terminada; aquí, como el escaneo principal,
                // solo con algún master que respondió.
                if result.ok() && self.current.is_none_or(|g| g.id != game) {
                    let servers = self.now.lists.get(game).cloned().unwrap_or_default();
                    self.saved.save(game, &servers, (self.deps.now)().0);
                    self.cache.insert(game, servers);
                    self.update_sidebar();
                }
                let next = self.now_next();
                self.now_show(None);
                next
            }
        }
    }

    /// Rearma la tabla (conservando la fila elegida) y el estado (`_now_show`).
    fn now_show(&mut self, message: Option<String>) {
        let now = &mut self.now;
        now.rows = now_playing::rows(&now.lists);
        if now.selected_row().is_none() {
            now.selected = None;
        }
        let pending = now.running.len() + now.queue.len();
        now.status = message.unwrap_or_else(|| now_playing::status_text(&now.lists, pending));
    }

    fn now_select(&mut self, i: usize) {
        if let Some(r) = self.now.rows.get(i) {
            self.now.selected = Some((r.game.clone(), r.address.clone()));
        }
    }

    fn now_choose(&mut self, i: usize) -> Task<Message> {
        self.now_select(i);
        let Some(r) = self.now.rows.get(i) else {
            return Task::none();
        };
        match game(&r.game) {
            Some(g) => self.reveal_server(g.id, r.address.clone()),
            None => Task::none(),
        }
    }
}
