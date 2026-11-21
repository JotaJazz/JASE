//! Menús propios, acciones compartidas y menú contextual (`test_main_window.py`): todo se
//! comprueba sobre `actions::entries` y `App::update`, lo mismo que dibuja el widget.

mod support;

use iced::keyboard::{Key, key::Named};
use iced::{Point, Rectangle, Size, Task};
use jase::actions::{Action, Menu, Msg as A, checked, enabled, entries};
use jase::app::{App, Dialog, Message, about_lines};
use jase::menu::{Check, Entry, Placement, menu_size, place};
use jase::presets;
use jase::scan::ScanMsg;
use jase::sidebar::{self, ping_options};
use jase::table;
use jase::theme::{VARIANTS, widget_style};
use jase::tree::{self, Kind};
use jase_core::config::{Config, load_config};
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use proptest::prelude::*;
use support::{clipboard_writes, fake_app, mk};

/// Etiqueta y si está encendido, de cada entrada que no es separador.
fn items(es: &[Entry<Message>]) -> Vec<(String, bool)> {
    es.iter()
        .filter_map(|e| match e {
            Entry::Item { label, on, .. } => Some((label.clone(), on.is_some())),
            Entry::Sub { label, entries, .. } => Some((label.clone(), !entries.is_empty())),
            Entry::Separator => None,
        })
        .collect()
}

fn labels(es: &[Entry<Message>]) -> Vec<String> {
    items(es).into_iter().map(|(l, _)| l).collect()
}

fn enabled_labels(es: &[Entry<Message>]) -> Vec<String> {
    items(es)
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(l, _)| l)
        .collect()
}

fn sub<'a>(es: &'a [Entry<Message>], name: &str) -> &'a [Entry<Message>] {
    es.iter()
        .find_map(|e| match e {
            Entry::Sub { label, entries, .. } if label == name => Some(entries.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("sin submenú {name:?}"))
}

fn item<'a>(es: &'a [Entry<Message>], name: &str) -> (&'a Check, Option<&'a Message>) {
    es.iter()
        .find_map(|e| match e {
            Entry::Item {
                label, check, on, ..
            } if label == name => Some((check, on.as_ref())),
            _ => None,
        })
        .unwrap_or_else(|| panic!("sin ítem {name:?}"))
}

/// Recorre los submenús de `path` y elige su último ítem (que debe estar encendido).
fn click(app: &mut App, menu: Menu, path: &[&str]) -> Task<Message> {
    let es = entries(app, menu);
    let (last, subs) = path.split_last().unwrap();
    let es = subs.iter().fold(es.as_slice(), |es, s| sub(es, s));
    let msg = item(es, last).1.cloned().expect("ítem apagado");
    app.update(msg)
}

fn radios(es: &[Entry<Message>]) -> Vec<String> {
    es.iter()
        .filter_map(|e| match e {
            Entry::Item {
                label,
                check: Check::Radio(true),
                ..
            } => Some(label.clone()),
            _ => None,
        })
        .collect()
}

fn complete(app: &mut App, servers: &[ServerInfo]) {
    let generation = app.scan.generation();
    for s in servers {
        let _ = app.update(Message::Scan {
            generation,
            msg: ScanMsg::Event(ScanEvent::Server(s.clone())),
        });
    }
    let _ = app.update(Message::Scan {
        generation,
        msg: ScanMsg::Finished(ScanResult {
            masters_ok: 1,
            servers_listed: servers.len(),
            servers_responded: servers.len(),
            errors: vec![],
        }),
    });
}

fn addr(host: &str) -> ServerAddress {
    ServerAddress::new(host, 28960)
}

fn select_row(app: &mut App, host: &str) {
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr(host))));
}

fn act(app: &mut App, m: A) -> Task<Message> {
    app.update(Message::Actions(m))
}

