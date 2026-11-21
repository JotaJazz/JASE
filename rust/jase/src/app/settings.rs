//! El diálogo de Ajustes en la ventana (`window_settings.py`): abrirlo, el estado de cada ruta
//! (en un hilo, con número por juego), la pestaña Masters, la clave de Steam y aplicar lo
//! aceptado en el orden de `open_settings` de Python.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use iced::widget::{Space, column, operation, row, text};
use iced::{Element, Fill, Task};
use indexmap::IndexMap;
use jase_core::detect::Finding;
use jase_core::games::{GAMES, GameDef, game};
use jase_core::i18n::{resolve_language, tr, tr_fmt};
use jase_core::masters::{CUSTOM_FILENAME, Master, save_custom};
use jase_core::paths::normpath;
use jase_core::pystr::py_strip;

use super::{App, Dialog, Message, PROMPT_ID, blocking, dialogs::card};
use crate::find_games::{RowState, Search};
use crate::masters_tab::{AddResult, MastersTab};
use crate::passwords::STEAM_KEY_USER;
use crate::settings::{CountryButton, Form, PathStatus, Tab, initial_status};
use crate::startup::system_locale_name;
use crate::theme::{style, widget_style};
use crate::widgets;

/// Espera tras la última tecla antes de buscar el exe de una ruta (el `QTimer` de 400 ms).
pub const PATH_DEBOUNCE: Duration = Duration::from_millis(400);
pub const STEAM_KEY_URL: &str = "https://steamcommunity.com/dev/apikey";

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    Open,
    Tab(Tab),
    Path {
        game: &'static str,
        text: String,
    },
    /// Venció la espera de la edición `generation` de la ruta de `game`.
    PathDebounced {
        game: &'static str,
        generation: u64,
    },
    /// El exe de cada ruta pedida con ese número (`None` = no está).
    PathsResolved {
        generation: u64,
        answers: Vec<(&'static str, Option<String>)>,
    },
    Browse(&'static str),
    Browsed {
        game: &'static str,
        path: Option<PathBuf>,
    },
    DownloadMaps(bool),
    Language(String),
    AutoRefresh(u32),
    CheckUpdates(bool),
    Tray(bool),
    Wine(String),
    SteamKey(String),
    SteamIncludeEmpty(bool),
    OpenSteamKeyPage,
    Theme(String),
    TerminalColor(String),
    /// Pestaña Barra: marcar/desmarcar, elegir, mover, separador y de fábrica.
    ToolbarCheck(usize, bool),
    ToolbarSelect(usize),
    ToolbarMove(bool),
    ToolbarSeparator,
    ToolbarReset,
    UpdateCountries,
    Masters(MastersMsg),
    Accept,
    Cancel,
    /// Guardar la clave en el llavero terminó (el diálogo ya se cerró).
    SteamKeySaved(Result<(), String>),
    /// *Buscar juegos…* de la pestaña Juegos.
    FindGames,
    /// Lo hallado por la búsqueda `request` y cómo está cada juego en la config.
    Found {
        request: u64,
        found: IndexMap<String, Vec<Finding>>,
        states: HashMap<String, RowState>,
    },
    FindCheck(&'static str, bool),
    FindChoose(&'static str, usize),
    FindCancelSearch,
    FindInFolder,
    /// El selector de *Buscar en carpeta…* respondió.
    FindFolder(Option<PathBuf>),
    FindApply,
    FindClose,
    /// Primer arranque: ¿algún juego tiene su exe?
    Offer {
        configured: bool,
    },
    OfferAccept,
    OfferDecline,
}

#[derive(Debug, Clone)]
pub enum MastersMsg {
    Game(&'static str),
    Select(usize),
    Toggle(usize),
    /// *Agregar…*: abre el prompt interno.
    Add,
    AddText(String),
    AddSubmit,
    AddCancel,
    CloseWarning,
    Remove,
    Probe,
    Probed {
        serial: u64,
        game: &'static str,
        master: Master,
        count: Option<usize>,
    },
    Restore,
    /// *Buscar actualización*: revisa `masters.json` sin esperar las 24 h.
    CheckUpdate,
}

/// Lo pedido de cada ruta (`_status_wanted`); el número no vuelve a empezar al reabrir, así
/// una respuesta de un diálogo cerrado nunca vale en el siguiente. También la clave de Steam
/// con que abrió Ajustes.
#[derive(Default)]
pub(crate) struct PathRequests {
    wanted: HashMap<&'static str, u64>,
    serial: u64,
    /// La que mostró el campo al abrir: el llavero puede responder con Ajustes abierto, y
    /// solo se guarda si el usuario la cambió.
    opened_key: String,
}

/// La búsqueda de juegos abierta y la bandera que cancela la que corre; el número no vuelve
/// a empezar, así la respuesta de una búsqueda cerrada nunca vale en la siguiente.
#[derive(Default)]
pub(crate) struct Find {
    search: Option<Search>,
    cancel: Arc<AtomicBool>,
    serial: u64,
}

impl App {
    /// Abre Ajustes con lo actual y pide el exe de las rutas no vacías, todas juntas.
    pub fn open_settings(&mut self) -> Task<Message> {
        let masters = MastersTab::new(
            self.official.clone(),
            self.custom.clone(),
            self.deps.probe.is_some(),
            self.deps.masters_fetch.is_some(),
        );
        let countries = if self.deps.country_db.is_none() || self.country_busy() {
            CountryButton::Disabled
        } else {
            CountryButton::Ready
        };
        let key = self.steam_key.clone().unwrap_or_default();
        let form = Form::new(&self.cfg, &key, masters, countries);
        self.open_dialog(Dialog::Settings);
        self.settings_paths.wanted.clear();
        self.settings_paths.opened_key = key;
        let paths = form
            .paths
            .iter()
            .filter_map(|(id, p)| Some((game(id)?, py_strip(p).to_owned())))
            .filter(|(_, p)| !p.is_empty())
            .collect();
        self.settings = Some(form);
        self.resolve_paths(paths)
    }

    pub fn settings_form(&self) -> Option<&Form> {
        self.settings.as_ref()
    }

    pub fn custom_masters(&self) -> &jase_core::masters::CustomMasters {
        &self.custom
    }

    /// Busca el exe de `paths` en un hilo con un número nuevo (`_request_paths`).
    fn resolve_paths(&mut self, paths: Vec<(&'static GameDef, String)>) -> Task<Message> {
        if paths.is_empty() {
            return Task::none();
        }
        let r = &mut self.settings_paths;
        r.serial += 1;
        let generation = r.serial;
        for (g, _) in &paths {
            r.wanted.insert(g.id, generation);
        }
        let installed = self.deps.game_installed.clone();
        Task::perform(
            blocking(move || {
                paths
                    .into_iter()
                    .map(|(g, dir)| (g.id, installed(g, &dir)))
                    .collect()
            }),
            move |r| {
                Message::Settings(SettingsMsg::PathsResolved {
                    generation,
                    answers: r.unwrap_or_default(),
                })
            },
        )
    }

    /// Cierra Ajustes sin aplicar: lo pedido de sus rutas ya no vale.
    fn close_settings(&mut self) -> Task<Message> {
        self.drop_settings();
        self.close_dialog()
    }

    /// Suelta el formulario (también cuando otro diálogo reemplaza a Ajustes).
    pub(super) fn drop_settings(&mut self) {
        self.settings = None;
        self.settings_paths.wanted.clear();
        self.drop_find();
    }

    pub(super) fn on_settings(&mut self, m: SettingsMsg) -> Task<Message> {
        use SettingsMsg as S;
        match m {
            S::Open => return self.open_settings(),
            S::SteamKeySaved(result) => self.on_steam_key_saved(result),
            // El selector respondió (elegida, cancelado o error): *Examinar…* vuelve.
            S::Browsed { game: id, path } => {
                self.files_open = false;
                if let (Some(p), true) = (path, self.settings.is_some()) {
                    let text = p.display().to_string();
                    return self.edit_settings(S::Path { game: id, text });
                }
            }
            S::Accept if self.settings.is_some() => return self.on_settings_accept(),
            S::Cancel if self.settings.is_some() => return self.close_settings(),
            S::PathsResolved {
                generation,
                answers,
            } => {
                let wanted = &self.settings_paths.wanted;
                if let Some(form) = &mut self.settings {
                    for (id, exe) in answers {
                        if wanted.get(id) == Some(&generation) {
                            let status = exe.map_or(PathStatus::Missing, PathStatus::Found);
                            form.status.insert(id, status);
                        }
                    }
                }
            }
            S::PathDebounced {
                game: id,
                generation,
            } if self.settings_paths.wanted.get(id) == Some(&generation) => {
                let path = self.settings.as_ref().and_then(|f| f.paths.get(id));
                if let (Some(g), Some(path)) = (game(id), path) {
                    let path = py_strip(path).to_owned();
                    return self.resolve_paths(vec![(g, path)]);
                }
            }
            m @ (S::FindGames
            | S::Found { .. }
            | S::FindCheck(..)
            | S::FindChoose(..)
            | S::FindCancelSearch
            | S::FindInFolder
            | S::FindFolder(_)
            | S::FindApply
            | S::FindClose
            | S::Offer { .. }
            | S::OfferAccept
            | S::OfferDecline) => return self.on_find(m),
            m => return self.edit_settings(m),
        }
        Task::none()
    }

    /// Lo que cambia el formulario abierto; sin Ajustes abierto, nada.
    fn edit_settings(&mut self, m: SettingsMsg) -> Task<Message> {
        use SettingsMsg as S;
        let Some(form) = &mut self.settings else {
            return Task::none();
        };
        match m {
            S::Tab(t) => form.tab = t,
            S::Path { game: id, text } => {
                let Some(g) = game(id) else {
                    return Task::none();
                };
                let status = initial_status(g, &text);
                form.paths.insert(id, text);
                form.status.insert(id, status.clone());
                // Una respuesta en camino ya no vale; el disco, tras la espera.
                let r = &mut self.settings_paths;
                r.serial += 1;
                let generation = r.serial;
                r.wanted.insert(id, generation);
                if status == PathStatus::Searching {
                    let wait = async { tokio::time::sleep(PATH_DEBOUNCE).await };
                    return Task::perform(wait, move |()| {
                        Message::Settings(S::PathDebounced {
                            game: id,
                            generation,
                        })
                    });
                }
            }
            // Un selector a la vez: no es modal (como *Exportar*/*Importar*).
            S::Browse(_) if self.files_open => {}
            S::Browse(id) => {
                let (Some(g), Some(path)) = (game(id), form.paths.get(id)) else {
                    return Task::none();
                };
                self.files_open = true;
                let title = tr_fmt("Carpeta de {name}", &[("name", &g.name)]);
                let (files, start) = (self.deps.files.clone(), path.clone());
                return Task::perform(
                    blocking(move || files.pick_folder(&title, &start)),
                    move |r| {
                        Message::Settings(S::Browsed {
                            game: id,
                            path: r.ok().flatten(),
                        })
                    },
                );
            }
            S::DownloadMaps(b) => form.download_maps = b,
            S::Language(code) => form.language = code,
            S::AutoRefresh(n) => form.auto_refresh = n,
            S::CheckUpdates(b) => form.check_updates = b,
            S::Tray(b) => form.tray = b,
            S::Wine(t) => form.wine = t,
            S::SteamKey(t) => form.steam_key = t,
            S::SteamIncludeEmpty(b) => form.steam_include_empty = b,
            S::Theme(id) => form.theme = id,
            S::TerminalColor(c) => form.terminal_color = c,
            S::ToolbarCheck(i, on) => {
                if let Some(row) = form.toolbar.get_mut(i) {
                    row.1 = on;
                }
            }
            S::ToolbarSelect(i) => form.toolbar_selected = Some(i),
            S::ToolbarMove(up) => form.toolbar_move(up),
            S::ToolbarSeparator => form.toolbar_add_separator(),
            S::ToolbarReset => {
                form.toolbar = crate::bars::default_toolbar();
                form.toolbar_selected = None;
            }
            S::OpenSteamKeyPage => {
                let open = self.deps.open_url.clone();
                return Task::perform(blocking(move || open(STEAM_KEY_URL)), |_| ()).discard();
            }
            S::UpdateCountries if form.countries == CountryButton::Ready => {
                form.countries = CountryButton::Downloading;
                if let Some(task) = self.start_country_db(true) {
                    let shown = self.show_message(tr("Descargando base de países…"), None);
                    return Task::batch([task, shown]);
                }
            }
            S::Masters(m) => return self.on_masters(m),
            _ => {}
        }
        Task::none()
    }

    fn on_masters(&mut self, m: MastersMsg) -> Task<Message> {
        use MastersMsg as M;
        let Some(tab) = self.settings.as_mut().map(|f| &mut f.masters) else {
            return Task::none();
        };
        match m {
            M::Game(id) => tab.select_game(id),
            M::Select(r) => tab.select(r),
            // Como `QTableWidget`: el clic en la casilla también elige la fila.
            M::Toggle(r) => {
                tab.select(r);
                tab.toggle(r);
            }
            M::Add => {
                tab.adding = Some(String::new());
                return operation::focus(PROMPT_ID);
            }
            M::AddText(t) => {
                if let Some(text) = &mut tab.adding {
                    *text = t;
                }
            }
            // Inválida o repetida: el aviso, sobre el prompt, que sigue abierto para corregir.
            M::AddSubmit => match tab.adding.clone().map(|t| tab.add(&t)) {
                Some(AddResult::Ok) => tab.adding = None,
                Some(AddResult::Invalid) => {
                    tab.warning =
                        Some(tr("Dirección no válida; usa host:puerto (puerto 1-65535)."));
                }
                Some(AddResult::Duplicate) => {
                    tab.warning = Some(tr("Ese master ya está en la lista."));
                }
                None => {}
            },
            M::AddCancel => tab.adding = None,
            M::CloseWarning => tab.warning = None,
            M::Remove => tab.remove_selected(),
            M::Restore => tab.restore_officials(),
            M::Probe => {
                let (Some(probe), Some((serial, id, master))) =
                    (self.deps.probe.clone(), tab.start_probe())
                else {
                    return Task::none();
                };
                let Some(g) = game(id) else {
                    return Task::none();
                };
                // La clave con que abrió Ajustes (`MastersContext.steam_key`).
                let key = self.settings_paths.opened_key.clone();
                let sent = master.clone();
                return Task::perform(async move { probe(g, sent, key).await }, move |count| {
                    Message::Settings(SettingsMsg::Masters(M::Probed {
                        serial,
                        game: id,
                        master: master.clone(),
                        count,
                    }))
                });
            }
            M::Probed {
                serial,
                game,
                master,
                count,
            } => {
                tab.on_probe(serial, game, &master, count);
            }
            M::CheckUpdate => {
                tab.update_status = tr("Buscando actualización…");
                return self.check_masters_update(true);
            }
        }
        Task::none()
    }

    /// Esc con Ajustes abierto: cierra el diálogo interno más adentro, o Ajustes.
    pub(super) fn cancel_settings(&mut self) -> Task<Message> {
        let tab = self.settings.as_ref().map(|f| &f.masters);
        if self.find.search.is_some() {
            self.on_find(SettingsMsg::FindClose)
        } else if tab.is_some_and(|t| t.warning.is_some()) {
            self.on_masters(MastersMsg::CloseWarning)
        } else if tab.is_some_and(|t| t.adding.is_some()) {
            self.on_masters(MastersMsg::AddCancel)
        } else {
            self.on_settings(SettingsMsg::Cancel)
        }
    }

    /// Enter fuera de un campo: el botón por defecto del diálogo más adentro.
    pub(super) fn submit_settings(&mut self) -> Task<Message> {
        let tab = self.settings.as_ref().map(|f| &f.masters);
        if self.find.search.is_some() {
            self.on_find(SettingsMsg::FindApply)
        } else if tab.is_some_and(|t| t.warning.is_some()) {
            self.on_masters(MastersMsg::CloseWarning)
        } else if tab.is_some_and(|t| t.adding.is_some()) {
            self.on_masters(MastersMsg::AddSubmit)
        } else {
            self.on_settings(SettingsMsg::Accept)
        }
    }

    /// Aplica lo aceptado en el orden de `open_settings` de Python.
    fn on_settings_accept(&mut self) -> Task<Message> {
        let Some(form) = self.settings.take() else {
            return Task::none();
        };
        let mut tasks = vec![self.close_settings()];
        let masters_changed = self.apply_custom_masters(form.masters.custom.clone());
        let previous_language = self.cfg.language.clone();
        let previous_theme = (self.cfg.theme.clone(), self.cfg.terminal_color.clone());
        let previous_empty = self.cfg.steam_include_empty;
        form.apply_to(&mut self.cfg);
        tasks.push(self.sync_tray());
        let mut steam_changed = false;
        let key = form.steam_key();
        if key != self.settings_paths.opened_key {
            // Rige en esta sesión aunque falle el guardado.
            self.steam_key = Some(key.clone());
            steam_changed = true;
            let keyring = self.deps.keyring.clone();
            tasks.push(Task::perform(
                blocking(move || keyring.set(STEAM_KEY_USER, &key)),
                |r| Message::Settings(SettingsMsg::SteamKeySaved(r.and_then(|r| r))),
            ));
        }
        if self.cfg.steam_include_empty != previous_empty {
            // La lista de los juegos de Steam ya no vale.
            let steam = |id: &str| game(id).is_some_and(GameDef::uses_steam_api);
            self.cache.retain(|id, _| !steam(id));
            self.saved.forget(steam);
            steam_changed = true; // fuerza relistar si el juego actual es de Steam
        }
        tasks.push(self.after_game_paths_changed());
        if (self.cfg.theme.clone(), self.cfg.terminal_color.clone()) != previous_theme {
            self.apply_theme();
        }
        let system = system_locale_name();
        if resolve_language(&self.cfg.language, &system)
            != resolve_language(&previous_language, &system)
        {
            self.open_dialog(Dialog::LanguageRestart);
        }
        let steam_game = self.current.is_some_and(GameDef::uses_steam_api);
        if (masters_changed || (steam_changed && steam_game))
            && self.current.is_some()
            && self.direct_view().is_none()
        {
            tasks.push(self.refresh(false));
        }
        Task::batch(tasks)
    }

    /// `_apply_custom_masters`: guarda los personales editados; ¿cambiaron?
    fn apply_custom_masters(&mut self, custom: jase_core::masters::CustomMasters) -> bool {
        if custom == self.custom {
            return false;
        }
        let path = self
            .cfg_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(CUSTOM_FILENAME);
        match save_custom(&custom, &path) {
            Err(e) => self.set_status(tr_fmt(
                "No se pudo guardar la configuración: {error}",
                &[("error", &e)],
            )),
            // Migración pendiente: ya no debe repetirse.
            Ok(()) if !self.cfg.masters.is_empty() => {
                self.cfg.masters.clear();
                self.save_config();
            }
            Ok(()) => {}
        }
        self.custom = custom; // en esta sesión rige lo editado aunque falle el guardado
        true
    }

    /// El aviso si el llavero no guardó la clave; con otro diálogo abierto, a la barra.
    fn on_steam_key_saved(&mut self, result: Result<(), String>) {
        let Err(e) = result else {
            return;
        };
        let title = tr("Clave de API de Steam");
        let text = tr_fmt(
            "No se pudo guardar la clave de Steam: {error}\nSe usará solo en esta sesión.",
            &[("error", &e)],
        );
        if self.dialog.is_some() {
            self.set_status(format!("{title}: {text}"));
        } else {
            self.open_dialog(Dialog::Message {
                title,
                text,
                informative: String::new(),
                settings: false,
            });
        }
    }

    /// `_notify_restart_for_language`: «Idioma» con *Entendido*.
    pub(super) fn restart_view(&self) -> Element<'static, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let ok = Message::Prompt(super::PromptMsg::Submit);
        let content = column![
            label(tr("Idioma")).font(style::header_font(s)),
            label(tr("El idioma cambiará al reiniciar JASE.")),
            row![
                Space::new().width(Fill),
                widgets::button(tr("Entendido"), ok, theme_id)
            ],
        ];
        card(content, 320.0, theme_id)
    }
}

impl App {
    pub fn find_search(&self) -> Option<&Search> {
        self.find.search.as_ref()
    }

    /// *Buscar juegos…* del menú: el diálogo, y lo elegido se guarda al aplicar.
    pub fn find_games(&mut self) -> Task<Message> {
        self.open_dialog(Dialog::FindGames);
        self.start_find(None, false)
    }

    /// `_start`: cancela la búsqueda anterior y lanza otra con un número nuevo; el estado de
    /// cada juego hallado se lee del disco en el mismo hilo.
    fn start_find(&mut self, extra: Option<String>, from_settings: bool) -> Task<Message> {
        self.find.cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.find.cancel = cancel.clone();
        self.find.serial += 1;
        let request = self.find.serial;
        self.find.search = Some(Search::new(request, from_settings));
        let (find, installed) = (
            self.deps.find_games.clone(),
            self.deps.game_installed.clone(),
        );
        let paths = self.cfg.game_paths.clone();
        let work = move || {
            let found = find(extra, &|| cancel.load(Ordering::Relaxed));
            let states = found
                .keys()
                .filter_map(|id| {
                    let g = game(id)?;
                    let path = paths.get(id).map_or("", String::as_str);
                    let state = if path.is_empty() {
                        RowState::New
                    } else if installed(g, path).is_some() {
                        RowState::Configured
                    } else {
                        RowState::SavedMissing
                    };
                    Some((id.clone(), state))
                })
                .collect();
            (found, states)
        };
        // Un fallo de la búsqueda equivale a no encontrar nada (como Python).
        Task::perform(blocking(work), move |r| {
            let (found, states) = r.unwrap_or_default();
            Message::Settings(SettingsMsg::Found {
                request,
                found,
                states,
            })
        })
    }

    /// Cierra la búsqueda (aplicada, cancelada o su diálogo reemplazado); la que corre se
    /// cancela.
    pub(super) fn drop_find(&mut self) -> Option<Search> {
        self.find.cancel.store(true, Ordering::Relaxed);
        self.find.search.take()
    }

    fn on_find(&mut self, m: SettingsMsg) -> Task<Message> {
        use SettingsMsg as S;
        match m {
            S::FindGames if self.settings.is_some() && self.find.search.is_none() => {
                return self.start_find(None, true);
            }
            S::Found {
                request,
                found,
                states,
            } => {
                // Una búsqueda reemplazada, cancelada o cerrada no toca las filas.
                if let Some(s) = &mut self.find.search
                    && s.request == request
                    && s.searching
                {
                    s.on_found(found, states);
                }
            }
            S::FindCheck(id, checked) => {
                if let Some(r) = self.find.search.as_mut().and_then(|s| s.row_mut(id)) {
                    r.checked = checked;
                }
            }
            S::FindChoose(id, i) => {
                if let Some(r) = self.find.search.as_mut().and_then(|s| s.row_mut(id))
                    && i < r.findings.len()
                {
                    r.chosen = i;
                }
            }
            S::FindCancelSearch => {
                if let Some(s) = self.find.search.as_mut().filter(|s| s.searching) {
                    self.find.cancel.store(true, Ordering::Relaxed);
                    s.cancel();
                }
            }
            // Un selector a la vez: no es modal.
            S::FindInFolder if !self.files_open && self.find.search.is_some() => {
                self.files_open = true;
                let (files, title) = (self.deps.files.clone(), tr("Buscar juegos en la carpeta"));
                return Task::perform(blocking(move || files.pick_folder(&title, "")), |r| {
                    Message::Settings(S::FindFolder(r.ok().flatten()))
                });
            }
            S::FindFolder(path) => {
                self.files_open = false;
                let from_settings = self.find.search.as_ref().map(|s| s.from_settings);
                if let (Some(p), Some(from_settings)) = (path, from_settings) {
                    // Sin separadores mezclados.
                    let extra = normpath(&p.display().to_string());
                    return self.start_find(Some(extra), from_settings);
                }
            }
            S::FindApply => {
                let Some(search) = self.drop_find() else {
                    return Task::none();
                };
                let chosen = search.selected_paths();
                if search.from_settings {
                    // Los campos de Ajustes, como una edición: nada se guarda hasta *Guardar*.
                    let edits: Vec<_> = chosen
                        .into_iter()
                        .filter_map(|(id, text)| Some((game(&id)?.id, text)))
                        .collect();
                    let tasks: Vec<_> = edits
                        .into_iter()
                        .map(|(id, text)| self.edit_settings(S::Path { game: id, text }))
                        .collect();
                    return Task::batch(tasks);
                }
                let close = self.close_dialog();
                if chosen.is_empty() {
                    return close;
                }
                self.cfg.game_paths.extend(chosen);
                return Task::batch([close, self.after_game_paths_changed()]);
            }
            S::FindClose => {
                if self.drop_find().is_some_and(|s| !s.from_settings) {
                    return self.close_dialog();
                }
            }
            S::Offer { configured } => self.on_offer(configured),
            S::OfferAccept if self.dialog() == Some(&Dialog::OfferFindGames) => {
                let close = self.close_dialog();
                return Task::batch([close, self.find_games()]);
            }
            S::OfferDecline if self.dialog() == Some(&Dialog::OfferFindGames) => {
                return self.close_dialog();
            }
            _ => {}
        }
        Task::none()
    }

    /// `offer_find_games`: en el primer arranque, ¿hay algún juego configurado? (en un hilo).
    pub fn offer_find_games(&mut self) -> Task<Message> {
        if self.cfg.find_games_offered {
            return Task::none();
        }
        let paths: Vec<(&'static GameDef, String)> = GAMES
            .iter()
            .filter_map(|g| {
                let path = self.cfg.game_paths.get(g.id).filter(|p| !p.is_empty())?;
                Some((g, path.clone()))
            })
            .collect();
        let installed = self.deps.game_installed.clone();
        let any = move || paths.iter().any(|(g, p)| installed(g, p).is_some());
        Task::perform(blocking(any), |r| {
            Message::Settings(SettingsMsg::Offer {
                configured: r.unwrap_or(false),
            })
        })
    }

    /// Se marca una sola vez; con otro diálogo abierto (p. ej. un enlace de `argv`) se ofrece
    /// en el próximo arranque.
    fn on_offer(&mut self, configured: bool) {
        if self.cfg.find_games_offered || (!configured && self.dialog.is_some()) {
            return;
        }
        self.cfg.find_games_offered = true;
        self.save_config();
        if !configured {
            self.open_dialog(Dialog::OfferFindGames);
        }
    }

    /// La pregunta del primer arranque: *Buscar juegos* / *Ahora no*.
    pub(super) fn offer_view(&self) -> Element<'static, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let msg = Message::Settings;
        let content = column![
            label(tr("Buscar juegos")).font(style::header_font(s)),
            label(tr("No hay juegos configurados. ¿Buscar juegos instalados?")),
            row![
                Space::new().width(Fill),
                widgets::button(tr("Buscar juegos"), msg(SettingsMsg::OfferAccept), theme_id),
                widgets::button(tr("Ahora no"), msg(SettingsMsg::OfferDecline), theme_id),
            ]
            .spacing(6),
        ];
        card(content, 360.0, theme_id)
    }
}
