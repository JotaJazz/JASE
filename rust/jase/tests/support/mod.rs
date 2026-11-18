//! Ayudas compartidas: servidores de prueba y un escáner falso (como `FakeScan` de Python).
#![allow(dead_code)] // cada binario de test usa una parte

pub mod lang;

use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::advanced::widget::operation::Focusable;
use iced::{Rectangle, Task};
use iced_runtime::Action;
use jase::app::{App, Message};
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
}

impl FakeScan {
    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }

    pub fn installed_calls(&self) -> Vec<&'static str> {
        self.installed.lock().unwrap().clone()
    }

    pub fn lan_calls(&self) -> Vec<&'static str> {
        self.lan.lock().unwrap().clone()
    }

    pub fn queries(&self) -> Vec<(&'static str, ServerAddress)> {
        self.queries.lock().unwrap().clone()
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
    Deps {
        scan: Arc::new(move |game, _masters, _opts| {
            calls.lock().unwrap().push(game.id);
            futures::stream::empty().boxed()
        }),
        exit: Arc::new(move |code| exits.lock().unwrap().push(code)),
        steam_key: steam_key.to_owned(),
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
    // El candado va antes de crear el `App`: su arranque ya traduce texto.
    let lang = lang::Spanish::hold();
    let fake = FakeScan::default();
    let (app, task) = App::new(cfg, dir.join("config.json"), fake_deps(&fake, ""));
    (TestApp { app, _lang: lang }, fake, task)
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
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(
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
    futures::executor::block_on(
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
