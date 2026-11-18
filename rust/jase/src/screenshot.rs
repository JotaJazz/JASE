//! Capturas `--screenshot <carpeta>` (`scripts/screenshot.py`): la ventana de 1280×800 con los
//! mismos servidores de ejemplo inventados, en inglés y sin red ni la config del usuario, y un
//! `<variante>.png` por cada tema de `VARIANTS`; luego `<variante>-password.png` con el diálogo
//! de contraseña del `SELECTED_HOST` abierto (guardada `"secreto"`).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use iced::widget::operation;
use iced::window::{self, Screenshot};
use iced::{Element, Size, Subscription, Task, Theme};
use jase_core::config::Config;
use jase_core::i18n::set_language;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::protocols::quake3::strip_colors;
use jase_core::scanner::scan::{ScanEvent, ScanResult};

use crate::app::{App, Message, PROMPT_ID};
use crate::details::{PlayerCol, PlayerSort};
use crate::passwords::Keyring;
use crate::scan::{Deps, ScanMsg};
use crate::startup::APP_NAME;
use crate::table::Column;
use crate::theme::VARIANTS;
use crate::tree::{Kind, Node};

const GAME: &str = "cod2";
const SIZE: Size = Size::new(1280.0, 800.0);
pub const SELECTED_HOST: &str = "192.0.2.14";
/// Cuadros dibujados tras cambiar de tema antes de capturar: en el mismo cuadro sale sin texto.
const SETTLE_FRAMES: u32 = 2;

type Row = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    i32,
    i32,
    u32,
    &'static str,
    &'static str,
);

/// host, nombre, mapa, gametype, jugadores, máx, ping, mod, país (`_SERVERS` de Python).
#[rustfmt::skip]
const SERVERS: [Row; 13] = [
    ("192.0.2.14", "^2[NOX] ^7Clan Nox | Search & Destroy | pam", "mp_toujane", "sd", 8, 12, 34, "pam", "cl"),
    ("192.0.2.31", "Iron Wolves Public Server", "mp_carentan", "war", 14, 20, 58, "", "us"),
    ("198.51.100.7", "Feldgrau Deutsch | TDM 24/7", "mp_burgundy", "war", 9, 16, 112, "", "de"),
    ("198.51.100.22", "Zombie Farmhouse | Survive the Night", "mp_farmhouse", "dm", 11, 24, 187, "zombie", "br"),
    ("203.0.113.5", "Los Cuervos | CTF | PAM 4.x", "mp_breakout", "ctf", 6, 14, 143, "pam", "es"),
    ("203.0.113.19", "Maquis FR | Recherche et Destruction", "mp_matmata", "sd", 10, 12, 96, "pam", "fr"),
    ("192.0.2.58", "Red Ball Express | Headquarters", "mp_dawnville", "hq", 3, 18, 71, "", "us"),
    ("198.51.100.41", "Pampa Gamers | DM abierto", "mp_toujane", "dm", 5, 16, 29, "", "cl"),
    ("203.0.113.77", "Panzerfaust Alley | Hardcore", "mp_carentan", "war", 12, 20, 208, "hardcore", "de"),
    ("192.0.2.90", "Night Shift Zombies [Co-op]", "mp_burgundy", "dm", 7, 20, 134, "zombie", "br"),
    ("198.51.100.64", "Veteranos SD | Sin lag | PAM", "mp_decoy", "sd", 12, 12, 52, "pam", "es"),
    ("203.0.113.101", "Les Loups de Verdun | TDM", "mp_toujane", "war", 0, 16, 98, "", "fr"),
    ("192.0.2.120", "Cold Steel Gaming | FFA", "mp_railyard", "dm", 2, 14, 163, "", "us"),
];

const PLAYERS: [(&str, i32, i32); 8] = [
    ("^1Sgt.Pepper", 42, 31),
    ("^7Bruma", 37, 45),
    ("^2Kestrel", 31, 28),
    ("^5Tovarich", 24, 77),
    ("^7Mosquito", 19, 52),
    ("^3Hans_G", 15, 90),
    ("^7Capi", 9, 38),
    ("^4Lobo", 4, 120),
];

