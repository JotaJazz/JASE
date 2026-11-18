//! «Agregar servidor…» en la ventana (`window_add_server.py`): abrir el diálogo con lo que
//! haya en el portapapeles, *Detectar* y consultar lo agregado servidor por servidor, como las
//! vistas directas (cada consulta con su número: lo de una reemplazada o cerrada se descarta).

use std::collections::HashSet;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use iced::task::Handle;
use iced::widget::operation;
use iced::{Task, clipboard};
use indexmap::IndexMap;
use jase_core::favorites::{add_favorite, offline_server};
use jase_core::games::{GameDef, game};
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::models::{ServerAddress, ServerInfo};

use super::{App, DIRECT_CONCURRENCY, Dialog, DirectView, Message};
use crate::add_server::{Form, Msg, field_id, has_address, query_candidates};
use crate::table;

/// El aviso de un portapapeles sin direcciones (4 s) y el resumen de lo agregado (8 s).
const NOTICE: Duration = Duration::from_secs(4);
const REPORT: Duration = Duration::from_secs(8);

/// El formulario abierto, *Detectar* y la consulta de lo agregado (`_manual_*`, `_detect_*`).
#[derive(Default)]
pub(super) struct AddServer {
    form: Option<Form>,
    /// Sube con cada detección nueva o con el diálogo cerrado.
    detect_request: u64,
    detect_handle: Option<Handle>,
    /// Sube con cada consulta nueva: lo de otro número se descarta.
    request: u64,
    handle: Option<Handle>,
    game: Option<&'static GameDef>,
    favorite: bool,
    /// Candidata → escrita; escrita → respuesta; las que ya eran favoritos.
    origin: IndexMap<ServerAddress, ServerAddress>,
    placed: IndexMap<ServerAddress, ServerInfo>,
    existing: HashSet<ServerAddress>,
}

impl AddServer {
    /// Las escritas, sin repetir, en su orden.
    fn typed(&self) -> Vec<ServerAddress> {
        let mut typed: Vec<ServerAddress> = Vec::new();
        for a in self.origin.values() {
            if !typed.contains(a) {
                typed.push(a.clone());
            }
        }
        typed
    }
}

impl App {
    /// El formulario del diálogo abierto.
    pub fn add_form(&self) -> Option<&Form> {
        self.add_server.form.as_ref()
    }

    /// El número de la consulta vigente de lo agregado.
    pub fn add_request(&self) -> u64 {
        self.add_server.request
    }

    pub fn detect_request(&self) -> u64 {
        self.add_server.detect_request
    }

    /// Abre el diálogo con lo que haya en el portapapeles, si trae alguna dirección; desde
    /// Favoritos (sin `favorite`), con la casilla marcada.
    pub fn add_server_manually(&mut self, favorite: Option<bool>) -> Task<Message> {
        let favorite = favorite.unwrap_or(self.direct_view() == Some(DirectView::Favorites));
        clipboard::read().map(move |clip| {
            let text = clip.filter(|c| has_address(c)).unwrap_or_default();
            Message::AddServer(Msg::Open { favorite, text })
        })
    }

    /// `Ctrl+V` en la tabla: abre el diálogo con lo copiado.
    pub fn paste_servers(&mut self) -> Task<Message> {
        clipboard::read().map(|clip| Message::AddServer(Msg::Pasted(clip)))
    }

    pub(super) fn on_add_server(&mut self, m: Msg) -> Task<Message> {
        let open = self.dialog() == Some(&Dialog::AddServer);
        match m {
            Msg::Open { favorite, text } => return self.open_add_dialog(favorite, &text),
            Msg::Pasted(clip) => {
                let text = clip.unwrap_or_default();
                if !has_address(&text) {
                    let text = tr("El portapapeles no tiene ninguna dirección IP:puerto");
                    return self.show_message(text, Some(NOTICE));
                }
                let favorite = self.direct_view() == Some(DirectView::Favorites);
                return self.open_add_dialog(favorite, &text);
            }
            Msg::Found { request, .. } | Msg::Finished { request }
                if request != self.add_server.request => {}
            Msg::Found {
                address, result, ..
            } => return self.on_manual_found(&address, result),
            Msg::Finished { .. } => return self.on_manual_finished(),
            Msg::Detected {
                request,
                address,
                games,
            } => {
                if open && request == self.add_server.detect_request {
                    self.add_server.detect_handle = None;
                    if let Some(form) = &mut self.add_server.form {
                        form.on_detected(&address, &games);
                    }
                }
            }
            _ if !open => {}
            m => return self.edit_add_form(m),
        }
        Task::none()
    }

