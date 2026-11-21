//! El diálogo de Ajustes (como `SettingsDialog` de `ui/dialogs.py`): lo editable, sin E/S, y
//! su vista; la app lo pega en `app/settings.rs`.

use std::collections::HashMap;
use std::path::Path;

use iced::widget::scrollable::Direction;
use iced::widget::text::Text;
use iced::widget::{
    Column, Id, Row, Space, column, container, mouse_area, rich_text, row, scrollable, span, svg,
    text,
};
use iced::{Border, Center, Color, Element, Fill, Padding};
use indexmap::IndexMap;
use jase_core::config::{AUTO_REFRESH_CHOICES, Config};
use jase_core::games::{Family, GAMES, GameDef, LaunchStyle, game};
use jase_core::i18n::{LANGUAGES, language_name, tr, tr_fmt, tr_n};
use jase_core::pystr::py_strip;

use crate::app::dialogs::card;
use crate::app::{Message, SettingsMsg};
use crate::bars::{ToolItem, tool_name, toolbar_layout, toolbar_to_config};
use crate::find_games::{self, Search};
use crate::icons;
use crate::masters_tab::{self, MastersTab};
use crate::theme::{
    TERMINAL_COLORS, THEME_IDS, WidgetStyle, framed, palette, style, terminal_color_name,
    theme_display_name, widget_style,
};
use crate::widgets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Games,
    Masters,
    General,
    Appearance,
    Toolbar,
    Notifications,
}

/// Estado de la carpeta de un juego.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStatus {
    Steam,
    Searching,
    /// El exe hallado.
    Found(String),
    Missing,
}

/// El botón «Actualizar base de países».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountryButton {
    Disabled,
    Ready,
    Downloading,
}

#[derive(Debug, Clone)]
pub struct Form {
    pub tab: Tab,
    pub paths: IndexMap<&'static str, String>,
    pub status: HashMap<&'static str, PathStatus>,
    pub download_maps: bool,
    pub language: String,
    pub auto_refresh: u32,
    pub check_updates: bool,
    pub tray: bool,
    pub wine: String,
    pub steam_key: String,
    pub steam_include_empty: bool,
    pub theme: String,
    pub terminal_color: String,
    pub masters: MastersTab,
    pub countries: CountryButton,
    /// Pestaña Barra: el orden con su visibilidad y la fila elegida.
    pub toolbar: Vec<(ToolItem, bool)>,
    pub toolbar_selected: Option<usize>,
    /// Pestaña Avisos.
    pub notify_friends: bool,
    pub watch_friends: bool,
    pub notify_party: bool,
    pub notify_attention: bool,
    pub party_alerts: IndexMap<String, IndexMap<String, u32>>,
}

/// Sin carpeta no hay nada que leer del disco; con carpeta hay que buscar el exe.
pub fn initial_status(game: &GameDef, path: &str) -> PathStatus {
    if !py_strip(path).is_empty() {
        PathStatus::Searching
    } else if game.launch_style == LaunchStyle::Steam {
        PathStatus::Steam
    } else {
        PathStatus::Missing
    }
}

pub fn path_status_text(game: &GameDef, status: &PathStatus) -> String {
    match status {
        PathStatus::Steam => tr("Se conecta mediante Steam; no necesita carpeta"),
        PathStatus::Searching => tr("Buscando…"),
        PathStatus::Found(exe) => {
            let name = Path::new(exe)
                .file_name()
                .map_or(exe.clone(), |n| n.to_string_lossy().into_owned());
            format!("\u{2713} {name}")
        }
        PathStatus::Missing => {
            let wanted = if game.family == Family::Valve && !game.valve_gamedir.is_empty() {
                tr_fmt(
                    "{exe} con la carpeta {folder}",
                    &[("exe", &game.exe_relpath), ("folder", &game.valve_gamedir)],
                )
            } else {
                game.exe_relpath.to_owned()
            };
            format!(
                "\u{26a0} {}",
                tr_fmt("No se encontró {wanted}", &[("wanted", &wanted)])
            )
        }
    }
}

