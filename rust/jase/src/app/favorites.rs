//! Estrella, notas, recientes y grupos de favoritos (`window_favorites.py`, `window_groups.py`):
//! solo tocan la config (pequeña: se guarda en el hilo de la UI). Exportar e importar pasan
//! el selector y el archivo a `blocking`.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::operation;
use iced::{Task, clipboard};
use jase_core::favorites::{
    add_favorite, clean_group, favorite_addresses, game_notes, get_group, get_note, group_names,
    is_favorite, remove_favorite, set_group, set_note,
};
use jase_core::favorites_io::{self, read_import_file};
use jase_core::fsutil::atomic_write;
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::recent::remove_recent;
use serde_json::Value;

use super::{App, Dialog, DirectView, Focus, Message, PROMPT_ID, blocking};
use crate::files::{FilesMsg, ImportFailure};
use crate::table;

/// Lo que muestran los mensajes de favoritos en la barra de estado (4 s, como Python).
const NOTICE: Duration = Duration::from_secs(4);
/// El resumen de lo importado (6 s).
const IMPORTED: Duration = Duration::from_secs(6);

/// Una opción del combo de grupo: `None` = todos, `Some("")` = sin grupo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupOption {
    pub key: Option<String>,
    pub label: String,
}

impl fmt::Display for GroupOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

