//! Amigos (`window_friends.py`): el diálogo, el aviso al aparecer uno (las apariciones de una
//! misma tanda van en un solo aviso), la negrita de la tabla de jugadores y su clic derecho.

use std::time::Duration;

use iced::{Task, widget::operation};
use jase_core::friends::{FriendSighting, FriendTracker, normalize, parse};
use jase_core::games::game;
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::models::ServerInfo;
use jase_core::protocols::quake3::strip_colors;
use jase_core::pystr::py_strip;
use serde_json::Value;

use super::{App, Click, Dialog, Message, PromptMsg};
use crate::actions::Menu;
use crate::friends_dialog::{FIELD_ID, FriendsForm};

/// Apariciones de una misma tanda de escaneo → un solo aviso (`NOTIFY_BATCH_MS`).
pub const NOTIFY_BATCH: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone)]
pub enum FriendsMsg {
    /// *Amigos…*
    Open,
    Text(String),
    Add,
    Select(usize),
    Remove,
    Watch(bool),
    Accept,
    Cancel,
    /// Venció el plazo de la tanda `n`.
    Flush(u64),
    /// Clic derecho en un jugador (su nombre plano).
    PlayerMenu(String),
    /// «Agregar a amigos» del menú del jugador.
    AddFriend(String),
}

/// Dónde se vio a cada amigo y la tanda que espera su aviso.
#[derive(Default)]
pub(crate) struct Friends {
    tracker: FriendTracker,
    pending: Vec<FriendSighting>,
    /// Número de la tanda vigente: el plazo de una cortada no avisa.
    batch: u64,
}

impl Friends {
    pub(super) fn new(friends: &[String]) -> Self {
        Friends {
            tracker: FriendTracker::new(friends.to_vec()),
            ..Friends::default()
        }
    }
}

impl App {
    pub(super) fn on_friends(&mut self, m: FriendsMsg) -> Task<Message> {
        match m {
            FriendsMsg::Open => {
                self.open_dialog(Dialog::Friends);
                let form = FriendsForm::new(&self.cfg.friends, self.cfg.watch_friends);
                self.friends_form = Some(form);
                return operation::focus(FIELD_ID);
            }
            FriendsMsg::Flush(n) if n == self.watch.friends.batch => return self.flush_friends(),
            FriendsMsg::Flush(_) => {}
            FriendsMsg::PlayerMenu(name) => {
                let name = py_strip(&name);
                if !name.is_empty() {
                    self.player_menu = Some(name.to_owned());
                    self.menu = Some(Menu::Players);
                }
            }
            FriendsMsg::AddFriend(name) => {
                self.menu = None;
                self.add_friend(&name);
            }
            m => return self.on_friends_form(m),
        }
        Task::none()
    }

    fn on_friends_form(&mut self, m: FriendsMsg) -> Task<Message> {
        let Some(form) = self.friends_form.as_mut() else {
            return Task::none();
        };
        match m {
            FriendsMsg::Text(t) => form.text = t,
            FriendsMsg::Add => form.add(),
            FriendsMsg::Select(i) => form.selected = Some(i),
            FriendsMsg::Remove => form.remove_selected(),
            FriendsMsg::Watch(on) => form.watch = on,
            FriendsMsg::Accept => {
                let (watch, names) = (form.watch, form.names());
                self.cfg.watch_friends = watch;
                self.set_friends(names);
                return self.close_friends();
            }
            FriendsMsg::Cancel => return self.close_friends(),
            _ => {}
        }
        Task::none()
    }

    /// Enter fuera del campo = *Guardar* (el `OK` por defecto); Esc = *Cancelar*.
    pub(super) fn on_friends_prompt(&mut self, m: PromptMsg) -> Task<Message> {
        match m {
            PromptMsg::Submit => self.on_friends(FriendsMsg::Accept),
            PromptMsg::Cancel => self.on_friends(FriendsMsg::Cancel),
            _ => Task::none(),
        }
    }

    fn close_friends(&mut self) -> Task<Message> {
        self.friends_form = None;
        self.close_dialog()
    }

    pub fn friends_form(&self) -> Option<&FriendsForm> {
        self.friends_form.as_ref()
    }

    pub fn player_menu(&self) -> Option<&str> {
        self.player_menu.as_deref()
    }

    pub fn set_friends(&mut self, names: Vec<String>) {
        self.cfg.friends = parse(&Value::from(names));
        self.watch.friends.tracker.friends = self.cfg.friends.clone();
        self.save_config();
        self.sync_watch();
    }

    pub fn add_friend(&mut self, name: &str) {
        let mut names = self.cfg.friends.clone();
        names.push(name.to_owned());
        self.set_friends(names);
    }

    pub fn is_friend(&self, name: &str) -> bool {
        let key = normalize(name);
        self.cfg.friends.iter().any(|f| normalize(f) == key)
    }

    /// Del gancho de respuestas: las apariciones nuevas esperan `NOTIFY_BATCH` a las demás.
    pub(super) fn check_friends(&mut self, game: &'static str, info: &ServerInfo) -> Task<Message> {
        if self.cfg.friends.is_empty() {
            return Task::none();
        }
        let found = self.watch.friends.tracker.update(game, info);
        let f = &mut self.watch.friends;
        let idle = f.pending.is_empty();
        f.pending.extend(found);
        if !idle || f.pending.is_empty() {
            return Task::none();
        }
        f.batch += 1;
        let n = f.batch;
        Task::perform(
            async { tokio::time::sleep(NOTIFY_BATCH).await },
            move |()| Message::Friends(FriendsMsg::Flush(n)),
        )
    }

    /// Al cerrar: la tanda pendiente no avisa.
    pub(super) fn stop_friend_batch(&mut self) {
        self.watch.friends.pending.clear();
        self.watch.friends.batch += 1;
    }

    fn flush_friends(&mut self) -> Task<Message> {
        let pending = std::mem::take(&mut self.watch.friends.pending);
        let Some(first) = pending.first() else {
            return Task::none();
        };
        if !self.cfg.notify_friends {
            return Task::none();
        }
        let def = game(&first.game_id);
        let text = if pending.len() == 1 {
            let game_name = def.map_or(first.game_id.as_str(), |g| g.name);
            tr_fmt(
                "{friend} está en {server} ({game})",
                &[
                    ("friend", &first.friend),
                    ("server", &strip_colors(&first.server.name)),
                    ("game", &game_name),
                ],
            )
        } else {
            let mut names: Vec<&str> = Vec::new();
            for s in &pending {
                if !names.contains(&s.friend.as_str()) {
                    names.push(&s.friend);
                }
            }
            tr_n(
                "{n} amigo está en línea: {friend}",
                "{n} amigos están en línea: {friend}",
                pending.len() as i64,
                &[("friend", &names.join(", "))],
            )
        };
        let click = def.map(|g| Click::Reveal(g.id, first.server.address.clone()));
        self.notify(&tr("Amigo en línea"), &text, click)
    }
}
