//! Pestaña «Masters» de Ajustes: la copia de los masters personales, sus acciones (sin E/S)
//! y su vista (como `ui/masters_tab.py`).

use std::collections::HashMap;

use iced::widget::scrollable::Direction;
use iced::widget::text::Text;
use iced::widget::{Column, Space, column, container, mouse_area, row, scrollable, text};
use iced::{Center, Element, Fill, Length};

use jase_core::games::GAMES;
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::masters::{
    CustomMasters, Master, OfficialList, master_key, official_for, valid_host,
};
use jase_core::pystr::py_strip;

use crate::app::dialogs::card;
use crate::app::{MastersMsg, Message, PROMPT_ID, SettingsMsg};
use crate::settings::{Choice, choice_combo};
use crate::theme::{WidgetStyle, framed, palette, style, widget_style};
use crate::widgets;

/// «host:puerto» → (host, puerto) con la misma validación que la lista oficial.
pub fn parse_master_text(text: &str) -> Option<Master> {
    let (host, port) = py_strip(text).rsplit_once(':')?;
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port = u16::try_from(port.parse::<u32>().ok()?)
        .ok()
        .filter(|&p| p >= 1)?;
    valid_host(host).then(|| (host.to_owned(), port))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddResult {
    Ok,
    Invalid,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Updated,
    Unchanged,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterRow {
    pub master: Master,
    pub official: bool,
    /// Los personales siempre están activos: se quitan, no se desactivan.
    pub active: bool,
    pub probe: String,
}

#[derive(Debug, Clone)]
pub struct MastersTab {
    pub official: OfficialList,
    pub custom: CustomMasters,
    pub game: &'static str,
    pub selected: Option<usize>,
    /// Clave del master → texto de la columna «Prueba».
    probe_text: HashMap<String, String>,
    /// (número, juego, clave) de la prueba en curso.
    probing: Option<(u64, &'static str, String)>,
    serial: u64,
    has_probe: bool,
    has_check: bool,
    /// Texto del prompt de «Agregar master» abierto.
    pub adding: Option<String>,
    /// Aviso de «Agregar master» abierto.
    pub warning: Option<String>,
    pub update_status: String,
}

impl MastersTab {
    pub fn new(
        official: OfficialList,
        custom: CustomMasters,
        has_probe: bool,
        has_check: bool,
    ) -> MastersTab {
        MastersTab {
            official,
            custom,
            game: GAMES[0].id,
            selected: None,
            probe_text: HashMap::new(),
            probing: None,
            serial: 0,
            has_probe,
            has_check,
            adding: None,
            warning: None,
            update_status: String::new(),
        }
    }

    pub fn rows(&self) -> Vec<MasterRow> {
        let disabled = self.custom.disabled.get(self.game);
        let row = |master: Master, official: bool| {
            let key = master_key(&master);
            MasterRow {
                active: !official || !disabled.is_some_and(|d| d.contains(&key)),
                probe: self.probe_text.get(&key).cloned().unwrap_or_default(),
                master,
                official,
            }
        };
        let mut rows: Vec<MasterRow> = official_for(self.game, &self.official)
            .into_iter()
            .map(|m| row(m, true))
            .collect();
        rows.extend(
            self.custom
                .added
                .get(self.game)
                .into_iter()
                .flatten()
                .map(|m| row(m.clone(), false)),
        );
        rows
    }

    fn row(&self, row: usize) -> Option<MasterRow> {
        self.rows().into_iter().nth(row)
    }

    fn selected_row(&self) -> Option<MasterRow> {
        self.selected.and_then(|r| self.row(r))
    }

    /// El resultado en vuelo se descartará y su «Probando…» se borra.
    fn cancel_probe(&mut self) {
        if let Some((_, _, key)) = self.probing.take() {
            self.probe_text.remove(&key);
        }
    }

    /// Como `_reload`: las filas cambian y sus pruebas se olvidan.
    fn reload(&mut self) {
        self.cancel_probe();
        self.probe_text.clear();
        if self.selected.is_some_and(|r| r >= self.rows().len()) {
            self.selected = None;
        }
    }

    pub fn select_game(&mut self, id: &'static str) {
        if id != self.game {
            self.game = id;
            self.selected = None;
            self.reload();
        }
    }

    pub fn select(&mut self, row: usize) {
        if self.selected != Some(row) && row < self.rows().len() {
            self.selected = Some(row);
            self.cancel_probe();
        }
    }

    /// Solo las oficiales tienen casilla.
    pub fn toggle(&mut self, row: usize) {
        let Some(info) = self.row(row).filter(|r| r.official) else {
            return;
        };
        let key = master_key(&info.master);
        let keys = self
            .custom
            .disabled
            .entry(self.game.to_owned())
            .or_default();
        if info.active {
            keys.insert(key);
        } else {
            keys.remove(&key);
        }
        if keys.is_empty() {
            self.custom.disabled.shift_remove(self.game);
        }
    }

    pub fn add(&mut self, text: &str) -> AddResult {
        let Some(parsed) = parse_master_text(text) else {
            return AddResult::Invalid;
        };
        let key = master_key(&parsed);
        if self.rows().iter().any(|r| master_key(&r.master) == key) {
            return AddResult::Duplicate;
        }
        self.custom
            .added
            .entry(self.game.to_owned())
            .or_default()
            .push(parsed);
        self.reload();
        self.selected = Some(self.rows().len() - 1);
        AddResult::Ok
    }

    pub fn remove_selected(&mut self) {
        let Some(info) = self.selected_row().filter(|r| !r.official) else {
            return;
        };
        let key = master_key(&info.master);
        if let Some(list) = self.custom.added.get_mut(self.game) {
            list.retain(|m| master_key(m) != key);
            if list.is_empty() {
                self.custom.added.shift_remove(self.game);
            }
        }
        self.selected = None;
        self.reload();
    }

    pub fn restore_officials(&mut self) {
        self.custom.disabled.shift_remove(self.game);
        self.reload();
    }

    pub fn can_remove(&self) -> bool {
        self.selected_row().is_some_and(|r| !r.official)
    }

    pub fn can_probe(&self) -> bool {
        self.has_probe && self.probing.is_none() && self.selected_row().is_some()
    }

    pub fn has_check(&self) -> bool {
        self.has_check
    }

    /// Marca la fila elegida como «Probando…» y da lo que hay que probar.
    pub fn start_probe(&mut self) -> Option<(u64, &'static str, Master)> {
        if !self.can_probe() {
            return None;
        }
        let master = self.selected_row()?.master;
        let key = master_key(&master);
        self.serial += 1;
        self.probe_text.insert(key.clone(), tr("Probando…"));
        self.probing = Some((self.serial, self.game, key));
        Some((self.serial, self.game, master))
    }

    /// `true` si el resultado se aplicó; `false` si la prueba ya no era la vigente.
    pub fn on_probe(
        &mut self,
        serial: u64,
        game: &'static str,
        master: &Master,
        count: Option<usize>,
    ) -> bool {
        let token = (serial, game, master_key(master));
        if self.probing.as_ref() != Some(&token) {
            return false;
        }
        self.probing = None;
        let text = match count {
            None => tr("✗ no responde"),
            Some(n) => tr_n(
                "✓ 1 servidor",
                "✓ {n} servidores",
                i64::try_from(n).unwrap_or(i64::MAX),
                &[],
            ),
        };
        self.probe_text.insert(token.2, text);
        true
    }

    /// `today` (AAAA-MM-DD) reemplaza una fecha `updated` vacía.
    pub fn show_update_result(
        &mut self,
        kind: Outcome,
        official: Option<OfficialList>,
        today: &str,
    ) {
        self.update_status = match (kind, official) {
            (Outcome::Updated, Some(official)) => {
                self.official = official;
                self.reload();
                let date = if self.official.updated.is_empty() {
                    today
                } else {
                    &self.official.updated
                };
                tr_fmt("Lista de masters actualizada ({date})", &[("date", &date)])
            }
            (Outcome::Unchanged, _) => tr("La lista de masters está al día"),
            _ => tr("No se pudo descargar la lista de masters"),
        };
    }

    pub fn official_label(&self) -> String {
        let date = match self.official.updated.as_str() {
            "" => "—",
            d => d,
        };
        tr_fmt("Lista oficial: {date}", &[("date", &date)])
    }
}

// ------------------------------------------------------------------------------- vista

const ACTIVE_WIDTH: f32 = 56.0;
const ORIGIN_WIDTH: f32 = 80.0;
const PROBE_WIDTH: f32 = 120.0;
/// Filas y cabecera de `QTableWidget` (30 y ~24 px en Qt).
const ROW_HEIGHT: f32 = 30.0;
const HEADER_HEIGHT: f32 = 24.0;

fn label<'a>(t: impl text::IntoFragment<'a>, s: WidgetStyle) -> Text<'a> {
    text(t).font(s.font()).size(s.text_size())
}

/// La pestaña: combo de juegos, la tabla de masters, sus botones y el pie de la lista oficial.
pub fn view<'a>(tab: &'a MastersTab, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let m = |x| Message::Settings(SettingsMsg::Masters(x));
    let games = GAMES
        .iter()
        .map(|g| Choice {
            label: g.name.to_owned(),
            value: g.id,
        })
        .collect();
    let combo = choice_combo(
        games,
        &tab.game,
        |id| SettingsMsg::Masters(MastersMsg::Game(id)),
        true,
        theme_id,
    );
    let cell = |content: Element<'a, Message>, width: Length| {
        container(content)
            .width(width)
            .height(Fill)
            .padding([0, 4])
            .center_y(Fill)
            .clip(true)
    };
    let (header_style, section) = style::table_header(p, s, theme_id);
    let header = row![
        cell(label(tr("Activo"), s).into(), ACTIVE_WIDTH.into()),
        cell(label(tr("Master"), s).into(), Fill),
        cell(label(tr("Origen"), s).into(), ORIGIN_WIDTH.into()),
        cell(label(tr("Prueba"), s).into(), PROBE_WIDTH.into()),
    ];
    let header = container(header)
        .height(HEADER_HEIGHT)
        .width(Fill)
        .style(move |_| header_style);
    let header: Element<'a, Message> = match section {
        Some(e) => framed(header, e),
        None => header.into(),
    };
    let body = Column::with_children(tab.rows().into_iter().enumerate().map(|(i, r)| {
        let active: Element<'a, Message> = if r.official {
            widgets::check(r.active, None, Some(m(MastersMsg::Toggle(i))), p, s)
        } else {
            // Los personales se quitan, no se desactivan.
            widgets::tip(
                Space::new().width(ACTIVE_WIDTH).height(ROW_HEIGHT),
                tr("Los masters personales se quitan con \"Quitar\""),
                p,
                s,
            )
        };
        let origin = if r.official {
            tr("Oficial")
        } else {
            tr("Personal")
        };
        let selected = tab.selected == Some(i);
        let line = row![
            cell(active, ACTIVE_WIDTH.into()),
            cell(
                label(format!("{}:{}", r.master.0, r.master.1), s).into(),
                Fill
            ),
            cell(label(origin, s).into(), ORIGIN_WIDTH.into()),
            cell(label(r.probe, s).into(), PROBE_WIDTH.into()),
        ];
        let line = container(line)
            .height(ROW_HEIGHT)
            .width(Fill)
            .style(move |_| style::table_row(p, s, selected, true));
        mouse_area(line).on_press(m(MastersMsg::Select(i))).into()
    }));
    let body = scrollable(body)
        .direction(Direction::Vertical(style::scrollbar(s)))
        .style(move |_, status| style::scrollable(p, s, status))
        .height(Fill);
    let (frame, edges) = style::table_frame(p, s);
    let table = container(column![header, body])
        .padding(frame.border.width)
        .width(Fill)
        .height(Fill)
        .style(move |_| frame);
    let table: Element<'a, Message> = match edges {
        Some(e) => framed(table, e),
        None => table.into(),
    };
    let buttons = row![
        widgets::button(tr("Agregar…"), m(MastersMsg::Add), theme_id),
        widgets::button_maybe(
            tr("Quitar"),
            tab.can_remove().then(|| m(MastersMsg::Remove)),
            theme_id
        ),
        widgets::button_maybe(
            tr("Probar"),
            tab.can_probe().then(|| m(MastersMsg::Probe)),
            theme_id
        ),
        widgets::button(tr("Restaurar oficiales"), m(MastersMsg::Restore), theme_id),
    ]
    .spacing(6);
    let footer = row![
        label(tab.official_label(), s),
        Space::new().width(Fill),
        widgets::button_maybe(
            tr("Buscar actualización"),
            tab.has_check().then(|| m(MastersMsg::CheckUpdate)),
            theme_id
        ),
    ]
    .align_y(Center);
    column![combo, table, buttons, footer]
        .push((!tab.update_status.is_empty()).then(|| label(tab.update_status.clone(), s)))
        .spacing(8)
        .into()
}