#[test]
fn menus_titles() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fake_app(dir.path(), Config::default());
    let titles: Vec<String> = Menu::BAR.iter().map(|m| m.title()).collect();
    assert_eq!(
        titles,
        [
            "Archivo",
            "Ver",
            "Filtro",
            "Servidor",
            "Jugador",
            "Amigos",
            "Herramientas",
            "Ayuda"
        ]
    );
    let _ = app.view();
}

#[test]
fn theme_menu_has_twelve_exclusive_actions() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let view = entries(&app, Menu::View);
    assert_eq!(labels(&view), ["Tema"]);
    let themes = sub(&view, "Tema");
    let names: Vec<String> = themes
        .iter()
        .filter_map(|e| match e {
            Entry::Item { label, check, .. } => {
                assert!(matches!(check, Check::Radio(_)));
                Some(label.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        [
            "Sistema",
            "Oscuro",
            "Claro",
            "Neón",
            "Marrón",
            "Verde",
            "Clásico",
            "Windows XP",
            "Terminal",
            "Synthwave",
            "Óxido",
            "GoldSrc"
        ]
    );
    assert!(matches!(themes[12], Entry::Separator));
    assert_eq!(radios(themes), ["GoldSrc"]);
    let _ = click(&mut app, Menu::View, &["Tema", "Windows XP"]);
    assert_eq!(
        radios(sub(&entries(&app, Menu::View), "Tema")),
        ["Windows XP"]
    );
    assert_eq!((app.config().theme.as_str(), app.theme_id), ("xp", "xp"));
    assert_eq!(app.open_menu(), None);
}

#[test]
fn terminal_color_menu_switches_to_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let view = entries(&app, Menu::View);
    let colors = sub(sub(&view, "Tema"), "Color del fósforo").to_vec();
    assert_eq!(labels(&colors), ["Verde", "Blanco", "Sepia", "Ámbar"]);
    assert_eq!(radios(&colors), ["Verde"]);
    let _ = click(
        &mut app,
        Menu::View,
        &["Tema", "Color del fósforo", "Sepia"],
    );
    let cfg = app.config();
    assert_eq!(
        (cfg.theme.as_str(), cfg.terminal_color.as_str()),
        ("terminal", "sepia")
    );
    assert_eq!(app.theme_id, "terminal_sepia");
    let view = entries(&app, Menu::View);
    let themes = sub(&view, "Tema");
    assert_eq!(radios(themes), ["Terminal"]);
    assert_eq!(radios(sub(themes, "Color del fósforo")), ["Sepia"]);
    let _ = click(&mut app, Menu::View, &["Tema", "Oscuro"]);
    assert_eq!(app.theme_id, "dark");
    assert_eq!(app.config().terminal_color, "sepia"); // se recuerda
}

#[test]
fn theme_choice_persists() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let mut seen = Vec::new();
    for name in ["Oscuro", "Clásico", "Sistema"] {
        let _ = click(&mut app, Menu::View, &["Tema", name]);
        seen.push(load_config(&dir.path().join("config.json")).theme);
    }
    assert_eq!(seen, ["dark", "classic", "system"]);
    // «Sistema» sigue el modo que informó el sistema.
    let _ = app.update(Message::SystemMode(iced::theme::Mode::Light));
    assert_eq!(app.theme_id, "light");
}

#[test]
fn quick_filter_actions_mirror_checkboxes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let check = |app: &App| *item(&entries(app, Menu::Filter), "Ocultar vacíos").0;
    assert_eq!(check(&app), Check::Check(app.quick_filters().hide_empty));
    let before = app.quick_filters().hide_empty;
    let _ = click(&mut app, Menu::Filter, &["Ocultar vacíos"]);
    assert_eq!(app.quick_filters().hide_empty, !before);
    assert_eq!(check(&app), Check::Check(!before));
    assert_eq!(app.table.model.filter().quick.hide_empty, !before);
    let _ = app.update(Message::Sidebar(sidebar::Msg::HideEmpty(before)));
    assert_eq!(check(&app), Check::Check(before));
    assert_eq!(checked(&app, Action::HideEmpty), Some(before));
}