/// Como `findData` + `max(index, 0)`: un valor que no está entre las opciones abre en la
/// primera (`first`).
fn known_or(value: &str, first: &str, rest: &[&str]) -> String {
    if value == first || rest.contains(&value) {
        value
    } else {
        first
    }
    .to_owned()
}

impl Form {
    pub fn new(
        cfg: &Config,
        steam_key: &str,
        masters: MastersTab,
        countries: CountryButton,
    ) -> Form {
        let paths: IndexMap<&'static str, String> = GAMES
            .iter()
            .map(|g| (g.id, cfg.game_paths.get(g.id).cloned().unwrap_or_default()))
            .collect();
        let status = GAMES
            .iter()
            .map(|g| (g.id, initial_status(g, &paths[g.id])))
            .collect();
        Form {
            tab: Tab::Games,
            paths,
            status,
            download_maps: cfg.download_map_images,
            language: known_or(&cfg.language, "auto", &LANGUAGES),
            auto_refresh: Some(cfg.auto_refresh_minutes)
                .filter(|m| AUTO_REFRESH_CHOICES.contains(m))
                .unwrap_or(0),
            check_updates: cfg.check_app_updates,
            tray: cfg.minimize_to_tray,
            wine: cfg.wine_command.clone(),
            steam_key: steam_key.to_owned(),
            steam_include_empty: cfg.steam_include_empty,
            theme: known_or(&cfg.theme, "system", &THEME_IDS),
            terminal_color: known_or(&cfg.terminal_color, TERMINAL_COLORS[0], &TERMINAL_COLORS),
            masters,
            countries,
            toolbar: toolbar_layout(&cfg.toolbar),
            toolbar_selected: None,
            notify_friends: cfg.notify_friends,
            watch_friends: cfg.watch_friends,
            notify_party: cfg.notify_party,
            notify_attention: cfg.notify_attention,
            party_alerts: cfg.party_alerts.clone(),
        }
    }

    /// Mueve la fila elegida un lugar (`up` o abajo); sigue elegida.
    pub fn toolbar_move(&mut self, up: bool) {
        let Some(i) = self.toolbar_selected else {
            return;
        };
        let j = if up { i.checked_sub(1) } else { Some(i + 1) };
        if let Some(j) = j.filter(|&j| j < self.toolbar.len()) {
            self.toolbar.swap(i, j);
            self.toolbar_selected = Some(j);
        }
    }

    /// Un separador visible bajo la fila elegida (o al final).
    pub fn toolbar_add_separator(&mut self) {
        let at = self.toolbar_selected.map_or(self.toolbar.len(), |i| i + 1);
        self.toolbar.insert(at, (ToolItem::Separator, true));
        self.toolbar_selected = Some(at);
    }

    pub fn apply_to(&self, cfg: &mut Config) {
        for (&id, path) in &self.paths {
            let path = py_strip(path);
            // Vaciar un juego con ruta por defecto la guarda como "" para que `load_config`
            // no la vuelva a proponer.
            if !path.is_empty() || game(id).is_some_and(|g| !g.default_path.is_empty()) {
                cfg.game_paths.insert(id.to_owned(), path.to_owned());
            } else {
                cfg.game_paths.shift_remove(id);
            }
        }
        cfg.download_map_images = self.download_maps;
        cfg.language = self.language.clone();
        cfg.auto_refresh_minutes = self.auto_refresh;
        cfg.check_app_updates = self.check_updates;
        cfg.minimize_to_tray = self.tray;
        cfg.steam_include_empty = self.steam_include_empty;
        cfg.wine_command = py_strip(&self.wine).to_owned();
        cfg.theme = self.theme.clone();
        cfg.terminal_color = self.terminal_color.clone();
        cfg.toolbar = toolbar_to_config(&self.toolbar);
        cfg.notify_friends = self.notify_friends;
        cfg.watch_friends = self.watch_friends;
        cfg.notify_party = self.notify_party;
        cfg.notify_attention = self.notify_attention;
        cfg.party_alerts = self.party_alerts.clone();
    }

