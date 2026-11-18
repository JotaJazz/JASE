//! Barra de herramientas (`_build_toolbar`), atajos de teclado (`_setup_shortcuts` y los de
//! las acciones) y barra de estado (`QStatusBar` con su `QProgressBar`).

use iced::keyboard::{Key, Modifiers, key::Named};
use iced::widget::{Space, button, center, container, hover, row, stack, svg, text};
use iced::{Center, Element, Fill, Padding, Right};
use jase_core::config::version_for;
use jase_core::i18n::{tr, tr_fmt};

use crate::actions::{self, Action, Menu, enabled};
use crate::app::{App, DirectMsg, DirectView, Focus, GroupOption, Message};
use crate::icons;
use crate::menu::{self, Placement};
use crate::sidebar::{self, VersionOption};
use crate::theme::{
    THEME_IDS, WidgetStyle, framed, palette, style, theme_display_name, widget_style,
};
use crate::widgets;

/// Atajos de toda la ventana (`Ctrl+C` solo vale con la tabla enfocada: lo decide `update`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    F5,
    Find,
    Lan,
    Copy,
    Quit,
}

/// La tecla como atajo: F5 sin modificadores, el resto con Ctrl solo. Salir solo donde
/// `QKeySequence.Quit` tiene atajo (`Action::shortcut`).
pub fn shortcut_for(key: &Key, mods: Modifiers) -> Option<Shortcut> {
    match key.as_ref() {
        Key::Named(Named::F5) if mods.is_empty() => Some(Shortcut::F5),
        Key::Character(c) if mods == Modifiers::COMMAND => match c.to_lowercase().as_str() {
            "f" => Some(Shortcut::Find),
            "l" => Some(Shortcut::Lan),
            "c" => Some(Shortcut::Copy),
            "q" if Action::Quit.shortcut().is_some() => Some(Shortcut::Quit),
            _ => None,
        },
        _ => None,
    }
}

/// Los grupos de botones, separados en la barra (`_build_toolbar`).
pub fn toolbar_groups() -> [&'static [Action]; 3] {
    [
        &[Action::Refresh, Action::Stop, Action::RefreshSelected],
        &[Action::Connect, Action::Favorite, Action::Copy],
        &[Action::Settings, Action::Theme],
    ]
}

/// Botón de una acción, solo icono; el de tema abre su menú debajo.
fn tool_button(app: &App, a: Action) -> Element<'_, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let on = enabled(app, a);
    let open = a == Action::Theme && app.open_menu() == Some(Menu::ThemeButton);
    let msg = match a {
        Action::Theme => Message::Actions(actions::Msg::Open(Menu::ThemeButton)),
        _ => Message::Actions(actions::Msg::Run(a)),
    };
    let side = icons::tool_icon_size(app.theme_id);
    let total = tool_button_side(s);
    // `hovered` solo elige el bisel de 98/VGUI; fondo y borde los da el estado del botón.
    let face = |hovered: bool| -> Element<'_, Message> {
        let icon: Element<'_, Message> = match a
            .icon()
            .and_then(|n| icons::tool_icon(n, app.theme_id, !on))
        {
            Some(h) => svg(h).width(side).height(side).into(),
            None => Space::new().width(side).height(side).into(),
        };
        let b = button(center(icon))
            .padding(0)
            .on_press_maybe(on.then(|| msg.clone()))
            .style(move |_, st| {
                let over = matches!(st, button::Status::Hovered | button::Status::Pressed);
                let down = st == button::Status::Pressed || open;
                style::tool_button(p, s, over, down).0
            });
        // El bisel ocupa el borde de 1 px que en los demás estilos dibuja Iced.
        match style::tool_button(p, s, hovered, open).1 {
            Some(e) => framed(b.width(total - 2.0).height(total - 2.0), e),
            None => b.width(total).height(total).into(),
        }
    };
    let face = if on && !open {
        hover(face(false), face(true))
    } else {
        face(false)
    };
    let tip = match a {
        Action::Theme => {
            let theme = app.config().theme.as_str();
            let choice = if THEME_IDS.contains(&theme) {
                theme
            } else {
                "system"
            };
            tr_fmt("Tema: {name}", &[("name", &theme_display_name(choice))])
        }
        _ => a.label(app),
    };
    // Con su menú abierto no hay tooltip (Qt lo esconde al abrir el menú).
    let face = if open {
        face
    } else {
        widgets::tip(face, tip, p, s)
    };
    match a {
        Action::Theme => menu::popup(
            face,
            actions::open_entries(app, Menu::ThemeButton),
            Placement::Below,
            Message::Actions(actions::Msg::Close),
            app.theme_id,
        ),
        _ => face,
    }
}

/// Lado del botón como lo arma Qt: icono de 20 px (`setIconSize`; el de 98 mide 16 y va
/// centrado), relleno, borde de 1 px y los 3 px que suma `QToolButton` (medido offscreen).
fn tool_button_side(s: WidgetStyle) -> f32 {
    icons::ICON_SIZE + 2.0 * style::tool_button_padding(s) + 2.0 + 3.0
}

