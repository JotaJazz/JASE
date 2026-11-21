//! Capturas `--screenshot <carpeta>` (como `screenshot.py` de 1.3.0): la ventana de 1280×800 con los
//! mismos servidores de ejemplo inventados, en inglés y sin red ni la config del usuario, y un
//! `<variante>.png` por cada tema de `VARIANTS`; luego `<variante>-password.png` con el diálogo
//! de contraseña del `SELECTED_HOST` abierto (guardada `"secreto"`) y `<variante>-add.png` con
//! «Agregar servidor» abierto con `ADD_TEXT` (sin favorito); después
//! `<variante>-settings-<pestaña>.png` con Ajustes de `Config::default()` en cada pestaña y
//! `<variante>-find.png` con *Buscar juegos* y lo hallado de `FOUND`. Luego las herramientas,
//! cada una capturada en su ventana: `<variante>-now.png` (*Ahora se juega* con CoD2, los
//! `NOW_SERVERS` de CoD1 y un escaneo sin terminar), `<variante>-search.png` (*Buscar jugador*
//! con «jan») y `<variante>-rcon.png` («> status» y `RCON_ANSWER`); por último, sobre la
//! principal y vigilando `WATCH_HOST`, `<variante>-friends.png` (*Amigos* con `FRIENDS`) y
//! `<variante>-party.png` (el umbral de partida del elegido); y, sobre la principal y el
//! elegido, `<variante>-context.png` (el menú contextual), `<variante>-about.png` (*Acerca de*),
//! `<variante>-note.png` (*Editar nota…* con `NOTE_TEXT`) y `<variante>-slotfree.png` («Hay
//! sitio»).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use iced::widget::operation;
use iced::window::{self, Screenshot};
use iced::{Element, Size, Subscription, Task, Theme};
use jase_core::config::Config;
use jase_core::detect::Finding;
use jase_core::favorites::{add_favorite, set_note};
use jase_core::i18n::set_language;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::protocols::quake3::strip_colors;
use jase_core::scanner::scan::{ScanEvent, ScanResult};

use crate::actions::{self, Action, Menu};
use crate::add_server::{self, field_id};
use crate::app::{
    App, FriendsMsg, Message, PROMPT_ID, PromptMsg, RconMsg, SearchMsg, SettingsMsg, Tool, WatchMsg,
};
use crate::details::{PlayerCol, PlayerSort};
use crate::passwords::{Keyring, STEAM_KEY_USER};
use crate::scan::{Deps, ScanMsg};
use crate::settings::{PathStatus, Tab};
use crate::table;
use crate::table::Column;
use crate::theme::VARIANTS;
use crate::tree::{Kind, Node};

const GAME: &str = "cod2";
const SIZE: Size = Size::new(1280.0, 800.0);
pub const SELECTED_HOST: &str = "192.0.2.14";
/// El texto con que se abre «Agregar servidor» (`AddServerDialog(…, ADD_TEXT)` de Python).
const ADD_TEXT: &str = "connect 192.0.2.14:28960";
/// Lo que halla *Buscar juegos* (`FOUND` de `screenshot.py` de 1.3.0): juego, ruta y origen.
#[rustfmt::skip]
const FOUND: [(&str, &str, &str); 3] = [
    ("cod2", "C:\\Program Files (x86)\\Activision\\Call of Duty 2", "registro"),
    ("cod2", "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Call of Duty 2", "steam"),
    ("cod1", "C:\\Program Files (x86)\\Activision\\Call of Duty", "registro"),
];
/// *Ahora se juega* escanea CoD1 (trae `NOW_SERVERS`) y CoD:UO (no termina nunca).
const NOW_GAME: &str = "cod1";
const PENDING_GAME: &str = "coduo";
type NowServer = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    i32,
    u32,
);
/// host, nombre, mapa, jugadores, máx, ping de CoD1 (`NOW_SERVERS` de Python): «jan» halla dos.
#[rustfmt::skip]
const NOW_SERVERS: [NowServer; 2] = [
    ("192.0.2.200", "Jan's Brecourt Night", "mp_brecourt", &["Jan", "Ziggy", "Moro"], 16, 61),
    ("203.0.113.150", "Old Guard | Vanilla SD", "mp_harbor", &["Janosch", "Kiwi"], 10, 88),
];
const SEARCH_TEXT: &str = "jan";
const RCON_COMMAND: &str = "status";
/// La respuesta del servidor falso a `status` (`RCON_ANSWER` de Python).
const RCON_ANSWER: &str = "map: mp_toujane
num score ping name            lastmsg address               qport rate
--- ----- ---- --------------- ------- --------------------- ----- -----
  0    42   31 Sgt.Pepper            0 203.0.113.50:28960     4823 25000
  1    37   45 Bruma                50 198.51.100.12:28960    1907 25000
  2    31   28 Kestrel               0 192.0.2.77:28960       3311 25000";
