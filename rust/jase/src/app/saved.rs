//! Cambiar de juego y las listas guardadas (de esta sesión o del disco).

use iced::Task;
use jase_core::config::version_for;
use jase_core::games::{GameDef, game};
use jase_core::i18n::tr_fmt;
use jase_core::models::ServerInfo;

use super::{App, Message, observe_version};
use crate::saved_lists::short_datetime;

impl App {
    pub(super) fn select_game(&mut self, id: &str) -> Task<Message> {
        let Some(g) = game(id) else {
            return Task::none();
        };
        if self.current.is_some_and(|c| c.id == g.id) {
            return Task::none();
        }
        self.stop_active_scan();
        self.scan.cancel_query();
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
        let task = if self.cache.contains_key(g.id) {
            self.show_cached_list(g);
            top
        } else if let Some((_, load)) = self.saved.load(g.id) {
            // La del disco llega en un hilo (`Message::SavedList`).
            self.status = tr_fmt(
                "Cargando la lista guardada de {name}…",
                &[("name", &g.name)],
            );
            Task::batch([top, load])
        } else {
            Task::batch([top, self.start_scan()])
        };
        // El desglose del árbol (con caché, poda marcas); el juego que se deja pasa a su caché.
        self.update_sidebar();
        if let Some(p) = previous.filter(|p| p.id != g.id) {
            self.set_tree_breakdown(p.id);
        }
        task
    }

    /// Tabla con la lista guardada (de esta sesión o, al abrir, del disco).
    pub(super) fn show_cached_list(&mut self, g: &'static GameDef) {
        let Some(cached) = self.cache.get(g.id) else {
            return;
        };
        for info in cached {
            observe_version(&mut self.observed_versions, &info.version);
            self.table.model.add_or_update(info.clone());
        }
        self.tree_prune_ok = true;
        self.status = match self.saved.from_disk(g.id) {
            Some(at) => tr_fmt(
                "{name}: lista guardada del {when}; F5 para refrescar",
                &[("name", &g.name), ("when", &short_datetime(at))],
            ),
            None => tr_fmt("{name}: lista guardada de la sesión", &[("name", &g.name)]),
        };
    }

    /// `_on_saved_list_loaded`: una lista de esta sesión siempre manda; la dañada o borrada
    /// es como si no hubiera (se escanea si la tabla la esperaba).
    pub(super) fn on_saved_list(
        &mut self,
        id: &'static str,
        result: Option<(f64, Vec<ServerInfo>)>,
    ) -> Task<Message> {
        let current = self
            .current
            .filter(|c| c.id == id && !self.is_scanning() && self.table.model.is_empty());
        let mut task = Task::none();
        match result {
            None => {
                self.saved.index.shift_remove(id);
                if current.is_some() {
                    task = self.start_scan();
                }
            }
            Some((at, servers)) => {
                if !self.cache.contains_key(id) {
                    self.cache.insert(id, servers);
                    self.saved.saved_at.insert(id, at);
                }
                if let Some(g) = current {
                    self.show_cached_list(g);
                }
            }
        }
        self.update_sidebar();
        task
    }
}
