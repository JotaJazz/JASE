//! Barra de herramientas, atajos y barra de estado (`test_main_window.py`): todo sobre
//! `bars::*` y `App::update`, lo mismo que dibuja la vista.

mod support;

use std::time::Duration;

use iced::Task;
use iced::keyboard::{Key, Modifiers, key::Named};
use jase::actions::{Action, Menu, Msg as A};
use jase::app::{App, Focus, Message};
use jase::bars::{Shortcut, shortcut_for, status_bar, toolbar, toolbar_groups};
use jase::scan::ScanMsg;
use jase::sidebar::{self, Combo};
use jase::table;
use jase::theme::VARIANTS;
use jase_core::config::Config;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::scanner::scan::{ScanEvent, ScanResult};
use support::{clipboard_writes, fake_app, fake_app_task, filter_ops, mk};

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

fn select_row(app: &mut App, host: &str) {
    let address = ServerAddress::new(host, 28960);
    let _ = app.update(Message::Table(table::Msg::RowClicked(address)));
}

fn shortcut(app: &mut App, s: Shortcut) -> Task<Message> {
    app.update(Message::Shortcut(s))
}

/// Con una fila elegida y la tabla enfocada.
fn app_with_row(dir: &std::path::Path) -> (support::TestApp, support::FakeScan) {
    let (mut app, fake) = fake_app(dir, Config::default());
    complete(&mut app, &[mk("1.1.1.1", "a", 40, 3, "1.3")]);
    select_row(&mut app, "1.1.1.1");
    assert_eq!(app.focus(), Focus::Table);
    (app, fake)
}

#[test]
fn toolbar_has_action_groups_in_order() {
    use Action as X;
    assert_eq!(
        toolbar_groups(),
        [
            &[X::Refresh, X::Stop, X::RefreshSelected][..],
            &[X::Connect, X::Favorite, X::Copy],
            &[X::Settings, X::Theme],
        ]
    );
    // Solo iconos, como `ToolButtonIconOnly`.
    assert!(toolbar_groups().concat().iter().all(|a| a.icon().is_some()));
}

#[test]
fn shortcut_for_maps_keys() {
    let ch = |c: &str| Key::Character(c.into());
    let ctrl = Modifiers::COMMAND;
    let none = Modifiers::empty();
    assert_eq!(
        shortcut_for(&Key::Named(Named::F5), none),
        Some(Shortcut::F5)
    );
    assert_eq!(shortcut_for(&ch("f"), ctrl), Some(Shortcut::Find));
    assert_eq!(shortcut_for(&ch("F"), ctrl), Some(Shortcut::Find));
    assert_eq!(shortcut_for(&ch("l"), ctrl), Some(Shortcut::Lan));
    assert_eq!(shortcut_for(&ch("c"), ctrl), Some(Shortcut::Copy));
    assert_eq!(shortcut_for(&ch("n"), ctrl), Some(Shortcut::AddServer));
    assert_eq!(shortcut_for(&ch("v"), ctrl), Some(Shortcut::Paste));
    // `QKeySequence.Quit` es Ctrl+Q fuera de Windows; en Windows no tiene atajo.
    let quit = shortcut_for(&ch("q"), ctrl);
    if cfg!(windows) {
        assert_eq!(quit, None);
        assert_eq!(Action::Quit.shortcut(), None);
    } else {
        assert_eq!(quit, Some(Shortcut::Quit));
        assert_eq!(Action::Quit.shortcut(), Some("Ctrl+Q"));
    }
    // Sin Ctrl, con otro modificador de más o con F5 modificado: nada.
    assert_eq!(shortcut_for(&ch("f"), none), None);
    assert_eq!(
        shortcut_for(&ch("f"), ctrl | Modifiers::SHIFT),
        Some(Shortcut::FindPlayer)
    );
    assert_eq!(
        shortcut_for(&ch("R"), ctrl | Modifiers::SHIFT),
        Some(Shortcut::Rcon)
    );
    assert_eq!(shortcut_for(&ch("l"), ctrl | Modifiers::SHIFT), None);
    assert_eq!(shortcut_for(&ch("f"), ctrl | Modifiers::ALT), None);
    assert_eq!(shortcut_for(&Key::Named(Named::F5), ctrl), None);
    assert_eq!(shortcut_for(&ch("x"), ctrl), None);
}

#[test]
fn f5_refreshes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_row(dir.path());
    let _ = shortcut(&mut app, Shortcut::F5);
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
    // También escribiendo en el campo: F5 es de toda la ventana.
    let _ = app.update(Message::FilterFocused(true));
    let _ = shortcut(&mut app, Shortcut::F5);
    assert_eq!(fake.calls(), ["cod2", "cod2", "cod2"]);
}

