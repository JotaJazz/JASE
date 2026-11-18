//! Diálogos modales dibujados sobre la ventana y los filtros guardados (`window_presets.py`).

use iced::Task;
use iced::widget::operation;
use jase_core::filter_presets::{FilterPreset, remove, upsert};
use jase_core::pystr::{casefold, py_strip};

use super::{App, Focus, Message};
use crate::presets::{self, PresetItem};
use crate::widgets::FILTER_ID;

/// El modal abierto (5f–5h agregan los suyos); `App.dialog` guarda además el foco de antes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    /// «Guardar filtros actuales…» con el nombre escrito.
    SavePreset(String),
}

impl App {
    /// Esc con un diálogo abierto: lo cancela según cuál sea.
    pub(super) fn cancel_dialog(&mut self) -> Task<Message> {
        match self.dialog {
            Some((Dialog::SavePreset(_), _)) => self.on_presets(presets::Msg::Cancel),
            None => Task::none(),
        }
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
                self.edit_filter(|f| {
                    f.quick = quick;
                    f.set_text(&text);
                });
                self.filter_text = text;
                self.save_config();
            }
            P::Pick(PresetItem::Save) => {
                self.dialog = Some((Dialog::SavePreset(String::new()), self.focus));
                self.set_focus(Focus::Dialog);
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
    fn close_dialog(&mut self) -> Task<Message> {
        let Some((_, previous)) = self.dialog.take() else {
            return Task::none();
        };
        self.set_focus(previous);
        if previous == Focus::Filter {
            return operation::focus(FILTER_ID);
        }
        Task::none()
    }

    pub fn dialog(&self) -> Option<&Dialog> {
        self.dialog.as_ref().map(|(d, _)| d)
    }

    /// El nombre escrito en el modal «Guardar filtros actuales…», si está abierto.
    pub fn preset_name(&self) -> Option<&str> {
        match &self.dialog {
            Some((Dialog::SavePreset(n), _)) => Some(n),
            None => None,
        }
    }

    pub fn filter_presets(&self) -> &[FilterPreset] {
        &self.cfg.filter_presets
    }
}
