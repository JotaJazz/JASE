//! Vistas directas «★ Favoritos» y «🕘 Recientes» (`window_favorites.py`): el juego actual
//! consultado servidor por servidor, sin master; los filtros rápidos, la versión y las marcas
//! del árbol no se aplican mientras dura la vista.

use std::collections::HashSet;

use futures::stream::{self, StreamExt};
use iced::Task;
use iced::task::Handle;
use jase_core::config::version_for;
use jase_core::favorites::{favorite_addresses, offline_server};
use jase_core::games::ALL_VERSIONS;
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::models::{QuickFilters, ServerAddress, ServerInfo};
use jase_core::recent::recent_addresses;

use super::{App, Focus, Message};
use crate::table;

/// Consultas a la vez (la concurrencia de `query_servers`).
pub const DIRECT_CONCURRENCY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectView {
    Favorites,
    Recent,
}

#[allow(clippy::large_enum_variant)] // `ServerInfo`, como `ScanMsg`
#[derive(Debug, Clone)]
pub enum DirectMsg {
    /// Clic en la fila de la lista lateral.
    Show(DirectView),
    /// Respuesta de una dirección; la de otro pedido se descarta.
    Found {
        request: u64,
        address: ServerAddress,
        result: Result<Option<ServerInfo>, String>,
    },
    /// Respondieron todas las del pedido.
    Finished { request: u64 },
}

/// La vista vigente y su consulta (`_direct_view`, `_favorites_worker`).
#[derive(Default)]
pub(super) struct Direct {
    view: Option<DirectView>,
    /// Sube con cada consulta nueva o cancelada: lo de otro número se descarta.
    request: u64,
    handle: Option<Handle>,
    /// Las direcciones consultadas, las que respondieron y el primer error.
    asked: Vec<ServerAddress>,
    answered: HashSet<ServerAddress>,
    error: Option<String>,
    /// La dirección de «Refrescar este servidor» (para dejarla sin conexión si no responde).
    pub(super) refreshing: Option<ServerAddress>,
}

impl App {
    pub fn direct_view(&self) -> Option<DirectView> {
        self.direct.view
    }

    pub fn direct_request(&self) -> u64 {
        self.direct.request
    }

    pub fn is_querying_direct(&self) -> bool {
        self.direct.handle.is_some()
    }

    /// Direcciones de la vista vigente, en su orden.
    pub fn direct_addresses(&self, game_id: &str) -> Vec<ServerAddress> {
        match self.direct.view {
            Some(DirectView::Recent) => recent_addresses(&self.cfg, game_id),
            _ => self.group_filter(game_id, favorite_addresses(&self.cfg, game_id)),
        }
    }

    /// Los favoritos del grupo elegido (la Task 4 agrega los grupos; hasta entonces, todos).
    fn group_filter(&self, _game_id: &str, addresses: Vec<ServerAddress>) -> Vec<ServerAddress> {
        addresses
    }

    pub(super) fn in_direct_list(&self, game_id: &str, address: &ServerAddress) -> bool {
        self.direct_addresses(game_id).contains(address)
    }

    /// «★ Favoritos · N» y «🕘 Recientes · M» del juego actual (sin juego, sin conteo).
    pub fn direct_list_labels(&self) -> [String; 2] {
        let texts = [tr("★ Favoritos"), tr("🕘 Recientes")];
        match self.current {
            Some(g) => {
                let counts = [
                    favorite_addresses(&self.cfg, g.id).len(),
                    recent_addresses(&self.cfg, g.id).len(),
                ];
                [0, 1].map(|i| format!("{} · {}", texts[i], counts[i]))
            }
            None => texts,
        }
    }

    pub(super) fn on_direct(&mut self, m: DirectMsg) -> Task<Message> {
        match m {
            DirectMsg::Show(view) => return self.show_direct(view),
            DirectMsg::Found { request, .. } | DirectMsg::Finished { request }
                if request != self.direct.request => {}
            DirectMsg::Found {
                address, result, ..
            } => self.on_direct_found(address, result),
            DirectMsg::Finished { .. } => self.on_direct_finished(),
        }
        Task::none()
    }

    /// `_show_direct`: sin juego o si ya es la vigente, nada.
    fn show_direct(&mut self, view: DirectView) -> Task<Message> {
        self.set_focus(Focus::Sidebar);
        if self.current.is_none() || self.direct.view == Some(view) {
            return Task::none();
        }
        self.stop_active_scan();
        self.scan.cancel_query();
        self.direct.view = Some(view);
        self.set_filters_bypassed(true);
        self.query_direct_list()
    }

    /// Sale de la vista (`_leave_direct_view`) y repone filtros, versión y marcas.
    pub fn leave_direct_view(&mut self) {
        if self.direct.view.take().is_none() {
            return;
        }
        self.cancel_direct_query();
        self.set_filters_bypassed(false);
    }

