//! Barra lateral (`_build_quick_filters` de `window_filters.py`): «Solo juegos instalados», el
//! árbol de juegos, la lista de vistas directas, los cuatro filtros rápidos y los combos «Ping
//! máx.» y «País» (el campo de texto y el combo de versión van en la barra de herramientas,
//! `bars.rs`).

use std::collections::HashMap;
use std::fmt;

use iced::widget::{Column, Space, column, container, mouse_area, row, text};
use iced::{Element, Fill, mouse};
use jase_core::games::{ALL_VERSIONS, GameDef};
use jase_core::geo::country_name;
use jase_core::i18n::tr;

use crate::app::{App, DirectMsg, DirectView, Focus, Message};
use crate::presets;
use crate::theme::{framed, palette, style, widget_style};
use crate::tree;
use crate::widgets;

/// Cada combo con lista: abrir uno y cerrar otro llegan en cualquier orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combo {
    Ping,
    Country,
    Version,
}

#[derive(Debug, Clone)]
pub enum Msg {
    HideEmpty(bool),
    HideFull(bool),
    HidePassword(bool),
    HideBots(bool),
    MaxPing(i64),
    Country(String),
    /// El combo de países se abre: se rellena con los países de la tabla (`CountryCombo`).
    CountryOpened,
    /// Se abre otro combo: se queda el foco (las flechas no llegan al árbol ni a la tabla).
    ComboOpened(Combo),
    /// La lista de un combo se cerró sin elegir.
    ComboClosed(Combo),
    OnlyInstalled(bool),
    Text(String),
    Version(String),
}

