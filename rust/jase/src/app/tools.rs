//! Ventanas aparte (`iced::daemon`): la principal y las herramientas no modales de Qt (*Buscar
//! jugador…*, *Ahora se juega*, cada consola RCON), y revelar en la principal el servidor que
//! una de ellas elige (`window_player_search.py`).

use iced::keyboard::{Key, key::Named};
use iced::widget::{Space, operation};
use iced::{Element, Size, Task, window};
use indexmap::IndexMap;
use jase_core::i18n::{tr, tr_fmt};
use jase_core::models::{ServerAddress, ServerInfo};

use super::{App, Message};
use crate::icons;
use crate::player_search::{DEBOUNCE, PlayerSearch};
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
        if let Some(Tool::PlayerSearch) = self.windows.tools.shift_remove(&id) {
            self.search = None;
        }
    }

    pub fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if id == self.windows.main {
            return self.view();
        }
        match (self.windows.tools.get(&id), &self.search) {
            (Some(Tool::PlayerSearch), Some(s)) => s.view(self.theme_id),
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

    /// Teclas no capturadas de una herramienta: ↑/↓ mueven la fila de la búsqueda (en Qt los
    /// toma la tabla con el foco; aquí llegan desde el campo).
    pub(super) fn on_tool_key(&mut self, id: window::Id, key: Key) {
        let (Some(Tool::PlayerSearch), Some(s)) = (self.windows.tools.get(&id), &mut self.search)
        else {
            return;
        };
        let last = s.hits.len().saturating_sub(1);
        // shortcut: la tabla no se desplaza hasta la fila elegida; con muchos hallazgos, si se pide.
        s.selected = match (key, s.selected) {
            (_, None) => None,
            (Key::Named(Named::ArrowDown), Some(i)) => Some((i + 1).min(last)),
            (Key::Named(Named::ArrowUp), Some(i)) => Some(i.saturating_sub(1)),
            (_, sel) => sel,
        };
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
