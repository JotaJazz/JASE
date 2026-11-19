//! Ayudas compartidas: servidores de prueba y un escáner falso (como `FakeScan` de Python).
#![allow(dead_code)] // cada binario de test usa una parte

pub mod lang;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use futures::StreamExt;
use iced::advanced::widget::operation::{Focusable, TextInput};
use iced::{Rectangle, Task};
use iced_runtime::Action;
use indexmap::IndexMap;
use jase::app::{App, Message};
use jase::files::FileDialogs;
use jase::launch::LaunchArgs;
use jase::passwords::Keyring;
use jase::scan::{Deps, FetchFn, RconFn, ScanMsg};
use jase::update::SelfUpdate;
use jase::widgets::FILTER_ID;
use jase_core::app_update::{DownloadCode, DownloadError};
use jase_core::config::Config;
use jase_core::detect::Finding;
use jase_core::masters::Master;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::rcon::RconError;
use jase_core::scanner::scan::{ScanEvent, ScanResult};

pub fn mk(host: &str, name: &str, ping: u32, players: i32, version: &str) -> ServerInfo {
    let a = ServerAddress::new(host, 28960);
    let mut info = ServerInfo::new(a.clone(), a);
    info.name = name.to_owned();
    info.map = "mp_toujane".to_owned();
    info.gametype = "tdm".to_owned();
    info.num_players = players;
    info.max_players = 20;
    info.version = version.to_owned();
    info.ping_ms = ping;
    info
}

/// Lo que responde la consulta falsa de un servidor.
pub type Answer = Result<Option<ServerInfo>, String>;