/// Opciones de «Ping máx.» (texto → ms; 0 = sin límite).
pub fn max_ping_options() -> Vec<(String, i64)> {
    let mut v = vec![(tr("Sin límite"), 0)];
    v.extend([50, 100, 150, 250].map(|ms| (ms.to_string(), ms)));
    v
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingOption {
    pub ms: i64,
    pub label: String,
}

impl fmt::Display for PingOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// [`max_ping_options`] y, si el guardado no está, él tal cual al final (`_select_ping`).
pub fn ping_options(saved: i64) -> Vec<PingOption> {
    let mut v: Vec<PingOption> = max_ping_options()
        .into_iter()
        .map(|(label, ms)| PingOption { ms, label })
        .collect();
    if !v.iter().any(|o| o.ms == saved) {
        v.push(PingOption {
            ms: saved,
            label: saved.to_string(),
        });
    }
    v
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryOption {
    /// Código ISO en minúsculas; `""` = todos.
    pub code: String,
    pub label: String,
}

impl fmt::Display for CountryOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// «Todos los países» y los de `counts` por nombre, «Nombre (n)» si hay servidores; el
/// elegido queda aunque ya no tenga (`_set_country_items`).
pub fn country_options(counts: &HashMap<String, usize>, selected: &str) -> Vec<CountryOption> {
    let mut codes: Vec<&str> = counts.keys().map(String::as_str).collect();
    if !selected.is_empty() && !counts.contains_key(selected) {
        codes.push(selected);
    }
    let mut named: Vec<(String, &str)> = codes
        .into_iter()
        .map(|c| (country_name(Some(c)), c))
        .collect();
    named.sort_by_cached_key(|(name, _)| name.to_lowercase());
    let mut v = vec![CountryOption {
        code: String::new(),
        label: tr("Todos los países"),
    }];
    v.extend(named.into_iter().map(|(name, code)| {
        let label = match counts.get(code) {
            Some(&n) if n > 0 => format!("{name} ({n})"),
            _ => name,
        };
        CountryOption {
            code: code.to_owned(),
            label,
        }
    }));
    v
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionOption {
    /// Lo que filtra.
    pub key: String,
    /// Lo que se ve (`version_label`: «1.4 (CoD2x)»).
    pub label: String,
}

impl fmt::Display for VersionOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// «Todas», las conocidas, la guardada si falta y las vistas en los servidores, en ese orden
/// (`_populate_version_combo` + `_add_observed_versions`).
pub fn version_options(game: &GameDef, selected: &str, observed: &[String]) -> Vec<VersionOption> {
    let mut keys: Vec<&str> = vec![ALL_VERSIONS];
    keys.extend(game.known_versions);
    for k in std::iter::once(selected).chain(observed.iter().map(String::as_str)) {
        if !k.is_empty() && !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys.into_iter()
        .map(|k| VersionOption {
            key: k.to_owned(),
            label: game.version_label(k).to_owned(),
        })
        .collect()
}

/// Columna izquierda: casilla de instalados, árbol en su marco, filtros rápidos y combos.
pub fn view(app: &App) -> Element<'_, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let label = |t: String| text(t).font(s.font()).size(s.text_size());
    let q = app.quick_filters();
    // En Favoritos/Recientes los filtros rápidos no se aplican: salen apagados.
    let on = app.direct_view().is_none();
    let check_maybe = |on: bool, checked: bool, t: String, msg: fn(bool) -> Msg| {
        widgets::check(
            checked,
            Some(label(t).into()),
            on.then(|| Message::Sidebar(msg(!checked))),
            p,
            s,
        )
    };
    let check = |checked, t, msg| check_maybe(on, checked, t, msg);
    let only_installed = widgets::tip(
        check_maybe(
            true,
            app.cfg_only_installed(),
            tr("Solo juegos instalados"),
            Msg::OnlyInstalled,
        ),
        tr("Lista solo los juegos con ruta válida en Ajustes y los que se abren con Steam"),
        p,
        s,
    );
    let list_style = style::sidebar(p, s);
    let games = container(tree::view(
        &app.tree,
        app.theme_id,
        app.focus() == Focus::Tree,
    ))
    .height(Fill)
    .padding(list_style.border.width)
    .style(move |_| list_style);
    let games = match style::table_frame(p, s).1 {
        Some(e) => framed(games, e),
        None => games.into(),
    };
    let direct = direct_list(app);
    let hide_bots = widgets::tip(
        check(q.hide_bots, tr("Ocultar solo bots"), Msg::HideBots),
        tr("Oculta los servidores cuyos jugadores son todos bots"),
        p,
        s,
    );
    let ping = widgets::tip(
        widgets::combo(
            ping_options(q.max_ping),
            ping_options(q.max_ping)
                .into_iter()
                .find(|o| o.ms == q.max_ping),
            |o: PingOption| Message::Sidebar(Msg::MaxPing(o.ms)),
            Some((
                Message::Sidebar(Msg::ComboOpened(Combo::Ping)),
                Message::Sidebar(Msg::ComboClosed(Combo::Ping)),
            )),
            on,
            app.theme_id,
        ),
        tr("Ocultar servidores con ping mayor"),
        p,
        s,
    );
    let countries = app.country_options();
    let chosen = countries.iter().find(|o| o.code == q.country).cloned();
    let country = widgets::tip(
        widgets::combo(
            countries,
            chosen,
            |o: CountryOption| Message::Sidebar(Msg::Country(o.code)),
            Some((
                Message::Sidebar(Msg::CountryOpened),
                Message::Sidebar(Msg::ComboClosed(Combo::Country)),
            )),
            on,
            app.theme_id,
        ),
        tr("Mostrar solo servidores de un país"),
        p,
        s,
    );
    // Etiquetas en una columna y combos en otra, como el `QGridLayout`: quedan alineados.
    let h = widgets::field_height(s);
    let cell = |t: String| container(label(t)).height(h).center_y(h);
    let combos = row![
        column![cell(tr("Ping máx.")), cell(tr("País"))].spacing(6),
        column![ping, country].spacing(6).width(Fill),
    ]
    .spacing(6);
    Column::new()
        .push(only_installed)
        .push(games)
        .push(direct)
        .push(check(q.hide_empty, tr("Ocultar vacíos"), Msg::HideEmpty))
        .push(check(q.hide_full, tr("Ocultar llenos"), Msg::HideFull))
        .push(check(
            q.hide_password,
            tr("Ocultar con contraseña"),
            Msg::HidePassword,
        ))
        .push(hide_bots)
        .push(combos)
        .push(presets::button(app))
        .spacing(6)
        .width(app.sidebar_width)
        .height(Fill)
        .into()
}

/// «★ Favoritos · N» y «🕘 Recientes · M» bajo el árbol (`favorites_list`): mismo marco y
/// fondo que el árbol, alto justo para sus dos filas; la de la vista vigente sale elegida.
fn direct_list(app: &App) -> Element<'_, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let h = tree::plain_row_height(s) as f32;
    let focused = app.focus() == Focus::Sidebar;
    let tips = [
        tr("Favoritos del juego seleccionado: se consultan directamente, sin master"),
        tr("Servidores a los que te conectaste, del más reciente al más antiguo"),
    ];
    let views = [DirectView::Favorites, DirectView::Recent];
    let rows = app
        .direct_list_labels()
        .into_iter()
        .zip(tips)
        .zip(views)
        .map(|((label, tip), view)| {
            let selected = app.direct_view() == Some(view);
            let item = container(text(label).font(s.font()).size(s.text_size()))
                .width(Fill)
                .height(h)
                .center_y(h)
                .padding(tree::item_padding(s))
                .clip(true)
                .style(move |_| style::list_item(p, s, selected, focused));
            let item: Element<'_, Message> = match style::list_item_edges(p, app.theme_id, selected)
            {
                Some(e) => framed(item, e),
                None => item.into(),
            };
            let item = mouse_area(item).on_press(Message::Direct(DirectMsg::Show(view)));
            widgets::tip(item, tip, p, s)
        });
    let list_style = style::sidebar(p, s);
    let list = container(Column::with_children(rows))
        .padding(list_style.border.width)
        .style(move |_| list_style);
    match style::table_frame(p, s).1 {
        Some(e) => framed(list, e),
        None => list.into(),
    }
}

/// El divisor con la tabla (`QSplitter::handle`): `SPLITTER_WIDTH` px del fondo de la
/// ventana; arrastrarlo cambia el ancho de la barra.
pub fn splitter<'a>() -> Element<'a, Message> {
    mouse_area(
        Space::new()
            .width(crate::window_state::SPLITTER_WIDTH)
            .height(Fill),
    )
    .interaction(mouse::Interaction::ResizingHorizontally)
    .on_press(Message::SplitterStart)
    // Un clic rápido llega antes que la suscripción del arrastre.
    .on_release(Message::DragEnd)
    .into()
}
