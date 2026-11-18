//! El escaneo de `jase-core` como `Stream` de Iced (el `ScanWorker` de Python).

use std::sync::Arc;

use futures::channel::mpsc;
use futures::stream::{self, BoxStream, StreamExt};
use jase_core::games::GameDef;
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

/// Lo que la app recibe de afuera; los tests ponen falsos (sin red ni salida del proceso).
pub struct Deps {
    pub scan: ScanFn,
    pub exit: Arc<dyn Fn(i32) + Send + Sync>,
    pub steam_key: String,
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
    let driver = async move {
        let result = fut.await;
        let _ = tx.unbounded_send(ScanMsg::Finished(result));
    };
    let driver = stream::once(driver).filter_map(|()| async { None });
    stream::select(rx, driver).boxed()
}