    /// Lo que se hace en el diálogo abierto.
    fn edit_add_form(&mut self, m: Msg) -> Task<Message> {
        let Some(form) = self.add_server.form.as_mut() else {
            return Task::none();
        };
        match m {
            Msg::Game(id) => form.set_game(id),
            Msg::Host(t) => form.host_changed(t),
            // Enter en IP o puerto: separa lo escrito y, si hay dirección, acepta.
            Msg::HostSubmit => {
                form.host_submitted();
                return self.submit_add();
            }
            Msg::Port(t) => form.port_changed(t),
            Msg::List(action) => form.list_action(action),
            Msg::ToggleMulti => {
                form.set_multi(!form.is_multi());
                return operation::focus(field_id(form.focus()));
            }
            Msg::Favorite(on) => form.set_favorite(on),
            Msg::Detect => return self.detect(),
            Msg::Submit => return self.submit_add(),
            Msg::Cancel => return self.close_add(),
            _ => {}
        }
        Task::none()
    }

    /// Un `Open` que llega con otro diálogo abierto (tarde) se descarta.
    fn open_add_dialog(&mut self, favorite: bool, text: &str) -> Task<Message> {
        if self.dialog.is_some() {
            return Task::none();
        }
        let current = self.current.map(|g| g.id);
        let form = Form::new(App::sorted_game_ids(&self.cfg), current, favorite, text);
        let focus = operation::focus(field_id(form.focus()));
        self.add_server.form = Some(form);
        self.open_dialog(Dialog::AddServer);
        focus
    }

    /// *Agregar*: cierra el diálogo y consulta lo escrito.
    fn submit_add(&mut self) -> Task<Message> {
        let Some(form) = self.add_server.form.as_ref().filter(|f| f.ok_enabled()) else {
            return Task::none();
        };
        let (id, addresses, favorite) = (form.game(), form.addresses(), form.favorite());
        let close = self.close_add();
        Task::batch([close, self.add_servers(id, addresses, favorite)])
    }

    /// Cerrar (aceptar o cancelar) corta *Detectar* (`_cancel_detect`).
    fn close_add(&mut self) -> Task<Message> {
        self.cancel_detect();
        self.add_server.form = None;
        self.close_dialog()
    }

    fn cancel_detect(&mut self) {
        if let Some(handle) = self.add_server.detect_handle.take() {
            handle.abort();
        }
        self.add_server.detect_request += 1;
    }

    /// *Detectar* la dirección escrita; ni la `DetectFn` se llama aquí.
    fn detect(&mut self) -> Task<Message> {
        let Some(address) = self
            .add_server
            .form
            .as_ref()
            .filter(|f| f.can_detect())
            .and_then(Form::result_address)
        else {
            return Task::none();
        };
        self.cancel_detect();
        if let Some(form) = &mut self.add_server.form {
            form.start_detect(&address);
        }
        let request = self.add_server.detect_request;
        let detect = self.deps.detect.clone();
        let (task, handle) = Task::perform(
            async move {
                let games = detect(address.clone()).await;
                (address, games)
            },
            move |(address, games)| {
                Message::AddServer(Msg::Detected {
                    request,
                    address,
                    games,
                })
            },
        )
        .abortable();
        self.add_server.detect_handle = Some(handle);
        task
    }

    /// `_add_servers`: va al juego (y a Favoritos si es favorito) y consulta cada dirección
    /// con su puerto de consulta posible. La anterior sin terminar se corta, pero sus
    /// favoritos sin respuesta quedan guardados.
    fn add_servers(
        &mut self,
        id: &'static str,
        addresses: Vec<ServerAddress>,
        favorite: bool,
    ) -> Task<Message> {
        let Some(g) = game(id) else {
            return Task::none();
        };
        let switch = if !favorite {
            self.select_game(id) // desde Favoritos/Recientes vuelve a la lista normal
        } else if self.current.is_none_or(|c| c.id != id)
            || self.direct_view() != Some(DirectView::Favorites)
        {
            let select = self.select_game(id);
            Task::batch([select, self.show_direct(DirectView::Favorites)])
        } else {
            Task::none()
        };
        let mut origin = IndexMap::new();
        for a in &addresses {
            for candidate in query_candidates(g, a.port, &a.host) {
                origin.entry(candidate).or_insert_with(|| a.clone());
            }
        }
        if let Some(handle) = self.add_server.handle.take() {
            handle.abort();
            if let (true, Some(old)) = (self.add_server.favorite, self.add_server.game) {
                for typed in self.add_server.typed() {
                    if !self.add_server.placed.contains_key(&typed) {
                        self.save_manual_favorite(old.id, &typed);
                    }
                }
            }
        }
        let s = &mut self.add_server;
        s.request += 1;
        s.game = Some(g);
        s.favorite = favorite;
        s.placed.clear();
        s.existing.clear();
        let candidates: Vec<ServerAddress> = origin.keys().cloned().collect();
        s.origin = origin;
        let request = s.request;
        let status = match addresses.as_slice() {
            [one] => tr_fmt("Consultando {address}…", &[("address", one)]),
            _ => tr_n(
                "Consultando 1 servidor…",
                "Consultando {n} servidores…",
                addresses.len() as i64,
                &[],
            ),
        };
        self.set_status(status);
        let query = self.deps.query.clone();
        let found = stream::iter(candidates)
            .map(move |address| {
                let query = query.clone();
                async move {
                    let result = query(g, address.clone()).await;
                    Message::AddServer(Msg::Found {
                        request,
                        address,
                        result,
                    })
                }
            })
            .buffer_unordered(DIRECT_CONCURRENCY)
            .chain(stream::once(async move {
                Message::AddServer(Msg::Finished { request })
            }));
        let (task, handle) = Task::stream(found).abortable();
        self.add_server.handle = Some(handle);
        Task::batch([switch, task])
    }