/// El diálogo interno abierto sobre Ajustes: el aviso encima del prompt de «Agregar master».
pub fn inner_view<'a>(tab: &'a MastersTab, theme_id: &'static str) -> Option<Element<'a, Message>> {
    let s = widget_style(theme_id);
    let m = |x| Message::Settings(SettingsMsg::Masters(x));
    let title = || label(tr("Agregar master"), s).font(style::header_font(s));
    let prompt = tab.adding.as_deref().map(|typed| {
        let field = widgets::field(
            PROMPT_ID,
            "",
            typed,
            move |t| m(MastersMsg::AddText(t)),
            Some(m(MastersMsg::AddSubmit)),
            true,
            theme_id,
        );
        let buttons = row![
            Space::new().width(Fill),
            widgets::button(tr("Guardar"), m(MastersMsg::AddSubmit), theme_id),
            widgets::button(tr("Cancelar"), m(MastersMsg::AddCancel), theme_id),
        ]
        .spacing(6);
        let content = column![
            title(),
            label(tr("Dirección del master (host:puerto):"), s),
            field,
            buttons
        ];
        card(content, 320.0, theme_id)
    });
    let warning = tab.warning.as_deref().map(|w| {
        let close = row![
            Space::new().width(Fill),
            widgets::button(tr("Cerrar"), m(MastersMsg::CloseWarning), theme_id),
        ];
        card(column![title(), label(w, s), close], 320.0, theme_id)
    });
    match (prompt, warning) {
        (Some(p), Some(w)) => Some(widgets::modal(p, w)),
        (p, w) => w.or(p),
    }
}