#[test]
fn ctrl_l_starts_lan_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let _ = shortcut(&mut app, Shortcut::Lan);
    assert_eq!(fake.lan_calls(), ["cod2"]);
    assert!(app.scan.lan());
}

#[test]
fn ctrl_f_focuses_filter_and_selects_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_row(dir.path());
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text("abc".into())));
    let task = shortcut(&mut app, Shortcut::Find);
    assert_eq!(app.focus(), Focus::Filter);
    let ops = filter_ops(task);
    assert!(ops.contains(&"focus"), "{ops:?}");
    assert!(ops.contains(&"select_all"), "{ops:?}");
    assert_eq!(app.filter_text(), "abc");
}

#[test]
fn ctrl_c_on_table_copies_address() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_row(dir.path());
    let task = shortcut(&mut app, Shortcut::Copy);
    assert_eq!(clipboard_writes(task), ["1.1.1.1:28960"]);
}

#[test]
fn ctrl_c_copies_only_with_table_focus() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_row(dir.path());
    // Escribiendo en el campo, Ctrl+C copia el texto (lo hace el `text_input`), no el servidor.
    let _ = app.update(Message::FilterFocused(true));
    assert_eq!(app.focus(), Focus::Filter);
    let status = app.status().to_owned();
    let task = shortcut(&mut app, Shortcut::Copy);
    assert!(clipboard_writes(task).is_empty());
    assert_eq!(app.status(), status);
    // Con el árbol tampoco.
    let _ = app.update(Message::Tree(jase::tree::Msg::Nav(jase::tree::Nav::Up)));
    assert_eq!(app.focus(), Focus::Tree);
    assert!(clipboard_writes(shortcut(&mut app, Shortcut::Copy)).is_empty());
}

#[test]
fn shortcuts_ignored_with_dialog_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_row(dir.path());
    let _ = app.update(Message::Actions(A::Run(Action::About)));
    assert_eq!(app.focus(), Focus::Dialog);
    for s in [
        Shortcut::F5,
        Shortcut::Find,
        Shortcut::Lan,
        Shortcut::Copy,
        Shortcut::Quit,
        Shortcut::AddServer,
        Shortcut::Paste,
    ] {
        let task = shortcut(&mut app, s);
        assert!(iced_runtime::task::into_stream(task).is_none(), "{s:?}");
    }
    assert_eq!(fake.calls(), ["cod2"]);
    assert!(fake.lan_calls().is_empty());
    assert!(fake.exits.lock().unwrap().is_empty());
    assert_eq!(app.focus(), Focus::Dialog);
    assert!(app.dialog().is_some());
}

/// Con un menú abierto (`QMenu` se queda con el teclado) o la lista de un combo abierta, ningún
/// atajo corre.
#[test]
fn shortcuts_ignored_with_menu_or_combo_open() {
    let all = [
        Shortcut::F5,
        Shortcut::Find,
        Shortcut::Lan,
        Shortcut::Copy,
        Shortcut::Quit,
        Shortcut::AddServer,
        Shortcut::Paste,
    ];
    for open in [
        Message::Actions(A::Open(Menu::File)),
        Message::Actions(A::Open(Menu::Context)),
        Message::Sidebar(sidebar::Msg::ComboOpened(Combo::Ping)),
        Message::Sidebar(sidebar::Msg::CountryOpened),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake) = app_with_row(dir.path());
        let _ = app.update(open.clone());
        for s in all {
            let task = shortcut(&mut app, s);
            assert!(
                iced_runtime::task::into_stream(task).is_none(),
                "{open:?} {s:?}"
            );
        }
        assert_eq!(fake.calls(), ["cod2"], "{open:?}");
        assert!(fake.lan_calls().is_empty(), "{open:?}");
        assert!(fake.exits.lock().unwrap().is_empty(), "{open:?}");
        assert_ne!(app.focus(), Focus::Filter, "{open:?}");
    }
    // Cerrada la lista (o elegida una opción), F5 vuelve a andar.
    for close in [
        sidebar::Msg::ComboClosed(Combo::Ping),
        sidebar::Msg::MaxPing(100),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake) = app_with_row(dir.path());
        let _ = app.update(Message::Sidebar(sidebar::Msg::ComboOpened(Combo::Ping)));
        let _ = app.update(Message::Sidebar(close));
        let _ = shortcut(&mut app, Shortcut::F5);
        assert_eq!(fake.calls(), ["cod2", "cod2"]);
    }
}

#[test]
fn quit_shortcut_closes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    // Pregunta a la ventana si está maximizada; la respuesta sigue en `Close`.
    let task = shortcut(&mut app, Shortcut::Quit);
    assert!(fake.exits.lock().unwrap().is_empty());
    assert_eq!(support::window_requests(task), 1);
    let _ = app.update(Message::Close { maximized: false });
    assert_eq!(*fake.exits.lock().unwrap(), [0]);
}

