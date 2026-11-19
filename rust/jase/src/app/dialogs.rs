//! Diálogos modales dibujados sobre la ventana, los filtros guardados (`window_presets.py`),
//! *Acerca de* (`show_about`), la contraseña del servidor (`PasswordDialog`), los avisos
//! (`QMessageBox`) y la nota y el grupo nuevo de un favorito (`QInputDialog`).

use iced::widget::{Column, Id, Space, column, container, operation, row, text};
use iced::{Element, Fill, Task};
use jase_core::filter_presets::{FilterPreset, remove, upsert};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::models::ServerAddress;
use jase_core::pystr::{casefold, py_strip};

use super::{App, BackgroundMsg, Focus, LinkMsg, Message, SettingsMsg, UpdateChoice};
use crate::add_server::{self, Msg as AddMsg};
use crate::presets::{self, PresetItem};
use crate::startup::APP_NAME;
use crate::theme::{framed, palette, style, widget_style};
use crate::widgets::{self, FILTER_ID};

/// El modal abierto (5f–5h agregan los suyos); `App.dialog` guarda además el foco de antes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    /// «Guardar filtros actuales…» con el nombre escrito.
    SavePreset(String),
    /// *Acerca de* (`QMessageBox.about`).
    About,
    /// La contraseña del servidor `name` (remoto: texto plano); `saved` = había una guardada
    /// (muestra *Olvidar contraseña*).
    Password {
        name: String,
        typed: String,
        saved: bool,
    },
    /// Un aviso (`QMessageBox`); con `settings`, *Abrir ajustes* y *Cancelar* en vez de
    /// *Cerrar* (`_message_with_settings`).
    Message {
        title: String,
        text: String,
        informative: String,
        settings: bool,
    },
    /// *Editar nota…* del favorito `address` de `game`, con el texto escrito.
    Note {
        game: &'static str,
        address: ServerAddress,
        text: String,
    },
    /// «Nuevo grupo…» con el nombre escrito.
    NewGroup(String),
    /// «Agregar servidor» (su formulario vive en `App`: `Content` no es `Clone` ni `Eq`).
    AddServer,
    /// La confirmación de un enlace `jase://`, ya en texto plano.
    Link { text: String },
    /// Ajustes (su formulario vive en `App.settings`).
    Settings,
    /// «El idioma cambiará al reiniciar JASE.»
    LanguageRestart,
    /// *Buscar juegos…* del menú (la búsqueda vive en `App.find`).
    FindGames,
    /// El ofrecimiento del primer arranque.
    OfferFindGames,
    /// «Nueva versión {version} disponible»: instalar, ver la página o cancelar.
    UpdateAsk { version: String },
    /// La descarga de la actualización (vive en `App.updates`).
    Download { name: String, percent: u8 },
    /// La actualización falló (texto ya traducido), con *Ver la página*.
    UpdateFailed { text: String },
}

/// Lo que llega de los diálogos de texto y avisos; obra según el diálogo abierto.
#[derive(Debug, Clone)]
pub enum PromptMsg {
    Input(String),
    /// Enter en el campo o el botón que acepta.
    Submit,
    Forget,
    /// *Cancelar*, *Cerrar* o Esc.
    Cancel,
}

/// El campo de texto del diálogo abierto (toma el foco al abrir).
pub const PROMPT_ID: Id = Id::new("prompt-field");

/// Título, nombre y versión de *Acerca de*.
pub fn about_lines() -> [String; 3] {
    [
        tr_fmt("Acerca de {name}", &[("name", &APP_NAME)]),
        APP_NAME.to_owned(),
        tr_fmt(
            "Versión {version}",
            &[("version", &env!("CARGO_PKG_VERSION"))],
        ),
    ]
}

