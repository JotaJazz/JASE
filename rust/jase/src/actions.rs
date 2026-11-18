//! Acciones compartidas (`ui_actions` de `window_actions.py`): menús, barra de herramientas,
//! menú contextual y atajos leen de aquí texto, atajo, si están encendidas y si están marcadas,
//! así que nunca se desincronizan (lo que Qt logra compartiendo cada `QAction`). Las de
//! subfases futuras salen como en Qt, pero apagadas (`ported`).

use iced::widget::{container, hover, mouse_area, row, text};
use iced::{Element, Fill};
use jase_core::i18n::tr;

use crate::app::{App, Message};
use crate::menu::{self, Check, Entry, Placement};
use crate::presets::{self, PresetItem};
use crate::sidebar::ping_options;
use crate::theme::{
    TERMINAL_COLORS, THEME_IDS, framed, palette, style, terminal_color_name, theme_display_name,
    widget_style,
};

/// Una por clave de `ui_actions` (`_build_actions`) más el submenú *Grupo*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Refresh,
    Stop,
    RefreshSelected,
    Connect,
    WatchSlot,
    PartyAlert,
    Favorite,
    EditNote,
    RemoveRecent,
    Copy,
    CopyLink,
    AddServer,
    Settings,
    Theme,
    FindGames,
    ExportFavorites,
    ImportFavorites,
    CopyFavorites,
    FindPlayer,
    NowPlaying,
    ScanLan,
    Rcon,
    Friends,
    ClearTreeMarks,
    Quit,
    About,
    CheckUpdates,
    HideEmpty,
    HideFull,
    HidePassword,
    HideBots,
    Group,
}

impl Action {
    pub const ALL: [Action; 32] = [
        Action::Refresh,
        Action::Stop,
        Action::RefreshSelected,
        Action::Connect,
        Action::WatchSlot,
        Action::PartyAlert,
        Action::Favorite,
        Action::EditNote,
        Action::RemoveRecent,
        Action::Copy,
        Action::CopyLink,
        Action::AddServer,
        Action::Settings,
        Action::Theme,
        Action::FindGames,
        Action::ExportFavorites,
        Action::ImportFavorites,
        Action::CopyFavorites,
        Action::FindPlayer,
        Action::NowPlaying,
        Action::ScanLan,
        Action::Rcon,
        Action::Friends,
        Action::ClearTreeMarks,
        Action::Quit,
        Action::About,
        Action::CheckUpdates,
        Action::HideEmpty,
        Action::HideFull,
        Action::HidePassword,
        Action::HideBots,
        Action::Group,
    ];

    /// Texto de la acción; los que cambian con la selección (favorito, vigilar, partida) se
    /// portan con su subfase.
    pub fn label(self, _app: &App) -> String {
        match self {
            Action::Refresh => tr("Refrescar lista"),
            Action::Stop => tr("Detener"),
            Action::RefreshSelected => tr("Refrescar este servidor"),
            Action::Connect => tr("Conectar"),
            Action::WatchSlot => tr("Avisar cuando haya sitio"),
            Action::PartyAlert => tr("Avisar cuando se arme partida…"),
            Action::Favorite => tr("Agregar a favoritos"),
            Action::EditNote => tr("Editar nota…"),
            Action::RemoveRecent => tr("Quitar de recientes"),
            Action::Copy => tr("Copiar IP:puerto"),
            Action::CopyLink => tr("Copiar enlace jase://"),
            Action::AddServer => tr("Agregar servidor…"),
            Action::Settings => tr("Ajustes"),
            Action::Theme => tr("Tema"),
            Action::FindGames => tr("Buscar juegos…"),
            Action::ExportFavorites => tr("Exportar favoritos…"),
            Action::ImportFavorites => tr("Importar favoritos…"),
            Action::CopyFavorites => tr("Copiar favoritos como texto"),
            Action::FindPlayer => tr("Buscar jugador…"),
            Action::NowPlaying => tr("Ahora se juega…"),
            Action::ScanLan => tr("Buscar en la red local"),
            Action::Rcon => tr("Consola RCON…"),
            Action::Friends => tr("Amigos…"),
            Action::ClearTreeMarks => tr("Quitar filtros de mod/mapa/país"),
            Action::Quit => tr("Salir"),
            Action::About => tr("Acerca de"),
            Action::CheckUpdates => tr("Buscar actualizaciones…"),
            Action::HideEmpty => tr("Ocultar vacíos"),
            Action::HideFull => tr("Ocultar llenos"),
            Action::HidePassword => tr("Ocultar con contraseña"),
            Action::HideBots => tr("Ocultar solo bots"),
            Action::Group => tr("Grupo"),
        }
    }

