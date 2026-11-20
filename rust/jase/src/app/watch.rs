//! *Avisar cuando haya sitio* (`slot_watch.py`, `window_slot_watch.py`), *Avisar cuando se
//! arme partida* (`window_party.py`) y la revisión de fondo de favoritos, recientes y
//! umbrales (`friend_watch.py`). Las consultas corren en `Task`s abortables con número: la
//! respuesta de una vigilancia terminada o de una vuelta apagada se descarta.

use std::time::Duration;

use futures::{StreamExt, stream};
use iced::task::Handle;
use iced::widget::{Space, column, row, text};
use iced::{Element, Fill, Subscription, Task, time};
use jase_core::favorites::{favorite_addresses, parse_address};
use jase_core::games::{GAMES, GameDef, game};
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::party::{DEFAULT_THRESHOLD, MAX_THRESHOLD, PartyTracker};
use jase_core::pystr::py_strip;
use jase_core::recent::recent_addresses;

use super::dialogs::{PROMPT_ID, PromptMsg, card};
use super::friends::Friends;
use super::notify::{Click, Prompt, slot_free_text};
use super::{App, DIRECT_CONCURRENCY, Dialog, Message};
use crate::theme::{style, widget_style};
use crate::widgets;

/// Cada cuánto se consulta el vigilado (`INTERVAL_MS`).
pub const SLOT_INTERVAL: Duration = Duration::from_secs(10);
/// Cuánto dura una vigilancia sin sitio (`MAX_MS`).
pub const SLOT_MAX: Duration = Duration::from_secs(30 * 60);
/// Cada cuánto da una vuelta la revisión de fondo (`FriendWatcher.INTERVAL_MS`).
pub const WATCH_INTERVAL: Duration = Duration::from_secs(120);

pub fn has_free_slot(info: &ServerInfo) -> bool {
    info.online && info.max_players > 0 && info.num_players < info.max_players
}

/// El umbral escrito, si `QSpinBox` lo dejaría (`0..=MAX_THRESHOLD`).
pub fn party_value(text: &str) -> Option<u32> {
    py_strip(text)
        .parse::<u32>()
        .ok()
        .filter(|n| *n <= MAX_THRESHOLD)
}

fn is_full(info: &ServerInfo) -> bool {
    info.online && info.max_players > 0 && info.num_players >= info.max_players
}

#[allow(clippy::large_enum_variant)] // por `ServerInfo`, como `Message::Query`
#[derive(Debug, Clone)]
pub enum WatchMsg {
    /// *Avisar cuando haya sitio* / *Dejar de vigilar* sobre la selección.
    ToggleSlot,
    SlotTick,
    SlotAnswer {
        serial: u64,
        result: Result<Option<ServerInfo>, String>,
    },
    SlotExpired(u64),
    RoundTick,
    RoundSeen {
        round: u64,
        game: &'static str,
        info: ServerInfo,
    },
    RoundDone(u64),
}

/// El servidor vigilado (uno a la vez).
pub(crate) struct Slot {
    serial: u64,
    game: &'static GameDef,
    address: ServerAddress,
    name: String,
    busy: bool,
    handle: Option<Handle>,
    /// El plazo de `SLOT_MAX`: se aborta al terminar.
    expiry: Handle,
}

#[derive(Default)]
pub(crate) struct Watch {
    slot: Option<Slot>,
    serial: u64,
    /// El servidor con sitio que espera *Conectar*.
    pub(super) free: Option<(&'static GameDef, ServerInfo)>,
    party: PartyTracker,
    round: u64,
    round_handle: Option<Handle>,
    pub(super) friends: Friends,
}

impl App {
    pub fn is_watching(&self, info: Option<&ServerInfo>) -> bool {
        match (info, &self.watch.slot, self.current) {
            (Some(i), Some(s), Some(g)) => s.address == i.address && s.game.id == g.id,
            _ => false,
        }
    }

    /// «Vigilando {name}…» (el nombre remoto, en texto plano).
    pub fn watch_label(&self) -> Option<String> {
        let slot = self.watch.slot.as_ref()?;
        Some(tr_fmt("Vigilando {name}…", &[("name", &slot.name)]))
    }

    pub(crate) fn watch_slot_enabled(&self) -> bool {
        let info = self.selected_server();
        info.is_some_and(is_full) || self.is_watching(info)
    }

