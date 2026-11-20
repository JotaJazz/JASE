//! *Buscar jugador…* (`player_search.py`): el texto, los hallazgos y su vista, sin `App`. La
//! ventana y el revelar el servidor elegido viven en `app/tools.rs`.

use std::time::Duration;

use iced::widget::text::Wrapping;
use iced::widget::{Column as Col, container, responsive, text};
use iced::{Element, Fill, Length, Task};
use indexmap::IndexMap;
use jase_core::games::game;
use jase_core::i18n::{tr, tr_n};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::player_search::find_players;

use crate::app::{Message, SearchMsg};
use crate::card::one_line;
use crate::details::{COLUMN_WIDTH, Head, TEXT_MARGIN, ToolScroll, detail_table};
use crate::theme::{palette, style, widget_style};
use crate::widgets;

/// Lo que se espera tras la última tecla para buscar (`DEBOUNCE_MS`).
pub const DEBOUNCE: Duration = Duration::from_millis(250);
/// Tope de hallazgos (el `limit` de `find_players`).
pub const MAX_HITS: usize = 500;

/// Una fila: el jugador (nombre plano), su servidor y dónde está.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub game: &'static str,
    pub address: ServerAddress,
    pub player: String,
    pub server: String,
    pub map: String,
}

#[derive(Debug)]
pub struct PlayerSearch {
    pub query: String,
    pub hits: Vec<Hit>,
    pub selected: Option<usize>,
    /// Juegos en los que se buscó (la nota de abajo).
    pub games: usize,
    pub field: iced::widget::Id,
    pub scroll: ToolScroll,
    /// Sube con cada tecla; `shown` es el texto de los hallazgos a la vista.
    seq: u64,
    shown: u64,
}

impl Default for PlayerSearch {
    fn default() -> Self {
        PlayerSearch {
            query: String::new(),
            hits: Vec::new(),
            selected: None,
            games: 0,
            // Las operaciones de foco van a todas las ventanas: una id propia.
            field: iced::widget::Id::unique(),
            scroll: ToolScroll::default(),
            seq: 0,
            shown: 0,
        }
    }
}

impl PlayerSearch {
    /// Busca el texto actual en `lists`, elige la primera fila y la desplaza a la vista.
    pub fn refresh(&mut self, lists: &IndexMap<String, Vec<ServerInfo>>) -> Task<Message> {
        self.games = lists.len();
        self.hits = find_players(lists, &self.query, MAX_HITS)
            .into_iter()
            .map(|h| Hit {
                game: h.game_id,
                address: h.server.address.clone(),
                player: h.player.name.clone(),
                server: h.server.name.clone(),
                map: h.server.map.clone(),
            })
            .collect();
        self.selected = (!self.hits.is_empty()).then_some(0);
        self.shown = self.seq;
        self.scroll.rebuilt(self.hits.len(), self.selected)
    }

    /// Texto nuevo; devuelve su número para el plazo de `DEBOUNCE`.
    pub fn edit(&mut self, text: String) -> u64 {
        self.query = text;
        self.seq += 1;
        self.seq
    }

    /// `seq` es el último texto escrito.
    pub fn is_latest(&self, seq: u64) -> bool {
        seq == self.seq
    }

    /// Los hallazgos a la vista son de un texto anterior.
    pub fn stale(&self) -> bool {
        self.shown != self.seq
    }

    pub fn note(&self) -> String {
        tr_n(
            "Busca en el juego cargado en esta sesión",
            "Busca en los {n} juegos cargados en esta sesión",
            self.games as i64,
            &[],
        )
    }

    /// Campo, tabla de 4 columnas sin orden (la última estira) y la nota.
    pub fn view(&self, theme_id: &'static str) -> Element<'_, Message> {
        let (p, s) = (palette(theme_id), widget_style(theme_id));
        let field = widgets::field(
            self.field.clone(),
            &tr("Nombre del jugador"),
            &self.query,
            |t| Message::Search(SearchMsg::Query(t)),
            Some(Message::Search(SearchMsg::Submit)),
            true,
            theme_id,
        );
        let table = responsive(move |size| {
            let head = |title: String, width: Length| Head {
                title,
                width,
                sort: None,
                margin: TEXT_MARGIN,
            };
            let heads = vec![
                head(tr("Jugador"), COLUMN_WIDTH.into()),
                head(tr("Servidor"), COLUMN_WIDTH.into()),
                head(tr("Juego"), COLUMN_WIDTH.into()),
                head(tr("Mapa"), Fill),
            ];
            // Texto plano (datos remotos), con el color de la fila elegida.
            let (_, chosen) = style::selection_colors(p, s, false);
            let rows = self
                .hits
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let color = if self.selected == Some(i) {
                        chosen
                    } else {
                        p.text
                    };
                    let short = game(h.game).map_or(h.game, |g| g.short_name());
                    [&h.player, &h.server, short, &h.map]
                        .into_iter()
                        .map(|t| {
                            text(one_line(t))
                                .color(color)
                                .font(s.font())
                                .size(s.text_size())
                                .wrapping(Wrapping::None)
                                .into()
                        })
                        .collect()
                })
                .collect();
            let on_row: fn(usize) -> (Message, Message) = |i| {
                (
                    Message::Search(SearchMsg::Select(i)),
                    Message::Search(SearchMsg::Choose(i)),
                )
            };
            detail_table(
                heads,
                rows,
                theme_id,
                size.height,
                self.selected,
                Some(on_row),
                Vec::new(),
                Some((self.scroll.id.clone(), |v| {
                    let (y, h) = (v.absolute_offset().y, v.bounds().height);
                    Message::Search(SearchMsg::Scrolled(y, h))
                })),
            )
        });
        let note = text(self.note())
            .color(p.text)
            .font(s.font())
            .size(s.text_size())
            .wrapping(Wrapping::Word);
        container(Col::new().push(field).push(table).push(note).spacing(6))
            .padding(10)
            .width(Fill)
            .height(Fill)
            .style(move |_| style::window(p))
            .into()
    }
}
