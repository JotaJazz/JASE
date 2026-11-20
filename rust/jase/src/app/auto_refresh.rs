//! Refresco automático de la lista (`window_auto_refresh.py`) y la selección pendiente: la
//! fila que se vuelve a elegir cuando llega su dirección.

use std::time::Duration;

use iced::{Subscription, Task, time, window};
use jase_core::games::GameDef;
use jase_core::masters::effective_masters;
use jase_core::models::ServerAddress;

use super::{App, Message};
use crate::table;

#[derive(Debug, Clone)]
pub enum AutoMsg {
    /// Venció el plazo de `cfg.auto_refresh_minutes`: se pregunta si está minimizada.
    Tick,
    /// `minimized`: lo que respondió la ventana (`None` = no se supo).
    Run { minimized: Option<bool> },
}

impl App {
    pub fn auto_refresh_minutes(&self) -> u32 {
        self.cfg.auto_refresh_minutes
    }

    /// Solo con el refresco activado; otro plazo cambia la identidad y lo reinicia.
    pub(super) fn auto_refresh_subscription(&self) -> Option<Subscription<Message>> {
        let minutes = u64::from(self.cfg.auto_refresh_minutes);
        (minutes > 0).then(|| {
            time::every(Duration::from_secs(60 * minutes))
                .map(|_| Message::AutoRefresh(AutoMsg::Tick))
        })
    }

    /// Hay masters efectivos y, si el juego usa la API web de Steam, hay clave.
    pub fn can_scan(&self, game: &GameDef) -> bool {
        let key = self.steam_key.as_deref().unwrap_or_default();
        !effective_masters(game.id, &self.official, &self.custom).is_empty()
            && !(game.steam_webapi && key.is_empty())
    }

    pub(super) fn on_auto_refresh(&mut self, m: AutoMsg) -> Task<Message> {
        match m {
            AutoMsg::Tick => window::is_minimized(self.windows.main)
                .map(|minimized| Message::AutoRefresh(AutoMsg::Run { minimized })),
            AutoMsg::Run { minimized } => self.auto_refresh(minimized == Some(true)),
        }
    }

    /// `_auto_refresh`: no pisa un escaneo, una consulta, un diálogo, un selector de archivos
    /// (modal en Qt) ni una ventana minimizada o en la bandeja, y conserva la fila elegida.
    fn auto_refresh(&mut self, minimized: bool) -> Task<Message> {
        let Some(g) = self.current else {
            return Task::none();
        };
        if minimized
            || self.in_tray()
            || self.is_scanning()
            || self.is_querying_direct()
            || self.dialog.is_some()
            || self.files_open
        {
            return Task::none();
        }
        let direct = self.direct_view().is_some();
        if !direct && !self.can_scan(g) {
            return Task::none(); // vaciaría la tabla sin poder llenarla
        }
        // Estas vistas se vacían: la fila vuelve a elegirse al llegar.
        if direct || self.scan.lan() {
            self.pending_select = self.table.selected.clone();
        }
        self.refresh(true)
    }

    pub fn pending_select(&self) -> Option<&ServerAddress> {
        self.pending_select.as_ref()
    }

    pub fn set_pending_select(&mut self, address: Option<ServerAddress>) {
        self.pending_select = address;
    }

    /// `_apply_pending_select`: llegó `address`; si es la pendiente y se ve, se elige.
    pub(super) fn apply_pending_select(&mut self, address: &ServerAddress) -> Task<Message> {
        if self.pending_select.as_ref() != Some(address) {
            return Task::none();
        }
        self.select_pending()
    }

    /// Elige la pendiente si está y se ve (y deja de esperarla).
    pub(super) fn select_pending(&mut self) -> Task<Message> {
        let Some(address) = self.pending_select.clone() else {
            return Task::none();
        };
        self.select_address(&address).unwrap_or_else(Task::none)
    }

    /// `select_address`: elige y muestra la fila; si se pudo, ya no hay ninguna pendiente
    /// (si no, una vieja volvería a robar la selección al llegar).
    pub(super) fn select_address(&mut self, address: &ServerAddress) -> Option<Task<Message>> {
        let task = table::select_address(&mut self.table, address)?;
        self.pending_select = None;
        Some(task)
    }

    /// `_clear_stale_pending`: el usuario eligió otra fila; al llegar no se la quita.
    pub(super) fn clear_stale_pending(&mut self) {
        if self.table.selected.is_some() && self.table.selected != self.pending_select {
            self.pending_select = None;
        }
    }
}