#[test]
fn ping_menu_mirrors_combo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let options = ping_options(app.quick_filters().max_ping);
    let ping = |app: &App| sub(&entries(app, Menu::Filter), "Ping máx.").to_vec();
    let texts: Vec<String> = options.iter().map(|o| o.label.clone()).collect();
    assert_eq!(labels(&ping(&app)), texts);
    let current = options
        .iter()
        .find(|o| o.ms == app.quick_filters().max_ping)
        .unwrap();
    assert_eq!(radios(&ping(&app)), std::slice::from_ref(&current.label));
    let _ = click(&mut app, Menu::Filter, &["Ping máx.", &options[2].label]); // menú → combo
    assert_eq!(app.quick_filters().max_ping, options[2].ms);
    let _ = app.update(Message::Sidebar(sidebar::Msg::MaxPing(options[1].ms))); // combo → menú
    assert_eq!(radios(&ping(&app)), [options[1].label.clone()]);
}

#[test]
fn clear_tree_marks_action_clears_current_game() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    let label = "Quitar filtros de mod/mapa/país";
    // Sin marcas la acción está apagada.
    assert!(item(&entries(&app, Menu::Filter), label).1.is_none());
    let _ = app.update(Message::Tree(tree::Msg::Toggle(
        "cod2",
        Kind::Maps,
        "mp_toujane".into(),
    )));
    assert!(enabled(&app, Action::ClearTreeMarks));
    let _ = click(&mut app, Menu::Filter, &[label]);
    assert!(app.tree.marks("cod2").is_empty());
    assert!(!enabled(&app, Action::ClearTreeMarks));
}

#[test]
fn actions_follow_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    let keys = [Action::RefreshSelected, Action::Copy, Action::Favorite];
    assert!(!keys.iter().any(|&a| enabled(&app, a)));
    select_row(&mut app, "1.1.1.1");
    assert!(keys.iter().all(|&a| enabled(&app, a)));
    assert_eq!(Action::Favorite.label(&app), "Agregar a favoritos");
    app.table.selected = None;
    assert!(!keys.iter().any(|&a| enabled(&app, a)));
}

const CONTEXT: [&str; 10] = [
    "Conectar",
    "Copiar IP:puerto",
    "Copiar enlace jase://",
    "Refrescar este servidor",
    "Agregar a favoritos",
    "Editar nota…",
    "Grupo",
    "Avisar cuando haya sitio",
    "Avisar cuando se arme partida…",
    "Agregar servidor…",
];

#[test]
fn context_menu_actions() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_row(&mut app, "1.1.1.1");
    let menu = entries(&app, Menu::Context);
    assert_eq!(labels(&menu), CONTEXT);
    // Python enciende todo salvo «Avisar cuando haya sitio» (no está lleno), «Editar nota…» y
    // «Grupo» (no es favorito).
    assert_eq!(
        enabled_labels(&menu),
        [
            "Conectar",
            "Copiar IP:puerto",
            "Copiar enlace jase://",
            "Refrescar este servidor",
            "Agregar a favoritos",
            "Avisar cuando se arme partida…",
            "Agregar servidor…"
        ]
    );
    app.table.selected = None;
    // Sin selección solo queda «Agregar servidor…».
    assert_eq!(
        enabled_labels(&entries(&app, Menu::Context)),
        ["Agregar servidor…"]
    );
}

#[test]
fn context_menu_reuses_actions() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_row(&mut app, "1.1.1.1");
    let shared = [
        Action::Connect,
        Action::Copy,
        Action::CopyLink,
        Action::RefreshSelected,
        Action::Favorite,
        Action::EditNote,
        Action::Group,
        Action::WatchSlot,
        Action::PartyAlert,
        Action::AddServer,
    ];
    let expected: Vec<(String, bool)> = shared
        .iter()
        .map(|&a| (a.label(&app), enabled(&app, a)))
        .collect();
    assert_eq!(items(&entries(&app, Menu::Context)), expected);
    // Y el mismo mensaje que el menú «Servidor».
    let server = entries(&app, Menu::Server);
    let context = entries(&app, Menu::Context);
    assert_eq!(
        format!("{:?}", item(&server, "Copiar IP:puerto").1),
        format!("{:?}", item(&context, "Copiar IP:puerto").1)
    );
}