/// Los amigos de *Amigos…* (`FRIENDS` de Python), con la revisión de fondo encendida.
const FRIENDS: [&str; 3] = ["Bruma", "Jan", "Kestrel"];
/// El servidor lleno que se vigila en las dos últimas vueltas («Vigilando…» en la barra).
const WATCH_HOST: &str = "198.51.100.64";
/// La nota del elegido en *Editar nota…*.
const NOTE_TEXT: &str = "Clan amigo, martes 21 h";
/// La espera, sin cuadros, entre el último rearmado de la vista y la captura.
const SHOT_DELAY: std::time::Duration = std::time::Duration::from_millis(150);
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

/// Los `NOW_SERVERS` de CoD1, en línea.
fn now_servers() -> Vec<ServerInfo> {
    NOW_SERVERS
        .iter()
        .map(|&(host, name, map, players, cap, ping)| {
            let addr = ServerAddress::new(host, 28960);
            let mut info = ServerInfo::new(addr.clone(), addr);
            info.players = (players.iter().enumerate())
                .map(|(i, p)| Player::new(*p, 10 - 3 * i as i32, 50 + 10 * i as i32))
                .collect();
            info.name = name.into();
            info.raw_name = name.into();
            info.map = map.into();
            info.gametype = "sd".into();
            info.num_players = players.len() as i32;
            info.max_players = cap;
            info.version = "1.5".into();
            info.ping_ms = ping;
            info
        })
        .collect()
}

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

/// El llavero de las capturas: siempre `"secreto"` guardada (`PasswordDialog(…, "secreto")`)
/// y sin clave de Steam (`SettingsDialog(…, steam_key="")`).
struct SavedSecret;

impl Keyring for SavedSecret {
    fn get(&self, user: &str) -> Option<String> {
        (user != STEAM_KEY_USER).then(|| "secreto".to_owned())
    }

    fn set(&self, _user: &str, _value: &str) -> Result<(), String> {
        Err("no keyring".to_owned())
    }
}

/// La base de países falsa (`_COUNTRY`).
fn country(host: &str) -> Option<String> {
    SERVERS.iter().find(|r| r.0 == host).map(|r| r.8.to_owned())
}

fn found_games() -> indexmap::IndexMap<String, Vec<Finding>> {
    let mut found = indexmap::IndexMap::<String, Vec<Finding>>::new();
    for (game_id, path, source) in FOUND {
        found.entry(game_id.to_owned()).or_default().push(Finding {
            game_id,
            path: path.to_owned(),
            source,
        });
    }
    found
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

/// Las vueltas por los temas: la ventana, con el diálogo de contraseña, con «Agregar
/// servidor», con cada pestaña de Ajustes y con *Buscar juegos*; las tres herramientas en su
/// ventana; *Amigos* y el umbral de partida; el menú contextual, *Acerca de*, la nota y «Hay
/// sitio».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Main,
    Password,
    Add,
    Settings(Tab),
    Find,
    Now,
    Search,
    Rcon,
    Friends,
    Party,
    Context,
    About,
    Note,
    SlotFree,
}

const PHASES: [Phase; 17] = [
    Phase::Main,
    Phase::Password,
    Phase::Add,
    Phase::Settings(Tab::Games),
    Phase::Settings(Tab::Masters),
    Phase::Settings(Tab::General),
    Phase::Settings(Tab::Appearance),
    Phase::Find,
    Phase::Now,
    Phase::Search,
    Phase::Rcon,
    Phase::Friends,
    Phase::Party,
    Phase::Context,
    Phase::About,
    Phase::Note,
    Phase::SlotFree,
];

impl Phase {
    fn next(self) -> Option<Phase> {
        let i = PHASES.iter().position(|&p| p == self)?;
        PHASES.get(i + 1).copied()
    }

    fn suffix(self) -> &'static str {
        match self {
            Phase::Main => "",
            Phase::Password => "-password",
            Phase::Add => "-add",
            Phase::Settings(Tab::Games) => "-settings-games",
            Phase::Settings(Tab::Masters) => "-settings-masters",
            Phase::Settings(Tab::General) => "-settings-general",
            Phase::Settings(Tab::Appearance) => "-settings-appearance",
            Phase::Find => "-find",
            Phase::Now => "-now",
            Phase::Search => "-search",
            Phase::Rcon => "-rcon",
            Phase::Friends => "-friends",
            Phase::Party => "-party",
            Phase::Context => "-context",
            Phase::About => "-about",
            Phase::Note => "-note",
            Phase::SlotFree => "-slotfree",
        }
    }

    /// La herramienta que se captura en su propia ventana.
    fn is_tool(self) -> bool {
        matches!(self, Phase::Now | Phase::Search | Phase::Rcon)
    }
}

