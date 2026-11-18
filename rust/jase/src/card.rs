//! Ficha del servidor elegido (`server_card.py`): se deriva en cada `view` del servidor
//! elegido, como `_update_card` en cada cambio de la lista; no guarda estado propio.

use iced::font::{Style as FontStyle, Weight};
use iced::widget::text::{LineHeight, Span, Wrapping};
use iced::widget::{Column as Col, Space, column, container, rich_text, row, span, svg, text};
use iced::{Center, Color, Element, Fill, Font, Length, Size};
use jase_core::favorites::{get_note, is_favorite};
use jase_core::geo::country_name;
use jase_core::i18n::tr;
use jase_core::models::ServerInfo;

use crate::actions::{Action, Msg as ActionMsg};
use crate::app::{App, Message};
use crate::colored::color_runs;
use crate::flags::flag;
use crate::theme::{Edges, Palette, WidgetStyle, framed, palette, ping_color, style, widget_style};
use crate::widgets::{button_with, text_width, tip};

pub const MAP_IMAGE_SIZE: Size = Size::new(160.0, 120.0);
pub const CARD_FLAG_SIZE: Size = Size::new(24.0, 18.0);
/// `MAP_IMAGE_SIZE.height + 18`: imagen y márgenes de 8 más el borde.
pub const CARD_HEIGHT: f32 = 138.0;
/// `contentsMargins(8, 8, 8, 8)` de la ficha.
const MARGIN: f32 = 8.0;
/// `grid.setColumnMinimumWidth(1, 200)`.
const VALUE_MIN: f32 = 200.0;

/// Lo que muestra la ficha, ya con sus textos y colores.
#[derive(Debug, Clone, PartialEq)]
pub struct CardVm {
    /// Tramos del nombre; `None` = color de texto.
    pub name: Vec<(String, Option<Color>)>,
    pub name_tip: String,
    pub address: String,
    pub password: bool,
    pub favorite: bool,
    /// Nota del favorito; vacía, no se muestra.
    pub note: String,
    /// Código de la bandera (si hay SVG) y nombre del país.
    pub country: Option<(Option<String>, String)>,
    pub map: String,
    pub mode: String,
    pub players: String,
    pub ping: String,
    pub ping_color: Color,
    pub ping_bold: bool,
    pub version: String,
    /// Texto del recuadro del mapa sin imagen: el nombre del mapa.
    pub placeholder: String,
}

pub fn empty_text() -> String {
    tr("Selecciona un servidor")
}

/// El servidor elegido, si se ve: el que quedó oculto por un filtro no cuenta.
pub(crate) fn server(app: &App) -> Option<&ServerInfo> {
    app.selected_server()
        .filter(|i| app.table.model.is_shown(&i.address))
}

pub fn vm(app: &App) -> Option<CardVm> {
    let info = server(app)?;
    let p = palette(app.theme_id);
    let cfg = app.config();
    let game = app.current_game();
    let favorite = game.is_some_and(|g| is_favorite(cfg, g.id, &info.address));
    let note = match game {
        Some(g) if favorite => get_note(cfg, g.id, &info.address.to_string()).to_owned(),
        _ => String::new(),
    };
    let name = if info.raw_name.is_empty() {
        vec![(info.name.clone(), None)]
    } else {
        color_runs(&info.raw_name, p)
    };
    let country = app.table.model.country_code(info).map(|code| {
        (
            flag(code).map(|_| code.to_owned()),
            country_name(Some(code)),
        )
    });
    let version = if info.version.is_empty() {
        "—".to_owned()
    } else {
        app.table.model.version_label(&info.version)
    };
    // Un favorito sin respuesta no tiene datos que mostrar.
    let (map, mode, players, ping, ping_color, ping_bold) = if info.online {
        (
            info.map.clone(),
            info.gametype.clone(),
            format!("{} / {}", info.num_players, info.max_players),
            format!("{} ms", info.ping_ms),
            ping_color(info.ping_ms, p),
            true,
        )
    } else {
        let dash = || "—".to_owned();
        (dash(), dash(), dash(), tr("Sin respuesta"), p.muted, false)
    };
    Some(CardVm {
        name,
        name_tip: info.name.clone(),
        address: info.connect_address.to_string(),
        password: info.password,
        favorite,
        note,
        country,
        map,
        mode,
        players,
        ping,
        ping_color,
        ping_bold,
        version,
        placeholder: info.map.clone(),
    })
}