/// `QToolBar#toolbar::separator`: sus líneas de 1 px con margen de 4 px arriba y abajo y 6 a
/// los lados.
fn separator<'a>(app: &App) -> Element<'a, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let lines = style::toolbar_separator(p, s).into_iter().map(|c| {
        container(Space::new())
            .width(1)
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(c.into()),
                ..container::Style::default()
            })
            .into()
    });
    container(row(lines))
        .padding([4, 6])
        .height(tool_button_side(s))
        .into()
}

/// La versión que muestra el combo: la que filtra o, en una vista directa (donde no se aplica),
/// la guardada del juego.
fn version_key(app: &App) -> String {
    match app.current_game() {
        Some(g) if app.direct_view().is_some() => version_for(g, app.config()).to_owned(),
        _ => app.table.model.filter().version.clone(),
    }
}

/// Lo que dice el combo de versión.
pub fn version_shown(app: &App) -> Option<String> {
    let key = version_key(app);
    let options = app.version_options();
    options.into_iter().find(|o| o.key == key).map(|o| o.label)
}

/// Tooltip del combo de versión: en una vista directa, que no aplica.
pub fn version_tip(app: &App) -> String {
    match app.direct_view() {
        None => tr("Versión del juego"),
        Some(DirectView::Favorites) => tr("No aplica en Favoritos"),
        Some(DirectView::Recent) => tr("No aplica en Recientes"),
    }
}

/// La barra: tres grupos de botones, el campo de texto que estira, el combo de versión y, en
/// Favoritos, el de grupo.
pub fn toolbar<'a>(app: &'a App) -> Element<'a, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    for (i, group) in toolbar_groups().into_iter().enumerate() {
        if i > 0 {
            items.push(separator(app));
        }
        items.extend(group.iter().map(|&a| tool_button(app, a)));
    }
    // Qt redondea hacia arriba lo que sobra al centrar; Iced dejaría el campo en medio píxel.
    let side = tool_button_side(s);
    let top = ((side - widgets::field_height(s)) / 2.0).floor();
    let place = |e: Element<'a, Message>| -> Element<'a, Message> {
        container(e)
            .height(side)
            .padding(Padding::ZERO.top(top))
            .into()
    };
    items.push(place(widgets::field(
        widgets::FILTER_ID,
        &tr("Filtrar servidores…"),
        app.filter_text(),
        |t| Message::Sidebar(sidebar::Msg::Text(t)),
        None,
        app.focus() == Focus::Filter,
        app.theme_id,
    )));
    let options = app.version_options();
    // Las opciones del juego, no las que llegan escaneando: el combo no cambia de ancho.
    let fixed = app.current_game().map_or(0, |g| 1 + g.known_versions.len());
    let labels: Vec<&str> = options
        .iter()
        .take(fixed)
        .map(|o| o.label.as_str())
        .collect();
    let width = widgets::combo_width(6, &labels, app.theme_id);
    let key = version_key(app);
    let chosen = options.iter().find(|o| o.key == key).cloned();
    items.push(place(widgets::tip(
        container(widgets::combo(
            options,
            chosen,
            |o: VersionOption| Message::Sidebar(sidebar::Msg::Version(o.key)),
            Some((
                Message::Sidebar(sidebar::Msg::ComboOpened(sidebar::Combo::Version)),
                Message::Sidebar(sidebar::Msg::ComboClosed(sidebar::Combo::Version)),
            )),
            app.direct_view().is_none(),
            app.theme_id,
        ))
        .width(width),
        version_tip(app),
        p,
        s,
    )));
    // Solo en Favoritos con algún grupo.
    let groups = app.group_options();
    if !groups.is_empty() {
        let labels: Vec<&str> = groups.iter().map(|o| o.label.as_str()).collect();
        let width = widgets::combo_width(8, &labels, app.theme_id);
        let current = app.current_group();
        let chosen = groups.iter().find(|o| o.key.as_deref() == current).cloned();
        items.push(place(widgets::tip(
            container(widgets::combo(
                groups,
                chosen,
                |o: GroupOption| Message::Direct(DirectMsg::Group(o.key)),
                Some((
                    Message::Sidebar(sidebar::Msg::ComboOpened(sidebar::Combo::Group)),
                    Message::Sidebar(sidebar::Msg::ComboClosed(sidebar::Combo::Group)),
                )),
                true,
                app.theme_id,
            ))
            .width(width),
            tr("Grupo de favoritos que se muestra"),
            p,
            s,
        )));
    }
    let (look, line) = style::toolbar(p, s, app.theme_id);
    // Margen de 8 px (relleno de 4 más el de `QToolBar`); la línea de abajo va dentro.
    let line_w = line.and_then(|e| e.bottom).map_or(0.0, |(w, _)| w);
    let bar = container(row(items).spacing(4).align_y(Center))
        .padding(Padding {
            top: 8.0,
            right: 8.0,
            bottom: 8.0 - line_w,
            left: 8.0,
        })
        .width(Fill)
        .style(move |_| look);
    match line {
        Some(e) => framed(bar, e),
        None => bar.into(),
    }
}

