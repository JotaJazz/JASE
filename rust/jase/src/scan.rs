//! El escaneo de `jase-core` como `Stream` de Iced (el `ScanWorker` de Python).

use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::Arc;

use futures::FutureExt;
use futures::channel::mpsc;
use futures::stream::{self, BoxStream, StreamExt};
use jase_core::detect::resolve_game_exe;
use jase_core::games::GameDef;
use jase_core::geo::{CountryDB, current_month, default_fetch, load_country_db};
use jase_core::i18n::tr_fmt;
use jase_core::masters::Master;
use jase_core::scanner::scan::{ScanEvent, ScanOptions, ScanResult, scan};

// Cada mensaje se mueve una vez a `update` (como `ScanEvent`): Box solo añadiría una asignación.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum ScanMsg {
    Event(ScanEvent),
    /// Siempre el último del stream.
    Finished(ScanResult),
}

pub type ScanFn = Arc<
    dyn Fn(&'static GameDef, Vec<Master>, ScanOptions) -> BoxStream<'static, ScanMsg> + Send + Sync,
>;

/// Carga (o con `refresh`, descarga) la base de países de esa ruta; bloquea: va en un hilo.
pub type CountryDbFn = Arc<dyn Fn(&Path, bool) -> Result<CountryDB, String> + Send + Sync>;

/// ¿Está el exe del juego en esa carpeta? Toca el disco: va en un hilo.
pub type InstalledFn = Arc<dyn Fn(&'static GameDef, &str) -> bool + Send + Sync>;

/// Lo que la app recibe de afuera; los tests ponen falsos (sin red ni salida del proceso).
pub struct Deps {
    pub scan: ScanFn,
    pub exit: Arc<dyn Fn(i32) + Send + Sync>,
    pub steam_key: String,
    /// `None` = sin base de países (País queda vacío), como `geo=None`.
    pub country_db: Option<CountryDbFn>,
    pub game_installed: InstalledFn,
}

/// La base real: del archivo en caché o bajada de db-ip.com.
pub fn country_db(path: &Path, refresh: bool) -> Result<CountryDB, String> {
    load_country_db(path, refresh, current_month(), &default_fetch)
}

/// El exe real (`detect.resolve_game_exe`, el que usa «Solo instalados» en Python).
pub fn game_installed(game: &'static GameDef, dir: &str) -> bool {
    resolve_game_exe(game, Some(dir)).is_some()
}

/// Escaneo real; soltar el stream suelta el future y lo cancela.
pub fn scan_stream(
    game: &'static GameDef,
    masters: Vec<Master>,
    opts: ScanOptions,
) -> BoxStream<'static, ScanMsg> {
    scan_stream_with(move |mut emit| async move { scan(game, &masters, &opts, &mut *emit).await })
}

pub type Emit = Box<dyn FnMut(ScanEvent) + Send>;

/// Como [`scan_stream`] con el escáner inyectado: `run` recibe dónde emitir los eventos.
pub fn scan_stream_with<F, Fut>(run: F) -> BoxStream<'static, ScanMsg>
where
    F: FnOnce(Emit) -> Fut,
    Fut: Future<Output = ScanResult> + Send + 'static,
{
    let (tx, rx) = mpsc::unbounded();
    let events = tx.clone();
    let fut = run(Box::new(move |e| {
        let _ = events.unbounded_send(ScanMsg::Event(e));
    }));
    // Finished va por el mismo canal, detrás de los eventos; el canal cierra al soltar
    // los dos emisores (el de `run` muere con el future).
    // Un pánico no deja la app «escaneando»: se informa y termina como fallido (`_on_error`),
    // con un resultado vacío que no reemplaza la lista guardada.
    let driver = async move {
        let result = match AssertUnwindSafe(fut).catch_unwind().await {
            Ok(result) => result,
            Err(panic) => {
                let error = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("panic");
                let status = tr_fmt("Error en el escaneo: {error}", &[("error", &error)]);
                let _ = tx.unbounded_send(ScanMsg::Event(ScanEvent::Status(status)));
                ScanResult::default()
            }
        };
        let _ = tx.unbounded_send(ScanMsg::Finished(result));
    };
    let driver = stream::once(driver).filter_map(|()| async { None });
    stream::select(rx, driver).boxed()
}
