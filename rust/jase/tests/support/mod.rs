//! Ayudas compartidas: servidores de prueba y un escáner falso (como `FakeScan` de Python).
#![allow(dead_code)] // cada binario de test usa una parte

pub mod lang;

use std::collections::{BTreeMap, HashMap, HashSet};
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
use jase::app::{App, Message};
use jase::passwords::Keyring;
use jase::scan::Deps;
use jase::widgets::FILTER_ID;
use jase_core::config::Config;
use jase_core::models::{ServerAddress, ServerInfo};

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
    pub maps: FakeMaps,
    pub clock: Clock,
    /// Juegos por los que se pidió el icono del exe (solo `q3` lo tiene).
    pub exe_icons: Arc<Mutex<Vec<&'static str>>>,
    /// La clave de Steam que recibió cada escaneo.
    pub steam_keys: Arc<Mutex<Vec<String>>>,
    pub keyring: FakeKeyring,
}

/// Llavero en memoria: anota cada llamada y el hilo que la hizo.
#[derive(Clone, Default)]
pub struct FakeKeyring {
    pub entries: Arc<Mutex<BTreeMap<String, String>>>,
    pub calls: Arc<Mutex<Vec<(String, ThreadId)>>>,
}

impl Keyring for FakeKeyring {
    fn get(&self, user: &str) -> Option<String> {
        self.note(user);
        self.entries.lock().unwrap().get(user).cloned()
    }

    fn set(&self, user: &str, value: &str) -> Result<(), String> {
        self.note(user);
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

    pub fn answer(&self, answer: Answer) {
        *self.answer.lock().unwrap() = Some(answer);
    }
}

pub fn fake_deps(fake: &FakeScan, steam_key: &str) -> Deps {
    let (calls, exits) = (fake.calls.clone(), fake.exits.clone());
    let installed = fake.installed.clone();
    let lan = fake.lan.clone();
    let (queries, answer) = (fake.queries.clone(), fake.answer.clone());
    let maps = fake.maps.clone();
    let clock = fake.clock.clone();
    let exe_icons = fake.exe_icons.clone();
    let steam_keys = fake.steam_keys.clone();
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
            futures::stream::empty().boxed()
        }),
        exit: Arc::new(move |code| exits.lock().unwrap().push(code)),
        keyring: Arc::new(fake.keyring.clone()),
        country_db: None,
        game_installed: Arc::new(move |game, _dir| {
            installed.lock().unwrap().push(game.id);
            false
        }),
        lan: Arc::new(move |game| {
            lan.lock().unwrap().push(game.id);
            futures::stream::empty().boxed()
        }),
        query: Arc::new(move |game, address| {
            queries.lock().unwrap().push((game.id, address));
            let answer = answer.lock().unwrap().clone().unwrap_or(Ok(None));
            Box::pin(async move { answer })
        }),
        map_image: Arc::new(move |game, dir, map, dl| maps.get(game.id, dir, map, dl)),
        now: Arc::new(move || *clock.0.lock().unwrap()),
        exe_icon: Arc::new(move |game, _dir| {
            exe_icons.lock().unwrap().push(game.id);
            (game.id == "q3").then(exe_icon)
        }),
    }
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
