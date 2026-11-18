//! Listas guardadas entre sesiones (`window_saved_lists.py`): al abrir JASE, cada juego muestra
//! su último escaneo completo (como el ASE original) hasta que se refresca.

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use iced::Task;
use indexmap::IndexMap;
use jase_core::models::ServerInfo;
use jase_core::server_cache;

use crate::app::{Message, blocking};

type Job = Box<dyn FnOnce() + Send>;

/// Hilo escritor (`_write_in_order`): un solo hilo y en orden, así dos guardados seguidos del
/// mismo juego nunca se pisan al revés. Termina al soltarse el emisor, tras vaciar la cola.
pub struct Writer(mpsc::Sender<Job>);

impl Writer {
    fn spawn() -> Option<Writer> {
        let (tx, rx) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("jase-saved-lists".into())
            .spawn(move || {
                for job in rx {
                    // Disco lleno, permisos o un pánico: la lista anterior sigue y el hilo
                    // atiende el trabajo siguiente (Python sigue tras una excepción).
                    let _ = panic::catch_unwind(AssertUnwindSafe(job));
                }
            })
            .ok()?;
        Some(Writer(tx))
    }
}

pub struct SavedLists {
    pub dir: PathBuf,
    /// {juego: (servidores, guardada en)} de `index.json`, leído al arrancar (pequeño).
    pub index: IndexMap<String, (u64, f64)>,
    /// Juego → fecha de la lista leída del disco (la de esta sesión no tiene).
    pub saved_at: HashMap<&'static str, f64>,
    /// Sube con cada lectura pedida o cancelada: la respuesta de otro número se descarta.
    pub request: u64,
    writer: Option<Writer>,
}

impl SavedLists {
    /// Lee el índice de `<cfg_dir>/cache/servers` (en el hilo de la UI, como Python).
    pub fn open(cfg_dir: &Path) -> SavedLists {
        let dir = cfg_dir.join("cache").join("servers");
        SavedLists {
            index: server_cache::read_index(&dir),
            dir,
            saved_at: HashMap::new(),
            request: 0,
            writer: None,
        }
    }

    /// La lista de la sesión de `game` es la del disco (no un escaneo de ahora).
    pub fn from_disk(&self, game: &str) -> Option<f64> {
        self.saved_at.get(game).copied()
    }

    /// Lista de un escaneo completo: pasa a ser la de la próxima sesión (`_save_list`).
    pub fn save(&mut self, game: &'static str, servers: &[ServerInfo], now: f64) {
        self.saved_at.remove(game);
        self.index
            .insert(game.to_owned(), (servers.len() as u64, now));
        let (dir, servers) = (self.dir.clone(), servers.to_vec());
        self.write(Box::new(move || {
            let _ = server_cache::save(&dir, game, &servers, now);
        }));
    }

    fn write(&mut self, job: Job) {
        if self.writer.is_none() {
            self.writer = Writer::spawn();
        }
        // Sin hilo (el sistema no dejó crearlo) no se guarda: la lista anterior sigue.
        if let Some(w) = &self.writer {
            let _ = w.0.send(job);
        }
    }

    /// Descarta la lectura en camino (F5): al llegar no debe tapar lo que haya en la tabla.
    pub fn cancel_load(&mut self) {
        self.request += 1;
    }

    /// Pide la lista guardada en un hilo (`_load_saved_list`); `None` si no hay (escanear).
    pub fn load(&mut self, game: &'static str) -> Option<(u64, Task<Message>)> {
        if !self.index.contains_key(game) {
            return None;
        }
        self.request += 1;
        let (request, dir) = (self.request, self.dir.clone());
        let task = Task::perform(blocking(move || server_cache::load(&dir, game)), move |r| {
            Message::SavedList {
                request,
                game,
                result: r.ok().flatten(),
            }
        });
        Some((request, task))
    }

    /// Espera a que el escritor vacíe su cola (los tests).
    pub fn flush(&self) {
        self.flush_within(Duration::MAX);
    }

    /// Como [`flush`](Self::flush), como mucho `limit`: el cierre no se cuelga con un disco
    /// trabado. `false` si quedó algo sin escribir.
    pub fn flush_within(&self, limit: Duration) -> bool {
        let Some(w) = &self.writer else {
            return true;
        };
        let (tx, rx) = mpsc::channel();
        if w.0
            .send(Box::new(move || {
                let _ = tx.send(());
            }))
            .is_err()
        {
            return true;
        }
        rx.recv_timeout(limit).is_ok()
    }
}

/// Segundos Unix → «AAAA-MM-DD HH:MM» en UTC.
/// shortcut: sin zona horaria local ni el formato corto de `QLocale`; cambiarlo cuando la 5e
/// necesite horas locales para el historial.
pub fn short_datetime(unix: f64) -> String {
    let secs = unix.max(0.0) as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // `civil_from_days` de Howard Hinnant (calendario gregoriano proléptico).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn writer_survives_panicking_job() {
        let dir = tempfile::tempdir().unwrap();
        let mut saved = SavedLists::open(dir.path());
        saved.write(Box::new(|| panic!("disco roto")));
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        saved.write(Box::new(move || flag.store(true, Ordering::SeqCst)));
        assert!(saved.flush_within(Duration::from_secs(30)));
        assert!(done.load(Ordering::SeqCst));
    }
}