#[test]
fn disabled_action_ignores_its_shortcut() {
    // Sin selección, Copiar está apagada: Ctrl+C con la tabla enfocada no hace nada.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::Table(table::Msg::BlankClicked));
    assert_eq!(app.focus(), Focus::Table);
    let task = shortcut(&mut app, Shortcut::Copy);
    assert!(iced_runtime::task::into_stream(task).is_none());
}

#[test]
fn timed_message_expires_only_with_its_seq() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_row(dir.path());
    // «Dirección copiada» dura 4 s (`showMessage(…, 4000)`).
    let task = shortcut(&mut app, Shortcut::Copy);
    assert!(iced_runtime::task::into_stream(task).is_some());
    assert_eq!(app.status(), "Dirección copiada: 1.1.1.1:28960");
    let old = app.status_seq();
    // Uno nuevo (sin plazo) antes de que venza: el plazo viejo ya no lo borra.
    let _ = app.update(Message::ScanLan);
    assert_ne!(app.status_seq(), old);
    let lan = app.status().to_owned();
    let _ = app.update(Message::StatusExpired(old));
    assert_eq!(app.status(), lan);
    // El vigente sí vence.
    let _ = app.show_message("hola".into(), Some(Duration::from_secs(4)));
    let seq = app.status_seq();
    let _ = app.update(Message::StatusExpired(seq));
    assert_eq!(app.status(), "");
}

#[test]
fn timed_message_task_sends_its_seq() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let task = app.show_message("hola".into(), Some(Duration::from_millis(1)));
    let msgs = support::run_task(task);
    assert!(
        matches!(msgs.as_slice(), [Message::StatusExpired(s)] if *s == app.status_seq()),
        "{msgs:?}"
    );
    // Sin plazo no hay `Task`.
    let task = app.show_message("fijo".into(), None);
    assert!(iced_runtime::task::into_stream(task).is_none());
}

#[test]
fn scanning_label_follows_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    assert!(app.is_scanning());
    assert_eq!(app.scanning_text(), "Escaneando…");
    let _ = app.update(Message::Stop);
    assert_eq!(app.scanning_text(), "");
    let _ = app.update(Message::ScanLan);
    assert_eq!(app.scanning_text(), "Escaneando…");
    complete(&mut app, &[]);
    assert_eq!(app.scanning_text(), "");
}

#[test]
fn menu_open_blocks_typing_in_filter() {
    // `QMenu` se queda con el teclado: lo escrito no entra en el campo enfocado.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let _ = app.update(Message::FilterFocused(true));
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text("ab".into())));
    let _ = app.update(Message::Actions(A::Open(Menu::File)));
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text("abc".into())));
    assert_eq!(app.filter_text(), "ab");
    let _ = app.update(Message::Actions(A::Close));
    let _ = app.update(Message::Sidebar(sidebar::Msg::Text("abc".into())));
    assert_eq!(app.filter_text(), "abc");
}

#[test]
fn startup_focus_matches_qt() {
    // Comprobado con la app Python (`QT_QPA_PLATFORM=offscreen`, `focusWidget()` tras `show()`):
    // Qt arranca en el `QCheckBox` «Solo juegos instalados», no en el campo del filtro.
    let dir = tempfile::tempdir().unwrap();
    let (app, _, task) = fake_app_task(dir.path(), Config::default());
    assert_eq!(app.focus(), Focus::Sidebar);
    assert!(!filter_ops(task).contains(&"focus"));
}

/// Barra de herramientas y de estado en las 14 variantes: quieto, escaneando con progreso y
/// con el menú del botón de tema abierto.
#[test]
fn toolbar_view_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_row(dir.path());
    for (id, _, _) in VARIANTS.iter() {
        app.theme_id = id;
        let _ = toolbar(&app);
        let _ = status_bar(&app);
        let _ = app.update(Message::Refresh);
        let generation = app.scan.generation();
        let _ = app.update(Message::Scan {
            generation,
            msg: ScanMsg::Event(ScanEvent::Progress { done: 2, total: 5 }),
        });
        let _ = status_bar(&app);
        let _ = app.update(Message::Actions(A::Open(Menu::ThemeButton)));
        let _ = toolbar(&app);
        let _ = app.view();
        let _ = app.update(Message::Actions(A::Close));
        let _ = app.update(Message::Stop);
    }
}