/// Alto fijo de la ficha (el gráfico de ocupación lo agrandará).
pub fn height(_app: &App) -> f32 {
    CARD_HEIGHT
}

/// Texto remoto en una sola línea: los saltos de línea no agrandan la celda.
pub(crate) fn one_line(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

fn edge_sum(e: Option<Edges>, horizontal: bool) -> f32 {
    let w = |side: Option<(f32, Color)>| side.map_or(0.0, |(w, _)| w);
    e.map_or(0.0, |e| {
        if horizontal {
            w(e.left) + w(e.right)
        } else {
            w(e.top) + w(e.bottom)
        }
    })
}

/// Etiqueta de texto plano en una línea, recortada si no cabe.
/// Con la línea de Qt (la de Iced, 1,3 × letra, separa más las filas de la rejilla).
fn label<'a>(s: &str, color: Color, font: Font, st: WidgetStyle) -> iced::widget::Text<'a> {
    text(one_line(s))
        .color(color)
        .font(font)
        .size(st.text_size())
        .line_height(LineHeight::Absolute(style::qt_line_height(st).into()))
        .wrapping(Wrapping::None)
}

pub fn view(app: &App) -> Element<'_, Message> {
    let id = app.theme_id;
    let (p, s) = (palette(id), widget_style(id));
    let (frame, edges) = style::card(p, s, id);
    let content: Element<'_, Message> = match vm(app) {
        None => container(label(&empty_text(), style::card_caption(p), s.font(), s))
            .center(Fill)
            .into(),
        Some(v) => filled(v, id, p, s),
    };
    // El borde de Iced se dibuja sobre el relleno; las franjas de `framed`, por fuera.
    let card = container(content)
        .padding(MARGIN + frame.border.width)
        .width(Fill)
        .height(height(app) - edge_sum(edges, false))
        .style(move |_| frame);
    match edges {
        Some(e) => framed(card, e),
        None => card.into(),
    }
}