#[derive(Debug, Clone)]
enum Msg {
    App(Box<Message>),
    /// Se dibujó un cuadro de esa ventana.
    Frame(window::Id),
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
    phase: Phase,
}

fn fail(error: &str) -> ! {
    println!("JASE screenshot FAIL: {error}");
    let _ = std::fs::remove_dir_all(config_dir());
    std::process::exit(1)
}

fn scan_msgs() -> Vec<ScanMsg> {
    finished(sample_servers())
}

fn now_msgs() -> Vec<ScanMsg> {
    finished(now_servers())
}

/// Cada servidor y el final con un master que respondió.
fn finished(servers: Vec<ServerInfo>) -> Vec<ScanMsg> {
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
            // CoD2 y lo de *Ahora se juega*; lo demás no termina nunca.
            scan: Arc::new(|g, _, _| match g.id {
                GAME => futures::stream::iter(scan_msgs()).boxed(),
                NOW_GAME => futures::stream::iter(now_msgs()).boxed(),
                _ => futures::stream::pending().boxed(),
            }),
            exit: Arc::new(move |code| {
                let _ = std::fs::remove_dir_all(config_dir());
                println!("JASE screenshot OK {}", PHASES.len() * VARIANTS.len());
                std::process::exit(code)
            }),
            // Nunca el llavero del usuario.
            keyring: Arc::new(SavedSecret),
            // Nunca un comando RCON real: la respuesta fija.
            rcon: Arc::new(|_, _, _, _| {
                Box::pin(futures::future::ready(Ok(RCON_ANSWER.to_owned())))
            }),
            // Nunca se lanza un juego del usuario.
            launch: Arc::new(|_| Err("no launch".to_owned())),
            // Sin base de países ni disco: nada que esperar ni leer del usuario.
            country_db: None,
            // Solo CoD2 está instalado (Ajustes: «✓» en su ruta y «⚠» en las demás).
            game_installed: Arc::new(|g, _| (g.id == GAME).then(|| g.exe_relpath.to_owned())),
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
            // Sin red ni navegador: ni pruebas de masters ni URL.
            probe: None,
            open_url: Arc::new(|_| Ok(())),
            // Sin disco del usuario: *Buscar juegos* halla siempre `FOUND`.
            find_games: Arc::new(|_, _| found_games()),
            // Sin red: la lista oficial empaquetada.
            masters_fetch: None,
            update_fetch: None,
            version: "dev",
            update_stream: Arc::new(|_| {
                Err(jase_core::app_update::DownloadError {
                    code: jase_core::app_update::DownloadCode::Network,
                    detail: String::new(),
                })
            }),
            spawn: Arc::new(|_, _| Err(std::io::Error::other("sin procesos"))),
            self_update: crate::update::SelfUpdate::Unsupported,
            // Nunca la bandeja ni las notificaciones del usuario.
            tray: None,
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
        let open = app.open_main(window::Settings {
            size: SIZE,
            ..crate::window_settings(&Config::default())
        });
        let shots = Shots {
            app,
            out,
            current: None,
            frames: 0,
            busy: false,
            phase: Phase::Main,
        };
        let mut task = Some(task.map(app_msg));
        (
            shots,
            open.then(move |_| task.take().unwrap_or_else(Task::none)),
        )
    }

    fn show(&mut self, i: usize) {
        self.current = Some(i);
        self.app.theme_id = VARIANTS[i].0;
        self.frames = 0;
        self.busy = false;
    }

    /// Conecta al `SELECTED_HOST` como si tuviera contraseña: el diálogo abre con la guardada.
    fn open_password(&mut self) -> Task<Msg> {
        self.phase = Phase::Password;
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

    /// Cierra la contraseña y abre «Agregar servidor» con `ADD_TEXT`, sin el portapapeles.
    fn open_add(&mut self) -> Task<Msg> {
        self.phase = Phase::Add;
        let close = self.app.update(Message::Prompt(PromptMsg::Cancel));
        let open = self.app.update(Message::AddServer(add_server::Msg::Open {
            favorite: false,
            text: ADD_TEXT.into(),
        }));
        Task::batch([close, open]).map(app_msg)
    }

    fn open_phase(&mut self, phase: Phase) -> Task<Msg> {
        match phase {
            Phase::Main => Task::none(),
            Phase::Password => self.open_password(),
            Phase::Add => self.open_add(),
            Phase::Settings(tab) => self.open_settings(tab),
            Phase::Find => {
                self.phase = phase;
                // `FindGamesDialog(Config())`; reemplaza a Ajustes.
                self.app.find_games().map(app_msg)
            }
            Phase::Now => {
                self.phase = phase;
                // Listos: CoD2 (la sesión), CoD1 y CoD:UO (se escanean).
                let cfg = self.app.config_mut();
                cfg.only_installed_games = false;
                cfg.game_paths = [GAME, NOW_GAME, PENDING_GAME]
                    .into_iter()
                    .map(|g| (g.to_owned(), format!("C:\\Games\\{g}")))
                    .collect();
                self.run(Action::NowPlaying)
            }
            Phase::Search => {
                self.phase = phase;
                let open = self.app.update(Message::Search(SearchMsg::Open));
                let query = Message::Search(SearchMsg::Query(SEARCH_TEXT.into()));
                Task::batch([open, self.app.update(query)]).map(app_msg)
            }
            Phase::Rcon => {
                self.phase = phase;
                // La clave guardada llega en un hilo; `drive` manda el comando después.
                self.run(Action::Rcon)
            }
            Phase::Friends => {
                self.phase = phase;
                // Se vigila el lleno y vuelve la selección de siempre.
                self.select(WATCH_HOST);
                let watch = self.run(Action::WatchSlot);
                self.select(SELECTED_HOST);
                self.app.config_mut().watch_friends = true;
                self.app.set_friends(FRIENDS.map(str::to_owned).to_vec());
                let open = self.app.update(Message::Friends(FriendsMsg::Open));
                Task::batch([watch, open.map(app_msg)])
            }
            Phase::Party => {
                self.phase = phase;
                self.run(Action::PartyAlert)
            }
            Phase::Context | Phase::About | Phase::Note | Phase::SlotFree => {
                self.phase = phase;
                // Esc: cada una abre sin el diálogo de la anterior.
                let esc = self.app.update(Message::Escape).map(app_msg);
                Task::batch([esc, self.open_parity(phase)])
            }
        }
    }

    /// El menú contextual, *Acerca de*, la nota o «Hay sitio» del elegido.
    fn open_parity(&mut self, phase: Phase) -> Task<Msg> {
        let selected = ServerAddress::new(SELECTED_HOST, 28960);
        match phase {
            Phase::Context => {
                let click = table::Msg::RightClicked(Some(selected));
                self.app.update(Message::Table(click)).map(app_msg)
            }
            Phase::About => self.run(Action::About),
            Phase::Note => {
                let cfg = self.app.config_mut();
                add_favorite(cfg, GAME, &selected);
                set_note(cfg, GAME, &selected.to_string(), NOTE_TEXT);
                self.app.sync_notes();
                self.app.edit_note_selected().map(app_msg)
            }
            _ => {
                // Solo se vigila un lleno: el de *Amigos* (si no sigue, otra vez), y la
                // respuesta trae al elegido con sitio (8/12), el texto que arma Python.
                let watch = if self.app.slot_serial().is_none() {
                    self.select(WATCH_HOST);
                    let watch = self.run(Action::WatchSlot);
                    self.select(SELECTED_HOST);
                    watch
                } else {
                    Task::none()
                };
                let (Some(serial), Some(info)) =
                    (self.app.slot_serial(), self.app.selected_server().cloned())
                else {
                    fail("sin vigilancia");
                };
                let answer = WatchMsg::SlotAnswer {
                    serial,
                    result: Ok(Some(info)),
                };
                let free = self.app.update(Message::Watch(answer)).map(app_msg);
                Task::batch([watch, free])
            }
        }
    }

    fn run(&mut self, action: Action) -> Task<Msg> {
        let m = Message::Actions(actions::Msg::Run(action));
        self.app.update(m).map(app_msg)
    }

    fn select(&mut self, host: &str) {
        self.app.table.selected = Some(ServerAddress::new(host, 28960));
    }

    /// La ventana que se captura: la de la herramienta, o la principal.
    fn target(&self) -> Option<window::Id> {
        let tool = |t: &Tool| match self.phase {
            Phase::Now => *t == Tool::NowPlaying,
            Phase::Search => *t == Tool::PlayerSearch,
            _ => matches!(t, Tool::Rcon(_)),
        };
        if !self.phase.is_tool() {
            return Some(self.app.main_window());
        }
        let tools = self.app.tool_windows();
        tools.into_iter().find(|(_, t)| tool(t)).map(|(id, _)| id)
    }

    /// La consola con la clave ya leída y vacía: escribe y manda `RCON_COMMAND`.
    fn drive(&mut self) -> Task<Msg> {
        let Some(id) = self.target().filter(|_| self.phase == Phase::Rcon) else {
            return Task::none();
        };
        let Some(c) = self.app.console(id) else {
            return Task::none();
        };
        if !c.loaded || c.busy || !c.output.text().trim().is_empty() {
            return Task::none();
        }
        let _ = self
            .app
            .update(Message::Rcon(RconMsg::Command(id, RCON_COMMAND.into())));
        self.app
            .update(Message::Rcon(RconMsg::Send(id)))
            .map(app_msg)
    }

    /// Cierra la ventana de la herramienta y pasa a la vuelta `next`.
    fn advance(&mut self, next: Phase) -> Task<Msg> {
        let close = match self.target().filter(|_| self.phase.is_tool()) {
            Some(id) => self.app.update(Message::CloseTool(id)).map(app_msg),
            None => Task::none(),
        };
        // Sin el runtime (tests) el cierre no llega: se suelta la herramienta igual.
        if let Some(id) = self.target().filter(|_| self.phase.is_tool()) {
            let _ = self.app.update(Message::WindowClosed(id));
        }
        self.show(0);
        Task::batch([close, self.open_phase(next)])
    }

    /// Ajustes en la pestaña `tab`; la primera vez lo abre con `Config::default()`, como
    /// `SettingsDialog(Config())` de Python.
    fn open_settings(&mut self, tab: Tab) -> Task<Msg> {
        self.phase = Phase::Settings(tab);
        let open = if self.app.settings_form().is_none() {
            *self.app.config_mut() = Config::default();
            self.app.open_settings()
        } else {
            Task::none()
        };
        let select = self.app.update(Message::Settings(SettingsMsg::Tab(tab)));
        Task::batch([open, select]).map(app_msg)
    }

    /// El diálogo de la vuelta está abierto y sin «Buscando…» (rutas o juegos).
    fn ready(&self) -> bool {
        match self.phase {
            Phase::Main => true,
            Phase::Settings(_) => self
                .app
                .settings_form()
                .is_some_and(|f| !f.status.values().any(|s| *s == PathStatus::Searching)),
            Phase::Find => self.app.find_search().is_some_and(|s| !s.searching),
            Phase::Password
            | Phase::Add
            | Phase::Friends
            | Phase::Party
            | Phase::About
            | Phase::Note
            | Phase::SlotFree => self.app.dialog().is_some(),
            Phase::Context => self.app.open_menu() == Some(Menu::Context),
            // CoD1 ya llegó; CoD:UO sigue escaneando.
            Phase::Now => self.app.now_rows().iter().any(|r| r.game == NOW_GAME),
            Phase::Search => self
                .app
                .player_search()
                .is_some_and(|s| !s.stale() && !s.hits.is_empty()),
            Phase::Rcon => self
                .target()
                .and_then(|id| self.app.console(id))
                .is_some_and(|c| {
                    !c.busy
                        && c.output
                            .text()
                            .contains(RCON_ANSWER.lines().next().unwrap_or(""))
                }),
        }
    }

    /// El campo enfocado del diálogo de la vuelta, para devolverle el foco al cambiar de estilo.
    fn refocus(&self) -> Task<Msg> {
        match (self.phase, self.app.add_form()) {
            (Phase::Password, _) => operation::focus(PROMPT_ID),
            // El texto elegido, como al abrirlos (`getText`, `QSpinBox`).
            (Phase::Note | Phase::Party, _) => focus_all(PROMPT_ID),
            (Phase::Add, Some(form)) => operation::focus(field_id(form.focus())),
            (Phase::Friends, _) => operation::focus(crate::friends_dialog::FIELD_ID),
            (Phase::Search, _) => match self.app.player_search() {
                Some(s) => focus_all(s.field.clone()),
                None => Task::none(),
            },
            (Phase::Rcon, _) => match self.target().and_then(|id| self.app.console(id)) {
                Some(c) => operation::focus(c.command_field.clone()),
                None => Task::none(),
            },
            _ => Task::none(),
        }
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            // Un cambio tardío del tema del sistema no pisa el tema que se captura.
            Msg::App(m) if matches!(*m, Message::SystemMode(_)) => {}
            Msg::App(m) => return self.app.update(*m).map(app_msg),
            Msg::Frame(id) => {
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
                // Solo cuentan los cuadros de la ventana que se captura (ya abierta).
                if self.busy || self.target() != Some(id) {
                    return Task::none();
                }
                if !self.ready() {
                    return self.drive();
                }
                self.frames += 1;
                if self.frames > SETTLE_FRAMES {
                    // Cada mensaje rearma la vista, y un `text_editor` rearmado deja sin
                    // texto el último dibujo (la salida RCON salía vacía): sin cuadros
                    // (`busy`), se espera el dibujo de este rearmado y se captura sin pasar
                    // por `update`.
                    self.busy = true;
                    let wait = async { tokio::time::sleep(SHOT_DELAY).await };
                    return Task::future(wait)
                        .then(move |()| window::screenshot(id))
                        .map(Msg::Shot);
                }
            }
            Msg::Shot(shot) => {
                let id = self.current.map_or("", |i| VARIANTS[i].0);
                let path = self.out.join(format!("{id}{}.png", self.phase.suffix()));
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
            Msg::Written(Ok(())) => match (self.current, self.phase.next()) {
                (Some(i), _) if i + 1 < VARIANTS.len() => {
                    self.show(i + 1);
                    // Otro estilo arma otro árbol de widgets: el campo pierde el foco.
                    return self.refocus();
                }
                (_, Some(next)) => return self.advance(next),
                (_, None) => return self.app.update(Message::CloseRequested).map(app_msg),
            },
        }
        Task::none()
    }

    fn view(&self, id: window::Id) -> Element<'_, Msg> {
        self.app.view_window(id).map(app_msg)
    }

    fn title(&self, id: window::Id) -> String {
        self.app.title(id)
    }

    fn theme(&self, _: window::Id) -> Theme {
        self.app.theme()
    }

    fn subscription(&self) -> Subscription<Msg> {
        let frames = iced::event::listen_raw(|e, _, id| match e {
            iced::Event::Window(window::Event::RedrawRequested(_)) => Some(Msg::Frame(id)),
            _ => None,
        });
        Subscription::batch([
            self.app.subscription().map(app_msg),
            if self.busy {
                Subscription::none()
            } else {
                frames
            },
        ])
    }
}