impl App {
    /// Abre el modal (el foco de antes vuelve al cerrarlo). La lista de un combo no sobrevive
    /// a un modal que abre sin clic: se da por cerrada.
    pub(super) fn open_dialog(&mut self, dialog: Dialog) {
        // Uno reemplaza a otro (un aviso que llega con un diálogo abierto): vuelve el foco de
        // antes del primero.
        let replaced = self.dialog.take();
        match replaced {
            Some((Dialog::Link { .. }, _)) => self.drop_link_confirm(),
            Some((Dialog::AddServer, _)) => self.drop_add_form(),
            Some((Dialog::Settings, _)) => self.drop_settings(),
            Some((Dialog::FindGames, _)) => {
                self.drop_find();
            }
            // Sin su diálogo no se podría cancelar ni ver el resultado.
            Some((Dialog::Download { .. }, _)) => self.stop_download(),
            // Sin respuesta: se vuelve a ofrecer en el próximo arranque.
            Some((Dialog::OfferFindGames, _)) => {
                self.cfg.find_games_offered = false;
                self.save_config();
            }
            _ => {}
        }
        let previous = replaced.map_or(self.focus, |(_, f)| f);
        self.dialog = Some((dialog, previous));
        self.set_focus(Focus::Dialog);
        self.combo_open = None;
        self.menu = None; // uno que abre sin clic (llavero, lanzamiento) no queda bajo el menú
    }

    /// Esc con un diálogo abierto: lo cancela según cuál sea.
    pub(super) fn cancel_dialog(&mut self) -> Task<Message> {
        match self.dialog {
            Some((Dialog::SavePreset(_), _)) => self.on_presets(presets::Msg::Cancel),
            Some((Dialog::About | Dialog::LanguageRestart | Dialog::UpdateFailed { .. }, _)) => {
                self.close_dialog()
            }
            Some((Dialog::UpdateAsk { .. }, _)) => {
                self.on_background(BackgroundMsg::UpdateChoice(UpdateChoice::Cancel))
            }
            Some((Dialog::Download { .. }, _)) => self.on_background(BackgroundMsg::CancelDownload),
            Some((
                Dialog::Password { .. }
                | Dialog::Message { .. }
                | Dialog::Note { .. }
                | Dialog::NewGroup(_),
                _,
            )) => self.on_prompt(PromptMsg::Cancel),
            Some((Dialog::AddServer, _)) => self.on_add_server(AddMsg::Cancel),
            Some((Dialog::Link { .. }, _)) => self.on_link(LinkMsg::Confirm(false)),
            Some((Dialog::Settings, _)) => self.cancel_settings(),
            Some((Dialog::FindGames, _)) => self.on_settings(SettingsMsg::FindClose),
            Some((Dialog::OfferFindGames, _)) => self.on_settings(SettingsMsg::OfferDecline),
            None => Task::none(),
        }
    }

