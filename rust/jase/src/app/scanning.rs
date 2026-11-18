//! El escaneo del juego actual (de internet o de la red local), lo que va llegando y
//! «Refrescar este servidor».

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iced::Task;
use jase_core::i18n::{tr, tr_fmt};
use jase_core::masters::effective_masters;
use jase_core::models::ServerInfo;
use jase_core::scanner::scan::{ScanEvent, ScanOptions};

use super::{App, Message, blocking, observe_version};
use crate::passwords::STEAM_KEY_USER;
use crate::scan::ScanMsg;
use crate::table;

impl App {
    /// Como `model.clear()` de Qt, que también suelta la selección y vuelve arriba; la tarea
    /// mueve el `scrollable`, que si no seguiría desplazado y dibujaría la tabla en blanco.
    pub(super) fn clear_table(&mut self) -> Task<Message> {
        self.table.model.clear();
        self.table.selected = None;
        self.scan.leave_lan(); // la tabla vacía ya no es la red local

        table::scroll_to_top(&mut self.table)
    }

    pub(super) fn start_scan(&mut self) -> Task<Message> {
        let Some(g) = self.current else {
            return Task::none();
        };
        let active = effective_masters(g.id, &self.official, &self.custom);
        let name: [(&str, &dyn std::fmt::Display); 1] = [("game", &g.name)];
        if active.is_empty() {
            self.set_status(tr_fmt(
                "{game}: no hay masters activos; revísalos en Ajustes → Masters",
                &name,
            ));
            return Task::none();
        }
        // Python lee la clave al crear la ventana; aquí se espera a que el llavero responda.
        if g.uses_steam_api() && self.steam_key.is_none() {
            self.steam_wait = Some(g.id);
            return Task::none();
        }
        let key = self.steam_key.clone().unwrap_or_default();
        if g.steam_webapi && key.is_empty() {
            self.set_status(tr_fmt(
                "{game}: falta la clave de API de Steam; ponla en Ajustes → General",
                &name,
            ));
            return Task::none();
        }
        self.tree_prune_ok = false;
        let opts = ScanOptions {
            steam_key: if g.uses_steam_api() {
                key
            } else {
                String::new()
            },
            steam_include_empty: self.cfg.steam_include_empty,
            ..ScanOptions::default()
        };
        let task = self.scan.begin((self.deps.scan)(g, active, opts), false);
        self.set_status(tr_fmt(
            "Consultando masters de {name}…",
            &[("name", &g.name)],
        ));
        task
    }

    /// Lee la clave de Steam en un hilo (el llavero puede tardar en desbloquearse).
    pub(super) fn load_steam_key(&self) -> Task<Message> {
        let keyring = self.deps.keyring.clone();
        let read = blocking(move || keyring.get(STEAM_KEY_USER).unwrap_or_default());
        // Si el hilo falló no hay clave: el juego que espera avisa en vez de quedar colgado.
        Task::perform(read, |key| Message::SteamKey(key.unwrap_or_default()))
    }

    /// Llegó la clave: escanea el juego que la esperaba si nada cambió desde entonces.
    pub(super) fn on_steam_key(&mut self, key: String) -> Task<Message> {
        self.steam_key = Some(key);
        let waiting = self.steam_wait.take();
        match self.current {
            Some(g)
                if waiting == Some(g.id) && !self.is_scanning() && self.table.model.is_empty() =>
            {
                let task = self.start_scan();
                self.update_sidebar();
                task
            }
            _ => Task::none(),
        }
    }

    pub fn steam_key(&self) -> Option<&str> {
        self.steam_key.as_deref()
    }

    /// Cancela sin esperar: soltar el future lo corta y sus mensajes quedan viejos.
    pub(super) fn stop_active_scan(&mut self) {
        if self.scan.stop() {
            self.tree_prune_ok = false;
            self.tree_dirty = false;
            // Un escaneo interrumpido no se guarda: se conserva el último completo.
        }
    }

    /// `scan_lan`: sin masters ni clave; vacía la tabla y su lista no reemplaza la guardada.
    pub(super) fn scan_lan(&mut self) -> Task<Message> {
        let Some(g) = self.current else {
            return Task::none();
        };
        self.stop_active_scan();
        self.scan.cancel_query();
        self.saved.cancel_load();
        let top = self.clear_table();
        self.tree_prune_ok = false;
        let task = self.scan.begin((self.deps.lan)(g), true);
        self.set_status(tr_fmt(
            "Buscando servidores de {name} en la red local…",
            &[("name", &g.name)],
        ));
        self.update_sidebar();
        Task::batch([top, task])
    }