    /// Atajo como lo muestra Qt (`QKeySequence.toString`).
    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            Action::Refresh => Some("F5"),
            Action::AddServer => Some("Ctrl+N"),
            Action::FindPlayer => Some("Ctrl+Shift+F"),
            Action::NowPlaying => Some("Ctrl+J"),
            Action::ScanLan => Some("Ctrl+L"),
            Action::Rcon => Some("Ctrl+Shift+R"),
            Action::Friends => Some("Ctrl+Shift+A"),
            // `QKeySequence.Quit`: Ctrl+Q en Linux y macOS; en Windows no tiene atajo.
            Action::Quit if !cfg!(windows) => Some("Ctrl+Q"),
            _ => None,
        }
    }

    /// Icono de la barra de herramientas (`ACTION_ICONS`).
    pub fn icon(self) -> Option<&'static str> {
        match self {
            Action::Refresh => Some("refresh"),
            Action::Stop => Some("stop"),
            Action::RefreshSelected => Some("refresh_one"),
            Action::Connect => Some("connect"),
            Action::Favorite => Some("favorite"),
            Action::Copy => Some("copy"),
            Action::Settings => Some("settings"),
            Action::Theme => Some("theme"),
            _ => None,
        }
    }

    /// ¿Ya funciona en Rust? Lo que no, sale apagado en menús, barra, contextual y atajos.
    pub fn ported(self) -> bool {
        matches!(
            self,
            Action::Refresh
                | Action::Stop
                | Action::RefreshSelected
                | Action::Copy
                | Action::ScanLan
                | Action::ClearTreeMarks
                | Action::Quit
                | Action::About
                | Action::HideEmpty
                | Action::HideFull
                | Action::HidePassword
                | Action::HideBots
                | Action::Theme
        )
    }
}

/// `_update_actions`: lo apagado no corre por ningún camino.
pub fn enabled(app: &App, a: Action) -> bool {
    a.ported()
        && match a {
            Action::Stop => app.is_scanning(),
            Action::RefreshSelected | Action::Copy => app.selected_server().is_some(),
            Action::ClearTreeMarks => app
                .current_game()
                .is_some_and(|g| !app.tree.marks(g.id).is_empty()),
            _ => true,
        }
}