#[test]
fn context_menu_copy_action_copies_connect_address() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let mut info = mk("1.1.1.1", "a", 40, 3, "1.3");
    info.connect_address = ServerAddress::new("1.1.1.1", 28961);
    complete(&mut app, &[info]);
    let _ = app.update(Message::Table(table::Msg::RightClicked(Some(addr(
        "1.1.1.1",
    )))));
    assert_eq!(app.open_menu(), Some(Menu::Context));
    let task = click(&mut app, Menu::Context, &["Copiar IP:puerto"]);
    assert_eq!(clipboard_writes(task), ["1.1.1.1:28961"]);
    assert!(app.status().contains("1.1.1.1:28961"));
    assert_eq!(app.open_menu(), None);
}

#[test]
fn right_click_on_blank_space_clears_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_row(&mut app, "1.1.1.1");
    let _ = app.update(Message::Table(table::Msg::RightClicked(None)));
    assert!(app.table.selected.is_none());
    assert_eq!(app.open_menu(), Some(Menu::Context));
    let on = |app: &App| -> Vec<bool> {
        items(&entries(app, Menu::Context))
            .into_iter()
            .map(|(_, on)| on)
            .collect()
    };
    // Sin selección solo «Agregar servidor…» (el último).
    let mut off = [false; 10];
    off[9] = true;
    assert_eq!(on(&app), off);
    let _ = act(&mut app, A::Close);
    let _ = app.update(Message::Table(table::Msg::RightClicked(Some(addr(
        "1.1.1.1",
    )))));
    assert_eq!(app.table.selected, Some(addr("1.1.1.1")));
    assert!(app.table.focused);
    // Python: [T, T, T, T, T, F, F, F, T, T].
    assert_eq!(
        on(&app),
        [
            true, true, true, true, true, false, false, false, true, true
        ]
    );
}

#[test]
fn about_shows_package_version() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = click(&mut app, Menu::Help, &["Acerca de"]);
    assert_eq!(app.dialog(), Some(&Dialog::About));
    let [title, name, version] = about_lines();
    assert_eq!(title, "Acerca de Jimmy's All Seeing Eye");
    assert!(name.contains("Jimmy"));
    assert!(version.contains(env!("CARGO_PKG_VERSION")));
    let _ = app.view();
    let _ = app.update(Message::CloseDialog);
    assert_eq!(app.dialog(), None);
}

#[test]
fn hide_bots_check_mirrors_menu_and_presets() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = click(&mut app, Menu::Filter, &["Ocultar solo bots"]);
    assert!(app.quick_filters().hide_bots);
    let _ = click(&mut app, Menu::Presets, &["Guardar filtros actuales…"]);
    let _ = app.update(Message::Presets(presets::Msg::NameChanged("B".into())));
    let _ = app.update(Message::Presets(presets::Msg::Submit));
    let _ = app.update(Message::Sidebar(sidebar::Msg::HideBots(false)));
    assert_eq!(checked(&app, Action::HideBots), Some(false));
    let _ = click(&mut app, Menu::Presets, &["B"]);
    assert!(app.quick_filters().hide_bots);
    assert_eq!(
        item(&entries(&app, Menu::Filter), "Ocultar solo bots").0,
        &Check::Check(true)
    );
}

