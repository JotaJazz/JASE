//! Filtros de la tabla: rápidos, texto, versión, marcas del árbol y la base de países.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::Task;
use jase_core::config::version_for;
use jase_core::geo::{CountryDB, DB_FILENAME};
use jase_core::i18n::tr;
use jase_core::models::QuickFilters;

use super::{App, Focus, Message, blocking};
use crate::sidebar::{self, Combo, CountryOption, VersionOption};
use crate::table::{self, ServerFilter};

impl App {
    /// El combo cuya lista está abierta (su tooltip no se dibuja).
    pub fn combo_open(&self) -> Option<Combo> {
        self.combo_open
    }

    /// Cambia el filtro de la tabla (y suelta la selección que deja de verse).
    pub(super) fn edit_filter(&mut self, edit: impl FnOnce(&mut ServerFilter)) {
        let mut f = self.table.model.filter().clone();
        edit(&mut f);
        table::set_filter(&mut self.table, f);
    }

    /// Marcas del juego actual → filtro de la tabla (mods y mapas solo cuentan con juego).
    pub(super) fn apply_tree_filter(&mut self) {
        // En Favoritos/Recientes las marcas no se aplican.
        let marks = self
            .current
            .filter(|_| self.direct_view().is_none())
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

    /// Filtros rápidos, texto y versión (`_on_quick_filters_changed`, `_on_version_changed`);
    /// cada cambio se guarda en la config.
    pub(super) fn on_sidebar(&mut self, m: sidebar::Msg) {
        use sidebar::Msg as S;
        if let S::Text(t) = m {
            self.set_focus(Focus::Filter);
            self.edit_filter(|f| f.set_text(&t));
            self.filter_text = t;
            return;
        }
        // Una casilla o un combo (también abierto) se queda el foco, como en Qt: las flechas
        // no llegan al árbol ni a la tabla.
        self.set_focus(Focus::Sidebar);
        // Abierta la lista se queda con el teclado. Cerrar solo cuenta para la propia (al pasar
        // de un combo a otro, el otro abre antes); elegir u otro clic la cierra.
        self.combo_open = match m {
            S::ComboOpened(c) => Some(c),
            S::CountryOpened => Some(Combo::Country),
            S::ComboClosed(c) if self.combo_open != Some(c) => self.combo_open,
            _ => None,
        };
        let q = &mut self.cfg.quick_filters;
        match m {
            S::HideEmpty(b) => q.hide_empty = b,
            S::HideFull(b) => q.hide_full = b,
            S::HidePassword(b) => q.hide_password = b,
            S::HideBots(b) => q.hide_bots = b,
            S::MaxPing(ms) => q.max_ping = ms,
            S::Country(code) => q.country = code,
            S::ComboOpened(_) | S::ComboClosed(_) => return,
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
                if self.direct_view().is_none() {
                    self.edit_filter(|f| f.version = v);
                }
                self.save_config();
                return;
            }
            S::Text(_) => unreachable!("atendido arriba"),
        }
        let quick = self.cfg.quick_filters.clone();
        if self.direct_view().is_none() {
            self.edit_filter(|f| f.quick = quick); // en Favoritos/Recientes no se aplican
        }
        self.save_config();
    }

    /// Pide la base de países (`_start_country_db`; con `refresh`, la vuelve a bajar); llega en
    /// `Message::CountryDb`. `None` sin base o con un pedido en curso.
    pub fn start_country_db(&mut self, refresh: bool) -> Option<Task<Message>> {
        let load = self.deps.country_db.clone()?;
        if self.country_busy.is_some() {
            return None;
        }
        self.country_busy = Some(refresh);
        self.country_request += 1;
        let request = self.country_request;
        let dir = self
            .cfg_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default();
        let path = dir.join("cache").join(DB_FILENAME);
        Some(Task::perform(
            blocking(move || load(&path, refresh).map(Arc::new)),
            move |r| Message::CountryDb {
                request,
                result: r.and_then(|db| db),
            },
        ))
    }

    pub fn country_busy(&self) -> bool {
        self.country_busy.is_some()
    }

    /// `_on_country_db_loaded`: el error solo va a la barra de estado, 8 s.
    pub(super) fn on_country_db(
        &mut self,
        result: Result<Arc<CountryDB>, String>,
    ) -> Task<Message> {
        let refresh = self.country_busy.take() == Some(true);
        match result {
            Ok(db) => {
                let lookup = move |ip: &str| db.lookup(ip).map(str::to_owned);
                self.table.model.set_country_lookup(Some(Arc::new(lookup)));
                table::drop_hidden_selection(&mut self.table);
                // La carpeta «Países» necesita la base.
                if let Some(g) = self.current {
                    self.set_tree_breakdown(g.id);
                }
                if refresh {
                    let done = tr("Base de países actualizada");
                    return self.show_message(done, Some(Duration::from_secs(5)));
                }
            }
            Err(e) => {
                let e = if e.is_empty() {
                    tr("Base de países no disponible")
                } else {
                    e
                };
                return self.show_message(e, Some(Duration::from_secs(8)));
            }
        }
        Task::none()
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

    pub fn country_options(&self) -> Vec<CountryOption> {
        sidebar::country_options(&self.country_counts, &self.cfg.quick_filters.country)
    }

    /// Opciones del combo de versión del juego actual (vacío sin juego).
    pub fn version_options(&self) -> Vec<VersionOption> {
        self.current.map_or_else(Vec::new, |g| {
            sidebar::version_options(g, version_for(g, &self.cfg), &self.observed_versions)
        })
    }
}