/// Las casillas de los filtros rápidos reflejan la config, como las de la barra lateral.
pub fn checked(app: &App, a: Action) -> Option<bool> {
    let q = app.quick_filters();
    match a {
        Action::HideEmpty => Some(q.hide_empty),
        Action::HideFull => Some(q.hide_full),
        Action::HidePassword => Some(q.hide_password),
        Action::HideBots => Some(q.hide_bots),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Menu {
    File,
    View,
    Filter,
    Server,
    Help,
    /// El botón de tema de la barra de herramientas (5d, Task 3).
    ThemeButton,
    Presets,
    Context,
}

impl Menu {
    pub const BAR: [Menu; 5] = [
        Menu::File,
        Menu::View,
        Menu::Filter,
        Menu::Server,
        Menu::Help,
    ];

    pub fn title(self) -> String {
        match self {
            Menu::File => tr("Archivo"),
            Menu::View => tr("Ver"),
            Menu::Filter => tr("Filtro"),
            Menu::Server => tr("Servidor"),
            Menu::Help => tr("Ayuda"),
            Menu::ThemeButton => tr("Tema"),
            Menu::Presets => tr("Filtros guardados"),
            Menu::Context => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Run(Action),
    /// Un radio de Filtro → Ping máx. (el mismo valor que el combo).
    Ping(i64),
    Theme(&'static str),
    TerminalColor(&'static str),
    Open(Menu),
    /// El ratón entró en un título de la barra: con otro menú de la barra abierto, lo cambia.
    Hover(Menu),
    Close,
}

fn run(a: Action) -> Message {
    Message::Actions(Msg::Run(a))
}

/// El ítem de una acción, encendido y marcado según el estado.
fn item(app: &App, a: Action) -> Entry<Message> {
    Entry::Item {
        label: a.label(app),
        shortcut: a.shortcut().map(str::to_owned),
        check: checked(app, a).map_or(Check::None, Check::Check),
        icon: a.icon(),
        on: enabled(app, a).then(|| run(a)),
    }
}

fn radio(label: String, on: bool, msg: Message) -> Entry<Message> {
    Entry::Item {
        label,
        shortcut: None,
        check: Check::Radio(on),
        icon: None,
        on: Some(msg),
    }
}

/// Grupo (5f): submenú sin entradas, apagado.
fn group(app: &App) -> Entry<Message> {
    Entry::Sub {
        label: Action::Group.label(app),
        icon: None,
        entries: Vec::new(),
    }
}

/// Ver → Tema y el botón de la barra: los 12 temas, separador y *Color del fósforo*.
fn theme_entries(app: &App) -> Vec<Entry<Message>> {
    let cfg = app.config();
    let choice = if THEME_IDS.contains(&cfg.theme.as_str()) {
        cfg.theme.as_str()
    } else {
        "system"
    };
    let color = if TERMINAL_COLORS.contains(&cfg.terminal_color.as_str()) {
        cfg.terminal_color.as_str()
    } else {
        "green"
    };
    let themes = std::iter::once("system").chain(THEME_IDS).map(|id| {
        radio(
            theme_display_name(id),
            id == choice,
            Message::Actions(Msg::Theme(id)),
        )
    });
    let colors = TERMINAL_COLORS
        .into_iter()
        .map(|c| {
            radio(
                terminal_color_name(c),
                c == color,
                Message::Actions(Msg::TerminalColor(c)),
            )
        })
        .collect();
    themes
        .chain([
            Entry::Separator,
            Entry::Sub {
                label: tr("Color del fósforo"),
                icon: None,
                entries: colors,
            },
        ])
        .collect()
}

/// Las combinaciones, separador si hay, «Guardar filtros actuales…» y *Borrar*
/// (`_rebuild_presets_menu`).
fn preset_entries(app: &App) -> Vec<Entry<Message>> {
    let names: Vec<String> = app
        .filter_presets()
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let pick = |i| Some(Message::Presets(presets::Msg::Pick(i)));
    let plain = |label: String, on| Entry::Item {
        label,
        shortcut: None,
        check: Check::None,
        icon: None,
        on,
    };
    let mut es: Vec<Entry<Message>> = names
        .iter()
        .map(|n| plain(n.clone(), pick(PresetItem::Apply(n.clone()))))
        .collect();
    if !es.is_empty() {
        es.push(Entry::Separator);
    }
    es.push(plain(
        tr("Guardar filtros actuales…"),
        pick(PresetItem::Save),
    ));
    es.push(Entry::Sub {
        label: tr("Borrar"),
        icon: None,
        entries: names
            .iter()
            .map(|n| plain(n.clone(), pick(PresetItem::Delete(n.clone()))))
            .collect(),
    });
    es
}

/// Entradas de cada menú (`_build_menus` y `_build_context_menu`).
pub fn entries(app: &App, m: Menu) -> Vec<Entry<Message>> {
    use Action as A;
    let items = |acts: &[Action]| acts.iter().map(|&a| item(app, a)).collect::<Vec<_>>();
    match m {
        Menu::File => [
            items(&[A::FindGames]),
            vec![Entry::Separator],
            items(&[A::ExportFavorites, A::ImportFavorites, A::CopyFavorites]),
            vec![Entry::Separator],
            items(&[A::Settings]),
            vec![Entry::Separator],
            items(&[A::Quit]),
        ]
        .concat(),
        Menu::View => vec![Entry::Sub {
            label: A::Theme.label(app),
            icon: A::Theme.icon(),
            entries: theme_entries(app),
        }],
        Menu::Filter => {
            let max_ping = app.quick_filters().max_ping;
            let ping = ping_options(max_ping)
                .into_iter()
                .map(|o| radio(o.label, o.ms == max_ping, Message::Actions(Msg::Ping(o.ms))))
                .collect();
            [
                items(&[A::HideEmpty, A::HideFull, A::HidePassword, A::HideBots]),
                vec![
                    Entry::Sub {
                        label: tr("Ping máx."),
                        icon: None,
                        entries: ping,
                    },
                    Entry::Separator,
                ],
                items(&[A::ClearTreeMarks]),
            ]
            .concat()
        }
        Menu::Server => [
            items(&[
                A::Connect,
                A::Refresh,
                A::Stop,
                A::RefreshSelected,
                A::Copy,
                A::Favorite,
                A::EditNote,
            ]),
            vec![group(app)],
            items(&[A::WatchSlot, A::PartyAlert]),
            vec![Entry::Separator],
            items(&[
                A::AddServer,
                A::ScanLan,
                A::Rcon,
                A::FindPlayer,
                A::NowPlaying,
                A::Friends,
            ]),
        ]
        .concat(),
        Menu::Help => [
            items(&[A::CheckUpdates]),
            vec![Entry::Separator],
            items(&[A::About]),
        ]
        .concat(),
        Menu::ThemeButton => theme_entries(app),
        Menu::Presets => preset_entries(app),
        // *Quitar de recientes* llega con la vista Recientes (5f).
        Menu::Context => [
            items(&[A::Connect, A::Copy, A::CopyLink, A::RefreshSelected]),
            vec![Entry::Separator],
            items(&[A::Favorite, A::EditNote]),
            vec![group(app)],
            items(&[A::WatchSlot, A::PartyAlert]),
            vec![Entry::Separator],
            items(&[A::AddServer]),
        ]
        .concat(),
    }
}

/// Las entradas de `m` si es el menú abierto.
pub fn open_entries(app: &App, m: Menu) -> Option<Vec<Entry<Message>>> {
    (app.open_menu() == Some(m)).then(|| entries(app, m))
}

/// `QMenuBar`: los cinco títulos, cada uno con su menú debajo.
pub fn menu_bar(app: &App) -> Element<'_, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let (bar, line, height, left) = style::menu_bar(p, s, app.theme_id);
    let (pad, spacing) = style::menu_bar_item_spacing(s);
    let titles = Menu::BAR.map(|m| {
        let open = app.open_menu() == Some(m);
        let title = |highlight: Option<bool>| -> Element<'_, Message> {
            let label = container(text(m.title()).font(s.font()).size(s.text_size()))
                .padding([0.0, pad])
                .height(Fill)
                .center_y(Fill);
            match highlight {
                Some(pressed) => {
                    let (look, edges) = style::menu_bar_item(p, s, pressed);
                    let label = label.style(move |_| look);
                    match edges {
                        Some(e) => framed(label, e),
                        None => label.into(),
                    }
                }
                None => label.into(),
            }
        };
        // Resaltado al pasar (`:selected`) y hundido con su menú abierto (`:pressed`).
        let face: Element<'_, Message> = if open {
            title(Some(true))
        } else {
            hover(title(None), title(Some(false)))
        };
        let face = mouse_area(face)
            .on_press(Message::Actions(Msg::Open(m)))
            .on_enter(Message::Actions(Msg::Hover(m)));
        menu::popup(
            face.into(),
            open_entries(app, m),
            Placement::Below,
            Message::Actions(Msg::Close),
            app.theme_id,
        )
    });
    let bar = container(row(titles).spacing(spacing).height(height))
        .padding([0.0, left])
        .width(Fill)
        .style(move |_| bar);
    match line {
        Some(e) => framed(bar, e),
        None => bar.into(),
    }
}