    /// Quita el umbral de partida de `address` en `game` (y el juego si queda vacío).
    pub fn remove_party_alert(&mut self, game: &str, address: &str) {
        if let Some(alerts) = self.party_alerts.get_mut(game) {
            alerts.shift_remove(address);
            if alerts.is_empty() {
                self.party_alerts.shift_remove(game);
            }
        }
    }

    pub fn steam_key(&self) -> String {
        py_strip(&self.steam_key).to_owned()
    }

    /// El color del fósforo solo cuenta con Terminal elegido.
    pub fn terminal_color_enabled(&self) -> bool {
        self.theme == "terminal"
    }
}

// ------------------------------------------------------------------------------- vista

/// Orden de Python (Juegos, Masters, General, Apariencia) y luego Barra.
pub const TABS: [Tab; 6] = [
    Tab::Games,
    Tab::Masters,
    Tab::General,
    Tab::Appearance,
    Tab::Toolbar,
    Tab::Notifications,
];

pub fn tab_labels() -> Vec<String> {
    vec![
        tr("Juegos"),
        tr("Masters"),
        tr("General"),
        tr("Apariencia"),
        tr("Barra"),
        tr("Avisos"),
    ]
}

/// Muestras de un tema (`theme_swatch_icon`): «Sistema» mezcla Oscuro y Claro y «Terminal»
/// muestra sus cuatro fósforos.
pub fn swatch_colors(theme_id: &str) -> Vec<Color> {
    match theme_id {
        "terminal" => TERMINAL_COLORS
            .iter()
            .map(|c| palette(&format!("terminal_{c}")).selection)
            .collect(),
        "system" => {
            let (dark, light) = (palette("dark"), palette("light"));
            vec![dark.bg, dark.accent, light.bg, light.accent]
        }
        id => {
            let t = palette(id);
            vec![t.bg, t.surface, t.accent, t.selection]
        }
    }
}

/// Una opción de combo con su texto (los combos de Iced muestran `Display`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Choice<V> {
    pub label: String,
    pub value: V,
}

impl<V> std::fmt::Display for Choice<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// El combo de `choices` con `current` elegido.
pub(crate) fn choice_combo<'a, V: Clone + PartialEq + 'a>(
    choices: Vec<Choice<V>>,
    current: &V,
    on_select: impl Fn(V) -> SettingsMsg + 'a,
    enabled: bool,
    theme_id: &'static str,
) -> Element<'a, Message> {
    let selected = choices.iter().find(|c| c.value == *current).cloned();
    widgets::combo(
        choices,
        selected,
        move |c| Message::Settings(on_select(c.value)),
        None,
        enabled,
        theme_id,
    )
}

const SWATCH: f32 = 14.0;
const SWATCH_GAP: f32 = 2.0;
/// Ancho de la columna de etiquetas de los formularios (`QFormLayout`: 155 px en Qt con dark).
const LABEL_WIDTH: f32 = 155.0;