impl App {
    /// `_is_selected_favorite`.
    pub fn is_selected_favorite(&self) -> bool {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return false;
        };
        is_favorite(&self.cfg, g.id, &info.address)
    }

    /// Agrega o quita el elegido (por su dirección de consulta); en Favoritos, quitar borra la
    /// fila. Nunca consulta.
    pub fn toggle_favorite_selected(&mut self) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        let address = info.address.clone();
        let shown = [("address", &address as &dyn fmt::Display)];
        let text = if remove_favorite(&mut self.cfg, g.id, &address) {
            self.sync_notes();
            if self.direct_view() == Some(DirectView::Favorites) {
                self.table.model.remove(&address);
                table::drop_hidden_selection(&mut self.table);
            }
            tr_fmt("Quitado de favoritos: {address}", &shown)
        } else {
            add_favorite(&mut self.cfg, g.id, &address);
            tr_fmt("Agregado a favoritos: {address}", &shown)
        };
        self.save_config();
        let groups = self.sync_groups();
        Task::batch([groups, self.show_message(text, Some(NOTICE))])
    }

    /// Pasa a la tabla las notas del juego actual (`_sync_notes`).
    pub fn sync_notes(&mut self) {
        let notes = self
            .current
            .map(|g| game_notes(&self.cfg, g.id))
            .unwrap_or_default();
        self.table.model.set_notes(notes);
    }

    /// Abre la nota del favorito elegido (*Guardar* vacía la borra).
    pub fn edit_note_selected(&mut self) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        if !is_favorite(&self.cfg, g.id, &info.address) {
            return Task::none();
        }
        let address = info.address.clone();
        let text = get_note(&self.cfg, g.id, &address.to_string()).to_owned();
        self.open_dialog(Dialog::Note {
            game: g.id,
            address,
            text,
        });
        // Como `QInputDialog.getText`: la nota elegida, lista para reemplazarla.
        Task::batch([
            operation::focus(PROMPT_ID),
            operation::select_all(PROMPT_ID),
        ])
    }

    /// Quita el elegido de los recientes (solo en esa vista).
    pub fn remove_recent_selected(&mut self) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        if self.direct_view() != Some(DirectView::Recent) {
            return Task::none();
        }
        let address = info.address.clone();
        if !remove_recent(&mut self.cfg, g.id, &address) {
            return Task::none();
        }
        self.table.model.remove(&address);
        table::drop_hidden_selection(&mut self.table);
        self.save_config();
        let text = tr_fmt("Quitado de recientes: {address}", &[("address", &address)]);
        self.show_message(text, Some(NOTICE))
    }

    /// El grupo elegido en la vista Favoritos; `None` = todos, `""` = sin grupo.
    pub fn current_group(&self) -> Option<&str> {
        match self.direct_view() {
            Some(DirectView::Favorites) => self.group.as_deref(),
            _ => None,
        }
    }

    /// Las opciones del combo de grupo; vacío = oculto (solo se ve en Favoritos con grupos).
    pub fn group_options(&self) -> Vec<GroupOption> {
        let Some(g) = self.current else {
            return Vec::new();
        };
        let names = group_names(&self.cfg, g.id);
        if self.direct_view() != Some(DirectView::Favorites) || names.is_empty() {
            return Vec::new();
        }
        let all = GroupOption {
            key: None,
            label: tr("Todos los grupos"),
        };
        let none = GroupOption {
            key: Some(String::new()),
            label: tr("Sin grupo"),
        };
        let named = names.into_iter().map(|n| GroupOption {
            key: Some(n.clone()),
            label: n,
        });
        std::iter::once(all).chain(named).chain([none]).collect()
    }

    /// `_sync_group_combo`: el grupo elegido que ya no existe vuelve a «Todos» y, en
    /// Favoritos, se reconsulta.
    pub fn sync_groups(&mut self) -> Task<Message> {
        let names = self.group_names();
        match &self.group {
            Some(name) if !name.is_empty() && !names.contains(name) => {
                self.group = None;
                if self.direct_view() == Some(DirectView::Favorites) {
                    return self.query_direct_list();
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Elegido en el combo de la barra: en Favoritos vuelve a consultar.
    pub(super) fn pick_group(&mut self, group: Option<String>) -> Task<Message> {
        // Como el combo de versión: el combo de la barra se queda el foco.
        self.set_focus(Focus::Sidebar);
        self.combo_open = None;
        self.group = group;
        if self.direct_view() == Some(DirectView::Favorites) {
            return self.query_direct_list();
        }
        Task::none()
    }

    /// Mueve el favorito elegido al grupo `name` (vacío = sin grupo); si se ve otro grupo, su
    /// fila sale.
    pub fn move_selected_to_group(&mut self, name: &str) -> Task<Message> {
        let (Some(g), Some(info)) = (self.current, self.selected_server()) else {
            return Task::none();
        };
        if !is_favorite(&self.cfg, g.id, &info.address) {
            return Task::none();
        }
        let (address, server) = (info.address.clone(), info.name.clone());
        set_group(&mut self.cfg, g.id, &address.to_string(), name);
        self.save_config();
        let clean = clean_group(name);
        if self.current_group().is_some_and(|shown| shown != clean) {
            self.table.model.remove(&address);
            table::drop_hidden_selection(&mut self.table);
        }
        let groups = self.sync_groups();
        let text = if clean.is_empty() {
            tr_fmt("{server} quedó sin grupo", &[("server", &server)])
        } else {
            tr_fmt(
                "{server} pasó al grupo «{group}»",
                &[("server", &server), ("group", &clean)],
            )
        };
        Task::batch([groups, self.show_message(text, Some(NOTICE))])
    }

    /// «Nuevo grupo…» del submenú *Grupo*.
    pub(super) fn new_group_for_selected(&mut self) -> Task<Message> {
        if !self.is_selected_favorite() {
            return Task::none();
        }
        self.open_dialog(Dialog::NewGroup(String::new()));
        operation::focus(PROMPT_ID)
    }

    /// *Guardar* de la nota o del grupo nuevo (el diálogo ya cerrado).
    pub(super) fn submit_text_dialog(&mut self, dialog: Dialog) -> Task<Message> {
        match dialog {
            Dialog::Note {
                game,
                address,
                text,
            } => {
                set_note(&mut self.cfg, game, &address.to_string(), &text);
                self.save_config();
                self.sync_notes();
                Task::none()
            }
            Dialog::NewGroup(name) if !clean_group(&name).is_empty() => {
                self.move_selected_to_group(&name)
            }
            _ => Task::none(),
        }
    }

    /// El grupo del favorito elegido (`""` = sin grupo), si lo es.
    pub fn selected_group(&self) -> Option<&str> {
        let (g, info) = (self.current?, self.selected_server()?);
        is_favorite(&self.cfg, g.id, &info.address)
            .then(|| get_group(&self.cfg, g.id, &info.address.to_string()))
    }

    pub fn group_names(&self) -> Vec<String> {
        self.current
            .map(|g| group_names(&self.cfg, g.id))
            .unwrap_or_default()
    }

    /// Copia los favoritos (del grupo elegido) como texto para un chat: «IP:puerto  nombre»
    /// por línea, que JASE vuelve a leer al pegarlo.
    pub fn copy_favorites_as_text(&mut self) -> Task<Message> {
        let Some(g) = self.current else {
            return Task::none();
        };
        let addresses = self.group_filter(g.id, favorite_addresses(&self.cfg, g.id));
        if addresses.is_empty() {
            let text = tr_fmt(
                "No hay favoritos de {name} para copiar",
                &[("name", &g.name)],
            );
            return self.show_message(text, Some(NOTICE));
        }
        // La caché de la sesión y, encima, lo que muestra la tabla.
        let known: HashMap<&ServerAddress, &ServerInfo> = self
            .cache
            .get(g.id)
            .into_iter()
            .flatten()
            .chain(self.table.model.servers())
            .map(|i| (&i.address, i))
            .collect();
        let mut text = match self.current_group() {
            Some(group) if !group.is_empty() => format!("{} · {group} · JASE\n", g.name),
            _ => format!("{} · JASE\n", g.name),
        };
        for a in &addresses {
            match known.get(a) {
                // La dirección para conectar, la del juego.
                Some(i) if i.online => text += &format!("{}  {}\n", i.connect_address, i.name),
                _ => text += &format!("{a}\n"),
            }
        }
        let status = tr_n(
            "{n} favorito copiado como texto",
            "{n} favoritos copiados como texto",
            addresses.len() as i64,
            &[],
        );
        Task::batch([
            clipboard::write(text),
            self.show_message(status, Some(NOTICE)),
        ])
    }

    /// Un selector de archivos está abierto.
    pub fn files_open(&self) -> bool {
        self.files_open
    }

    /// *Exportar favoritos…*: elige el archivo en un hilo; se escribe al volver la ruta.
    pub(super) fn export_favorites(&mut self) -> Task<Message> {
        if self.files_open {
            return Task::none();
        }
        self.files_open = true;
        let files = self.deps.files.clone();
        let (title, filter) = (tr("Exportar favoritos…"), tr("Archivos JSON (*.json)"));
        Task::perform(
            blocking(move || files.save_json(&title, &filter, "jase-favorites.json")),
            |path| Message::Files(FilesMsg::ExportTo(path.ok().flatten())),
        )
    }

    /// *Importar favoritos…*: elige el archivo en un hilo; se lee en otro.
    pub(super) fn import_favorites(&mut self) -> Task<Message> {
        if self.files_open {
            return Task::none();
        }
        self.files_open = true;
        let files = self.deps.files.clone();
        let (title, filter) = (tr("Importar favoritos…"), tr("Archivos JSON (*.json)"));
        Task::perform(blocking(move || files.open_json(&title, &filter)), |path| {
            Message::Files(FilesMsg::ImportFrom(path.ok().flatten()))
        })
    }

    pub(super) fn on_files(&mut self, m: FilesMsg) -> Task<Message> {
        match m {
            FilesMsg::ExportTo(path) => {
                self.files_open = false;
                if let Some(path) = path {
                    return self.write_export(path);
                }
            }
            FilesMsg::Exported {
                path,
                result: Ok(()),
            } => {
                let text = tr_fmt(
                    "Favoritos exportados a {path}",
                    &[("path", &path.display())],
                );
                return self.show_message(text, Some(NOTICE));
            }
            FilesMsg::Exported { result: Err(e), .. } => {
                self.warn(tr("Exportar favoritos…"), e);
            }
            FilesMsg::ImportFrom(path) => {
                self.files_open = false;
                if let Some(path) = path {
                    return Task::perform(
                        blocking(move || read_import_file(&path).map_err(ImportFailure::from)),
                        |r| {
                            let result = r.unwrap_or_else(|e| Err(ImportFailure::Read(e)));
                            Message::Files(FilesMsg::Imported { result })
                        },
                    );
                }
            }
            FilesMsg::Imported { result } => return self.on_imported(result),
        }
        Task::none()
    }

    /// La config de este momento (no la de cuando se abrió el selector), en un hilo al disco.
    fn write_export(&mut self, path: PathBuf) -> Task<Message> {
        let data = serde_json::to_string_pretty(&favorites_io::export_favorites(&self.cfg))
            .unwrap_or_default();
        Task::perform(
            blocking(move || {
                let result = atomic_write(&path, data.as_bytes()).map_err(|e| e.to_string());
                (path, result)
            }),
            |r| {
                let (path, result) = r.unwrap_or_else(|e| (PathBuf::new(), Err(e)));
                Message::Files(FilesMsg::Exported { path, result })
            },
        )
    }

    /// Fusiona lo leído en los favoritos (memoria: va en `update`) o avisa por qué no.
    fn on_imported(&mut self, result: Result<Value, ImportFailure>) -> Task<Message> {
        let merged = result.and_then(|data| {
            favorites_io::import_favorites(&mut self.cfg, &data).map_err(ImportFailure::from)
        });
        let added = match merged {
            Ok(added) => added,
            Err(failure) => {
                let text = match failure {
                    ImportFailure::Read(e) => {
                        tr_fmt("No se pudo leer el archivo: {error}", &[("error", &e)])
                    }
                    ImportFailure::TooLarge => {
                        tr("El archivo es demasiado grande para ser una exportación de favoritos")
                    }
                    ImportFailure::Invalid => {
                        tr("El archivo no es una exportación válida de favoritos de JASE")
                    }
                };
                self.warn(tr("Importar favoritos…"), text);
                return Task::none();
            }
        };
        self.save_config();
        self.sync_notes();
        self.update_sidebar();
        let mut tasks = vec![self.sync_groups()];
        if self.direct_view() == Some(DirectView::Favorites) {
            tasks.push(self.query_direct_list());
        }
        let text = tr_n(
            "{n} favorito importado, {skipped} descartados",
            "{n} favoritos importados, {skipped} descartados",
            added.added as i64,
            &[("skipped", &added.skipped)],
        );
        tasks.push(self.show_message(text, Some(IMPORTED)));
        Task::batch(tasks)
    }

    /// `QMessageBox.warning`: título y texto, *Cerrar*.
    /// Con Ajustes o la descarga abiertos (el selector no es modal) va a la barra de estado:
    /// reemplazarlos descartaría lo editado o cancelaría la descarga.
    fn warn(&mut self, title: String, text: String) {
        if matches!(
            self.dialog.as_ref().map(|(d, _)| d),
            Some(Dialog::Settings | Dialog::Download { .. })
        ) {
            self.set_status(format!("{title}: {text}"));
            return;
        }
        self.open_dialog(Dialog::Message {
            title,
            text,
            informative: String::new(),
            settings: false,
        });
    }
}
