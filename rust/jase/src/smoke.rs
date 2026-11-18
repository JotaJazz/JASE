//! Autocomprobación `--smoke` (`smoke.py`): abre la ventana con 5.000 servidores falsos, sin red
//! ni la config del usuario, y sale por el camino normal del cierre.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use iced::{Element, Subscription, Task, Theme, window};
use jase_core::config::Config;
use jase_core::i18n::{set_language, tr};
use jase_core::masters::bundled_official;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};

use crate::app::{App, Message};
use crate::scan::{Deps, ScanMsg};
use crate::startup::APP_NAME;
use crate::theme::{VARIANTS, palette};

const SERVERS: usize = 5000;

#[allow(clippy::large_enum_variant)] // como `Message`
#[derive(Debug, Clone)]
enum Msg {
    App(Message),
    Frame,
}

struct Smoke {
    app: App,
    checked: bool,
    closing: bool,
}

fn fail(error: &str) -> ! {
    println!("JASE smoke FAIL: {error}");
    let _ = std::fs::remove_dir_all(smoke_dir());
    std::process::exit(1)
}

fn servers() -> Vec<ScanMsg> {
    let mut msgs: Vec<ScanMsg> = (0..SERVERS)
        .map(|i| {
            let a = ServerAddress::new(format!("10.0.{}.{}", i / 256, i % 256), 28960);
            let mut info = ServerInfo::new(a.clone(), a);
            info.name = format!("^1Smoke ^7{i}");
            info.map = "mp_toujane".into();
            info.num_players = (i % 20) as i32;
            info.max_players = 20;
            info.ping_ms = (i % 300) as u32;
            info.version = "1.3".into(); // la de CoD2 por defecto: el filtro de versión los deja
            ScanMsg::Event(ScanEvent::Server(info))
        })
        .collect();
    msgs.push(ScanMsg::Finished(ScanResult {
        masters_ok: 1,
        servers_listed: SERVERS,
        servers_responded: SERVERS,
        errors: vec![],
    }));
    msgs
}

fn checks() -> Result<(), String> {
    set_language("en").map_err(|e| format!("{e:?}"))?;
    if tr("Refrescar lista") != "Refresh list" {
        return Err("el catálogo inglés no traduce «Refrescar lista»".into());
    }
    for (id, p, _) in &VARIANTS {
        if !std::ptr::eq(palette(id), p) {
            return Err(format!("falta la paleta del tema {id}"));
        }
    }
    let official = bundled_official().map_err(|e| e.to_string())?;
    if official.updated.is_empty() {
        return Err("masters.json empaquetado sin fecha".into());
    }
    Ok(())
}

impl Smoke {
    fn boot() -> (Smoke, Task<Msg>) {
        let deps = Deps {
            scan: Arc::new(|_, _, _| futures::stream::iter(servers()).boxed()),
            // El mismo `process::exit` del cierre real: no espera a la tarea colgada.
            exit: Arc::new(move |code| {
                let _ = std::fs::remove_dir_all(smoke_dir());
                println!("JASE smoke OK {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(code)
            }),
            steam_key: String::new(),
            // Sin base de países ni disco: nada que esperar ni leer del usuario.
            country_db: None,
            game_installed: Arc::new(|_, _| true),
            // Sin red: ni red local ni consultas.
            lan: Arc::new(|_| futures::stream::empty().boxed()),
            query: Arc::new(|_, _| Box::pin(futures::future::ready(Ok(None)))),
        };
        let (app, task) = App::new(Config::default(), smoke_dir().join("config.json"), deps);
        let smoke = Smoke {
            app,
            checked: false,
            closing: false,
        };
        (smoke, task.map(Msg::App))
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        if let Msg::App(m) = msg {
            return self.app.update(m).map(Msg::App);
        }
        if !self.checked {
            self.checked = true;
            if let Err(e) = checks() {
                fail(&e);
            }
            // La tarea que el cierre no debe esperar (mucho más larga que el plazo del test).
            drop(tokio::task::spawn_blocking(|| {
                std::thread::sleep(Duration::from_secs(3600))
            }));
        }
        // Con un cuadro ya dibujado y la lista completa.
        if self.closing || self.app.is_scanning() {
            return Task::none();
        }
        self.closing = true;
        let rows = self.app.servers().len();
        if rows != SERVERS {
            fail(&format!("{rows} filas en vez de {SERVERS}"));
        }
        self.app.update(Message::CloseRequested).map(Msg::App)
    }

    fn view(&self) -> Element<'_, Msg> {
        self.app.view().map(Msg::App)
    }

    fn theme(&self) -> Theme {
        self.app.theme()
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            self.app.subscription().map(Msg::App),
            window::frames().map(|_| Msg::Frame),
        ])
    }
}

fn smoke_dir() -> PathBuf {
    std::env::temp_dir().join(format!("jase-smoke-{}", std::process::id()))
}

/// Sale con 0 si todo anduvo (lo imprime el cierre) o con 1 y el error.
pub fn run() -> ! {
    if let Err(e) = std::fs::create_dir_all(smoke_dir()) {
        fail(&e.to_string());
    }
    // winit entra en pánico si no hay pantalla: también es un fallo del smoke.
    let result = std::panic::catch_unwind(|| {
        iced::application(Smoke::boot, Smoke::update, Smoke::view)
            .title(APP_NAME)
            .settings(crate::settings())
            .window(crate::window_settings(&Config::default()))
            .theme(Smoke::theme)
            .subscription(Smoke::subscription)
            .run()
    });
    // Solo se llega aquí si la ventana no abrió o se cerró sin pasar por `CloseRequested`.
    match result {
        Ok(Err(e)) => fail(&e.to_string()),
        Ok(Ok(())) => fail("la ventana se cerró sin pasar por el cierre"),
        Err(panic) => fail(
            panic
                .downcast_ref::<String>()
                .map_or("pánico al abrir la ventana", String::as_str),
        ),
    }
}
