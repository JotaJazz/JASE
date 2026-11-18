//! El escaneo del juego actual: arrancarlo, detenerlo y lo que va llegando.

use std::time::{SystemTime, UNIX_EPOCH};

use iced::Task;
use jase_core::i18n::{tr, tr_fmt};
use jase_core::masters::effective_masters;
use jase_core::scanner::scan::{ScanEvent, ScanOptions};

use super::{App, Message, observe_version};
use crate::scan::ScanMsg;
use crate::table;

impl App {
    /// Como `model.clear()` de Qt, que también suelta la selección y vuelve arriba; la tarea
    /// mueve el `scrollable`, que si no seguiría desplazado y dibujaría la tabla en blanco.
    pub(super) fn clear_table(&mut self) -> Task<Message> {
        self.table.model.clear();
        self.table.selected = None;
        table::scroll_to_top(&mut self.table)
    }

    pub(super) fn start_scan(&mut self) -> Task<Message> {
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
    pub(super) fn stop_active_scan(&mut self) {
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

    pub(super) fn on_scan(&mut self, msg: ScanMsg) {
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
                    let servers = self.table.model.servers().to_vec();
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0.0, |d| d.as_secs_f64());
                    self.saved.save(g.id, &servers, now);
                    self.cache.insert(g.id, servers);
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
}
