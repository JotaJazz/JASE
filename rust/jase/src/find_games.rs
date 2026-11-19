//! «Buscar juegos» (`ui/find_games.py`): las filas de lo hallado, sin E/S, y su vista; la
//! búsqueda corre en un hilo desde `app/settings.rs`.

use std::collections::HashMap;

use iced::widget::scrollable::Direction;
use iced::widget::{Column, Space, column, row, scrollable, text};
use iced::{Center, Element, Fill, Padding};
use indexmap::IndexMap;
use jase_core::detect::Finding;
use jase_core::games::{GAMES, game};
use jase_core::i18n::{tr, tr_n};

use crate::app::dialogs::card;
use crate::app::{Message, SettingsMsg};
use crate::settings::{Choice, choice_combo};
use crate::theme::{palette, style, widget_style};
use crate::widgets;

/// Cómo está el juego hallado en la config (`detect.is_configured`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    /// Su ruta guardada tiene el exe.
    Configured,
    /// Tiene ruta guardada, sin el exe.
    SavedMissing,
    New,
}

impl RowState {
    pub fn text(self) -> String {
        match self {
            RowState::Configured => tr("ya configurado"),
            RowState::SavedMissing => tr("ruta guardada no encontrada"),
            RowState::New => tr("nuevo"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FoundRow {
    pub game: &'static str,
    /// Ordenados por origen (`detect.SOURCES`); nunca vacío.
    pub findings: Vec<Finding>,
    pub chosen: usize,
    pub checked: bool,
    pub state: RowState,
}

/// Una búsqueda abierta: su número (la respuesta de otro se descarta), de dónde se abrió y
/// sus filas.
#[derive(Debug, Clone)]
pub struct Search {
    pub request: u64,
    /// Abierta desde Ajustes: *Aplicar* llena sus campos en vez de guardar.
    pub from_settings: bool,
    pub status: String,
    /// La búsqueda sigue en curso (*Cancelar búsqueda* encendido).
    pub searching: bool,
    pub rows: Vec<FoundRow>,
}

impl Search {
    pub fn new(request: u64, from_settings: bool) -> Search {
        Search {
            request,
            from_settings,
            status: tr("Buscando…"),
            searching: true,
            rows: Vec::new(),
        }
    }

    /// `_on_found`: una fila por juego hallado en el orden de `GAMES`, marcada salvo si ya
    /// está configurado.
    pub fn on_found(
        &mut self,
        mut found: IndexMap<String, Vec<Finding>>,
        states: HashMap<String, RowState>,
    ) {
        self.searching = false;
        self.rows = GAMES
            .iter()
            .filter_map(|g| {
                let findings = found.shift_remove(g.id).filter(|f| !f.is_empty())?;
                let state = states.get(g.id).copied().unwrap_or(RowState::New);
                Some(FoundRow {
                    game: g.id,
                    findings,
                    chosen: 0,
                    checked: state != RowState::Configured,
                    state,
                })
            })
            .collect();
        let n = self.rows.len();
        self.status = if n > 0 {
            tr_n(
                "1 juego encontrado",
                "{n} juegos encontrados",
                n as i64,
                &[],
            )
        } else {
            tr("No se encontraron juegos")
        };
    }

    /// *Cancelar búsqueda*: lo que llegue después ya no vale.
    pub fn cancel(&mut self) {
        self.searching = false;
        self.status = tr("Búsqueda cancelada");
    }

    pub fn row_mut(&mut self, game: &str) -> Option<&mut FoundRow> {
        self.rows.iter_mut().find(|r| r.game == game)
    }

    /// Juego → ruta elegida de las filas marcadas.
    pub fn selected_paths(&self) -> IndexMap<String, String> {
        self.rows
            .iter()
            .filter(|r| r.checked)
            .filter_map(|r| Some((r.game.to_owned(), r.findings.get(r.chosen)?.path.clone())))
            .collect()
    }
}

/// El origen para mostrar (`detect.SOURCES` conserva el español para ordenar).
pub fn source_label(source: &str) -> String {
    match source {
        "registro" => tr("registro"),
        "steam" => tr("steam"),
        "carpeta típica" => tr("carpeta típica"),
        "carpeta elegida" => tr("carpeta elegida"),
        other => other.to_owned(),
    }
}

/// La opción del combo de un hallazgo.
pub fn option_label(f: &Finding) -> String {
    format!("{} ({})", f.path, source_label(f.source))
}

/// La tarjeta (640 × 420 como mínimo): estado y botones de búsqueda, la rejilla desplazable y
/// *Aplicar*/*Cancelar*. `browse`: *Buscar en carpeta…* encendido (no hay otro selector).
pub fn view<'a>(search: &'a Search, browse: bool, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let label = |t: String| text(t).font(s.font()).size(s.text_size());
    let msg = Message::Settings;
    let top = row![
        label(search.status.clone()).width(Fill),
        widgets::button_maybe(
            tr("Cancelar búsqueda"),
            search.searching.then(|| msg(SettingsMsg::FindCancelSearch)),
            theme_id,
        ),
        widgets::button_maybe(
            tr("Buscar en carpeta…"),
            browse.then(|| msg(SettingsMsg::FindInFolder)),
            theme_id,
        ),
    ]
    .spacing(6)
    .align_y(Center);
    let rows = Column::with_children(search.rows.iter().map(|r| {
        let id = r.game;
        let check = widgets::check(
            r.checked,
            None,
            Some(msg(SettingsMsg::FindCheck(id, !r.checked))),
            p,
            s,
        );
        let choices = (0..r.findings.len())
            .map(|i| Choice {
                label: option_label(&r.findings[i]),
                value: i,
            })
            .collect();
        let combo = choice_combo(
            choices,
            &r.chosen,
            move |i| SettingsMsg::FindChoose(id, i),
            true,
            theme_id,
        );
        let name = game(id).map_or(id, |g| g.name);
        row![
            check,
            label(name.to_owned()).width(150),
            combo,
            label(r.state.text())
        ]
        .spacing(8)
        .align_y(Center)
        .into()
    }))
    .spacing(6)
    .padding(Padding {
        right: style::scrollbar_width(s) + 4.0,
        ..Padding::ZERO
    });
    let scroll = scrollable(rows)
        .direction(Direction::Vertical(style::scrollbar(s)))
        .style(move |_, status| style::scrollable(p, s, status))
        .width(Fill)
        .height(310);
    let buttons = row![
        Space::new().width(Fill),
        widgets::button(tr("Aplicar"), msg(SettingsMsg::FindApply), theme_id),
        widgets::button(tr("Cancelar"), msg(SettingsMsg::FindClose), theme_id),
    ]
    .spacing(6);
    let content = column![
        label(tr("Buscar juegos")).font(style::header_font(s)),
        top,
        scroll,
        buttons
    ];
    card(content, 640.0, theme_id)
}