    pub(super) fn on_prompt(&mut self, m: PromptMsg) -> Task<Message> {
        match (&mut self.dialog, m) {
            (Some((Dialog::Password { typed, .. }, _)), PromptMsg::Input(t)) => *typed = t,
            (Some((Dialog::Password { .. }, _)), m) => return self.on_password(m),
            // Enter = *Abrir ajustes*, el botón de aceptar del aviso; si no, *Cerrar*.
            (Some((Dialog::Message { settings: true, .. }, _)), PromptMsg::Submit) => {
                return self.open_settings();
            }
            (Some((Dialog::Message { .. }, _)), PromptMsg::Submit | PromptMsg::Cancel) => {
                return self.close_dialog();
            }
            (
                Some((Dialog::Note { text, .. } | Dialog::NewGroup(text), _)),
                PromptMsg::Input(t),
            ) => *text = t,
            (Some((Dialog::Note { .. } | Dialog::NewGroup(_), _)), PromptMsg::Submit) => {
                let Some((dialog, _)) = self.dialog.clone() else {
                    return Task::none();
                };
                let close = self.close_dialog();
                return Task::batch([close, self.submit_text_dialog(dialog)]);
            }
            (Some((Dialog::Note { .. } | Dialog::NewGroup(_), _)), PromptMsg::Cancel) => {
                return self.close_dialog();
            }
            // Enter con el foco fuera del campo: el botón por defecto (*Cerrar*, *Guardar*,
            // *Agregar*).
            (Some((Dialog::About, _)), PromptMsg::Submit) => return self.close_dialog(),
            (Some((Dialog::LanguageRestart, _)), PromptMsg::Submit | PromptMsg::Cancel) => {
                return self.close_dialog();
            }
            (Some((Dialog::Settings, _)), PromptMsg::Submit) => return self.submit_settings(),
            (Some((Dialog::Settings, _)), PromptMsg::Cancel) => return self.cancel_settings(),
            (Some((Dialog::FindGames, _)), PromptMsg::Submit) => {
                return self.on_settings(SettingsMsg::FindApply);
            }
            (Some((Dialog::FindGames, _)), PromptMsg::Cancel) => {
                return self.on_settings(SettingsMsg::FindClose);
            }
            // Enter = *Buscar juegos*, el `AcceptRole` de Qt.
            (Some((Dialog::OfferFindGames, _)), PromptMsg::Submit) => {
                return self.on_settings(SettingsMsg::OfferAccept);
            }
            (Some((Dialog::OfferFindGames, _)), PromptMsg::Cancel) => {
                return self.on_settings(SettingsMsg::OfferDecline);
            }
            // Enter = el `AcceptRole`: *Descargar e instalar*, *Ver la página*.
            (Some((Dialog::UpdateAsk { .. }, _)), PromptMsg::Submit) => {
                return self.on_background(BackgroundMsg::UpdateChoice(UpdateChoice::Install));
            }
            (Some((Dialog::UpdateFailed { .. }, _)), PromptMsg::Submit) => {
                return self.on_background(BackgroundMsg::OpenPage);
            }
            (Some((Dialog::UpdateAsk { .. } | Dialog::Download { .. }, _)), PromptMsg::Cancel)
            | (Some((Dialog::UpdateFailed { .. }, _)), PromptMsg::Cancel) => {
                return self.cancel_dialog();
            }
            (Some((Dialog::AddServer, _)), PromptMsg::Submit) => {
                return self.on_add_server(AddMsg::Submit);
            }
            (Some((Dialog::AddServer, _)), PromptMsg::Cancel) => {
                return self.on_add_server(AddMsg::Cancel);
            }
            // Enter = *Cancelar*, el botón por defecto de `confirm_plain`.
            (Some((Dialog::Link { .. }, _)), PromptMsg::Submit | PromptMsg::Cancel) => {
                return self.on_link(LinkMsg::Confirm(false));
            }
            (Some((Dialog::SavePreset(name), _)), PromptMsg::Submit)
                if !py_strip(name).is_empty() =>
            {
                return self.on_presets(presets::Msg::Submit);
            }
            _ => {}
        }
        Task::none()
    }

    /// Filtros guardados; cada cambio se guarda en la config.
    pub(super) fn on_presets(&mut self, m: presets::Msg) -> Task<Message> {
        use presets::Msg as P;
        match m {
            P::Pick(PresetItem::Apply(name)) => {
                let Some(p) = self.cfg.filter_presets.iter().find(|p| p.name == name) else {
                    return Task::none();
                };
                // `_set_quick_filter_widgets` + `_on_quick_filters_changed` + el texto.
                let (quick, text) = (p.quick.clone(), p.text.clone());
                self.cfg.quick_filters = quick.clone();
                // En Favoritos/Recientes los rápidos quedan para la lista normal.
                let bypassed = self.direct_view().is_some();
                self.edit_filter(|f| {
                    if !bypassed {
                        f.quick = quick;
                    }
                    f.set_text(&text);
                });
                self.filter_text = text;
                self.save_config();
            }
            P::Pick(PresetItem::Save) => {
                self.open_dialog(Dialog::SavePreset(String::new()));
                return operation::focus(presets::NAME_ID);
            }
            P::Pick(PresetItem::Delete(name)) => {
                self.cfg.filter_presets = remove(&self.cfg.filter_presets, &name);
                self.save_config();
            }
            P::NameChanged(t) => {
                if let Some((Dialog::SavePreset(name), _)) = &mut self.dialog {
                    *name = t;
                }
            }
            P::Submit => {
                let Some((Dialog::SavePreset(name), _)) = &self.dialog else {
                    return Task::none();
                };
                let name = py_strip(name).to_owned();
                let task = self.close_dialog();
                if name.is_empty() {
                    return task;
                }
                let preset = FilterPreset {
                    name: name.clone(),
                    quick: self.cfg.quick_filters.clone(),
                    text: self.filter_text.clone(),
                };
                let presets = upsert(&self.cfg.filter_presets, preset);
                let key = casefold(&name);
                if presets.iter().any(|p| casefold(&p.name) == key) {
                    self.cfg.filter_presets = presets;
                    self.save_config();
                } // si no, tope de combinaciones alcanzado: nada cambia
                return task;
            }
            P::Cancel => return self.close_dialog(),
        }
        Task::none()
    }