    /// Vuelve a consultar el seleccionado (por su dirección de consulta), en una `Task`.
    pub(super) fn refresh_selected(&mut self) -> Task<Message> {
        let (Some(g), Some(address)) = (self.current, self.table.selected.clone()) else {
            return Task::none();
        };
        self.set_status(tr_fmt("Consultando {address}…", &[("address", &address)]));
        // Ni la `QueryFn` se llama aquí: todo corre al ejecutar la `Task`.
        let query = self.deps.query.clone();
        let fut = async move { query(g, address).await };
        self.scan.begin_query(g.id, Box::pin(fut)).1
    }

    /// `_on_query_server_found` + `_on_query_finished`.
    pub(super) fn on_query(
        &mut self,
        request: u64,
        game: &'static str,
        result: Result<Option<ServerInfo>, String>,
    ) -> Task<Message> {
        if request != self.scan.query_request() || self.current.is_none_or(|c| c.id != game) {
            return Task::none(); // de otro pedido (F5, otro juego, cierre) o del juego anterior
        }
        self.scan.cancel_query(); // ya respondió: suelta su `Handle`
        let (text, timeout) = match result {
            Ok(Some(info)) => {
                let address = info.address.clone();
                observe_version(&mut self.observed_versions, &info.version);
                self.record_response(game, &info);
                self.table.model.add_or_update(info);
                table::drop_hidden_selection(&mut self.table);
                // Solo si la tabla es la lista completa (no un escaneo detenido ni la red local).
                if self.tree_prune_ok && !self.scan.lan() {
                    self.cache.insert(game, self.table.model.servers().to_vec());
                }
                let text = tr_fmt("Servidor actualizado: {address}", &[("address", &address)]);
                (text, Some(Duration::from_secs(4)))
            }
            Ok(None) => (tr("El servidor no respondió"), None),
            Err(e) => (
                tr_fmt("Error al consultar: {error}", &[("error", &e)]),
                None,
            ),
        };
        self.show_message(text, timeout)
    }

    pub(super) fn on_scan(&mut self, msg: ScanMsg) {
        match msg {
            ScanMsg::Event(ScanEvent::Server(info)) => {
                observe_version(&mut self.observed_versions, &info.version);
                if let Some(g) = self.current {
                    self.record_response(g.id, &info); // también los de la red local
                }
                self.table.model.add_or_update(info);
                // La fila nueva puede ya no pasar el filtro.
                table::drop_hidden_selection(&mut self.table);
                if let Some(g) = self.current {
                    self.tree
                        .set_label(g.id, self.game_label(g.id), g.name.to_owned());
                }
                self.tree_dirty = true;
            }
            ScanMsg::Event(ScanEvent::Status(s)) => self.set_status(s),
            ScanMsg::Event(ScanEvent::Progress { done, total }) => {
                self.scan.set_progress(done, total);
            }
            ScanMsg::Finished(result) => {
                self.scan.finish();
                // Solo un escaneo de internet con algún master que respondió reemplaza la
                // lista guardada; la de la red local tampoco poda marcas.
                if let (Some(g), true) = (self.current, result.ok() && !self.scan.lan()) {
                    let servers = self.table.model.servers().to_vec();
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0.0, |d| d.as_secs_f64());
                    self.saved.save(g.id, &servers, now);
                    self.cache.insert(g.id, servers);
                    self.tree_prune_ok = true;
                    self.set_status(tr_fmt(
                        "{responded} de {listed} servidores respondieron",
                        &[
                            ("responded", &result.servers_responded),
                            ("listed", &result.servers_listed),
                        ],
                    ));
                }
                self.update_sidebar();
            }
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.scan.is_scanning()
    }

    /// Texto de la barra de progreso (el `%v / %m` de `QProgressBar`).
    pub fn progress_text(&self) -> String {
        let Some((done, total)) = self.scan.progress() else {
            return String::new();
        };
        tr("Consultando %v / %m")
            .replace("%v", &done.to_string())
            .replace("%m", &total.to_string())
    }
}