#[test]
fn presets_menu_layout() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let menu = entries(&app, Menu::Presets);
    // Sin combinaciones: sin separador y «Borrar» apagado.
    assert_eq!(
        items(&menu),
        [
            ("Guardar filtros actuales…".to_owned(), true),
            ("Borrar".to_owned(), false)
        ]
    );
    assert!(!menu.iter().any(|e| matches!(e, Entry::Separator)));
    for name in ["Uno", "Dos"] {
        let _ = click(&mut app, Menu::Presets, &["Guardar filtros actuales…"]);
        let _ = app.update(Message::Presets(presets::Msg::NameChanged(name.into())));
        let _ = app.update(Message::Presets(presets::Msg::Submit));
    }
    let menu = entries(&app, Menu::Presets);
    assert_eq!(
        labels(&menu),
        ["Uno", "Dos", "Guardar filtros actuales…", "Borrar"]
    );
    assert!(matches!(menu[2], Entry::Separator));
    assert_eq!(labels(sub(&menu, "Borrar")), ["Uno", "Dos"]);
    let _ = click(&mut app, Menu::Presets, &["Borrar", "Uno"]);
    let names: Vec<&str> = app
        .filter_presets()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(names, ["Dos"]);
}

fn fits(p: Point, size: Size, window: Size) -> bool {
    // Un ulp de holgura: `(w - a) + a` puede pasarse por redondeo.
    let eps = 0.01;
    p.x >= 0.0
        && p.y >= 0.0
        && p.x + size.width <= window.width + eps
        && p.y + size.height <= window.height + eps
}

#[test]
fn place_follows_qmenu() {
    let window = Size::new(800.0, 600.0);
    let anchor = Rectangle::new(Point::new(10.0, 0.0), Size::new(40.0, 20.0));
    let size = Size::new(100.0, 200.0);
    assert_eq!(
        place(anchor, size, window, Placement::Below),
        Point::new(10.0, 20.0)
    );
    assert_eq!(
        place(anchor, size, window, Placement::Right),
        Point::new(50.0, 0.0)
    );
    let cursor = Rectangle::new(Point::new(300.0, 300.0), Size::ZERO);
    assert_eq!(
        place(cursor, size, window, Placement::Cursor),
        Point::new(300.0, 300.0)
    );
    // Sin lugar a la derecha ni abajo: a la izquierda y arriba del cursor, como `QMenu`.
    let corner = Rectangle::new(Point::new(750.0, 550.0), Size::ZERO);
    assert_eq!(
        place(corner, size, window, Placement::Cursor),
        Point::new(650.0, 350.0)
    );
    // Un submenú que no cabe a la derecha se abre a la izquierda de su ítem.
    let item = Rectangle::new(Point::new(700.0, 100.0), Size::new(90.0, 20.0));
    assert_eq!(
        place(item, size, window, Placement::Right),
        Point::new(600.0, 100.0)
    );
}

proptest! {
    #[test]
    fn place_keeps_popup_inside_window(
        (ww, wh) in (50u16..2000, 50u16..2000),
        fw in 0.0f32..1.0, fh in 0.0f32..1.0,
        ax in 0.0f32..1.0, ay in 0.0f32..1.0, aw in 0.0f32..200.0, ah in 0.0f32..40.0,
        which in 0usize..3,
    ) {
        let window = Size::new(f32::from(ww), f32::from(wh));
        let size = Size::new((fw * window.width).max(1.0), (fh * window.height).max(1.0));
        let anchor = Rectangle::new(
            Point::new(ax * window.width, ay * window.height),
            Size::new(aw.min(window.width * (1.0 - ax)), ah.min(window.height * (1.0 - ay))),
        );
        let placement = [Placement::Below, Placement::Right, Placement::Cursor][which];
        let p = place(anchor, size, window, placement);
        prop_assert!(fits(p, size, window), "{p:?} {size:?} {window:?} {anchor:?}");
    }
}

#[test]
fn hover_switches_open_bar_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    // Sin menú abierto, pasar por un título no abre nada.
    let _ = act(&mut app, A::Hover(Menu::View));
    assert_eq!(app.open_menu(), None);
    let _ = act(&mut app, A::Open(Menu::File));
    assert_eq!(app.open_menu(), Some(Menu::File));
    let _ = act(&mut app, A::Hover(Menu::View));
    assert_eq!(app.open_menu(), Some(Menu::View));
    let _ = app.view();
    let _ = act(&mut app, A::Close);
    assert_eq!(app.open_menu(), None);
    // El de «Filtros guardados» no se cambia por la barra.
    let _ = act(&mut app, A::Open(Menu::Presets));
    let _ = act(&mut app, A::Hover(Menu::Help));
    assert_eq!(app.open_menu(), Some(Menu::Presets));
}