fn filled<'a>(
    v: CardVm,
    id: &'static str,
    p: &'static Palette,
    s: WidgetStyle,
) -> Element<'a, Message> {
    let (font, size) = (s.font(), s.text_size());
    let muted = style::card_caption(p);
    // 8 pt de época, la ficha 9 pt (`_font_rules`); los demás, la letra × 1,15. En negrita.
    let name_size = match s {
        WidgetStyle::Win98 | WidgetStyle::Luna | WidgetStyle::Vgui => 9.0 * 96.0 / 72.0,
        WidgetStyle::Flat | WidgetStyle::Terminal => size * 1.15,
    };
    let bold = Font {
        weight: Weight::Bold,
        ..font
    };
    let spans: Vec<Span<'static>> = v
        .name
        .into_iter()
        .map(|(t, c)| span(one_line(&t)).color(c.unwrap_or(p.text)))
        .collect();
    let name = container(
        rich_text(spans)
            .font(bold)
            .size(name_size)
            .wrapping(Wrapping::None),
    )
    .width(Fill)
    .clip(true);
    let name: Element<'a, Message> = if v.name_tip.is_empty() {
        name.into()
    } else {
        tip(name, v.name_tip, p, s)
    };
    let lock = v.password.then(|| {
        tip(
            label(&tr("🔒 Contraseña"), muted, font, s),
            tr("El servidor requiere contraseña"),
            p,
            s,
        )
    });
    let (star, star_tip) = if v.favorite {
        ("★", tr("Quitar de favoritos"))
    } else {
        ("☆", tr("Agregar a favoritos"))
    };
    // Apagada hasta la 5f (`Action::Favorite` no corre): se dibuja igual que encendida.
    let star = button_with(
        text(star)
            .color(style::favorite_star(p, v.favorite))
            .font(font)
            .size(size),
        [0, 6].into(),
        Message::Actions(ActionMsg::Run(Action::Favorite)),
        id,
    );
    let name_row = row![name]
        .push(lock)
        .push(tip(star, star_tip, p, s))
        .spacing(6)
        .align_y(Center);
    let copy = button_with(
        text(tr("Copiar")).font(font).size(size),
        style::copy_button(),
        Message::Actions(ActionMsg::Run(Action::Copy)),
        id,
    );
    let mut address_row = row![
        label(&v.address, p.text, font, s),
        tip(copy, tr("Copiar IP:puerto al portapapeles"), p, s),
        Space::new().width(6) // `addSpacing(12)` más la separación de la fila
    ]
    .spacing(6)
    .align_y(Center);
    if let Some((code, country)) = &v.country {
        let flag = code.as_deref().and_then(flag).map(|h| {
            svg(h)
                .width(CARD_FLAG_SIZE.width)
                .height(CARD_FLAG_SIZE.height)
        });
        address_row = address_row.push(flag).push(label(country, p.text, font, s));
    }
    let note = (!v.note.is_empty()).then(|| {
        let italic = Font {
            style: FontStyle::Italic,
            ..font
        };
        let note = container(label(&v.note, p.text, italic, s)).clip(true);
        tip(note, v.note.clone(), p, s)
    });
    // Rejilla de 10 × 4: las columnas de títulos miden su título más largo.
    let [map_c, mode_c, players_c, ping_c, version_c] = [
        tr("Mapa"),
        tr("Modo"),
        tr("Jugadores"),
        tr("Ping"),
        tr("Versión"),
    ];
    let caption_w = |titles: &[&String]| {
        titles
            .iter()
            .map(|t| text_width(t, font, size))
            .fold(0.0, f32::max)
            .ceil()
    };
    let (cap0, cap2) = (
        caption_w(&[&map_c, &players_c, &version_c]),
        caption_w(&[&mode_c, &ping_c]),
    );
    let cell = |content: iced::widget::Text<'a>, w: Length| container(content).width(w).clip(true);
    let caption = |title: &str, w: f32| cell(label(title, muted, font, s), w.into());
    let value = |t: &str, w: Length| cell(label(t, p.text, font, s), w);
    let ping_font = if v.ping_bold {
        Font {
            weight: Weight::Semibold,
            ..font
        }
    } else {
        font
    };
    let ping = cell(label(&v.ping, v.ping_color, ping_font, s), Fill);
    let grid = column![
        row![
            caption(&map_c, cap0),
            value(&v.map, VALUE_MIN.into()),
            caption(&mode_c, cap2),
            value(&v.mode, Fill)
        ]
        .spacing(10),
        row![
            caption(&players_c, cap0),
            value(&v.players, VALUE_MIN.into()),
            caption(&ping_c, cap2),
            ping
        ]
        .spacing(10),
        row![caption(&version_c, cap0), value(&v.version, Fill)].spacing(10),
    ]
    .spacing(4);
    let text_col = Col::new()
        .push(name_row)
        .push(address_row)
        .push(note)
        .push(grid)
        .spacing(4)
        .width(Fill);
    row![map_box(&v.placeholder, p, s), text_col]
        .spacing(12)
        .into()
}

/// Recuadro del mapa sin imagen: el nombre del mapa centrado, con ajuste de línea.
fn map_box<'a>(map: &str, p: &'static Palette, s: WidgetStyle) -> Element<'a, Message> {
    let (frame, edges) = style::map_frame(p, s);
    let inner = container(
        text(map.to_owned())
            .color(p.muted)
            .font(s.font())
            .size(s.text_size())
            .align_x(Center),
    )
    .center(Fill)
    .padding(frame.border.width)
    .clip(true)
    .width(MAP_IMAGE_SIZE.width - edge_sum(edges, true))
    .height(MAP_IMAGE_SIZE.height - edge_sum(edges, false))
    .style(move |_| frame);
    match edges {
        Some(e) => framed(inner, e),
        None => inner.into(),
    }
}