    /// Cierra el modal y devuelve el foco a quien lo tenía (al campo, también en el widget).
    pub(super) fn close_dialog(&mut self) -> Task<Message> {
        let Some((_, previous)) = self.dialog.take() else {
            return Task::none();
        };
        self.set_focus(previous);
        // Un enlace que quedó esperando sin reintento (su confirmación fue reemplazada).
        let link = self.resume_pending_link();
        if previous == Focus::Filter {
            return Task::batch([operation::focus(FILTER_ID), link]);
        }
        link
    }

    pub fn dialog(&self) -> Option<&Dialog> {
        self.dialog.as_ref().map(|(d, _)| d)
    }

    /// El nombre escrito en el modal «Guardar filtros actuales…», si está abierto.
    pub fn preset_name(&self) -> Option<&str> {
        match &self.dialog {
            Some((Dialog::SavePreset(n), _)) => Some(n),
            _ => None,
        }
    }

    pub fn filter_presets(&self) -> &[FilterPreset] {
        &self.cfg.filter_presets
    }
}

/// La tarjeta de un diálogo: fondo y marco del estilo, como `QDialog`.
pub(crate) fn card<'a>(
    content: Column<'a, Message>,
    width: f32,
    theme_id: &'static str,
) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let card = container(content.spacing(8))
        .padding(12)
        .width(width)
        .style(move |_| style::dialog(p, s).0);
    match style::dialog(p, s).1 {
        Some(e) => framed(card, e),
        None => card.into(),
    }
}