#[test]
fn escape_closes_menu_before_dialog() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = click(&mut app, Menu::Presets, &["Guardar filtros actuales…"]);
    assert!(app.preset_name().is_some());
    let _ = act(&mut app, A::Open(Menu::Help));
    let _ = app.update(Message::Escape);
    assert_eq!(app.open_menu(), None);
    assert!(app.preset_name().is_some());
    let _ = app.update(Message::Escape);
    assert!(app.dialog().is_none());
}

#[test]
fn arrows_go_nowhere_while_menu_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_row(&mut app, "1.1.1.1");
    let down = Key::Named(Named::ArrowDown);
    assert!(app.key_message(down.clone()).is_some());
    let _ = act(&mut app, A::Open(Menu::Server));
    assert!(app.key_message(down.clone()).is_none());
    let _ = act(&mut app, A::Close);
    assert!(app.key_message(down).is_some());
}

/// Todos los menús, con sus submenús, en las 14 variantes: medidas positivas y vista armada.
/// Qt corre el texto para dejar la columna de la casilla (lo que mide `QMenu::indicator`) o
/// del icono (16 + 4) si algún ítem del menú la usa.
#[test]
fn check_or_icon_column_widens_menu() {
    let plain = |check, icon| Entry::<Message>::Item {
        label: "Abc".into(),
        shortcut: None,
        check,
        icon,
        on: None,
    };
    for (id, _, _) in VARIANTS.iter() {
        let s = widget_style(id);
        let base = menu_size(&[plain(Check::None, None)], s).width;
        let indicator = if s == jase::theme::WidgetStyle::Flat {
            16.0
        } else {
            13.0
        };
        let radio = menu_size(
            &[plain(Check::None, None), plain(Check::Radio(false), None)],
            s,
        );
        assert_eq!(radio.width, base + indicator, "{id}");
        let icon = menu_size(
            &[plain(Check::None, None), plain(Check::None, Some("copy"))],
            s,
        );
        assert_eq!(icon.width, base + 20.0, "{id}");
    }
}

/// Alto de Archivo, Ver y Ver → Tema como `QMenu.size()` de la app Qt (offscreen, 1.3.0): ítem =
/// `QFontMetrics::height()` más el relleno, separador y marco de cada hoja.
#[test]
fn menu_heights_match_qt() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    // Con icono (Archivo → Configuración, Ver → Tema) la fila mide lo que el icono de 16 px.
    for (id, file, view, theme) in [
        ("dark", 175.0, 28.0, 323.0),
        ("classic", 147.0, 28.0, 261.0),
        ("xp", 144.0, 28.0, 260.0),
        ("goldsrc", 145.0, 26.0, 259.0),
        ("terminal_green", 151.0, 30.0, 275.0),
    ] {
        app.theme_id = id;
        let s = widget_style(id);
        // Qt se midió con Ajustes y su separador en Archivo (hoy en Herramientas): se reponen.
        let mut old_file = entries(&app, Menu::File);
        let tools = entries(&app, Menu::Tools);
        let quit = old_file.pop().unwrap();
        old_file.extend([tools[2].clone(), Entry::Separator, quit]);
        assert_eq!(menu_size(&old_file, s).height, file, "{id}");
        let es = entries(&app, Menu::View);
        assert_eq!(menu_size(&es, s).height, view, "{id}");
        let Some(Entry::Sub { entries: sub, .. }) = es.first() else {
            panic!("{id}: Ver sin Tema")
        };
        assert_eq!(menu_size(sub, s).height, theme, "{id}");
    }
}