/// Servidores de ejemplo de CoD2: nombres de clan inventados, IPs de documentación (RFC 5737).
pub fn sample_servers() -> Vec<ServerInfo> {
    SERVERS
        .iter()
        .map(|&(host, name, map, gt, n, cap, ping, md, _)| {
            let addr = ServerAddress::new(host, 28960);
            let mut info = ServerInfo::new(addr.clone(), addr);
            info.players = if host == SELECTED_HOST {
                PLAYERS[..n as usize]
                    .iter()
                    .map(|&(raw, score, p)| Player {
                        raw_name: raw.to_owned(),
                        ..Player::new(strip_colors(raw), score, p)
                    })
                    .collect()
            } else {
                (0..n)
                    .map(|i| {
                        Player::new(format!("Player{}", i + 1), (20 - 3 * i).max(0), 40 + 7 * i)
                    })
                    .collect()
            };
            let fs_game = if md.is_empty() {
                String::new()
            } else {
                format!("mods/{md}")
            };
            info.rules = [
                ("fs_game", fs_game),
                ("sv_maxclients", cap.to_string()),
                ("shortversion", "1.3".into()),
                ("g_gametype", gt.into()),
                ("sv_pure", "1".into()),
                ("mapname", map.into()),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
            info.name = strip_colors(name);
            info.raw_name = name.to_owned();
            info.map = map.into();
            info.gametype = gt.into();
            info.num_players = n;
            info.max_players = cap;
            info.version = "1.3".into();
            info.ping_ms = ping;
            info
        })
        .collect()
}

/// El llavero de las capturas: siempre `"secreto"` guardada (`PasswordDialog(…, "secreto")`).
struct SavedSecret;

impl Keyring for SavedSecret {
    fn get(&self, _user: &str) -> Option<String> {
        Some("secreto".to_owned())
    }

    fn set(&self, _user: &str, _value: &str) -> Result<(), String> {
        Err("no keyring".to_owned())
    }
}

/// La base de países falsa (`_COUNTRY`).
fn country(host: &str) -> Option<String> {
    SERVERS.iter().find(|r| r.0 == host).map(|r| r.8.to_owned())
}

/// RGBA de 8 bits, como lo da `window::screenshot`.
pub fn write_png(path: &Path, shot: &Screenshot) -> io::Result<()> {
    let file = io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, shot.size.width, shot.size.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&shot.rgba)?;
    writer.finish()?;
    Ok(())
}

#[derive(Debug, Clone)]
enum Msg {
    App(Box<Message>),
    Frame,
    Shot(Screenshot),
    Written(Result<(), String>),
}

fn app_msg(m: Message) -> Msg {
    Msg::App(Box::new(m))
}

struct Shots {
    app: App,
    out: PathBuf,
    /// Índice en `VARIANTS` del tema que se captura; `None` mientras llega la lista.
    current: Option<usize>,
    frames: u32,
    busy: bool,
    /// Segunda vuelta: los mismos temas con el diálogo de contraseña.
    password: bool,
}

fn fail(error: &str) -> ! {
    println!("JASE screenshot FAIL: {error}");
    let _ = std::fs::remove_dir_all(config_dir());
    std::process::exit(1)
}

fn scan_msgs() -> Vec<ScanMsg> {
    let servers = sample_servers();
    let n = servers.len();
    let mut msgs: Vec<ScanMsg> = servers
        .into_iter()
        .map(|s| ScanMsg::Event(ScanEvent::Server(s)))
        .collect();
    msgs.push(ScanMsg::Finished(ScanResult {
        masters_ok: 1,
        servers_listed: n,
        servers_responded: n,
        errors: vec![],
    }));
    msgs
}

impl Shots {
    fn boot(out: PathBuf) -> (Shots, Task<Msg>) {
        if let Err(e) = set_language("en") {
            fail(&format!("{e:?}"));
        }
        let deps = Deps {
            scan: Arc::new(|_, _, _| futures::stream::iter(scan_msgs()).boxed()),
            exit: Arc::new(move |code| {
                let _ = std::fs::remove_dir_all(config_dir());
                println!("JASE screenshot OK {}", 2 * VARIANTS.len());
                std::process::exit(code)
            }),
            // Nunca el llavero del usuario.
            keyring: Arc::new(SavedSecret),
            // Nunca se lanza un juego del usuario.
            launch: Arc::new(|_| Err("no launch".to_owned())),
            // Sin base de países ni disco: nada que esperar ni leer del usuario.
            country_db: None,
            game_installed: Arc::new(|_, _| true),
            // Sin imágenes de mapas: el recuadro con el nombre (`_FakeMaps`).
            map_image: Arc::new(|_, _, _, _| None),
            now: Arc::new(crate::scan::system_now),
            // Sin icono de exe: la insignia (`exe_icon_fn=lambda p: None`).
            exe_icon: Arc::new(|_, _| None),
            // Sin red: ni red local ni consultas.
            lan: Arc::new(|_| futures::stream::empty().boxed()),
            query: Arc::new(|_, _| Box::pin(futures::future::ready(Ok(None)))),
            // «Detectar» no responde: la captura queda con «Detectando…».
            detect: Arc::new(|_| Box::pin(futures::future::pending())),
            files: Arc::new(crate::files::NoFiles),
            link_endpoint: None,
        };
        let mut cfg = Config {
            // Sin «system»: el modo del sistema no debe cambiar el tema entre capturas.
            theme: "dark".into(),
            language: "en".into(),
            ..Config::default()
        };
        cfg.ui_state.insert("last_game".into(), GAME.into());
        let (mut app, task) = App::new(cfg, config_dir().join("config.json"), deps);
        app.table
            .model
            .set_country_lookup(Some(Arc::new(|host: &str| country(host))));
        let shots = Shots {
            app,
            out,
            current: None,
            frames: 0,
            busy: false,
            password: false,
        };
        (shots, task.map(app_msg))
    }