fn focus_all(id: iced::widget::Id) -> Task<Msg> {
    Task::batch([operation::focus(id.clone()), operation::select_all(id)])
}

fn config_dir() -> PathBuf {
    std::env::temp_dir().join(format!("jase-screenshot-{}", std::process::id()))
}

/// Sale con 0 tras escribir los 238 PNG en `dir` (lo imprime el cierre) o con 1 y el error.
pub fn run(dir: &Path) -> ! {
    for d in [dir, &config_dir()] {
        if let Err(e) = std::fs::create_dir_all(d) {
            fail(&format!("{}: {e}", d.display()));
        }
    }
    let out = dir.to_path_buf();
    let result = std::panic::catch_unwind(|| {
        iced::daemon(move || Shots::boot(out.clone()), Shots::update, Shots::view)
            .title(Shots::title)
            .settings(crate::settings())
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
    use crate::find_games::RowState;
    use crate::menu::Entry;
    use crate::now_playing::NowRow;
    use crate::passwords::Keyring;
    use crate::tree::Item;
    use iced::theme::Mode;
    use jase_core::i18n::tr;

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
        let _ = shots.update(Msg::Frame(shots.app.main_window()));
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
        let _ = shots.update(Msg::Frame(shots.app.main_window()));
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
        let _ = shots.view(shots.app.main_window());
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
        let _ = shots.update(Msg::Frame(shots.app.main_window()));
        // Como `Config()` de Python: sin favoritos ni recientes.
        let [fav, recent] = shots.app.direct_list_labels();
        let expected = [tr("Favoritos"), tr("Recientes")].map(|t| format!("{t} · 0"));
        assert_eq!([fav, recent], expected);
        let _ = shots.open_password();
        assert_eq!(shots.phase, Phase::Password);
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
        let _ = shots.view(shots.app.main_window());
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }

    #[test]
    fn screenshot_shows_add_server_dialog() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let _ = shots.update(Msg::Frame(shots.app.main_window()));
        let _ = shots.open_password();
        let _ = shots.open_add();
        assert_eq!(shots.phase, Phase::Add);
        assert_eq!(shots.app.dialog(), Some(&Dialog::AddServer));
        let form = shots.app.add_form().expect("diálogo «Agregar servidor»");
        assert_eq!(
            (form.game(), form.host(), form.port()),
            (GAME, SELECTED_HOST, "28960")
        );
        assert!(form.can_detect()); // *Detectar* encendido
        assert!(!form.favorite());
        let _ = shots.view(shots.app.main_window());
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }

    #[test]
    fn screenshot_shows_settings_and_find_games() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let _ = shots.update(Msg::Frame(shots.app.main_window()));
        let _ = shots.open_password();
        let _ = shots.open_add();
        let mut seen = Vec::new();
        let mut next = Some(Phase::Settings(Tab::Games));
        while let Some(Phase::Settings(tab)) = next {
            let _ = shots.open_phase(Phase::Settings(tab));
            assert_eq!(shots.app.dialog(), Some(&Dialog::Settings));
            let form = shots.app.settings_form().expect("Ajustes abierto");
            assert_eq!(form.tab, tab);
            // `SettingsDialog(Config())`: sin clave y con las rutas por omisión.
            assert_eq!(form.steam_key, "");
            assert_eq!(form.paths["cod2"], Config::default().game_paths["cod2"]);
            let _ = shots.view(shots.app.main_window());
            seen.push(tab);
            next = shots.phase.next();
        }
        assert_eq!(
            seen,
            [Tab::Games, Tab::Masters, Tab::General, Tab::Appearance]
        );
        assert_eq!(next, Some(Phase::Find));
        let _ = shots.open_phase(Phase::Find);
        assert_eq!(shots.app.dialog(), Some(&Dialog::FindGames));
        let request = shots.app.find_search().expect("búsqueda abierta").request;
        let found = (shots.app.deps.find_games)(None, &|| false);
        let states = [("cod2".to_owned(), RowState::Configured)].into();
        let _ = shots.update(app_msg(Message::Settings(SettingsMsg::Found {
            request,
            found,
            states,
        })));
        let search = shots.app.find_search().unwrap();
        let rows: Vec<_> = search
            .rows
            .iter()
            .map(|r| (r.game, r.findings.len(), r.checked))
            .collect();
        assert_eq!(rows, [("cod2", 2, false), ("cod1", 1, true)]);
        assert_eq!(search.status, "2 games found");
        let _ = shots.view(shots.app.main_window());
        assert_eq!(shots.phase.next(), Some(Phase::Now));
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }

    /// Los primeros `n` mensajes de la app que da `task`, con tokio (plazos y `spawn_blocking`);
    /// las acciones de ventana y de widgets se descartan.
    fn outputs(task: Task<Msg>, n: usize) -> Vec<Message> {
        let Some(stream) = iced_runtime::task::into_stream(task) else {
            return Vec::new();
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        rt.block_on(
            stream
                .filter_map(|a| async move {
                    match a {
                        iced_runtime::Action::Output(Msg::App(m)) => Some(*m),
                        _ => None,
                    }
                })
                .take(n)
                .collect(),
        )
    }

    fn feed(shots: &mut Shots, task: Task<Msg>, n: usize) {
        for m in outputs(task, n) {
            let _ = shots.update(app_msg(m));
        }
    }

    #[test]
    fn screenshot_shows_tools_and_watch_dialogs() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let main = shots.app.main_window();
        let _ = shots.update(Msg::Frame(main));
        assert_eq!(Phase::Find.next(), Some(Phase::Now));
        // *Ahora se juega*: CoD2 de la sesión, CoD1 escaneado y CoD:UO sin terminar.
        let task = shots.open_phase(Phase::Now);
        feed(&mut shots, task, 3);
        let now = shots.app.tool_window(&Tool::NowPlaying).expect("ventana");
        assert_eq!(shots.target(), Some(now));
        let rows = shots.app.now_rows();
        assert_eq!(rows.len(), 14);
        assert!(rows.iter().any(|r| r.game == NOW_GAME));
        // De más a menos gente y, a igualdad, menor ping.
        let key = |r: &NowRow| {
            let n: i32 = r.players.split('/').next().unwrap().parse().unwrap();
            (-n, r.ping)
        };
        assert!(rows.windows(2).all(|w| key(&w[0]) <= key(&w[1])));
        assert_eq!(
            shots.app.now_status(),
            "104 players in your games · scanning 1 game…"
        );
        assert!(shots.ready());
        let _ = shots.view(now);
        // *Buscar jugador…* con «jan»: los dos de CoD1 que dejó *Ahora se juega*.
        let task = shots.advance(Phase::Search);
        feed(&mut shots, task, 1);
        assert!(shots.app.tool_window(&Tool::NowPlaying).is_none());
        let search = shots.app.player_search().expect("búsqueda");
        let hits: Vec<_> = search.hits.iter().map(|h| h.player.as_str()).collect();
        assert_eq!(hits, ["Jan", "Janosch"]);
        assert_eq!(search.games, 2);
        assert!(shots.ready());
        let _ = shots.view(shots.target().unwrap());
        // La consola RCON: «> status» y la respuesta fija.
        let task = shots.advance(Phase::Rcon);
        feed(&mut shots, task, 1);
        assert!(shots.app.player_search().is_none());
        let rcon = shots.target().expect("consola");
        assert!(!shots.ready());
        let task = shots.drive();
        feed(&mut shots, task, 1);
        let c = shots.app.console(rcon).unwrap();
        assert_eq!(
            c.output.text().trim_end(),
            format!("> status\n{RCON_ANSWER}")
        );
        assert!(shots.ready());
        let _ = shots.view(rcon);
        // *Amigos…* sobre la principal, vigilando un servidor lleno.
        let _ = shots.advance(Phase::Friends);
        assert_eq!(shots.target(), Some(main));
        assert_eq!(shots.app.dialog(), Some(&Dialog::Friends));
        let form = shots.app.friends_form().unwrap();
        assert_eq!(form.names(), FRIENDS);
        assert!(form.watch);
        assert_eq!(
            shots.app.watch_label().as_deref(),
            Some("Watching Veteranos SD | Sin lag | PAM…")
        );
        assert_eq!(
            shots.app.selected_server().unwrap().address.host,
            SELECTED_HOST
        );
        let _ = shots.view(main);
        // El umbral de partida del elegido.
        let _ = shots.advance(Phase::Party);
        let Some(Dialog::PartyAlert { name, text, .. }) = shots.app.dialog() else {
            panic!("{:?}", shots.app.dialog());
        };
        assert_eq!(name, &shots.app.selected_server().unwrap().name);
        assert_eq!(text, "4");
        let _ = shots.view(main);
        assert_eq!(shots.phase.next(), Some(Phase::Context));
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }

    #[test]
    fn screenshot_shows_parity_dialogs() {
        let (mut shots, _) = Shots::boot(std::env::temp_dir());
        let generation = shots.app.scan.generation();
        for msg in scan_msgs() {
            let _ = shots.update(app_msg(Message::Scan { generation, msg }));
        }
        let main = shots.app.main_window();
        let _ = shots.update(Msg::Frame(main));
        // Desde el umbral de partida, como en la serie.
        let task = shots.advance(Phase::Party);
        feed(&mut shots, task, 1);
        assert!(matches!(
            shots.app.dialog(),
            Some(Dialog::PartyAlert { .. })
        ));
        // El menú contextual del elegido, sin el umbral.
        let task = shots.advance(Phase::Context);
        feed(&mut shots, task, 1);
        assert_eq!(shots.target(), Some(main));
        assert_eq!(shots.app.dialog(), None);
        assert_eq!(shots.app.open_menu(), Some(Menu::Context));
        let labels: Vec<String> = actions::open_entries(&shots.app, Menu::Context)
            .unwrap()
            .into_iter()
            .filter_map(|e| match e {
                Entry::Item { label, .. } => Some(label),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&"Connect".to_owned()), "{labels:?}");
        assert!(labels.contains(&"Copy IP:port".to_owned()), "{labels:?}");
        assert!(shots.ready());
        let _ = shots.view(main);
        // *Acerca de*, sin el menú.
        let task = shots.advance(Phase::About);
        feed(&mut shots, task, 1);
        assert_eq!(shots.app.open_menu(), None);
        assert_eq!(shots.app.dialog(), Some(&Dialog::About));
        assert!(
            crate::app::about_lines()
                .iter()
                .any(|l| l.contains(crate::startup::APP_NAME))
        );
        assert!(shots.ready());
        let _ = shots.view(main);
        // *Editar nota…* del elegido, ya favorito con su nota.
        let task = shots.advance(Phase::Note);
        feed(&mut shots, task, 1);
        let Some(Dialog::Note { address, text, .. }) = shots.app.dialog() else {
            panic!("{:?}", shots.app.dialog());
        };
        assert_eq!(address.host, SELECTED_HOST);
        assert_eq!(text, NOTE_TEXT);
        assert!(shots.ready());
        let _ = shots.view(main);
        // «Hay sitio» del elegido, sin la nota.
        let task = shots.advance(Phase::SlotFree);
        feed(&mut shots, task, 1);
        let Some(Dialog::SlotFree { text }) = shots.app.dialog() else {
            panic!("{:?}", shots.app.dialog());
        };
        let name = &shots.app.selected_server().unwrap().name;
        assert_eq!(
            text,
            &format!("There is room on {name} (8/12). Connect now?")
        );
        assert!(shots.app.watch_label().is_none());
        assert!(shots.ready());
        let _ = shots.view(main);
        assert_eq!(shots.phase.next(), None);
        assert_eq!(PHASES.len(), 17);
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
        let _ = shots.update(Msg::Frame(shots.app.main_window()));
        assert_eq!(shots.current, Some(0));
        // `setColumnWidth(COL_COUNTRY, 70)` del script de Python.
        assert_eq!(shots.app.table.widths[Column::Country as usize], 70.0);
        assert_eq!(shots.app.open_menu(), None);
        // La vista entera (barra de menús, de herramientas con el campo, tabla y estado) se arma.
        let _ = shots.view(shots.app.main_window());
        shots.app.saved.flush();
        let _ = std::fs::remove_dir_all(config_dir());
    }
}