/// Anota los juegos pedidos; el stream no trae nada (los tests meten los mensajes a mano).
#[derive(Clone, Default)]
pub struct FakeScan {
    pub calls: Arc<Mutex<Vec<&'static str>>>,
    pub exits: Arc<Mutex<Vec<i32>>>,
    /// Juegos por los que se preguntó «¿está instalado?» (responde que no).
    pub installed: Arc<Mutex<Vec<&'static str>>>,
    /// Juegos con búsqueda en la red local (el stream no trae nada).
    pub lan: Arc<Mutex<Vec<&'static str>>>,
    /// Consultas de un servidor (al correr la `Task`) y lo que responden.
    pub queries: Arc<Mutex<Vec<(&'static str, ServerAddress)>>>,
    /// `None` = no respondió.
    pub answer: Arc<Mutex<Option<Answer>>>,
    /// Respuesta por dirección (manda sobre `answer`), como `FakeQuery({address: info})`.
    pub answers: Arc<Mutex<HashMap<ServerAddress, Answer>>>,
    /// Consultas en curso y el máximo a la vez.
    pub query_running: Arc<AtomicUsize>,
    pub query_max: Arc<AtomicUsize>,
    pub maps: FakeMaps,
    pub clock: Clock,
    /// Juegos por los que se pidió el icono del exe (solo `q3` lo tiene).
    pub exe_icons: Arc<Mutex<Vec<&'static str>>>,
    /// La clave de Steam que recibió cada escaneo.
    pub steam_keys: Arc<Mutex<Vec<String>>>,
    pub keyring: FakeKeyring,
    /// Cada lanzamiento del juego, con lo que recibió el lanzador.
    pub launches: Arc<Mutex<Vec<LaunchArgs>>>,
    /// `Some` = el lanzador falla con ese error.
    pub launch_error: Arc<Mutex<Option<String>>>,
    /// Lo que responde «Detectar» y las direcciones por las que se preguntó (al correr la `Task`).
    pub detect: Arc<Mutex<Vec<(&'static str, ServerAddress)>>>,
    pub detect_calls: Arc<Mutex<Vec<ServerAddress>>>,
    pub files: FakeFiles,
    /// Juego → exe que «encuentra» `game_installed` si la carpeta pedida es la suya.
    pub exes: Arc<Mutex<HashMap<&'static str, String>>>,
    /// Cada carpeta por la que preguntó `game_installed` y el hilo que lo hizo.
    pub installed_at: Arc<Mutex<Vec<(String, ThreadId)>>>,
    /// Cada prueba de master: juego, master, clave de Steam y el hilo.
    pub probes: Arc<Mutex<Vec<ProbeCall>>>,
    /// Lo que responde la prueba (`None` = no responde).
    pub probe_answer: Arc<Mutex<Option<usize>>>,
    /// Cada URL abierta y el hilo que la abrió.
    pub opened: Arc<Mutex<Vec<(String, ThreadId)>>>,
    pub find: FakeFind,
    /// Lo que trae `update_stream` (`None` = falla la red) y de a cuántos bytes por lectura.
    pub stream_body: Arc<Mutex<Option<Vec<u8>>>>,
    pub stream_chunk: Arc<Mutex<usize>>,
    /// Cada URL que pidió `update_stream` y el hilo.
    pub streams: Arc<Mutex<Vec<(String, ThreadId)>>>,
    /// Cada proceso lanzado (`Deps.spawn`): programa, argumentos y el hilo; nada se ejecuta.
    pub spawned: Arc<Mutex<Vec<SpawnCall>>>,
    /// `true` = lanzar falla.
    pub spawn_fails: Arc<Mutex<bool>>,
    /// Lo que trae el escaneo de un juego: sus servidores y luego `Finished` con esos masters
    /// que respondieron. Sin entrada, el stream no trae nada.
    pub scripts: Arc<Mutex<HashMap<&'static str, Script>>>,
    pub rcon: FakeRcon,
}

/// Un comando RCON: juego, dirección, clave, comando y el hilo que llamó.
pub type RconCall = (&'static str, ServerAddress, String, String, ThreadId);

/// `Deps.rcon` falso: anota cada comando y responde lo puesto (por defecto, texto vacío).
#[derive(Clone, Default)]
pub struct FakeRcon {
    calls: Arc<Mutex<Vec<RconCall>>>,
    answer: Arc<Mutex<Option<Result<String, RconError>>>>,
}

impl FakeRcon {
    pub fn answer(&self, answer: Result<String, RconError>) {
        *self.answer.lock().unwrap() = Some(answer);
    }

    pub fn calls(&self) -> Vec<RconCall> {
        self.calls.lock().unwrap().clone()
    }

    fn as_fn(&self) -> RconFn {
        let (calls, answer) = (self.calls.clone(), self.answer.clone());
        Arc::new(move |game, address, password, command| {
            let call = (
                game.id,
                address,
                password.0,
                command,
                std::thread::current().id(),
            );
            calls.lock().unwrap().push(call);
            let answer = answer.lock().unwrap().clone().unwrap_or(Ok(String::new()));
            Box::pin(futures::future::ready(answer))
        })
    }
}

/// Lo que trae el escaneo falso de un juego: servidores y masters que respondieron.
pub type Script = (Vec<ServerInfo>, usize);

/// Un proceso lanzado: programa, argumentos y el hilo.
pub type SpawnCall = (PathBuf, Vec<String>, ThreadId);

/// Un cuerpo que se lee de a `chunk` bytes, como la respuesta de la red.
struct Chunks {
    data: Vec<u8>,
    pos: usize,
    chunk: usize,
}

impl std::io::Read for Chunks {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.chunk).min(self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Lo que encuentra *Buscar juegos* (por carpeta extra).
pub type Found = IndexMap<String, Vec<Finding>>;

/// Una búsqueda de juegos: la carpeta extra y el hilo.
pub type FindCall = (Option<String>, ThreadId);

/// `find_games` falso: responde según la carpeta extra (`None` = la búsqueda normal), anota
/// cada llamada con su hilo y, con `hold`, deja en espera la búsqueda sin carpeta.
#[derive(Clone, Default)]
pub struct FakeFind {
    pub results: Arc<Mutex<HashMap<Option<String>, Found>>>,
    calls: Arc<(Mutex<Vec<FindCall>>, Condvar)>,
    held: Arc<(Mutex<bool>, Condvar)>,
}

impl FakeFind {
    pub fn find(&self, extra: Option<String>) -> Found {
        {
            let (calls, cv) = &*self.calls;
            let call = (extra.clone(), std::thread::current().id());
            calls.lock().unwrap().push(call);
            cv.notify_all();
        }
        if extra.is_none() {
            let (held, cv) = &*self.held;
            let _held = cv.wait_while(held.lock().unwrap(), |h| *h).unwrap();
        }
        let results = self.results.lock().unwrap();
        results.get(&extra).cloned().unwrap_or_default()
    }

    pub fn set(&self, extra: Option<&str>, found: Found) {
        let mut results = self.results.lock().unwrap();
        results.insert(extra.map(str::to_owned), found);
    }

    pub fn calls(&self) -> Vec<FindCall> {
        self.calls.0.lock().unwrap().clone()
    }

    pub fn extras(&self) -> Vec<Option<String>> {
        self.calls().into_iter().map(|c| c.0).collect()
    }

    /// Espera (con un plazo amplio) a que haya `n` llamadas.
    pub fn wait_calls(&self, n: usize) {
        let (calls, cv) = &*self.calls;
        let limit = Duration::from_secs(60);
        let (_calls, timeout) = cv
            .wait_timeout_while(calls.lock().unwrap(), limit, |c| c.len() < n)
            .unwrap();
        assert!(!timeout.timed_out(), "esperaba {n} búsquedas de juegos");
    }

    pub fn hold(&self) {
        *self.held.0.lock().unwrap() = true;
    }

    pub fn release(&self) {
        let (held, cv) = &*self.held;
        *held.lock().unwrap() = false;
        cv.notify_all();
    }
}

/// `masters_fetch` falso: responde lo que hay en la cola (vacía = sin red) y anota cada URL
/// con el hilo que la pidió.
#[derive(Clone, Default)]
pub struct FakeFetch {
    pub queue: Arc<Mutex<VecDeque<Option<Vec<u8>>>>>,
    pub calls: Arc<Mutex<Vec<(String, ThreadId)>>>,
}

impl FakeFetch {
    pub fn new(answers: &[Option<&[u8]>]) -> FakeFetch {
        let fetch = FakeFetch::default();
        let queue = answers.iter().map(|a| a.map(<[u8]>::to_vec)).collect();
        *fetch.queue.lock().unwrap() = queue;
        fetch
    }

    pub fn calls(&self) -> Vec<(String, ThreadId)> {
        self.calls.lock().unwrap().clone()
    }

    pub fn as_fn(&self) -> FetchFn {
        let (queue, calls) = (self.queue.clone(), self.calls.clone());
        Arc::new(move |url| {
            let call = (url.to_owned(), std::thread::current().id());
            calls.lock().unwrap().push(call);
            queue.lock().unwrap().pop_front().flatten()
        })
    }
}

/// App con la config `cfg` y ese `masters_fetch` (`None` = sin red), con la `Task` del arranque.
pub fn fetch_app(
    dir: &Path,
    cfg: Config,
    fetch: Option<&FakeFetch>,
) -> (TestApp, FakeScan, Task<Message>) {
    let lang = lang::Spanish::hold();
    let fake = FakeScan::default();
    let mut deps = fake_deps(&fake, "");
    deps.masters_fetch = fetch.map(FakeFetch::as_fn);
    let (app, task) = App::new(cfg, dir.join("config.json"), deps);
    (TestApp { app, _lang: lang }, fake, task)
}

/// Una prueba de master: juego, master, clave de Steam y el hilo.
pub type ProbeCall = (&'static str, Master, String, ThreadId);

/// Una llamada al selector falso: `"save"` u `"open"`, el título y el hilo que la hizo.
pub type FileCall = (&'static str, String, ThreadId);

/// Selector de archivos falso: devuelve `save`/`open` y anota cada llamada.
#[derive(Clone, Default)]
pub struct FakeFiles {
    pub save: Arc<Mutex<Option<PathBuf>>>,
    pub open: Arc<Mutex<Option<PathBuf>>>,
    /// Lo que devuelve `pick_folder` (`"folder"` en `calls`) y la carpeta de inicio de cada uno.
    pub folder: Arc<Mutex<Option<PathBuf>>>,
    pub folder_starts: Arc<Mutex<Vec<String>>>,
    pub calls: Arc<Mutex<Vec<FileCall>>>,
}

impl FileDialogs for FakeFiles {
    fn save_json(&self, title: &str, _filter: &str, _file_name: &str) -> Option<PathBuf> {
        let call = ("save", title.to_owned(), std::thread::current().id());
        self.calls.lock().unwrap().push(call);
        self.save.lock().unwrap().clone()
    }

    fn open_json(&self, title: &str, _filter: &str) -> Option<PathBuf> {
        let call = ("open", title.to_owned(), std::thread::current().id());
        self.calls.lock().unwrap().push(call);
        self.open.lock().unwrap().clone()
    }

    fn pick_folder(&self, title: &str, start: &str) -> Option<PathBuf> {
        let call = ("folder", title.to_owned(), std::thread::current().id());
        self.calls.lock().unwrap().push(call);
        self.folder_starts.lock().unwrap().push(start.to_owned());
        self.folder.lock().unwrap().clone()
    }
}

impl FakeFiles {
    pub fn calls(&self) -> Vec<FileCall> {
        self.calls.lock().unwrap().clone()
    }
}

/// Llavero en memoria: anota cada llamada y el hilo que la hizo.
#[derive(Clone, Default)]
pub struct FakeKeyring {
    pub entries: Arc<Mutex<BTreeMap<String, String>>>,
    pub calls: Arc<Mutex<Vec<(String, ThreadId)>>>,
    /// `Some` = guardar (no borrar) falla con ese error, como `BrokenSetKeyring`.
    pub set_error: Arc<Mutex<Option<String>>>,
}

impl Keyring for FakeKeyring {
    fn get(&self, user: &str) -> Option<String> {
        self.note(user);
        self.entries.lock().unwrap().get(user).cloned()
    }

    fn set(&self, user: &str, value: &str) -> Result<(), String> {
        self.note(user);
        if let Some(e) = self.set_error.lock().unwrap().clone()
            && !value.is_empty()
        {
            return Err(e);
        }
        let mut entries = self.entries.lock().unwrap();
        if value.is_empty() {
            entries.remove(user);
        } else {
            entries.insert(user.to_owned(), value.to_owned());
        }
        Ok(())
    }
}

impl FakeKeyring {
    fn note(&self, user: &str) {
        let call = (user.to_owned(), std::thread::current().id());
        self.calls.lock().unwrap().push(call);
    }

    pub fn calls(&self) -> Vec<(String, ThreadId)> {
        self.calls.lock().unwrap().clone()
    }
}

/// El icono que da el falso para `q3`: 16×16 opaco.
pub fn exe_icon() -> iced::widget::image::Handle {
    iced::widget::image::Handle::from_rgba(16, 16, vec![200u8; 16 * 16 * 4])
}

/// Reloj falso de `Deps.now`: segundos Unix y desfase local (por defecto `T0` en UTC).
#[derive(Clone)]
pub struct Clock(pub Arc<Mutex<(f64, i64)>>);

impl Default for Clock {
    fn default() -> Self {
        Clock(Arc::new(Mutex::new((1_700_000_000.0, 0))))
    }
}

impl Clock {
    pub fn set(&self, unix: f64, offset_s: i64) {
        *self.0.lock().unwrap() = (unix, offset_s);
    }
}

/// Una llamada al servicio de imágenes: juego, carpeta, mapa y «descargar».
pub type MapCall = (&'static str, Option<String>, String, bool);

/// `MapImageService` falso (el `FakeMaps` de Python): `paths` = mapa → PNG; los mapas de
/// `blocked` esperan en la barrera hasta `open`. Cuenta llamadas, hilos y concurrencia.
#[derive(Clone, Default)]
pub struct FakeMaps {
    pub paths: Arc<Mutex<HashMap<String, PathBuf>>>,
    pub blocked: Arc<Mutex<HashSet<String>>>,
    calls: Arc<(Mutex<Vec<MapCall>>, Condvar)>,
    pub threads: Arc<Mutex<Vec<ThreadId>>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    running: Arc<AtomicUsize>,
    pub max_running: Arc<AtomicUsize>,
    pub done: Arc<Mutex<Vec<String>>>,
}

impl FakeMaps {
    pub fn get(
        &self,
        game: &'static str,
        dir: Option<String>,
        map: String,
        dl: bool,
    ) -> Option<PathBuf> {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_running.fetch_max(now, Ordering::SeqCst);
        {
            let (calls, cv) = &*self.calls;
            calls.lock().unwrap().push((game, dir, map.clone(), dl));
            cv.notify_all();
        }
        if self.blocked.lock().unwrap().contains(&map) {
            let (open, cv) = &*self.gate;
            let _open = cv.wait_while(open.lock().unwrap(), |o| !*o).unwrap();
        }
        self.running.fetch_sub(1, Ordering::SeqCst);
        self.done.lock().unwrap().push(map.clone());
        self.paths.lock().unwrap().get(&map).cloned()
    }

    pub fn calls(&self) -> Vec<MapCall> {
        self.calls.0.lock().unwrap().clone()
    }

    pub fn maps_called(&self) -> Vec<String> {
        self.calls().into_iter().map(|c| c.2).collect()
    }

    /// Espera (con un plazo amplio) a que haya `n` llamadas.
    pub fn wait_calls(&self, n: usize) {
        assert!(
            self.calls_within(n, Duration::from_secs(60)),
            "esperaba {n} llamadas al servicio de imágenes"
        );
    }

    /// ¿Llegó a haber `n` llamadas antes de `limit`? Para comprobar que algo NO llama: el
    /// plazo solo da tiempo a fallar, nunca hace fallar a lo correcto.
    pub fn calls_within(&self, n: usize, limit: Duration) -> bool {
        let (calls, cv) = &*self.calls;
        let (_calls, timeout) = cv
            .wait_timeout_while(calls.lock().unwrap(), limit, |c| c.len() < n)
            .unwrap();
        !timeout.timed_out()
    }

    pub fn block(&self, map: &str) {
        self.blocked.lock().unwrap().insert(map.to_owned());
    }

    pub fn open(&self) {
        let (open, cv) = &*self.gate;
        *open.lock().unwrap() = true;
        cv.notify_all();
    }
}

impl FakeScan {
    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }

    pub fn installed_calls(&self) -> Vec<&'static str> {
        self.installed.lock().unwrap().clone()
    }

    pub fn exe_icon_calls(&self) -> Vec<&'static str> {
        self.exe_icons.lock().unwrap().clone()
    }

    pub fn lan_calls(&self) -> Vec<&'static str> {
        self.lan.lock().unwrap().clone()
    }

    pub fn queries(&self) -> Vec<(&'static str, ServerAddress)> {
        self.queries.lock().unwrap().clone()
    }

    pub fn steam_keys(&self) -> Vec<String> {
        self.steam_keys.lock().unwrap().clone()
    }

    pub fn launches(&self) -> Vec<LaunchArgs> {
        self.launches.lock().unwrap().clone()
    }

    pub fn fail_launch(&self, error: &str) {
        *self.launch_error.lock().unwrap() = Some(error.to_owned());
    }

    pub fn answer(&self, answer: Answer) {
        *self.answer.lock().unwrap() = Some(answer);
    }

    /// Lo que responde `address` (`None` = no respondió).
    pub fn answer_for(&self, address: &ServerAddress, answer: Option<ServerInfo>) {
        let mut answers = self.answers.lock().unwrap();
        answers.insert(address.clone(), Ok(answer));
    }
}

pub fn fake_deps(fake: &FakeScan, steam_key: &str) -> Deps {
    let (calls, exits) = (fake.calls.clone(), fake.exits.clone());
    let installed = fake.installed.clone();
    let lan = fake.lan.clone();
    let (queries, answer) = (fake.queries.clone(), fake.answer.clone());
    let answers = fake.answers.clone();
    let (running, max) = (fake.query_running.clone(), fake.query_max.clone());
    let maps = fake.maps.clone();
    let clock = fake.clock.clone();
    let exe_icons = fake.exe_icons.clone();
    let steam_keys = fake.steam_keys.clone();
    let (launches, launch_error) = (fake.launches.clone(), fake.launch_error.clone());
    let (detect, detect_calls) = (fake.detect.clone(), fake.detect_calls.clone());
    let (exes, installed_at) = (fake.exes.clone(), fake.installed_at.clone());
    let (probes, probe_answer) = (fake.probes.clone(), fake.probe_answer.clone());
    let opened = fake.opened.clone();
    let find = fake.find.clone();
    let (body, chunk, streams) = (
        fake.stream_body.clone(),
        fake.stream_chunk.clone(),
        fake.streams.clone(),
    );
    let (spawned, spawn_fails) = (fake.spawned.clone(), fake.spawn_fails.clone());
    let scripts = fake.scripts.clone();
    let rcon = fake.rcon.as_fn();
    if !steam_key.is_empty() {
        let user = jase::passwords::STEAM_KEY_USER.to_owned();
        fake.keyring
            .entries
            .lock()
            .unwrap()
            .insert(user, steam_key.to_owned());
    }
    Deps {
        scan: Arc::new(move |game, _masters, opts| {
            calls.lock().unwrap().push(game.id);
            steam_keys.lock().unwrap().push(opts.steam_key);
            let Some((servers, masters_ok)) = scripts.lock().unwrap().get(game.id).cloned() else {
                return futures::stream::empty().boxed();
            };
            let done = ScanResult {
                masters_ok,
                servers_listed: servers.len(),
                servers_responded: servers.len(),
                ..ScanResult::default()
            };
            let events = servers
                .into_iter()
                .map(|info| ScanMsg::Event(ScanEvent::Server(info)));
            futures::stream::iter(events.chain([ScanMsg::Finished(done)])).boxed()
        }),
        exit: Arc::new(move |code| exits.lock().unwrap().push(code)),
        keyring: Arc::new(fake.keyring.clone()),
        launch: Arc::new(move |args| {
            launches.lock().unwrap().push(args.clone());
            launch_error.lock().unwrap().clone().map_or(Ok(()), Err)
        }),
        country_db: None,
        game_installed: Arc::new(move |game, dir| {
            installed.lock().unwrap().push(game.id);
            let call = (dir.to_owned(), std::thread::current().id());
            installed_at.lock().unwrap().push(call);
            let exe = exes.lock().unwrap().get(game.id).cloned();
            exe.filter(|e| Path::new(e).parent() == Some(Path::new(dir)))
        }),
        lan: Arc::new(move |game| {
            lan.lock().unwrap().push(game.id);
            futures::stream::empty().boxed()
        }),
        query: Arc::new(move |game, address| {
            queries.lock().unwrap().push((game.id, address.clone()));
            let by_address = answers.lock().unwrap().get(&address).cloned();
            let answer = by_address
                .or_else(|| answer.lock().unwrap().clone())
                .unwrap_or(Ok(None));
            let (running, max) = (running.clone(), max.clone());
            Box::pin(async move {
                // Cede una vez: las consultas lanzadas a la vez se ven en curso juntas.
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                max.fetch_max(now, Ordering::SeqCst);
                tokio::task::yield_now().await;
                running.fetch_sub(1, Ordering::SeqCst);
                answer
            })
        }),
        map_image: Arc::new(move |game, dir, map, dl| maps.get(game.id, dir, map, dl)),
        now: Arc::new(move || *clock.0.lock().unwrap()),
        exe_icon: Arc::new(move |game, _dir| {
            exe_icons.lock().unwrap().push(game.id);
            (game.id == "q3").then(exe_icon)
        }),
        detect: Arc::new(move |address| {
            detect_calls.lock().unwrap().push(address);
            Box::pin(futures::future::ready(detect.lock().unwrap().clone()))
        }),
        files: Arc::new(fake.files.clone()),
        link_endpoint: None,
        probe: Some(Arc::new(move |game, master, key| {
            let call = (game.id, master, key, std::thread::current().id());
            probes.lock().unwrap().push(call);
            Box::pin(futures::future::ready(*probe_answer.lock().unwrap()))
        })),
        open_url: Arc::new(move |url| {
            let call = (url.to_owned(), std::thread::current().id());
            opened.lock().unwrap().push(call);
            Ok(())
        }),
        find_games: Arc::new(move |extra, _cancelled| find.find(extra)),
        masters_fetch: None,
        update_fetch: None,
        version: "1.3.0",
        update_stream: Arc::new(move |url| {
            let call = (url.to_owned(), std::thread::current().id());
            streams.lock().unwrap().push(call);
            let Some(data) = body.lock().unwrap().clone() else {
                return Err(DownloadError {
                    code: DownloadCode::Network,
                    detail: "sin red".into(),
                });
            };
            // 0 = de a 64 KiB, como `CHUNK_BYTES`.
            let chunk = match *chunk.lock().unwrap() {
                0 => 64 * 1024,
                n => n,
            };
            Ok(Box::new(Chunks {
                data,
                pos: 0,
                chunk,
            }) as Box<dyn std::io::Read + Send>)
        }),
        spawn: Arc::new(move |program, args| {
            let call = (
                program.to_owned(),
                args.to_vec(),
                std::thread::current().id(),
            );
            spawned.lock().unwrap().push(call);
            if *spawn_fails.lock().unwrap() {
                return Err(std::io::Error::other("no se pudo"));
            }
            Ok(())
        }),
        self_update: SelfUpdate::Unsupported,
        rcon,
    }
}

/// App con ese `update_fetch` (`None` = sin red) y esa forma de actualizarse, con la `Task`
/// del arranque.
pub fn update_app(
    dir: &Path,
    cfg: Config,
    fetch: Option<&FakeFetch>,
    self_update: SelfUpdate,
) -> (TestApp, FakeScan, Task<Message>) {
    let lang = lang::Spanish::hold();
    let fake = FakeScan::default();
    let mut deps = fake_deps(&fake, "");
    deps.update_fetch = fetch.map(FakeFetch::as_fn);
    deps.self_update = self_update;
    let (app, task) = App::new(cfg, dir.join("config.json"), deps);
    (TestApp { app, _lang: lang }, fake, task)
}

/// `App` de prueba: mantiene el candado del idioma (español) y, al soltarse, espera al hilo
/// escritor de listas antes de que el test borre su carpeta (si no, la recrearía).
pub struct TestApp {
    app: App,
    _lang: lang::Spanish,
}

impl TestApp {
    /// Para los tests que arman sus propias `Deps` y llaman a `App::new` ellos mismos.
    pub fn new(app: App) -> TestApp {
        TestApp {
            app,
            _lang: lang::Spanish::hold(),
        }
    }
}

impl Deref for TestApp {
    type Target = App;
    fn deref(&self) -> &App {
        &self.app
    }
}

impl DerefMut for TestApp {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.app
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.app.saved.flush();
    }
}

/// App con la config en `dir/config.json`; arranca con el juego inicial (cod2).
pub fn fake_app(dir: &Path, cfg: Config) -> (TestApp, FakeScan) {
    let (app, fake, _task) = fake_app_task(dir, cfg);
    (app, fake)
}

/// Como [`fake_app`], con la `Task` del arranque (escaneo, base de países, exes).
pub fn fake_app_task(dir: &Path, cfg: Config) -> (TestApp, FakeScan, Task<Message>) {
    fake_app_with_key(dir, cfg, "")
}

/// Como [`fake_app_task`], con esa clave de Steam en el llavero falso.
pub fn fake_app_with_key(
    dir: &Path,
    cfg: Config,
    steam_key: &str,
) -> (TestApp, FakeScan, Task<Message>) {
    // El candado va antes de crear el `App`: su arranque ya traduce texto.
    let lang = lang::Spanish::hold();
    let fake = FakeScan::default();
    let (app, task) = App::new(cfg, dir.join("config.json"), fake_deps(&fake, steam_key));
    (TestApp { app, _lang: lang }, fake, task)
}

/// Tokio como el de Iced (por `spawn_blocking`), con el reloj en pausa: los plazos de la barra
/// de estado vencen al instante en vez de hacer esperar al test.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
}

/// Corre la `Task` como el runtime de Iced (con tokio, por `spawn_blocking`) y devuelve los
/// mensajes que produce; las operaciones de widgets se descartan.
pub fn run_task(task: Task<Message>) -> Vec<Message> {
    run_task_with(task, None)
}

/// Campo de texto falso para `operation::is_focused`.
struct FakeField(bool);

impl Focusable for FakeField {
    fn is_focused(&self) -> bool {
        self.0
    }
    fn focus(&mut self) {
        self.0 = true;
    }
    fn unfocus(&mut self) {
        self.0 = false;
    }
}

/// Como [`run_task`], y las operaciones de widgets ven el campo del filtro (`FILTER_ID`) con
/// o sin foco, como si la vista lo tuviera.
pub fn run_task_with(task: Task<Message>, filter_focused: Option<bool>) -> Vec<Message> {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    runtime().block_on(
        stream
            .filter_map(|action| async move {
                match action {
                    Action::Output(m) => Some(m),
                    Action::Widget(mut op) => {
                        if let Some(focused) = filter_focused {
                            let mut field = FakeField(focused);
                            op.focusable(Some(&FILTER_ID), Rectangle::default(), &mut field);
                            // `finish` entrega la respuesta por el canal de la `Task`.
                            let _ = op.finish();
                        }
                        None
                    }
                    _ => None,
                }
            })
            .collect(),
    )
}

/// Lo que la `Task` escribe en el portapapeles (`clipboard::write`), sin correr lo demás.
pub fn clipboard_writes(task: Task<Message>) -> Vec<String> {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    runtime().block_on(
        stream
            .filter_map(|action| async move {
                match action {
                    Action::Clipboard(iced_runtime::clipboard::Action::Write {
                        contents, ..
                    }) => Some(contents),
                    _ => None,
                }
            })
            .collect(),
    )
}

/// Como [`run_task`], y cada lectura del portapapeles recibe `clip`.
pub fn run_task_with_clipboard(task: Task<Message>, clip: Option<&str>) -> Vec<Message> {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let clip = clip.map(str::to_owned);
    runtime().block_on(
        stream
            .filter_map(|action| {
                let clip = clip.clone();
                async move {
                    match action {
                        Action::Output(m) => Some(m),
                        Action::Clipboard(iced_runtime::clipboard::Action::Read {
                            channel,
                            ..
                        }) => {
                            let _ = channel.send(clip);
                            None
                        }
                        _ => None,
                    }
                }
            })
            .collect(),
    )
}

/// Cuántas lecturas del portapapeles (`clipboard::read`) pide la `Task`; no se contestan.
pub fn clipboard_reads(task: Task<Message>) -> usize {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return 0;
    };
    runtime().block_on(
        stream
            .filter(|action| {
                futures::future::ready(matches!(
                    action,
                    Action::Clipboard(iced_runtime::clipboard::Action::Read { .. })
                ))
            })
            .count(),
    )
}