/// La tarjeta de Ajustes con la pestaña elegida y, encima, su diálogo interno si hay.
/// `find`: *Buscar juegos* abierto desde aquí. `browse`: *Examinar…* encendido (no hay otro
/// selector abierto).
pub fn view<'a>(
    form: &'a Form,
    find: Option<&'a Search>,
    browse: bool,
    theme_id: &'static str,
) -> Element<'a, Message> {
    let s = widget_style(theme_id);
    let page = match form.tab {
        Tab::Games => games_page(form, browse, theme_id),
        Tab::Masters => masters_tab::view(&form.masters, theme_id),
        Tab::General => general_page(form, theme_id),
        Tab::Appearance => appearance_page(form, theme_id),
        Tab::Toolbar => toolbar_page(form, theme_id),
        Tab::Notifications => notifications_page(form, theme_id),
    };
    // Todas las pestañas del alto de la más alta, como `QTabWidget`: la tarjeta no salta.
    let page = container(page).height(PAGE_HEIGHT).into();
    let active = TABS.iter().position(|t| *t == form.tab).unwrap_or(0);
    let tabs = widgets::tabs(
        tab_labels(),
        active,
        |i| Message::Settings(SettingsMsg::Tab(TABS[i])),
        page,
        theme_id,
    );
    let msg = |m| Message::Settings(m);
    let buttons = row![
        Space::new().width(Fill),
        widgets::button(tr("Guardar"), msg(SettingsMsg::Accept), theme_id),
        widgets::button(tr("Cancelar"), msg(SettingsMsg::Cancel), theme_id),
    ]
    .spacing(6);
    let content = column![
        label(tr("Ajustes"), s).font(style::header_font(s)),
        tabs,
        buttons
    ];
    let card = card(content, 580.0, theme_id);
    let inner = find.map(|f| find_games::view(f, browse, theme_id));
    match inner.or_else(|| masters_tab::inner_view(&form.masters, theme_id)) {
        Some(inner) => widgets::modal(card, inner),
        None => card,
    }
}

fn label<'a>(t: impl text::IntoFragment<'a>, s: WidgetStyle) -> Text<'a> {
    text(t).font(s.font()).size(s.text_size())
}

/// Una fila de formulario: la etiqueta a la izquierda y el control.
fn form_row<'a>(
    name: String,
    control: impl Into<Element<'a, Message>>,
    s: WidgetStyle,
) -> Element<'a, Message> {
    row![label(name, s).width(LABEL_WIDTH), control.into()]
        .spacing(6)
        .align_y(Center)
        .into()
}

/// El alto de las páginas (el de «Juegos» en Qt: sus rutas llenan lo que sobra).
const PAGE_HEIGHT: f32 = 476.0;

/// La columna de los nombres: la de «Star Wars Jedi Knight II: Jedi Outcast» en Qt, que no
/// parte las filas.
const GAME_LABEL_WIDTH: f32 = 216.0;

/// «Juegos»: la carpeta de cada juego con *Examinar…* y su estado, desplazable.
fn games_page<'a>(form: &'a Form, browse: bool, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let msg = |m| Message::Settings(m);
    let rows = Column::with_children(GAMES.iter().map(|g| {
        let id = g.id;
        let path = form.paths.get(id).map_or("", String::as_str);
        let status = form
            .status
            .get(id)
            .map(|st| path_status_text(g, st))
            .unwrap_or_default();
        let field = widgets::field(
            Id::from(format!("settings-path-{id}")),
            &tr("Carpeta de instalación"),
            path,
            move |text| msg(SettingsMsg::Path { game: id, text }),
            None,
            false,
            theme_id,
        );
        let browse = widgets::button_maybe(
            tr("Examinar…"),
            browse.then(|| msg(SettingsMsg::Browse(id))),
            theme_id,
        );
        let cell = column![
            row![field, browse].spacing(6).align_y(Center),
            label(status, s)
        ]
        // Qt separa 6 px; el texto de Iced es más alto (B9 de la 5d).
        .spacing(4);
        row![label(g.name, s).width(GAME_LABEL_WIDTH), cell]
            .spacing(8)
            .into()
    }))
    .spacing(6)
    // El margen del `QFormLayout` dentro del área desplazable.
    .padding(Padding {
        left: 9.0,
        right: style::scrollbar_width(s) + 4.0,
        ..Padding::ZERO
    });
    let scroll = scrollable(rows)
        .direction(Direction::Vertical(style::scrollbar(s)))
        .style(move |_, status| style::scrollable(p, s, status))
        .height(Fill);
    let find = widgets::button(tr("Buscar juegos…"), msg(SettingsMsg::FindGames), theme_id);
    column![
        label(tr("Rutas de instalación de los juegos:"), s),
        scroll,
        find
    ]
    .spacing(8)
    .into()
}