    /// `_on_manual_found`: la primera respuesta de cada escrita (la otra candidata se ignora).
    fn on_manual_found(
        &mut self,
        address: &ServerAddress,
        result: Result<Option<ServerInfo>, String>,
    ) -> Task<Message> {
        let s = &mut self.add_server;
        let (Ok(Some(info)), Some(g)) = (result, s.game) else {
            return Task::none();
        };
        let Some(typed) = s.origin.get(address).cloned() else {
            return Task::none();
        };
        if s.placed.contains_key(&typed) {
            return Task::none(); // ya respondió por el otro puerto
        }
        s.placed.insert(typed, info.clone());
        let first = s.placed.len() == 1;
        if s.favorite {
            // Se guarda aunque se haya cambiado de juego.
            self.save_manual_favorite(g.id, &info.address);
        }
        if self.current.is_none_or(|c| c.id != g.id) {
            return Task::none();
        }
        self.record_response(g.id, &info);
        self.place_manual(info, first)
    }

    /// `_on_manual_finished`: los favoritos sin respuesta quedan guardados (y, en el juego
    /// actual, como filas sin conexión); el resumen va 8 s.
    fn on_manual_finished(&mut self) -> Task<Message> {
        self.add_server.handle = None;
        let Some(g) = self.add_server.game else {
            return Task::none();
        };
        let typed = self.add_server.typed();
        let missing: Vec<ServerAddress> = typed
            .iter()
            .filter(|a| !self.add_server.placed.contains_key(*a))
            .cloned()
            .collect();
        let favorite = self.add_server.favorite;
        if favorite {
            for a in &missing {
                self.save_manual_favorite(g.id, a);
            }
        }
        if self.current.is_none_or(|c| c.id != g.id) {
            return Task::none(); // se cambió de juego: guardado, sin filas ni aviso
        }
        let mut tasks = Vec::new();
        if favorite {
            for a in &missing {
                let select = typed.first() == Some(a) && self.add_server.placed.is_empty();
                tasks.push(self.place_manual(offline_server(a), select));
            }
        }
        let text = self.manual_report(&typed, &missing);
        tasks.push(self.show_message(text, Some(REPORT)));
        Task::batch(tasks)
    }

    fn save_manual_favorite(&mut self, game_id: &str, address: &ServerAddress) {
        if add_favorite(&mut self.cfg, game_id, address) {
            self.save_config();
            self.update_sidebar();
        } else {
            self.add_server.existing.insert(address.clone());
        }
    }

    /// `_place_manual`: la fila entra si se ve la lista que corresponde (Favoritos para un
    /// favorito, la normal si no) y, si toca, queda elegida.
    fn place_manual(&mut self, info: ServerInfo, select: bool) -> Task<Message> {
        let show = if self.add_server.favorite {
            self.direct_view() == Some(DirectView::Favorites)
        } else {
            self.direct_view().is_none()
        };
        let mut task = Task::none();
        if show {
            let address = info.address.clone();
            self.table.model.add_or_update(info);
            table::drop_hidden_selection(&mut self.table);
            if select {
                task = table::select_address(&mut self.table, &address).unwrap_or_else(Task::none);
            }
        }
        self.update_sidebar();
        task
    }

    /// `_report_manual`.
    fn manual_report(&self, typed: &[ServerAddress], missing: &[ServerAddress]) -> String {
        let s = &self.add_server;
        if let [address] = typed {
            let placed = s.placed.get(address).map(|i| &i.address);
            return match placed {
                None if s.favorite => tr_fmt(
                    "{address} no respondió; quedó en favoritos",
                    &[("address", address)],
                ),
                None => tr_fmt(
                    "{address} no respondió; para guardarlo agrégalo como favorito",
                    &[("address", address)],
                ),
                Some(a) if s.favorite && s.existing.contains(a) => {
                    tr_fmt("{address} ya estaba en favoritos", &[("address", a)])
                }
                Some(a) if s.favorite => {
                    tr_fmt("Agregado a favoritos: {address}", &[("address", a)])
                }
                Some(a) => tr_fmt("Servidor agregado: {address}", &[("address", a)]),
            };
        }
        if s.favorite {
            tr_n(
                "1 favorito agregado ({missing} sin respuesta)",
                "{n} favoritos agregados ({missing} sin respuesta)",
                typed.len() as i64,
                &[("missing", &missing.len())],
            )
        } else {
            tr_n(
                "1 servidor respondió de {total}",
                "{n} servidores respondieron de {total}",
                (typed.len() - missing.len()) as i64,
                &[("total", &typed.len())],
            )
        }
    }
}
