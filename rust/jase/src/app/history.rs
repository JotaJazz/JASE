//! Historial de ocupación (`window_history.py`): se registra cada respuesta de un favorito o
//! reciente, se lee al arrancar y se escribe cada 10 min y al cerrar, siempre fuera del hilo
//! de la UI.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::Duration;

use iced::Task;
use jase_core::favorites::is_favorite;
use jase_core::fsutil::atomic_write;
use jase_core::history::History;
use jase_core::models::{ServerAddress, ServerInfo};

use super::{App, Message, blocking};

/// Cada cuánto se guarda (`SAVE_INTERVAL_S`).
pub const SAVE_INTERVAL: Duration = Duration::from_secs(600);

/// `<carpeta de la config>/cache/history.json`.
pub(super) fn history_path(cfg_path: &std::path::Path) -> PathBuf {
    let dir = cfg_path.parent().unwrap_or(std::path::Path::new(""));
    dir.join("cache").join("history.json")
}

/// Escrituras del historial numeradas: la que corre tarde con datos más viejos (un guardado
/// periódico en vuelo al cerrar) no pisa a una más nueva.
#[derive(Debug, Default)]
pub(crate) struct Writes {
    seq: u64,
    done: Arc<Mutex<u64>>,
}

impl Writes {
    fn job(&mut self, path: PathBuf, data: Vec<u8>) -> impl FnOnce() + Send + 'static {
        self.seq += 1;
        let (seq, done) = (self.seq, self.done.clone());
        move || {
            let mut last = done.lock().unwrap_or_else(PoisonError::into_inner);
            if seq > *last {
                *last = seq;
                let _ = atomic_write(&path, &data);
            }
        }
    }
}

impl App {
    /// Lee el historial en un hilo; hasta que llega no se registra ni se guarda.
    pub(super) fn load_history(&self) -> Task<Message> {
        let path = self.history.path.clone();
        let now = (self.deps.now)().0;
        let read = blocking(move || {
            let mut h = History::new(path);
            h.load(now);
            Arc::new(h)
        });
        // Si el hilo falló, el historial nunca llega: no se pisa el archivo.
        Task::perform(read, Result::ok).and_then(|h| Task::done(Message::HistoryLoaded(h)))
    }

    /// Lo leído reemplaza lo que hubiera: antes de llegar no se registró nada.
    pub(super) fn on_history_loaded(&mut self, h: Arc<History>) {
        self.history = Arc::try_unwrap(h).unwrap_or_else(|h| History {
            path: h.path.clone(),
            data: h.data.clone(),
        });
        self.history_loaded = true;
    }

    pub fn is_history_server(&self, game_id: &str, address: &ServerAddress) -> bool {
        is_favorite(&self.cfg, game_id, address)
            || self
                .cfg
                .recent
                .get(game_id)
                .is_some_and(|r| r.contains(&address.to_string()))
    }

    /// Favoritos y recientes de la config: lo que el guardado no poda.
    pub fn history_keep(&self) -> HashSet<(String, String)> {
        let entries = self.cfg.favorites.iter().chain(&self.cfg.recent);
        entries
            .flat_map(|(game, list)| list.iter().map(|a| (game.clone(), a.clone())))
            .collect()
    }

    /// `_on_server_response`: solo favoritos y recientes que responden con máximo.
    pub fn record_response(&mut self, game_id: &str, info: &ServerInfo) {
        if self.history_loaded
            && info.online
            && info.max_players > 0
            && self.is_history_server(game_id, &info.address)
        {
            let now = (self.deps.now)().0;
            self.history.record(
                game_id,
                &info.address.to_string(),
                i64::from(info.num_players),
                i64::from(info.max_players),
                now,
            );
        }
    }

    /// Poda y serializa aquí (como Python); la escritura, en un hilo.
    fn history_write(&mut self) -> Option<impl FnOnce() + Send + 'static> {
        if !self.history_loaded {
            return None;
        }
        let keep = self.history_keep();
        self.history.prune(&keep);
        let (path, data) = (self.history.path.clone(), self.history.to_bytes());
        Some(self.history_writes.job(path, data))
    }

    /// El guardado de cada `SAVE_INTERVAL`.
    pub(super) fn save_history(&mut self) -> Task<Message> {
        let Some(write) = self.history_write() else {
            return Task::none();
        };
        Task::future(blocking(write)).discard()
    }

    /// Al cerrar: escribe en un hilo y lo espera como mucho `limit` (un disco trabado no
    /// cuelga el cierre); si hay un guardado periódico escribiendo, espera a que termine.
    pub(super) fn save_history_within(&mut self, limit: Duration) {
        let Some(write) = self.history_write() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("jase-history-save".into())
            .spawn(move || {
                write();
                let _ = tx.send(());
            });
        if spawned.is_ok() {
            let _ = rx.recv_timeout(limit);
        }
    }
}