/// «General»: mapas, idioma, refresco, avisos, Wine, base de países y la clave de Steam.
fn general_page<'a>(form: &'a Form, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let msg = |m| Message::Settings(m);
    let check = |checked: bool, t: String, m: fn(bool) -> SettingsMsg| {
        widgets::check(
            checked,
            Some(label(t, s).into()),
            Some(msg(m(!checked))),
            p,
            s,
        )
    };
    let maps = widgets::tip(
        check(
            form.download_maps,
            tr("Descargar imágenes de mapas desde internet (GameTracker)"),
            SettingsMsg::DownloadMaps,
        ),
        tr("Si el mapa no está instalado, busca su imagen en image.gametracker.com"),
        p,
        s,
    );
    // Los idiomas en su propio idioma; solo «Automático» se traduce.
    let languages = std::iter::once(Choice {
        label: tr("Automático"),
        value: "auto".to_owned(),
    })
    .chain(LANGUAGES.iter().map(|code| Choice {
        label: language_name(code).to_owned(),
        value: (*code).to_owned(),
    }))
    .collect();
    let refresh = AUTO_REFRESH_CHOICES
        .iter()
        .map(|&n| Choice {
            label: if n == 0 {
                tr("Desactivado")
            } else {
                tr_fmt("Cada {n} min", &[("n", &n)])
            },
            value: n,
        })
        .collect();
    let mut page = column![
        maps,
        form_row(
            tr("Idioma"),
            choice_combo(
                languages,
                &form.language,
                SettingsMsg::Language,
                true,
                theme_id
            ),
            s
        ),
        form_row(
            tr("Refrescar automáticamente"),
            choice_combo(
                refresh,
                &form.auto_refresh,
                SettingsMsg::AutoRefresh,
                true,
                theme_id
            ),
            s
        ),
        check(
            form.check_updates,
            tr("Avisar si hay una versión nueva de JASE"),
            SettingsMsg::CheckUpdates
        ),
        check(
            form.tray,
            tr("Minimizar a la bandeja del sistema"),
            SettingsMsg::Tray
        ),
    ]
    .spacing(8);
    if !cfg!(windows) {
        let wine = widgets::field(
            Id::new("settings-wine"),
            &tr("vacío = abrir el .exe directamente"),
            &form.wine,
            move |t| msg(SettingsMsg::Wine(t)),
            None,
            false,
            theme_id,
        );
        let tip = tr(
            "Programa con el que se abren los juegos de Windows en Linux (p. ej. wine, umu-run o un script propio)",
        );
        page = page.push(form_row(
            tr("Abrir juegos con (Wine)"),
            widgets::tip(wine, tip, p, s),
            s,
        ));
    }
    let ready = form.countries == CountryButton::Ready;
    let countries = widgets::tip(
        widgets::button_maybe(
            tr("Actualizar base de países"),
            ready.then(|| msg(SettingsMsg::UpdateCountries)),
            theme_id,
        ),
        tr("Descarga de nuevo la base «IP to Country Lite» de DB-IP (db-ip.com)"),
        p,
        s,
    );
    let downloading = (form.countries == CountryButton::Downloading)
        .then(|| label(tr("Descargando en segundo plano…"), s));
    let key = widgets::password_field(
        Id::new("settings-steam-key"),
        "",
        &form.steam_key,
        move |t| msg(SettingsMsg::SteamKey(t)),
        None,
        theme_id,
    );
    let link = tr("Consíguela en {link} (cualquier dominio sirve)");
    let (before, after) = link.split_once("{link}").unwrap_or((&link, ""));
    let link = rich_text([
        span(before.to_owned()),
        span("steamcommunity.com/dev/apikey")
            .color(p.accent)
            .underline(true)
            .link(()),
        span(after.to_owned()),
    ])
    .on_link_click(|()| Message::Settings(SettingsMsg::OpenSteamKeyPage))
    .font(s.font())
    .size(s.text_size());
    let indent = |e: Element<'a, Message>| row![Space::new().width(LABEL_WIDTH + 6.0), e];
    page.push(row![countries].push(downloading).spacing(8).align_y(Center))
        .push(form_row(tr("Clave de API de Steam"), key, s))
        .push(indent(link.into()))
        .push(indent(check(
            form.steam_include_empty,
            tr("Incluir servidores vacíos de Steam (lista mucho más larga)"),
            SettingsMsg::SteamIncludeEmpty,
        )))
        .into()
}