    pub(crate) fn watch_slot_label(&self) -> String {
        if self.is_watching(self.selected_server()) {
            tr("Dejar de vigilar")
        } else {
            tr("Avisar cuando haya sitio")
        }
    }

    pub(crate) fn party_label(&self) -> String {
        let threshold = match (self.current, self.selected_server()) {
            (Some(g), Some(i)) => self.party_threshold(g.id, &i.address),
            _ => 0,
        };
        if threshold == 0 {
            return tr("Avisar cuando se arme partida…");
        }
        tr_n(
            "Avisar de partida: {n} jugador…",
            "Avisar de partida: {n} jugadores…",
            i64::from(threshold),
            &[],
        )
    }

    pub(super) fn watch_subscriptions(&self) -> Vec<Subscription<Message>> {
        let mut subs = Vec::new();
        if self.watch.slot.is_some() {
            subs.push(time::every(SLOT_INTERVAL).map(|_| Message::Watch(WatchMsg::SlotTick)));
        }
        if self.wants_rounds() {
            subs.push(time::every(WATCH_INTERVAL).map(|_| Message::Watch(WatchMsg::RoundTick)));
        }
        subs
    }

    pub(super) fn on_watch(&mut self, m: WatchMsg) -> Task<Message> {
        match m {
            WatchMsg::ToggleSlot => return self.toggle_slot(),
            WatchMsg::SlotTick => return self.slot_tick(),
            WatchMsg::SlotAnswer { serial, result } => return self.slot_answer(serial, result),
            WatchMsg::SlotExpired(serial) if self.slot_serial() == Some(serial) => {
                let name = self.watch.slot.as_ref().map(|s| s.name.clone());
                self.end_slot();
                let minutes = SLOT_MAX.as_secs() / 60;
                let text = tr_fmt(
                    "Se dejó de vigilar {name}: {minutes} min sin sitio",
                    &[("name", &name.unwrap_or_default()), ("minutes", &minutes)],
                );
                return self.show_message(text, None);
            }
            WatchMsg::SlotExpired(_) => {}
            WatchMsg::RoundTick => return self.round_tick(),
            WatchMsg::RoundSeen { round, game, info } if round == self.watch.round => {
                return self.on_server_response(game, &info);
            }
            WatchMsg::RoundDone(round) if round == self.watch.round => {
                self.watch.round_handle = None;
            }
            WatchMsg::RoundSeen { .. } | WatchMsg::RoundDone(_) => {}
        }
        Task::none()
    }

    pub(crate) fn slot_serial(&self) -> Option<u64> {
        self.watch.slot.as_ref().map(|s| s.serial)
    }

    /// `toggle_slot_watch`: la vigilada termina; si no, reemplaza a la anterior.
    fn toggle_slot(&mut self) -> Task<Message> {
        let (Some(info), Some(game)) = (self.selected_server().cloned(), self.current) else {
            return Task::none();
        };
        let watching = self.is_watching(Some(&info));
        self.end_slot();
        if watching {
            return Task::none();
        }
        self.watch.serial += 1;
        let serial = self.watch.serial;
        let (task, expiry) =
            Task::perform(async { tokio::time::sleep(SLOT_MAX).await }, move |()| {
                Message::Watch(WatchMsg::SlotExpired(serial))
            })
            .abortable();
        self.watch.slot = Some(Slot {
            serial,
            game,
            address: info.address,
            name: info.name,
            busy: false,
            handle: None,
            expiry,
        });
        task
    }

    fn end_slot(&mut self) {
        if let Some(slot) = self.watch.slot.take() {
            slot.expiry.abort();
            if let Some(handle) = slot.handle {
                handle.abort();
            }
        }
    }

    /// Una consulta a la vez.
    fn slot_tick(&mut self) -> Task<Message> {
        let query = self.deps.query.clone();
        let Some(slot) = self.watch.slot.as_mut().filter(|s| !s.busy) else {
            return Task::none();
        };
        let (game, address, serial) = (slot.game, slot.address.clone(), slot.serial);
        let (task, handle) = Task::perform(async move { query(game, address).await }, move |r| {
            Message::Watch(WatchMsg::SlotAnswer { serial, result: r })
        })
        .abortable();
        slot.busy = true;
        slot.handle = Some(handle);
        task
    }

