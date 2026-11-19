//! *Ahora se juega* (`now_playing.py`): los servidores con gente de los juegos listos y su
//! vista, sin `App`. Los escaneos y la ventana viven en `app/tools.rs`.

use iced::widget::text::Wrapping;
use iced::widget::{Column as Col, Space, container, responsive, row, text};
use iced::{Element, Fill, Length, window};
use indexmap::IndexMap;
use jase_core::games::game;
use jase_core::i18n::{tr, tr_n};
use jase_core::models::{ServerAddress, ServerInfo};

use crate::app::{Message, NowMsg};
use crate::card::one_line;
use crate::details::{COLUMN_WIDTH, Head, TEXT_MARGIN, detail_table};
use crate::table::players_text;
use crate::theme::{palette, style, widget_style};
use crate::widgets;

pub const MAX_ROWS: usize = 300;
/// Un servidor solo con bots no cuenta.
pub const MIN_PEOPLE: i32 = 1;
/// Escaneos a la vez.
pub const PARALLEL_SCANS: usize = 3;

/// (juego, servidor) en línea con al menos `MIN_PEOPLE` personas, de más a menos gente y a
/// igualdad menor ping.
pub fn busy_servers(
    lists: &IndexMap<String, Vec<ServerInfo>>,
    limit: usize,
) -> Vec<(String, &ServerInfo)> {
    let mut rows: Vec<(&String, &ServerInfo)> = lists
        .iter()
        .flat_map(|(g, servers)| servers.iter().map(move |i| (g, i)))
        .filter(|(_, i)| i.online && i.humans() >= MIN_PEOPLE)
        .collect();
    rows.sort_by_key(|(_, i)| (-i.humans(), i.ping_ms));
    rows.truncate(limit);
    // El juego se copia solo para los que quedan.
    rows.into_iter().map(|(g, i)| (g.clone(), i)).collect()
}

/// Una fila de la tabla, con los textos ya armados.
#[derive(Debug, Clone, PartialEq)]
pub struct NowRow {
    pub game: String,
    pub address: ServerAddress,
    pub short: String,
    pub name: String,
    pub players: String,
    pub map: String,
    pub ping: u32,
}

/// Las filas de `busy_servers` (hasta `MAX_ROWS`).
pub fn rows(lists: &IndexMap<String, Vec<ServerInfo>>) -> Vec<NowRow> {
    busy_servers(lists, MAX_ROWS)
        .into_iter()
        .map(|(g, info)| NowRow {
            short: game(&g).map_or_else(|| g.clone(), |d| d.short_name().to_owned()),
            game: g,
            address: info.address.clone(),
            name: info.name.clone(),
            players: players_text(info),
            map: info.map.clone(),
            ping: info.ping_ms,
        })
        .collect()
}

/// «{n} jugadores en tus juegos» (personas en línea) y, con escaneos pendientes, cuántos.
pub fn status_text(lists: &IndexMap<String, Vec<ServerInfo>>, pending: usize) -> String {
    let people: i64 = lists
        .values()
        .flatten()
        .filter(|i| i.online)
        .map(|i| i64::from(i.humans()))
        .sum();
    let mut text = tr_n(
        "{n} jugador en tus juegos",
        "{n} jugadores en tus juegos",
        people,
        &[],
    );
    if pending > 0 {
        text.push_str(" · ");
        text.push_str(&tr_n(
            "escaneando 1 juego…",
            "escaneando {n} juegos…",
            pending as i64,
            &[],
        ));
    }
    text
}

/// Cabecera, tabla de 5 columnas sin orden (*Servidor* estira), estado y *Actualizar* (apagado
/// con escaneos pendientes) / *Cerrar*. Los nombres van como texto plano.
pub fn view<'a>(
    rows: &'a [NowRow],
    selected: Option<usize>,
    status: &'a str,
    busy: bool,
    window: window::Id,
    theme_id: &'static str,
) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let label = |t: String, wrap: Wrapping| {
        text(t)
            .color(p.text)
            .font(s.font())
            .size(s.text_size())
            .wrapping(wrap)
    };
    let header = label(
        tr(
            "Servidores con gente en tus juegos instalados, de más a menos jugadores (sin contar bots). Doble clic para verlo.",
        ),
        Wrapping::Word,
    );
    let table = responsive(move |size| {
        let head = |title: String, width: Length| Head {
            title,
            width,
            sort: None,
            margin: TEXT_MARGIN,
        };
        let heads = vec![
            head(tr("Juego"), COLUMN_WIDTH.into()),
            head(tr("Servidor"), Fill),
            head(tr("Jugadores"), COLUMN_WIDTH.into()),
            head(tr("Mapa"), COLUMN_WIDTH.into()),
            head(tr("Ping"), COLUMN_WIDTH.into()),
        ];
        let (_, chosen) = style::selection_colors(p, s, false);
        let cells = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let color = if selected == Some(i) { chosen } else { p.text };
                let cell = |t: &str| {
                    text(one_line(t))
                        .color(color)
                        .font(s.font())
                        .size(s.text_size())
                        .wrapping(Wrapping::None)
                };
                let long = game(&r.game).map_or(r.game.as_str(), |g| g.name);
                vec![
                    widgets::tip(cell(&r.short), long.to_owned(), p, s),
                    widgets::tip(cell(&r.name), one_line(&r.name), p, s),
                    cell(&r.players).into(),
                    cell(&r.map).into(),
                    cell(&r.ping.to_string()).into(),
                ]
            })
            .collect();
        let on_row: fn(usize) -> (Message, Message) = |i| {
            (
                Message::Now(NowMsg::Select(i)),
                Message::Now(NowMsg::Choose(i)),
            )
        };
        detail_table(heads, cells, theme_id, size.height, selected, Some(on_row))
    });
    let refresh = (!busy).then_some(Message::Now(NowMsg::Refresh));
    let buttons = row![
        Space::new().width(Fill),
        widgets::button_maybe(tr("Actualizar"), refresh, theme_id),
        widgets::button(tr("Cerrar"), Message::CloseTool(window), theme_id),
    ]
    .spacing(6);
    let content = Col::new()
        .push(header)
        .push(table)
        .push(label(status.to_owned(), Wrapping::Word))
        .push(buttons)
        .spacing(6);
    container(content)
        .padding(10)
        .width(Fill)
        .height(Fill)
        .style(move |_| style::window(p))
        .into()
}