/// Campo falso que anota lo que le hacen las operaciones (`focus`, `select_all`).
#[derive(Default)]
struct RecField(Vec<&'static str>);

impl Focusable for RecField {
    fn is_focused(&self) -> bool {
        false
    }
    fn focus(&mut self) {
        self.0.push("focus");
    }
    fn unfocus(&mut self) {
        self.0.push("unfocus");
    }
}

impl TextInput for RecField {
    fn text(&self) -> &str {
        ""
    }
    fn move_cursor_to_front(&mut self) {}
    fn move_cursor_to_end(&mut self) {}
    fn move_cursor_to(&mut self, _: usize) {}
    fn select_all(&mut self) {
        self.0.push("select_all");
    }
    fn select_range(&mut self, _: usize, _: usize) {}
}

/// Lo que las operaciones de la `Task` le hacen al campo del filtro (`FILTER_ID`).
pub fn filter_ops(task: Task<Message>) -> Vec<&'static str> {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let ops: Vec<Vec<&'static str>> = runtime().block_on(
        stream
            .filter_map(|action| async move {
                let Action::Widget(mut op) = action else {
                    return None;
                };
                let mut field = RecField::default();
                op.text_input(Some(&FILTER_ID), Rectangle::default(), &mut field);
                op.focusable(Some(&FILTER_ID), Rectangle::default(), &mut field);
                Some(field.0)
            })
            .collect(),
    );
    ops.concat()
}