    fn slot_answer(
        &mut self,
        serial: u64,
        result: Result<Option<ServerInfo>, String>,
    ) -> Task<Message> {
        let Some(slot) = self.watch.slot.as_mut().filter(|s| s.serial == serial) else {
            return Task::none(); // de una vigilancia terminada o reemplazada
        };
        slot.busy = false;
        slot.handle = None;
        let game = slot.game;
        let Ok(Some(info)) = result else {
            return Task::none();
        };
        let hook = self.on_server_response(game.id, &info);
        if !has_free_slot(&info) {
            return hook;
        }
        self.end_slot();
        let text = slot_free_text(&info);
        self.watch.free = Some((game, info));
        let ask = self.notify_prompt(&tr("Hay sitio"), &text, Prompt::SlotFree);
        Task::batch([hook, ask])
    }

    /// `_on_server_response`: el gancho de toda respuesta de servidor aceptada.
    pub fn on_server_response(&mut self, game: &'static str, info: &ServerInfo) -> Task<Message> {
        self.record_response(game, info);
        let party = self.check_party(game, info);
        Task::batch([party, self.check_friends(game, info)])
    }

    pub fn party_threshold(&self, game: &str, address: &ServerAddress) -> u32 {
        let alerts = self.cfg.party_alerts.get(game);
        alerts
            .and_then(|a| a.get(&address.to_string()))
            .copied()
            .unwrap_or(0)
    }

    fn party_addresses(&self, game: &str) -> Vec<ServerAddress> {
        let alerts = self.cfg.party_alerts.get(game).into_iter().flatten();
        alerts.filter_map(|(a, _)| parse_address(a)).collect()
    }

    /// `edit_party_alert`: el diálogo con el umbral guardado o el de partida.
    pub(super) fn edit_party_alert(&mut self) -> Task<Message> {
        let (Some(info), Some(g)) = (self.selected_server().cloned(), self.current) else {
            return Task::none();
        };
        let current = self.party_threshold(g.id, &info.address);
        let current = if current == 0 {
            DEFAULT_THRESHOLD
        } else {
            current
        };
        self.open_dialog(Dialog::PartyAlert {
            game: g.id,
            address: info.address,
            name: info.name,
            text: current.to_string(),
        });
        // Como `QSpinBox`: el número elegido, listo para reemplazarlo.
        use iced::widget::operation::{focus, select_all};
        Task::batch([focus(PROMPT_ID), select_all(PROMPT_ID)])
    }

    pub(super) fn on_party_prompt(&mut self, m: PromptMsg) -> Task<Message> {
        let Some((
            Dialog::PartyAlert {
                game,
                address,
                name,
                text,
            },
            _,
        )) = &mut self.dialog
        else {
            return Task::none();
        };
        match m {
            PromptMsg::Input(t) => *text = t,
            PromptMsg::Submit => {
                let Some(threshold) = party_value(text) else {
                    return Task::none(); // *Guardar* apagado
                };
                let (game, address, name) = (*game, address.clone(), name.clone());
                let close = self.close_dialog();
                // El servidor como está en la tabla; si ya no está, sin punto de partida y
                // con el nombre del diálogo.
                let info = self
                    .servers()
                    .iter()
                    .find(|s| s.address == address)
                    .cloned()
                    .unwrap_or_else(|| {
                        let mut i = ServerInfo::new(address.clone(), address);
                        i.online = false;
                        i.name = name;
                        i
                    });
                return Task::batch([close, self.set_party_alert(game, &info, threshold)]);
            }
            PromptMsg::Cancel | PromptMsg::Forget => return self.close_dialog(),
        }
        Task::none()
    }

    /// Guarda o quita el umbral (0 = no avisar) y fija el punto de partida.
    pub fn set_party_alert(
        &mut self,
        game: &'static str,
        info: &ServerInfo,
        threshold: u32,
    ) -> Task<Message> {
        let key = info.address.to_string();
        let alerts = self.cfg.party_alerts.entry(game.to_owned()).or_default();
        let message = if threshold > 0 {
            alerts.insert(key.clone(), threshold);
            tr_n(
                "Avisaré cuando {server} tenga {n} jugador",
                "Avisaré cuando {server} tenga {n} jugadores",
                i64::from(threshold),
                &[("server", &info.name)],
            )
        } else {
            alerts.shift_remove(&key);
            tr_fmt(
                "Ya no se avisa de partidas en {server}",
                &[("server", &info.name)],
            )
        };
        if alerts.is_empty() {
            self.cfg.party_alerts.shift_remove(game);
        }
        self.watch.party.reset(game, &key);
        self.watch.party.update(game, info, threshold.max(1)); // punto de partida
        self.save_config();
        self.sync_watch();
        self.show_message(message, Some(Duration::from_secs(6)))
    }