impl App {
    /// La tarjeta del diálogo abierto, si hay.
    pub(super) fn dialog_view(&self) -> Option<Element<'_, Message>> {
        Some(match &self.dialog.as_ref()?.0 {
            Dialog::SavePreset(name) => presets::dialog(self, name),
            Dialog::About => self.about_view(),
            Dialog::Password { name, typed, saved } => self.password_view(name, typed, *saved),
            Dialog::Message {
                title,
                text,
                informative,
                settings,
            } => self.message_view(title, text, informative, *settings),
            Dialog::Note { address, text, .. } => self.text_view(
                tr("Editar nota…"),
                tr_fmt("Nota para {address}", &[("address", address)]),
                text,
            ),
            Dialog::NewGroup(name) => {
                self.text_view(tr("Nuevo grupo…"), tr("Nombre del grupo:"), name)
            }
            Dialog::AddServer => add_server::view(self.add_form()?, self.theme_id),
            Dialog::Link { text } => self.link_view(text),
            Dialog::Settings => {
                let form = self.settings.as_ref()?;
                let find = self.find_search().filter(|s| s.from_settings);
                crate::settings::view(form, find, !self.files_open, self.theme_id)
            }
            Dialog::LanguageRestart => self.restart_view(),
            Dialog::FindGames => {
                crate::find_games::view(self.find_search()?, !self.files_open, self.theme_id)
            }
            Dialog::OfferFindGames => self.offer_view(),
            Dialog::UpdateAsk { version } => self.update_ask_view(version),
            Dialog::Download { name, percent } => self.download_view(name, *percent),
            Dialog::UpdateFailed { text } => self.update_failed_view(text),
        })
    }

    /// La tarjeta de *Acerca de*: título, nombre, versión y «Cerrar» (Qt pone su «OK», que no
    /// está en los catálogos).
    pub(super) fn about_view(&self) -> Element<'_, Message> {
        let s = widget_style(self.theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let [title, name, version] = about_lines();
        let close = row![
            Space::new().width(Fill),
            widgets::button(tr("Cerrar"), Message::CloseDialog, self.theme_id),
        ];
        let content = column![
            label(title).font(style::header_font(s)),
            label(name),
            label(version),
            close
        ];
        card(content, 320.0, self.theme_id)
    }

    /// `PasswordDialog`: el nombre remoto como texto plano, campo oculto y *Olvidar contraseña*
    /// (solo si había una guardada) entre *Conectar* y *Cancelar*, como el `QDialogButtonBox`
    /// de Windows (rol destructivo tras el de aceptar).
    pub(super) fn password_view<'a>(
        &self,
        name: &str,
        typed: &'a str,
        saved: bool,
    ) -> Element<'a, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let prompt = Message::Prompt;
        let field = widgets::password_field(
            PROMPT_ID,
            &tr("Contraseña"),
            typed,
            move |t| prompt(PromptMsg::Input(t)),
            Some(prompt(PromptMsg::Submit)),
            theme_id,
        );
        let forget = saved.then(|| {
            widgets::button(
                tr("Olvidar contraseña"),
                prompt(PromptMsg::Forget),
                theme_id,
            )
        });
        let buttons = row![
            Space::new().width(Fill),
            widgets::button(tr("Conectar"), prompt(PromptMsg::Submit), theme_id),
            forget,
            widgets::button(tr("Cancelar"), prompt(PromptMsg::Cancel), theme_id),
        ]
        .spacing(6);
        let content = column![
            label(tr("Contraseña del servidor")).font(style::header_font(s)),
            label(tr_fmt(
                "El servidor «{name}» requiere contraseña:",
                &[("name", &name)]
            )),
            field,
            buttons,
        ];
        card(content, 360.0, theme_id)
    }

    /// Un aviso: título, texto, texto informativo y sus botones (*Abrir ajustes*, que lo
    /// reemplaza por Ajustes, y *Cancelar*; o *Cerrar*).
    pub(super) fn message_view(
        &self,
        title: &str,
        body: &str,
        informative: &str,
        settings: bool,
    ) -> Element<'static, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: &str| text(t.to_owned()).font(s.font()).size(s.text_size());
        let cancel = Message::Prompt(PromptMsg::Cancel);
        let buttons = if settings {
            row![
                Space::new().width(Fill),
                widgets::button(
                    tr("Abrir ajustes"),
                    Message::Settings(super::SettingsMsg::Open),
                    theme_id,
                ),
                widgets::button(tr("Cancelar"), cancel, theme_id),
            ]
        } else {
            row![
                Space::new().width(Fill),
                widgets::button(tr("Cerrar"), cancel, theme_id),
            ]
        };
        let content = column![label(title).font(style::header_font(s)), label(body)]
            .push((!informative.is_empty()).then(|| label(informative)))
            .push(buttons.spacing(6));
        card(content, 360.0, theme_id)
    }

    /// `QInputDialog.getText`: título, etiqueta, campo y *Guardar*/*Cancelar* (como el de los
    /// filtros guardados: no hay `OK` en los catálogos).
    fn text_view<'a>(&self, title: String, prompt: String, value: &'a str) -> Element<'a, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let msg = Message::Prompt;
        let field = widgets::field(
            PROMPT_ID,
            "",
            value,
            move |t| msg(PromptMsg::Input(t)),
            Some(msg(PromptMsg::Submit)),
            true, // el foco es suyo desde que abre el modal
            theme_id,
        );
        let buttons = row![
            Space::new().width(Fill),
            widgets::button(tr("Guardar"), msg(PromptMsg::Submit), theme_id),
            widgets::button(tr("Cancelar"), msg(PromptMsg::Cancel), theme_id),
        ]
        .spacing(6);
        let content = column![
            label(title).font(style::header_font(s)),
            label(prompt),
            field,
            buttons
        ];
        card(content, 320.0, theme_id)
    }
}