/// Medidas de la barra de menús y de herramientas tomadas de la app Qt (offscreen,
/// `menuBar().geometry()`, `actionGeometry`, `version_combo.geometry()`) en dark, classic, xp,
/// goldsrc y terminal.
#[test]
fn bar_metrics_match_qt() {
    use jase::theme::{WidgetStyle, palette, style, widget_style};
    // Alto de la barra de menús con su línea de abajo.
    for (id, qt) in [
        ("dark", 19.0),
        ("classic", 19.0),
        ("xp", 17.0),
        ("goldsrc", 17.0),
        ("terminal_green", 18.0),
    ] {
        let (_, line, h, _) = style::menu_bar(palette(id), widget_style(id), id);
        let line = line.and_then(|e| e.bottom).map_or(0.0, |(w, _)| w);
        assert_eq!(h + line, qt, "{id}");
    }
    // Títulos desde x=6 y a 6 px entre sí en todos los estilos; plano (Fusion), 4 px de relleno.
    for (id, _, _) in VARIANTS.iter() {
        let s = widget_style(id);
        assert_eq!(style::menu_bar(palette(id), s, id).3, 6.0, "{id}");
        assert_eq!(style::menu_bar_item_spacing(s).1, 6.0, "{id}");
    }
    assert_eq!(style::menu_bar_item_spacing(WidgetStyle::Flat).0, 4.0);
    // `QStatusBar`: 27 px en Fusion (planos), 22 con las hojas de época.
    for (id, qt) in [
        ("dark", 27.0),
        ("light", 27.0),
        ("classic", 22.0),
        ("xp", 22.0),
        ("goldsrc", 22.0),
        ("terminal_amber", 22.0),
    ] {
        assert_eq!(style::status_bar_height(widget_style(id)), qt, "{id}");
    }
    // Terminal no fija el relleno del botón: hereda los 4 px de la hoja base (barra de 49 px).
    assert_eq!(style::tool_button_padding(WidgetStyle::Terminal), 4.0);
    // El combo de versión mide su opción más larga (`AdjustToContentsOnFirstShow`), no solo
    // los 6 caracteres mínimos.
    for (id, _, _) in VARIANTS.iter() {
        let six = jase::widgets::combo_width(6, &[], id);
        let long = jase::widgets::combo_width(6, &["1.4 (CoD2x)"], id);
        // El ancho del texto depende de la fuente: lo que se suma es lo que mide el texto más
        // largo que los 6 caracteres. Los 20 px salen de Qt con DejaVu; solo valen en Linux.
        let s = widget_style(id);
        let x = |t| jase::widgets::text_width(t, s.font(), s.text_size());
        let extra = x("1.4 (CoD2x)") - 6.0 * x("X");
        assert!(
            extra > 0.0 && (long - six - extra).abs() < 1e-3,
            "{id}: {six} {long}"
        );
        if cfg!(target_os = "linux") {
            assert!(long > six + 20.0, "{id}: {six} {long}");
        }
        assert_eq!(jase::widgets::combo_width(6, &["1.3"], id), six, "{id}");
    }
}

/// Con una lista abierta, un clic en otro combo abre la suya antes de que la primera avise que
/// se cerró: la abierta sigue mandando.
#[test]
fn combo_open_follows_its_own_id() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_row(dir.path());
    let side = |app: &mut App, m| app.update(Message::Sidebar(m));
    let _ = side(&mut app, sidebar::Msg::ComboOpened(Combo::Ping));
    let _ = side(&mut app, sidebar::Msg::ComboOpened(Combo::Version));
    let _ = side(&mut app, sidebar::Msg::ComboClosed(Combo::Ping));
    let _ = shortcut(&mut app, Shortcut::F5);
    assert_eq!(fake.calls(), ["cod2"]);
    let _ = side(&mut app, sidebar::Msg::ComboClosed(Combo::Version));
    let _ = shortcut(&mut app, Shortcut::F5);
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
}

/// Un diálogo que abre sin clic deja al `pick_list` sin su lista: los atajos no quedan muertos.
#[test]
fn dialog_clears_open_combo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = app_with_row(dir.path());
    let _ = app.update(Message::Sidebar(sidebar::Msg::ComboOpened(Combo::Ping)));
    let _ = app.update(Message::Actions(A::Run(Action::About)));
    assert!(app.dialog().is_some());
    let _ = app.update(Message::CloseDialog);
    let _ = shortcut(&mut app, Shortcut::F5);
    assert_eq!(fake.calls(), ["cod2", "cod2"]);
}

/// Como Qt, el clic derecho en la tabla le quita el foco al campo de filtro.
#[test]
fn right_click_unfocuses_filter() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_row(dir.path());
    let addr = app.servers()[0].address.clone();
    let task = app.update(Message::Table(table::Msg::RightClicked(Some(addr))));
    assert!(filter_ops(task).contains(&"unfocus"));
    assert_eq!(app.focus(), Focus::Table);
}
