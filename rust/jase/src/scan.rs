//! El escaneo de `jase-core` como `Stream` de Iced (el `ScanWorker` de Python).

use std::io;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::FutureExt;
use futures::channel::mpsc;
use futures::future::BoxFuture;
use futures::stream::{self, BoxStream, StreamExt};
use iced::Task;
use iced::task::Handle;
use iced::widget::image::Handle as Image;
use jase_core::detect::resolve_game_exe;
use jase_core::games::{Family, GameDef};
use jase_core::geo::{CountryDB, current_month, default_fetch, load_country_db};
use jase_core::i18n::tr_fmt;
use jase_core::identify::{detected_games, probe_delay, probe_plan};
use jase_core::lan::{LanOptions, scan_lan};
use jase_core::maps::service::MapImageService;
use jase_core::masters::Master;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::query::{QueryOptions, game_query_options, query_servers};
use jase_core::scanner::scan::{ScanEvent, ScanOptions, ScanResult, scan};

use crate::app::Message;
use crate::files::FilesFn;
use crate::launch::LaunchFn;
use crate::localtime::local_offset_s;
use crate::passwords::KeyringFn;

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

/// Icono del exe del juego en esa carpeta; `None` = la insignia. Toca el disco: va en un hilo.
pub type ExeIconFn = Arc<dyn Fn(&'static GameDef, &str) -> Option<Image> + Send + Sync>;

/// Búsqueda en la red local del juego (sin masters), como stream del escaneo.
pub type LanFn = Arc<dyn Fn(&'static GameDef) -> BoxStream<'static, ScanMsg> + Send + Sync>;

/// Consulta un servidor; `None` = no respondió. Va en una `Task`.
pub type QueryFn = Arc<
    dyn Fn(
            &'static GameDef,
            ServerAddress,
        ) -> BoxFuture<'static, Result<Option<ServerInfo>, String>>
        + Send
        + Sync,
>;

/// «Detectar»: los juegos (con su dirección de consulta) que reconocen la dirección. Va en
/// una `Task`.
pub type DetectFn = Arc<
    dyn Fn(ServerAddress) -> BoxFuture<'static, Vec<(&'static str, ServerAddress)>> + Send + Sync,
>;

/// Ruta a un PNG con la imagen del mapa (juego, carpeta, mapa, «descargar»); bloquea (lee
/// archivos del juego y puede descargar): va en un hilo.
pub type MapImageFn =
    Arc<dyn Fn(&'static GameDef, Option<String>, String, bool) -> Option<PathBuf> + Send + Sync>;

/// Lo que la app recibe de afuera; los tests ponen falsos (sin red ni salida del proceso).
pub struct Deps {
    pub scan: ScanFn,
    pub lan: LanFn,
    pub query: QueryFn,
    pub exit: Arc<dyn Fn(i32) + Send + Sync>,
    /// El llavero: bloquea (puede esperar a que se desbloquee), solo en `app::blocking`.
    pub keyring: KeyringFn,
    /// Lanza el juego (lee su carpeta y crea el proceso): solo en `app::blocking`.
    pub launch: LaunchFn,
    /// `None` = sin base de países (País queda vacío), como `geo=None`.
    pub country_db: Option<CountryDbFn>,
    pub game_installed: InstalledFn,
    pub map_image: MapImageFn,
    /// Segundos Unix y desfase de la hora local en segundos; los tests ponen un reloj falso.
    pub now: Arc<dyn Fn() -> (f64, i64) + Send + Sync>,
    pub exe_icon: ExeIconFn,
    pub detect: DetectFn,
    /// Selectores de archivo: bloquean hasta que el usuario elige, solo en `app::blocking`.
    pub files: FilesFn,
}

/// El reloj real: `SystemTime` y la zona del sistema.
pub fn system_now() -> (f64, i64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    (now, local_offset_s(now as i64))
}

/// La caché de imágenes junto a la config (`%APPDATA%\JimmysASE\cache\maps` en Windows).
pub fn map_cache_dir(cfg_path: &Path) -> PathBuf {
    cfg_path
        .parent()
        .unwrap_or(Path::new(""))
        .join("cache")
        .join("maps")
}

/// El servicio real, compartido por los hilos (recuerda en memoria los «no encontrado»).
pub fn map_image_service(cache_dir: PathBuf) -> MapImageFn {
    let service = Arc::new(MapImageService::new(cache_dir));
    Arc::new(move |game, dir, map, download| service.get(game, dir.as_deref(), &map, download))
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

/// Búsqueda real en la red local; soltar el stream la cancela.
pub fn lan_stream(game: &'static GameDef) -> BoxStream<'static, ScanMsg> {
    scan_stream_with(move |mut emit| async move {
        let result = scan_lan(game, &LanOptions::default(), &mut *emit).await;
        lan_finish(result, &mut emit)
    })
}

/// A diferencia del escaneo, `scan_lan` propaga el error de `bind`: se informa y termina como
/// fallido, con un resultado vacío.
pub fn lan_finish(result: io::Result<ScanResult>, emit: &mut Emit) -> ScanResult {
    result.unwrap_or_else(|e| {
        emit(ScanEvent::Status(tr_fmt(
            "Error en el escaneo: {error}",
            &[("error", &e)],
        )));
        ScanResult::default()
    })
}

/// Consulta real de un servidor con los valores de `scanner.query_servers` de Python.
pub fn query_one(
    game: &'static GameDef,
    address: ServerAddress,
) -> BoxFuture<'static, Result<Option<ServerInfo>, String>> {
    async move {
        let mut found = None;
        let mut on_server = |info: ServerInfo| {
            found.get_or_insert(info);
        };
        query_servers(
            game.family,
            vec![address],
            &game_query_options(game),
            Duration::from_millis(1500),
            1,
            64,
            &mut on_server,
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(found)
    }
    .boxed()
}

/// «Detectar» real: cada familia con los valores por defecto de `scanner.query_servers` (los
/// de `DetectWorker`); un error de una familia solo la deja sin respuestas.
pub fn detect_game(
    address: ServerAddress,
) -> BoxFuture<'static, Vec<(&'static str, ServerAddress)>> {
    detect_with(address, |family, addresses| {
        async move {
            let mut found = Vec::new();
            let mut on_server = |info: ServerInfo| found.push(info);
            let _ = query_servers(
                family,
                addresses,
                &QueryOptions::default(),
                Duration::from_millis(1500),
                1,
                64,
                &mut on_server,
                None,
            )
            .await;
            found
        }
        .boxed()
    })
}

/// `DetectWorker` con la consulta por familia inyectada: todas a la vez, cada una tras su
/// `probe_delay`; las respuestas, en orden de llegada, pasan por `detected_games`.
pub fn detect_with<F>(
    address: ServerAddress,
    query_family: F,
) -> BoxFuture<'static, Vec<(&'static str, ServerAddress)>>
where
    F: Fn(Family, Vec<ServerAddress>) -> BoxFuture<'static, Vec<ServerInfo>>
        + Send
        + Sync
        + 'static,
{
    async move {
        let responses = std::sync::Mutex::new(Vec::new());
        let probes = probe_plan(&address).into_iter().map(|(family, addresses)| {
            let (query_family, responses) = (&query_family, &responses);
            async move {
                tokio::time::sleep(probe_delay(family)).await;
                let found = query_family(family, addresses).await;
                if let Ok(mut r) = responses.lock() {
                    r.extend(found.into_iter().map(|info| (family, info)));
                }
            }
        });
        futures::future::join_all(probes).await;
        let responses = responses.into_inner().unwrap_or_default();
        detected_games(&responses)
    }
    .boxed()
}

/// El escaneo en curso y la consulta de un servidor (`_worker`, `_query_worker`): cada uno
/// con su número, para descartar lo que llegue de uno detenido o reemplazado.
#[derive(Default)]
pub struct ScanState {
    /// Sube con cada escaneo nuevo o detenido: los mensajes de otra generación se descartan.
    generation: u64,
    handle: Option<Handle>,
    /// Escaneo cancelado: la barra lateral cuenta las filas que quedaron.
    stopped: bool,
    progress: Option<(usize, usize)>,
    /// La tabla muestra la red local (`_lan_view`): no va a la caché ni al disco.
    lan: bool,
    query: Option<Handle>,
    query_request: u64,
}

impl ScanState {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_scanning(&self) -> bool {
        self.handle.is_some()
    }

    pub fn lan(&self) -> bool {
        self.lan
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn progress(&self) -> Option<(usize, usize)> {
        self.progress
    }

    pub fn query_request(&self) -> u64 {
        self.query_request
    }

    /// Arranca un escaneo nuevo (de internet o, con `lan`, de la red local).
    pub fn begin(&mut self, stream: BoxStream<'static, ScanMsg>, lan: bool) -> Task<Message> {
        self.generation += 1;
        self.stopped = false;
        self.progress = None;
        self.lan = lan;
        let generation = self.generation;
        let (task, handle) =
            Task::run(stream, move |msg| Message::Scan { generation, msg }).abortable();
        self.handle = Some(handle);
        task
    }

    /// Cancela sin esperar: soltar el future lo corta y sus mensajes quedan viejos. `false` si
    /// no había escaneo.
    pub fn stop(&mut self) -> bool {
        let Some(handle) = self.handle.take() else {
            return false;
        };
        handle.abort();
        self.generation += 1;
        self.stopped = true;
        self.progress = None;
        true
    }

    /// Llegó el `Finished` del escaneo vigente.
    pub(crate) fn finish(&mut self) {
        self.handle = None;
        self.progress = None;
    }

    pub(crate) fn set_progress(&mut self, done: usize, total: usize) {
        self.progress = Some((done, total));
    }

    /// La tabla se vació: ya no es la vista de la red local.
    pub(crate) fn leave_lan(&mut self) {
        self.lan = false;
    }

    /// Consulta un servidor del juego `game` (cancela la anterior); la respuesta llega en
    /// `Message::Query` con el número de pedido que se devuelve.
    pub fn begin_query(
        &mut self,
        game: &'static str,
        fut: BoxFuture<'static, Result<Option<ServerInfo>, String>>,
    ) -> (u64, Task<Message>) {
        self.cancel_query();
        let request = self.query_request;
        let (task, handle) = Task::perform(fut, move |result| Message::Query {
            request,
            game,
            result,
        })
        .abortable();
        self.query = Some(handle);
        (request, task)
    }

    /// Suelta la consulta en curso (si hay) y deja viejo su pedido.
    pub fn cancel_query(&mut self) {
        if let Some(handle) = self.query.take() {
            handle.abort();
        }
        self.query_request += 1;
    }
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