/// «Apariencia»: la lista de temas con sus muestras (se aplica al guardar) y el fósforo.
fn appearance_page<'a>(form: &'a Form, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let items = Column::with_children(std::iter::once("system").chain(THEME_IDS).map(|id| {
        // Python pinta el borde `#808080`: aquí, el del tema activo.
        let swatches = Row::with_children(swatch_colors(id).into_iter().map(|c| {
            container(Space::new())
                .width(SWATCH)
                .height(SWATCH)
                .style(move |_| container::Style {
                    background: Some(c.into()),
                    border: Border {
                        color: p.border,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                })
                .into()
        }))
        .spacing(SWATCH_GAP);
        let selected = form.theme == id;
        let item = container(
            row![swatches, label(theme_display_name(id), s)]
                .spacing(6)
                .align_y(Center),
        )
        .padding([2, 4])
        .width(Fill)
        .style(move |_| style::list_item(p, s, selected, true));
        mouse_area(item)
            .on_press(Message::Settings(SettingsMsg::Theme(id.to_owned())))
            .into()
    }));
    let (frame, edges) = style::table_frame(p, s);
    let list = container(items)
        .padding(2)
        .width(Fill)
        .height(Fill)
        .style(move |_| frame);
    let list: Element<'a, Message> = match edges {
        Some(e) => framed(list, e),
        None => list.into(),
    };
    let colors = TERMINAL_COLORS
        .iter()
        .map(|c| Choice {
            label: terminal_color_name(c),
            value: (*c).to_owned(),
        })
        .collect();
    let color = choice_combo(
        colors,
        &form.terminal_color,
        SettingsMsg::TerminalColor,
        form.terminal_color_enabled(),
        theme_id,
    );
    column![
        label(tr("Tema de la interfaz:"), s),
        list,
        row![
            label(tr("Color del fósforo:"), s),
            container(color).width(160),
            Space::new().width(Fill)
        ]
        .spacing(6)
        .align_y(Center),
    ]
    .spacing(8)
    .into()
}