/// Cuántas peticiones a la ventana (`window::latest`, `is_maximized`…) lleva la `Task`; el
/// runner de los tests no las contesta.
pub fn window_requests(task: Task<Message>) -> usize {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return 0;
    };
    runtime().block_on(
        stream
            .filter(|action| futures::future::ready(matches!(action, Action::Window(_))))
            .count(),
    )
}

/// Cuántas operaciones de widgets (`scroll_to`, foco…) lleva la `Task`; no se contestan.
pub fn widget_ops(task: Task<Message>) -> usize {
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return 0;
    };
    runtime().block_on(
        stream
            .filter(|action| futures::future::ready(matches!(action, Action::Widget(_))))
            .count(),
    )
}

/// Las ventanas de herramientas abiertas.
pub fn tool_ids(app: &App) -> Vec<iced::window::Id> {
    app.tool_windows().into_iter().map(|(id, _)| id).collect()
}

/// Las peticiones a ventanas de la `Task` (sin contestar): qué piden y a cuál
/// (`"latest"` no lleva id: elige la última abierta).
pub fn window_actions(task: Task<Message>) -> Vec<(&'static str, Option<iced::window::Id>)> {
    use iced_runtime::window::Action as W;
    let Some(stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    runtime().block_on(
        stream
            .filter_map(|action| async move {
                let Action::Window(w) = action else {
                    return None;
                };
                Some(match w {
                    W::Open(id, ..) => ("open", Some(id)),
                    W::Close(id) => ("close", Some(id)),
                    W::GainFocus(id) => ("focus", Some(id)),
                    W::Minimize(id, _) => ("minimize", Some(id)),
                    W::GetMaximized(id, _) => ("maximized", Some(id)),
                    W::GetMinimized(id, _) => ("minimized", Some(id)),
                    W::GetLatest(_) => ("latest", None),
                    _ => ("other", None),
                })
            })
            .collect(),
    )
}

/// Los textos (`text` y campos, ya medidos) de la vista armada sin ventana.
pub fn view_texts(app: &App) -> Vec<String> {
    element_texts(app.view())
}

/// Como [`view_texts`], de cualquier vista (p. ej. una ventana de herramienta).
pub fn element_texts(element: iced::Element<'_, Message>) -> Vec<String> {
    use iced::advanced::widget::Operation;
    use iced::advanced::widget::operation::TextInput;
    struct Collect(Vec<String>);
    impl Operation for Collect {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
            operate(self);
        }
        fn text(&mut self, _: Option<&iced::widget::Id>, _: iced::Rectangle, text: &str) {
            self.0.push(text.to_owned());
        }
        fn text_input(
            &mut self,
            _: Option<&iced::widget::Id>,
            _: iced::Rectangle,
            state: &mut dyn TextInput,
        ) {
            self.0.push(state.text().to_owned());
        }
    }
    let mut renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
        iced::Font::DEFAULT,
        iced::Pixels(13.0),
    ));
    let mut ui = iced_runtime::UserInterface::<Message, iced::Theme, iced::Renderer>::build(
        element,
        iced::Size::new(1280.0, 800.0),
        iced_runtime::user_interface::Cache::default(),
        &mut renderer,
    );
    let mut op = Collect(Vec::new());
    ui.operate(&renderer, &mut op);
    op.0
}