    fn show(&mut self, i: usize) {
        self.current = Some(i);
        self.app.theme_id = VARIANTS[i].0;
        self.frames = 0;
        self.busy = false;
    }

    /// Conecta al `SELECTED_HOST` como si tuviera contraseña: el diálogo abre con la guardada.
    fn open_password(&mut self) -> Task<Msg> {
        self.password = true;
        // Una ruta cualquiera: sin ella sale «Ruta no configurada». Nunca se lanza nada.
        let paths = &mut self.app.config_mut().game_paths;
        paths.insert(GAME.into(), "C:\\Games\\CoD2".into());
        let (Some(game), Some(info)) = (self.app.current_game(), self.app.selected_server()) else {
            fail("sin juego o servidor elegido");
        };
        let info = ServerInfo {
            password: true,
            ..info.clone()
        };
        self.app.connect_to(game, info).map(app_msg)
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            // Un cambio tardío del tema del sistema no pisa el tema que se captura.
            Msg::App(m) if matches!(*m, Message::SystemMode(_)) => {}
            Msg::App(m) => return self.app.update(*m).map(app_msg),
            Msg::Frame => {
                let Some(_) = self.current else {
                    if !self.app.is_scanning() && self.app.servers().len() == SERVERS.len() {
                        self.app.table.selected = Some(ServerAddress::new(SELECTED_HOST, 28960));
                        // `players_table.sortByColumn(1, DescendingOrder)`: mejores puntajes arriba.
                        self.app.player_sort = PlayerSort {
                            col: PlayerCol::Score,
                            ascending: false,
                        };
                        // `body_splitter.setSizes([300, …])` del script de Python.
                        self.app.sidebar_width = 300.0;
                        // `table.setColumnWidth(COL_COUNTRY, 70)` del script de Python.
                        self.app.table.widths[Column::Country as usize] = 70.0;
                        // Como `_expand_tree`: el juego y sus carpetas abiertos.
                        self.app.tree.set_expanded(GAME, Node::Game, true);
                        for k in [Kind::Mods, Kind::Maps, Kind::Countries] {
                            self.app.tree.set_expanded(GAME, Node::Folder(k), true);
                        }
                        self.show(0);
                    }
                    return Task::none();
                };
                if self.busy || (self.password && self.app.dialog().is_none()) {
                    return Task::none();
                }
                self.frames += 1;
                if self.frames > SETTLE_FRAMES {
                    self.busy = true;
                    return window::latest().and_then(window::screenshot).map(Msg::Shot);
                }
            }
            Msg::Shot(shot) => {
                let id = self.current.map_or("", |i| VARIANTS[i].0);
                let suffix = if self.password { "-password" } else { "" };
                let path = self.out.join(format!("{id}{suffix}.png"));
                // El PNG de 4 MB no se codifica en el hilo de la UI.
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            write_png(&path, &shot).map_err(|e| format!("{}: {e}", path.display()))
                        })
                        .await
                        .map_err(|e| e.to_string())?
                    },
                    Msg::Written,
                );
            }
            Msg::Written(Err(e)) => fail(&e),
            Msg::Written(Ok(())) => match self.current {
                Some(i) if i + 1 < VARIANTS.len() => {
                    self.show(i + 1);
                    // Otro estilo arma otro árbol de widgets: el campo pierde el foco.
                    if self.password {
                        return operation::focus(PROMPT_ID);
                    }
                }
                _ if !self.password => {
                    self.show(0);
                    return self.open_password();
                }
                _ => return self.app.update(Message::CloseRequested).map(app_msg),
            },
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Msg> {
        self.app.view().map(app_msg)
    }

    fn theme(&self) -> Theme {
        self.app.theme()
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            self.app.subscription().map(app_msg),
            window::frames().map(|_| Msg::Frame),
        ])
    }
}