/// Arma la vista con tiny-skia sin ventana y la dibuja: así también se arma y se dibuja el
/// overlay del menú abierto (`app.view()` solo no lo construye).
fn render(app: &App) {
    let mut renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
        iced::Font::DEFAULT,
        iced::Pixels(13.0),
    ));
    let mut ui = iced_runtime::UserInterface::<Message, iced::Theme, iced::Renderer>::build(
        app.view(),
        Size::new(1280.0, 800.0),
        iced_runtime::user_interface::Cache::default(),
        &mut renderer,
    );
    let at = iced::mouse::Cursor::Available(Point::new(400.0, 300.0));
    // Un clic derecho sobre la tabla: el menú contextual toma de ahí su posición (lo que
    // publique se descarta).
    let _ = ui.update(
        &[iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
            iced::mouse::Button::Right,
        ))],
        at,
        &mut renderer,
        &mut iced::advanced::clipboard::Null,
        &mut Vec::new(),
    );
    ui.draw(
        &mut renderer,
        &app.theme(),
        &iced::advanced::renderer::Style::default(),
        at,
    );
}

#[test]
fn menu_view_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_row(&mut app, "1.1.1.1");
    fn sizes(es: &[Entry<Message>], id: &str) {
        let s = menu_size(es, widget_style(id));
        assert!(s.width > 50.0 && s.height > 20.0, "{id}: {s:?}");
        for e in es {
            if let Entry::Sub { entries, .. } = e
                && !entries.is_empty()
            {
                sizes(entries, id);
            }
        }
    }
    for (id, _, _) in VARIANTS.iter() {
        app.theme_id = id;
        for m in [
            Menu::File,
            Menu::View,
            Menu::Filter,
            Menu::Server,
            Menu::Player,
            Menu::Friends,
            Menu::Tools,
            Menu::Help,
            Menu::Presets,
        ] {
            let _ = act(&mut app, A::Open(m));
            sizes(&entries(&app, m), id);
            render(&app);
        }
        let _ = app.update(Message::Table(table::Msg::RightClicked(None)));
        render(&app);
        let _ = act(&mut app, A::Close);
        let _ = act(&mut app, A::Run(Action::About));
        render(&app);
        let _ = app.update(Message::CloseDialog);
    }
}

/// El widget de menú sin ventana: `UserInterface` con tiny-skia y eventos sintéticos sobre un
/// `popup` abierto bajo un ancla de 40×20 en la esquina.
mod widget {
    use iced::advanced::clipboard;
    use iced::mouse::{self, Button, Cursor};
    use iced::widget::{Space, container};
    use iced::{Element, Event, Point, Size};
    use iced_runtime::user_interface::{Cache, UserInterface};
    use jase::menu::{Check, Entry, Placement, menu_size, popup};
    use jase::theme::{VARIANTS, widget_style};

    fn item(label: &str) -> Entry<String> {
        Entry::Item {
            label: label.into(),
            shortcut: None,
            check: Check::None,
            icon: None,
            on: Some(label.into()),
        }
    }

    fn entries() -> Vec<Entry<String>> {
        vec![
            item("a"),
            Entry::Sub {
                label: "sub".into(),
                icon: None,
                entries: vec![item("b")],
            },
        ]
    }

