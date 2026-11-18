//! Ayudas compartidas: servidores de prueba y un escáner falso (como `FakeScan` de Python).
#![allow(dead_code)] // cada binario de test usa una parte

pub mod lang;

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

/// Anota los juegos pedidos; el stream no trae nada (los tests meten los mensajes a mano).
#[derive(Clone, Default)]
pub struct FakeScan {
    pub calls: Arc<Mutex<Vec<&'static str>>>,
    pub exits: Arc<Mutex<Vec<i32>>>,
    /// Juegos por los que se preguntó «¿está instalado?» (responde que no).
    pub installed: Arc<Mutex<Vec<&'static str>>>,
}

impl FakeScan {
    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }

    pub fn installed_calls(&self) -> Vec<&'static str> {
        self.installed.lock().unwrap().clone()
    }
}

pub fn fake_deps(fake: &FakeScan, steam_key: &str) -> Deps {
    let (calls, exits) = (fake.calls.clone(), fake.exits.clone());
    let installed = fake.installed.clone();
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
    }
}

/// App con la config en `dir/config.json`; arranca con el juego inicial (cod2).
pub fn fake_app(dir: &Path, cfg: Config) -> (App, FakeScan) {
    let (app, fake, _task) = fake_app_task(dir, cfg);
    (app, fake)
}

/// Como [`fake_app`], con la `Task` del arranque (escaneo, base de países, exes).
pub fn fake_app_task(dir: &Path, cfg: Config) -> (App, FakeScan, Task<Message>) {
    let fake = FakeScan::default();
    let (app, task) = App::new(cfg, dir.join("config.json"), fake_deps(&fake, ""));
    (app, fake, task)
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