fn config_dir() -> PathBuf {
    std::env::temp_dir().join(format!("jase-screenshot-{}", std::process::id()))
}

/// Sale con 0 tras escribir los 28 PNG en `dir` (lo imprime el cierre) o con 1 y el error.
pub fn run(dir: &Path) -> ! {
    for d in [dir, &config_dir()] {
        if let Err(e) = std::fs::create_dir_all(d) {
            fail(&format!("{}: {e}", d.display()));
        }
    }
    let out = dir.to_path_buf();
    let result = std::panic::catch_unwind(|| {
        iced::application(move || Shots::boot(out.clone()), Shots::update, Shots::view)
            .title(APP_NAME)
            .settings(crate::settings())
            .window(window::Settings {
                size: SIZE,
                ..crate::window_settings(&Config::default())
            })
            .theme(Shots::theme)
            .subscription(Shots::subscription)
            .run()
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ConnectMsg, Dialog};
    use crate::details::players_rows;
    use crate::passwords::Keyring;
    use crate::tree::Item;
    use iced::theme::Mode;

    #[test]
    fn late_system_mode_keeps_captured_theme() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let neon = VARIANTS.iter().position(|v| v.0 == "neon").unwrap();
        shots.show(neon);
        let _ = shots.update(app_msg(Message::SystemMode(Mode::Light)));
        assert_eq!(shots.app.theme_id, "neon");
    }

    #[test]
    fn screenshot_shows_expanded_tree_and_countries() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let _ = shots.update(Msg::Frame);
        assert_eq!(shots.current, Some(0));
        assert_eq!(shots.app.sidebar_width, 300.0);
        let rows = shots.app.tree.rows();
        for kind in [Kind::Maps, Kind::Countries] {
            assert!(
                rows.iter()
                    .any(|r| r.game == GAME && r.item == Item::Folder(kind) && r.expanded),
                "{kind:?}"
            );
        }
        assert!(rows.iter().any(|r| r.flag.as_deref() == Some("cl")));
        let model = &shots.app.table.model;
        let first = &model.servers()[0];
        assert_eq!(model.country_code(first), Some("cl"));
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }

    #[test]
    fn screenshot_shows_card_and_details() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let _ = shots.update(Msg::Frame);
        let vm = crate::card::vm(&shots.app).expect("ficha del SELECTED_HOST");
        assert_eq!(vm.address, format!("{SELECTED_HOST}:28960"));
        assert_eq!(vm.placeholder, "mp_toujane");
        assert!(!vm.favorite);
        assert!(shots.app.map.image.is_none());
        assert!(shots.app.chart.is_none());
        // `sortByColumn(1, DescendingOrder)`: mejores puntajes arriba.
        assert_eq!(
            shots.app.player_sort,
            PlayerSort {
                col: PlayerCol::Score,
                ascending: false
            }
        );
        let info = shots.app.selected_server().unwrap();
        let scores: Vec<i32> = players_rows(info, shots.app.player_sort)
            .iter()
            .map(|p| p.score)
            .collect();
        assert_eq!(scores, [42, 37, 31, 24, 19, 15, 9, 4]);
        let _ = shots.view();
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }

    #[test]
    fn screenshot_shows_direct_list_and_password_dialog() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let _ = shots.update(Msg::Frame);
        // Como `Config()` de Python: sin favoritos ni recientes.
        let [fav, recent] = shots.app.direct_list_labels();
        assert!(fav.starts_with('★') && fav.ends_with(" · 0"), "{fav}");
        assert!(
            recent.starts_with('🕘') && recent.ends_with(" · 0"),
            "{recent}"
        );
        let _ = shots.open_password();
        assert!(shots.password);
        let request = shots.app.connect_request();
        let _ = shots.update(app_msg(Message::Connect(ConnectMsg::Stored {
            request,
            password: SavedSecret.get("x"),
        })));
        let name = shots.app.selected_server().unwrap().name.clone();
        assert_eq!(
            shots.app.dialog(),
            Some(&Dialog::Password {
                name,
                typed: "secreto".into(),
                saved: true, // *Olvidar contraseña* visible
            })
        );
        let _ = shots.view();
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }

    #[test]
    fn screenshot_shows_menu_bar_and_toolbar() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let _ = shots.update(Msg::Frame);
        assert_eq!(shots.current, Some(0));
        // `setColumnWidth(COL_COUNTRY, 70)` del script de Python.
        assert_eq!(shots.app.table.widths[Column::Country as usize], 70.0);
        assert_eq!(shots.app.open_menu(), None);
        // La vista entera (barra de menús, de herramientas con el campo, tabla y estado) se arma.
        let _ = shots.view();
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }
}
