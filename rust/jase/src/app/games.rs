//! El árbol de juegos: filas, textos, desglose y «Solo instalados».

use iced::Task;
use iced::widget::image::Handle;
use indexmap::IndexMap;
use jase_core::browse::breakdown;
use jase_core::config::Config;
use jase_core::games::{GAMES, GameDef, LaunchStyle, game};
use jase_core::models::ServerInfo;

use super::{App, Message, blocking};

impl App {
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
    pub(super) fn update_sidebar(&mut self) {
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
    /// su lista guardada (siempre poda); el actual en una vista directa, de su lista guardada sin
    /// podar (`_tree_source`). Una marca podada del actual cambia el filtro.
    pub(super) fn set_tree_breakdown(&mut self, id: &'static str) {
        let Some(g) = game(id) else {
            return;
        };
        let model = &self.table.model;
        let current = self.current.is_some_and(|c| c.id == id);
        let (b, prune) = if current && self.direct_view().is_none() {
            let country = |info: &ServerInfo| model.country_code(info).map(str::to_owned);
            let b = breakdown(g, model.servers(), Some(&country));
            (Some(b), self.tree_prune_ok)
        } else {
            let country = |info: &ServerInfo| model.lookup(info);
            let b = self.cache.get(id).map(|s| breakdown(g, s, Some(&country)));
            (b, !current)
        };
        if self.tree.set_breakdown(id, b, prune) && current {
            self.apply_tree_filter();
        }
    }

    /// Busca el exe de cada juego con ruta y su icono en un hilo (`_refresh_game_icons`); un
    /// juego sin ruta pierde el icono enseguida. Un cambio de tema no la repite: la insignia se
    /// dibuja con la paleta en `view` y el icono del exe no depende del tema.
    pub(super) fn request_installed(&mut self) -> Task<Message> {
        let paths: Vec<(&'static GameDef, String)> = GAMES
            .iter()
            .filter_map(|g| {
                let dir = self.cfg.game_paths.get(g.id).filter(|d| !d.is_empty())?;
                Some((g, dir.clone()))
            })
            .collect();
        self.tree
            .exe_icons
            .retain(|id, _| paths.iter().any(|(g, _)| g.id == *id));
        self.installed_gen += 1;
        if paths.is_empty() {
            return Task::none();
        }
        let generation = self.installed_gen;
        let (installed, icon) = (self.deps.game_installed.clone(), self.deps.exe_icon.clone());
        Task::perform(
            blocking(move || {
                paths
                    .into_iter()
                    .map(|(g, dir)| (g.id, installed(g, &dir), icon(g, &dir)))
                    .collect::<Vec<_>>()
            }),
            move |r| Message::Installed {
                generation,
                answers: r.unwrap_or_default(),
            },
        )
    }

    pub(super) fn on_installed(&mut self, answers: Vec<(&'static str, bool, Option<Handle>)>) {
        for (id, installed, icon) in answers {
            self.installed.insert(id, installed);
            match icon {
                Some(icon) => self.tree.exe_icons.insert(id, icon),
                None => self.tree.exe_icons.remove(id), // sin exe o sin icono: la insignia
            };
        }
        if self.cfg.only_installed_games {
            self.refresh_tree_games();
        }
    }

    /// Rutas nuevas de los juegos: vuelve a buscar los exes y sus iconos. El diálogo de ajustes
    /// de la 5f le sumará guardar la config, la ficha y el orden (`_after_game_paths_changed`).
    pub fn set_game_paths(&mut self, paths: IndexMap<String, String>) -> Task<Message> {
        self.cfg.game_paths = paths;
        self.request_installed()
    }

    /// Filas del árbol: con «Solo instalados», los juegos con exe encontrado o que conectan
    /// por Steam, más el actual (`_tree_game_ids`). Sin respuesta, una ruta cuenta como lista.
    pub(super) fn tree_game_ids(&self) -> Vec<&'static str> {
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

    pub(super) fn refresh_tree_games(&mut self) {
        let ids = self.tree_game_ids();
        if ids != self.tree.game_ids() {
            self.tree.set_games(ids);
            self.update_sidebar();
        }
    }

    pub fn installed_generation(&self) -> u64 {
        self.installed_gen
    }

    pub fn clear_tree_marks(&mut self) {
        if let Some(g) = self.current {
            self.tree.clear_marks(g.id);
            self.apply_tree_filter();
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
        } else if let Some((n, _)) = self.saved.index.get(id) {
            n.to_string() // lista guardada de otra sesión, sin leer
        } else if current && self.scan.stopped() && self.direct_view().is_none() && rows > 0 {
            rows.to_string()
        } else {
            "—".to_owned()
        };
        format!("{} · {count}", g.short_name())
    }
}
