//! Pestaña «Masters» de Ajustes sin vista: la copia de los masters personales y sus acciones
//! (como `ui/masters_tab.py`).

use std::collections::HashMap;

use jase_core::games::GAMES;
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::masters::{
    CustomMasters, Master, OfficialList, master_key, official_for, valid_host,
};
use jase_core::pystr::py_strip;

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