    fn set_filters_bypassed(&mut self, bypassed: bool) {
        let quick = if bypassed {
            QuickFilters::default()
        } else {
            self.cfg.quick_filters.clone()
        };
        let version = match self.current {
            Some(g) if !bypassed => version_for(g, &self.cfg).to_owned(),
            _ => ALL_VERSIONS.to_owned(),
        };
        self.edit_filter(|f| {
            f.quick = quick;
            f.version = version;
        });
        self.apply_tree_filter(); // en la vista las marcas no se aplican
    }

    /// (Re)consulta la lista de la vista; las que no respondan quedan sin conexión.
    pub fn query_direct_list(&mut self) -> Task<Message> {
        let (Some(g), Some(view)) = (self.current, self.direct.view) else {
            return Task::none();
        };
        self.cancel_direct_query();
        self.scan.cancel_query();
        let top = self.clear_table();
        let addresses = self.direct_addresses(g.id);
        let name: [(&str, &dyn std::fmt::Display); 1] = [("name", &g.name)];
        let n = addresses.len() as i64;
        let status = match (view, addresses.is_empty()) {
            (DirectView::Recent, true) => tr_fmt("No hay servidores recientes de {name}", &name),
            (DirectView::Favorites, true) => tr_fmt(
                "No hay favoritos de {name}: agrégalos con clic derecho, con la estrella ☆ de la ficha o con Servidor → Agregar servidor…",
                &name,
            ),
            (DirectView::Recent, false) => tr_n(
                "Consultando 1 servidor reciente de {name}…",
                "Consultando {n} servidores recientes de {name}…",
                n,
                &name,
            ),
            (DirectView::Favorites, false) => tr_n(
                "Consultando 1 favorito de {name}…",
                "Consultando {n} favoritos de {name}…",
                n,
                &name,
            ),
        };
        self.set_status(status);
        if addresses.is_empty() {
            return top;
        }
        let request = self.direct.request;
        self.direct.asked = addresses.clone();
        self.direct.answered.clear();
        self.direct.error = None;
        // Ni la `QueryFn` se llama aquí: todo corre al ejecutar la `Task`.
        let query = self.deps.query.clone();
        let found = stream::iter(addresses)
            .map(move |address| {
                let query = query.clone();
                async move {
                    let result = query(g, address.clone()).await;
                    Message::Direct(DirectMsg::Found {
                        request,
                        address,
                        result,
                    })
                }
            })
            .buffer_unordered(DIRECT_CONCURRENCY)
            .chain(stream::once(async move {
                Message::Direct(DirectMsg::Finished { request })
            }));
        let (task, handle) = Task::stream(found).abortable();
        self.direct.handle = Some(handle);
        Task::batch([top, task])
    }

    /// Corta la consulta en curso y deja viejo su número (`_cancel_favorites_query`).
    pub(super) fn cancel_direct_query(&mut self) {
        if let Some(handle) = self.direct.handle.take() {
            handle.abort();
        }
        self.direct.request += 1;
    }

    /// `_on_favorite_found`: la que se quitó de la lista mientras se consultaba no entra.
    fn on_direct_found(
        &mut self,
        address: ServerAddress,
        result: Result<Option<ServerInfo>, String>,
    ) {
        let Some(g) = self.current else {
            return;
        };
        match result {
            Ok(Some(info)) => {
                self.direct.answered.insert(address.clone());
                if self.in_direct_list(g.id, &address) {
                    self.record_response(g.id, &info);
                    self.table.model.add_or_update(info);
                    table::drop_hidden_selection(&mut self.table);
                }
            }
            Ok(None) => {}
            Err(e) => {
                self.direct.error.get_or_insert(e);
            }
        }
    }

    /// `_on_favorites_finished`: solo cuentan las que siguen en la lista.
    fn on_direct_finished(&mut self) {
        self.direct.handle = None;
        let Some(g) = self.current else {
            return;
        };
        let list = self.direct_addresses(g.id);
        let current: Vec<ServerAddress> = std::mem::take(&mut self.direct.asked)
            .into_iter()
            .filter(|a| list.contains(a))
            .collect();
        let mut online = 0;
        for address in &current {
            if self.direct.answered.contains(address) {
                online += 1;
            } else {
                self.table.model.add_or_update(offline_server(address));
            }
        }
        table::drop_hidden_selection(&mut self.table);
        let total = current.len();
        let counts: [(&str, &dyn std::fmt::Display); 2] = [("online", &online), ("total", &total)];
        let text = match (&self.direct.error, self.direct.view) {
            (Some(e), _) => tr_fmt("Error al consultar favoritos: {error}", &[("error", e)]),
            (None, Some(DirectView::Recent)) => {
                tr_fmt("Recientes: {online} de {total} respondieron", &counts)
            }
            (None, _) => tr_fmt("Favoritos: {online} de {total} respondieron", &counts),
        };
        self.set_status(text);
    }
}