/// Ancho de la barra de progreso (`setMaximumWidth(220)`; su política la estira hasta ahí).
const PROGRESS_WIDTH: f32 = 220.0;

/// Alto total: `max-height: 14px` más el borde en los de época; la plana de Fusion mide 22 en
/// una barra de estado de 27, aquí 18 en la de 22.
fn progress_height(s: WidgetStyle) -> f32 {
    if s == WidgetStyle::Flat { 18.0 } else { 16.0 }
}

/// `QProgressBar` con «Consultando %v / %m» encima; en los de época, bloques de 6 px con
/// margen de 2×1 (`::chunk`).
fn progress<'a>(app: &App, done: usize, total: usize) -> Element<'a, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let (frame, edges, fill, ink) = style::progress_bar(p, s);
    // El bisel (98/VGUI) va por fuera del contenedor; el borde de Iced, por dentro.
    let bevel = edges.map_or(0.0, |e| e.left.map_or(0.0, |(w, _)| w));
    let inner = PROGRESS_WIDTH - 2.0 * (bevel + frame.border.width);
    let filled = if total == 0 {
        0.0
    } else {
        inner * done.min(total) as f32 / total as f32
    };
    let chunk = move |w: f32| {
        container(Space::new())
            .width(w)
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(fill),
                ..container::Style::default()
            })
    };
    let bar: Element<'a, Message> = if s == WidgetStyle::Flat {
        chunk(filled).into()
    } else {
        let n = ((filled / 8.0).round() as usize).min(((inner - 2.0) / 8.0) as usize);
        row((0..n).map(|_| chunk(6.0).into()))
            .spacing(2)
            .padding([2, 1])
            .into()
    };
    let label = text(app.progress_text())
        .font(s.font())
        .size(s.text_size())
        .color(ink);
    let bar = container(stack![bar, center(label)])
        .width(PROGRESS_WIDTH - 2.0 * bevel)
        .height(progress_height(s) - 2.0 * bevel)
        .padding(frame.border.width)
        .style(move |_| frame);
    match edges {
        Some(e) => framed(bar, e),
        None => bar.into(),
    }
}

/// Mensaje, barra de progreso (solo con progreso), «Escaneando…» y el conteo, de izquierda a
/// derecha como los widgets permanentes de `QStatusBar`.
pub fn status_bar(app: &App) -> Element<'_, Message> {
    let (p, s) = (palette(app.theme_id), widget_style(app.theme_id));
    let label = |t: String| text(t).font(s.font()).size(s.text_size());
    let count = label(app.count_text()).align_x(Right);
    let count = match style::status_label(p, s) {
        // El bisel de 98 y VGUI: 17 px en total, como la `QLabel` de Qt.
        Some(e) if e.top.is_some() => framed(
            container(count)
                .padding(style::status_label_padding(s))
                .height(17.0 - e.top.map_or(0.0, |t| t.0) - e.bottom.map_or(0.0, |b| b.0))
                .align_y(Center),
            e,
        ),
        Some(e) => framed(container(count).padding(style::status_label_padding(s)), e),
        None => container(count)
            .padding(style::status_label_padding(s))
            .into(),
    };
    let mut items: Vec<Element<'_, Message>> =
        vec![label(app.status().to_owned()).width(Fill).into()];
    if let Some((done, total)) = app.scan.progress() {
        items.push(progress(app, done, total));
    }
    let scanning = app.scanning_text();
    if !scanning.is_empty() {
        // `scanning_label` va sin el bisel de las otras etiquetas.
        items.push(
            container(label(scanning))
                .padding(style::status_label_padding(s))
                .into(),
        );
    }
    items.push(count);
    let (bar_style, bar_edges) = style::status_bar(p, s);
    // Alto fijo como el `QStatusBar` de 1.3.0: con el alto de la línea de Iced (16,9) todo lo
    // de arriba quedaba en medio píxel y los bordes salían borrosos. Relleno izquierdo de 6:
    // la tinta del mensaje cae en x=9, como en Qt.
    // El borde de arriba (`border-top` de 98 y VGUI) va dentro de ese alto.
    let height =
        style::status_bar_height(s) - bar_edges.and_then(|e| e.top).map_or(0.0, |(w, _)| w);
    let bar = container(row(items).spacing(12).align_y(Center))
        .width(Fill)
        .height(height)
        .center_y(height)
        .padding(Padding {
            left: 6.0,
            right: 10.0,
            ..Padding::ZERO
        })
        .style(move |_| bar_style);
    match bar_edges {
        Some(e) => framed(bar, e),
        None => bar.into(),
    }
}