    fn check_party(&mut self, game_id: &'static str, info: &ServerInfo) -> Task<Message> {
        let threshold = self.party_threshold(game_id, &info.address);
        if threshold == 0 || !self.watch.party.update(game_id, info, threshold) {
            return Task::none();
        }
        let name = game(game_id).map_or(game_id, |g| g.name);
        let text = tr_n(
            "{server}: {n} jugador en {game}",
            "{server}: {n} jugadores en {game}",
            i64::from(info.humans()),
            &[("server", &info.name), ("game", &name)],
        );
        let click = Click::Reveal(game_id, info.address.clone());
        self.notify(&tr("Se armó partida"), &text, Some(click))
    }

    pub fn watching_friends(&self) -> bool {
        self.cfg.watch_friends && !self.cfg.friends.is_empty()
    }

    fn wants_rounds(&self) -> bool {
        self.watching_friends() || !self.cfg.party_alerts.is_empty()
    }

    /// Favoritos y recientes (si se vigilan amigos) más los umbrales, sin repetir.
    pub fn watch_targets(&self) -> Vec<(&'static GameDef, Vec<ServerAddress>)> {
        let friends = self.watching_friends();
        GAMES
            .iter()
            .filter_map(|g| {
                let mut all = if friends {
                    let mut mine = favorite_addresses(&self.cfg, g.id);
                    mine.extend(recent_addresses(&self.cfg, g.id));
                    mine
                } else {
                    Vec::new()
                };
                all.extend(self.party_addresses(g.id));
                let mut addresses: Vec<ServerAddress> = Vec::new();
                for a in all {
                    if !addresses.contains(&a) {
                        addresses.push(a);
                    }
                }
                (!addresses.is_empty()).then_some((g, addresses))
            })
            .collect()
    }

    /// Tras cambiar amigos, la casilla o un umbral: si ya no toca, la vuelta se corta.
    pub fn sync_watch(&mut self) {
        if !self.wants_rounds() {
            if let Some(handle) = self.watch.round_handle.take() {
                handle.abort();
            }
            self.watch.round += 1;
        }
    }

    /// Una vuelta a la vez: los juegos uno tras otro, cada uno con sus consultas en paralelo.
    fn round_tick(&mut self) -> Task<Message> {
        if self.watch.round_handle.is_some() || !self.wants_rounds() {
            return Task::none();
        }
        self.watch.round += 1;
        let round = self.watch.round;
        let query = self.deps.query.clone();
        let seen = stream::iter(self.watch_targets()).flat_map(move |(g, addresses)| {
            let query = query.clone();
            stream::iter(addresses)
                .map(move |a| {
                    let query = query.clone();
                    async move { query(g, a).await }
                })
                .buffer_unordered(DIRECT_CONCURRENCY)
                .filter_map(move |r| async move {
                    let info = r.ok().flatten()?;
                    Some(Message::Watch(WatchMsg::RoundSeen {
                        round,
                        game: g.id,
                        info,
                    }))
                })
        });
        let done = stream::once(async move { Message::Watch(WatchMsg::RoundDone(round)) });
        let (task, handle) = Task::stream(seen.chain(done)).abortable();
        self.watch.round_handle = Some(handle);
        task
    }

    /// `QInputDialog.getInt`: etiqueta con el servidor, campo y *Guardar* (apagado si el
    /// número no vale) / *Cancelar*.
    pub(super) fn party_view<'a>(&self, name: &str, value: &'a str) -> Element<'a, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let msg = Message::Prompt;
        let field = widgets::field(
            PROMPT_ID,
            "",
            value,
            move |t| msg(PromptMsg::Input(t)),
            Some(msg(PromptMsg::Submit)),
            true,
            theme_id,
        );
        let save = party_value(value).map(|_| msg(PromptMsg::Submit));
        let buttons = row![
            Space::new().width(Fill),
            widgets::button_maybe(tr("Guardar"), save, theme_id),
            widgets::button(tr("Cancelar"), msg(PromptMsg::Cancel), theme_id),
        ]
        .spacing(6);
        let content = column![
            label(tr("Avisar cuando se arme partida")).font(style::header_font(s)),
            label(tr_fmt(
                "Avisar cuando {server} tenga al menos estos jugadores (0 = no avisar):",
                &[("server", &name)],
            )),
            field,
            buttons,
        ];
        card(content, 360.0, theme_id)
    }
}