    fn view(theme_id: &'static str) -> Element<'static, String> {
        let anchor = container(Space::new()).width(40).height(20);
        popup(
            anchor.into(),
            Some(entries()),
            Placement::Below,
            "dismiss".into(),
            theme_id,
        )
    }

    /// Manda cada evento con el cursor en su punto (reconstruyendo la vista entre uno y otro,
    /// como el runtime) y devuelve los mensajes publicados.
    fn send(theme_id: &'static str, steps: &[(Point, Event)]) -> Vec<String> {
        let mut renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
            iced::Font::DEFAULT,
            iced::Pixels(13.0),
        ));
        let mut cache = Cache::default();
        let mut messages = Vec::new();
        for (at, event) in steps {
            let mut ui = UserInterface::<String, iced::Theme, iced::Renderer>::build(
                view(theme_id),
                Size::new(800.0, 600.0),
                cache,
                &mut renderer,
            );
            let _ = ui.update(
                std::slice::from_ref(event),
                Cursor::Available(*at),
                &mut renderer,
                &mut clipboard::Null,
                &mut messages,
            );
            // El overlay también se dibuja (estilos y marcas de cada tema).
            ui.draw(
                &mut renderer,
                &iced::Theme::Dark,
                &iced::advanced::renderer::Style::default(),
                Cursor::Available(*at),
            );
            cache = ui.into_cache();
        }
        messages
    }

    fn moved(at: Point) -> (Point, Event) {
        (at, Event::Mouse(mouse::Event::CursorMoved { position: at }))
    }

    fn press(at: Point) -> (Point, Event) {
        (at, Event::Mouse(mouse::Event::ButtonPressed(Button::Left)))
    }

    fn release(at: Point) -> (Point, Event) {
        (at, Event::Mouse(mouse::Event::ButtonReleased(Button::Left)))
    }

    /// Centro de la fila `i` de la lista que abre en `origin` (sin separadores).
    fn row(theme_id: &str, origin: Point, i: usize) -> Point {
        let size = menu_size(&entries(), widget_style(theme_id));
        let h = (size.height - 2.0 * frame(theme_id)) / 2.0;
        Point::new(
            origin.x + size.width / 2.0,
            origin.y + frame(theme_id) + h * (i as f32 + 0.5),
        )
    }

    fn frame(theme_id: &str) -> f32 {
        jase::theme::style::menu_frame(jase::theme::palette(theme_id), widget_style(theme_id)).2
    }

    /// Estado de `update` para un solo evento con el menú abierto.
    fn status(
        theme_id: &'static str,
        at: Point,
        event: Event,
    ) -> (iced::event::Status, Vec<String>) {
        let mut renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
            iced::Font::DEFAULT,
            iced::Pixels(13.0),
        ));
        let mut ui = UserInterface::<String, iced::Theme, iced::Renderer>::build(
            view(theme_id),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let (_, statuses) = ui.update(
            &[event],
            Cursor::Available(at),
            &mut renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        (statuses[0], messages)
    }

    #[test]
    fn wheel_outside_is_swallowed() {
        // Qt captura el ratón mientras el menú está abierto: la tabla de abajo no se desplaza.
        let wheel = Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        });
        let (st, msgs) = status("dark", Point::new(700.0, 500.0), wheel);
        assert_eq!(st, iced::event::Status::Captured);
        assert!(msgs.is_empty());
    }

    #[test]
    fn losing_window_focus_dismisses() {
        let event = Event::Window(iced::window::Event::Unfocused);
        let (_, msgs) = status("dark", Point::new(700.0, 500.0), event);
        assert_eq!(msgs, ["dismiss"]);
    }

    #[test]
    fn click_outside_dismisses() {
        for (id, _, _) in VARIANTS.iter() {
            let far = Point::new(700.0, 500.0);
            assert_eq!(send(id, &[moved(far), press(far)]), ["dismiss"], "{id}");
        }
    }

    #[test]
    fn release_over_item_picks_it() {
        for (id, _, _) in VARIANTS.iter() {
            let a = row(id, Point::new(0.0, 20.0), 0);
            assert_eq!(send(id, &[moved(a), press(a), release(a)]), ["a"], "{id}");
        }
    }

    #[test]
    fn hovering_submenu_opens_it() {
        for (id, _, _) in VARIANTS.iter() {
            let width = menu_size(&entries(), widget_style(id)).width;
            let sub = row(id, Point::new(0.0, 20.0), 1);
            // El submenú abre a la derecha de la lista, alineado con su ítem.
            let b = Point::new(width + 10.0, sub.y);
            // Sin pasar antes por «sub», ahí no hay nada: el clic cierra el menú.
            assert_eq!(send(id, &[moved(b), press(b)]), ["dismiss"], "{id}");
            assert_eq!(send(id, &[moved(sub), moved(b), release(b)]), ["b"], "{id}");
        }
    }
}