/// Pestaña Barra: la lista con casilla, icono y nombre (clic = elegir) y, al lado, mover,
/// agregar separador y volver a la de fábrica.
fn toolbar_page<'a>(form: &'a Form, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let msg = |m| Message::Settings(m);
    let side = s.text_size().round();
    let rows = form.toolbar.iter().enumerate().map(|(i, &(item, on))| {
        let selected = form.toolbar_selected == Some(i);
        let (icon, name): (Element<'a, Message>, String) = match item {
            ToolItem::Button(a) => (
                match a.icon().and_then(|n| icons::tool_icon(n, theme_id, false)) {
                    Some(h) => svg(h).width(side).height(side).into(),
                    None => Space::new().width(side).height(side).into(),
                },
                tool_name(a),
            ),
            ToolItem::Separator => (
                Space::new().width(side).height(side).into(),
                tr("— Separador —"),
            ),
        };
        let check = widgets::check(on, None, Some(msg(SettingsMsg::ToolbarCheck(i, !on))), p, s);
        let item = container(row![check, icon, label(name, s)].spacing(6).align_y(Center))
            .padding([2, 4])
            .width(Fill)
            .style(move |_| style::list_item(p, s, selected, true));
        mouse_area(item)
            .on_press(msg(SettingsMsg::ToolbarSelect(i)))
            .into()
    });
    let (frame, edges) = style::table_frame(p, s);
    let list = container(scrollable(Column::with_children(rows)).height(Fill))
        .padding(2)
        .width(Fill)
        .height(Fill)
        .style(move |_| frame);
    let list: Element<'a, Message> = match edges {
        Some(e) => framed(list, e),
        None => list.into(),
    };
    let chosen = form.toolbar_selected;
    let last = form.toolbar.len().saturating_sub(1);
    let buttons = column![
        widgets::button_maybe(
            tr("Subir"),
            chosen
                .filter(|&i| i > 0)
                .map(|_| msg(SettingsMsg::ToolbarMove(true))),
            theme_id,
        ),
        widgets::button_maybe(
            tr("Bajar"),
            chosen
                .filter(|&i| i < last)
                .map(|_| msg(SettingsMsg::ToolbarMove(false))),
            theme_id,
        ),
        widgets::button(
            tr("Agregar separador"),
            msg(SettingsMsg::ToolbarSeparator),
            theme_id
        ),
        widgets::button(tr("Restablecer"), msg(SettingsMsg::ToolbarReset), theme_id),
    ]
    .spacing(6);
    column![
        label(
            tr("Botones de la barra de herramientas (marca los visibles):"),
            s
        ),
        row![list, buttons].spacing(8).height(Fill),
    ]
    .spacing(6)
    .into()
}

/// Pestaña Avisos: qué avisa JASE y los umbrales de partida guardados, con *Quitar*. Vigilar
/// sitio y los umbrales se agregan desde *Servidor* o el clic derecho (son por servidor).
fn notifications_page<'a>(form: &'a Form, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let msg = |m| Message::Settings(m);
    let check = |checked: bool, t: String, m: fn(bool) -> SettingsMsg| {
        widgets::check(
            checked,
            Some(label(t, s).into()),
            Some(msg(m(!checked))),
            p,
            s,
        )
    };
    let rows = form.party_alerts.iter().flat_map(|(game_id, alerts)| {
        let name = game(game_id).map_or(game_id.as_str(), |g| g.name);
        alerts.iter().map(move |(address, n)| {
            let text = tr_n(
                "{game} · {address}: {n} jugador",
                "{game} · {address}: {n} jugadores",
                i64::from(*n),
                &[("game", &name), ("address", address)],
            );
            row![
                label(text, s).width(Fill),
                widgets::button(
                    tr("Quitar"),
                    msg(SettingsMsg::RemovePartyAlert(
                        game_id.clone(),
                        address.clone()
                    )),
                    theme_id,
                ),
            ]
            .spacing(6)
            .align_y(Center)
            .into()
        })
    });
    let alerts: Element<'a, Message> = if form.party_alerts.is_empty() {
        label(tr("No hay umbrales de partida."), s).into()
    } else {
        scrollable(Column::with_children(rows).spacing(4))
            .height(Fill)
            .into()
    };
    column![
        check(
            form.notify_friends,
            tr("Avisar cuando un amigo aparece en un servidor"),
            SettingsMsg::NotifyFriends,
        ),
        check(
            form.watch_friends,
            tr("Revisar favoritos y recientes cada 2 min, también con la ventana en la bandeja"),
            SettingsMsg::WatchFriends,
        ),
        check(
            form.notify_party,
            tr("Avisar cuando se arma partida en un servidor con umbral"),
            SettingsMsg::NotifyParty,
        ),
        check(
            form.notify_attention,
            tr("Pedir atención en la barra de tareas al avisar"),
            SettingsMsg::NotifyAttention,
        ),
        label(tr("Umbrales de partida:"), s),
        alerts,
        label(
            tr("Vigilar sitio y los umbrales se agregan desde el menú Servidor o el clic derecho en un servidor."),
            s,
        ),
    ]
    .spacing(6)
    .into()
}
