//! Exportar, importar y copiar favoritos y el clic derecho en ★ Favoritos
//! (`window_favorites.py`, `window_add_server.py` y sus tests de `tests/test_favorites_io.py`,
//! `tests/test_add_server.py` y `tests/test_groups.py`): el selector de archivos es un falso
//! que devuelve rutas de un `tempdir`.

mod support;

use std::path::{Path, PathBuf};

use iced::Task;
use jase::actions::{self, Action, Menu, enabled, entries};
use jase::add_server::{Msg as AddMsg, find_addresses};
use jase::app::{App, Dialog, DirectMsg, DirectView, Message, PromptMsg};
use jase::files::FilesMsg;
use jase::menu::Entry;
use jase::sidebar;
use jase::table::{self, Column, visible_rows};
use jase::theme::{VARIANTS, WidgetStyle, palette};
use jase_core::config::{Config, load_config};
use jase_core::favorites::{add_favorite, set_group, set_note};
use jase_core::favorites_io::{export_favorites, read_import_file};
use jase_core::models::{ServerAddress, ServerInfo};
use support::{
    FakeScan, clipboard_reads, clipboard_writes, fake_app, mk, run_task, run_task_with_clipboard,
};

fn addr0() -> ServerAddress {
    ServerAddress::new("9.9.9.9", 28960)
}

fn other() -> ServerAddress {
    ServerAddress::new("5.6.7.8", 28960)
}

/// Un favorito de cod2 con nota.
fn sample_cfg() -> Config {
    let mut cfg = Config::default();
    add_favorite(&mut cfg, "cod2", &addr0());
    set_note(&mut cfg, "cod2", &addr0().to_string(), "el del clan");
    cfg
}

fn run(app: &mut App, a: Action) -> Task<Message> {
    app.update(Message::Actions(actions::Msg::Run(a)))
}

/// Corre la `Task` y entrega a la app lo que vuelve, y lo que eso devuelve, hasta el final
/// (menos el vencimiento de los mensajes de estado: el reloj en pausa los vence al instante).
fn drive(app: &mut App, task: Task<Message>) {
    for m in run_task(task) {
        if matches!(m, Message::StatusExpired(_)) {
            continue;
        }
        let next = app.update(m);
        drive(app, next);
    }
}

fn label(e: &Entry<Message>) -> String {
    match e {
        Entry::Item { label, .. } | Entry::Sub { label, .. } => label.clone(),
        Entry::Separator => String::new(),
    }
}

/// El aviso abierto: (título, texto).
fn warning(app: &App) -> Option<(String, String)> {
    match app.dialog() {
        Some(Dialog::Message { title, text, .. }) => Some((title.clone(), text.clone())),
        _ => None,
    }
}

fn import_from(app: &mut App, fake: &FakeScan, path: &Path) {
    *fake.files.open.lock().unwrap() = Some(path.to_path_buf());
    let task = run(app, Action::ImportFavorites);
    drive(app, task);
}

fn export_to(app: &mut App, fake: &FakeScan, path: &Path) {
    *fake.files.save.lock().unwrap() = Some(path.to_path_buf());
    let task = run(app, Action::ExportFavorites);
    drive(app, task);
}

#[test]
fn menu_has_actions() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = fake_app(dir.path(), Config::default());
    let file = entries(&app, Menu::File);
    for a in [
        Action::ExportFavorites,
        Action::ImportFavorites,
        Action::CopyFavorites,
    ] {
        assert!(enabled(&app, a), "{a:?}");
        let item = file.iter().find(|e| label(e) == a.label(&app)).unwrap();
        assert!(matches!(item, Entry::Item { on: Some(_), .. }));
    }
}

#[test]
fn export_import_via_dialogs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.json");
    let (mut app, fake) = fake_app(dir.path(), sample_cfg());
    export_to(&mut app, &fake, &path);
    assert_eq!(
        read_import_file(&path).unwrap(),
        export_favorites(&sample_cfg())
    );
    let shown = path.display().to_string();
    assert_eq!(app.status(), format!("Favoritos exportados a {shown}"));
    assert!(app.dialog().is_none());

    let dir2 = tempfile::tempdir().unwrap();
    let (mut app2, fake2) = fake_app(dir2.path(), Config::default());
    import_from(&mut app2, &fake2, &path);
    let cfg = sample_cfg();
    assert_eq!(app2.config().favorites, cfg.favorites);
    assert_eq!(app2.config().favorite_notes, cfg.favorite_notes);
    let saved = load_config(&dir2.path().join("config.json"));
    assert_eq!(saved.favorites, cfg.favorites);
    assert_eq!(app2.status(), "1 favorito importado, 0 descartados");
    assert_eq!(app2.direct_list_labels()[0], "★ Favoritos · 1");
}

#[test]
fn export_uses_config_when_path_arrives() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.json");
    let (mut app, fake) = fake_app(dir.path(), sample_cfg());
    *fake.files.save.lock().unwrap() = Some(path.clone());
    let task = run(&mut app, Action::ExportFavorites);
    let arrived = run_task(task);
    assert!(matches!(
        arrived.as_slice(),
        [Message::Files(FilesMsg::ExportTo(Some(_)))]
    ));
    // Mientras el selector estaba abierto se agregó otro.
    add_favorite(app.config_mut(), "cod2", &other());
    for m in arrived {
        let next = app.update(m);
        drive(&mut app, next);
    }
    assert_eq!(
        read_import_file(&path).unwrap(),
        export_favorites(app.config())
    );
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("5.6.7.8:28960"));
}

#[test]
fn import_bad_files_warn() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let invalid = "El archivo no es una exportación válida de favoritos de JASE";
    let large = "El archivo es demasiado grande para ser una exportación de favoritos";
    let write = |name: &str, data: Vec<u8>| -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, data).unwrap();
        path
    };
    let cases = [
        (write("bad.json", b"{nope".to_vec()), invalid.to_owned()),
        (
            write(
                "other.json",
                br#"{"format": "otro", "version": 1}"#.to_vec(),
            ),
            invalid.to_owned(),
        ),
        (write("deep.json", vec![b'['; 10_000]), invalid.to_owned()),
        (
            write("big.json", vec![b' '; 2 * 1024 * 1024]),
            large.to_owned(),
        ),
        (
            dir.path().join("missing.json"),
            "No se pudo leer el archivo: ".to_owned(),
        ),
    ];
    let before = app.config().clone();
    for (path, expected) in cases {
        import_from(&mut app, &fake, &path);
        let (title, text) = warning(&app).unwrap_or_else(|| panic!("{path:?}"));
        assert_eq!(title, "Importar favoritos…");
        assert!(text.starts_with(&expected), "{text}");
        assert_eq!(app.config(), &before);
        let _ = app.update(Message::Prompt(PromptMsg::Cancel));
        assert!(app.dialog().is_none());
    }
}

#[test]
fn export_error_warns() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("file.txt");
    std::fs::write(&blocker, "x").unwrap();
    let (mut app, fake) = fake_app(dir.path(), sample_cfg());
    export_to(&mut app, &fake, &blocker.join("x.json"));
    let (title, text) = warning(&app).unwrap();
    assert_eq!(title, "Exportar favoritos…");
    assert!(!text.is_empty());
}

#[test]
fn cancelled_dialog_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), sample_cfg());
    let status = app.status().to_owned();
    let before = app.config().clone();
    for a in [Action::ExportFavorites, Action::ImportFavorites] {
        let task = run(&mut app, a);
        drive(&mut app, task);
        assert!(app.dialog().is_none());
        assert!(!app.files_open());
        assert_eq!(app.status(), status);
        assert_eq!(app.config(), &before);
    }
    assert_eq!(fake.files.calls().len(), 2);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn file_dialogs_never_run_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.json");
    let (mut app, fake) = fake_app(dir.path(), sample_cfg());
    *fake.files.save.lock().unwrap() = Some(path.clone());
    let task = run(&mut app, Action::ExportFavorites);
    assert!(fake.files.calls().is_empty());
    let arrived = run_task(task);
    let calls = fake.files.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].0, calls[0].1.as_str()),
        ("save", "Exportar favoritos…")
    );
    assert_ne!(calls[0].2, std::thread::current().id());
    let [m] = <[Message; 1]>::try_from(arrived).unwrap();
    let write = app.update(m);
    assert!(!path.exists());
    drive(&mut app, write);
    assert!(path.exists());
    // Importar: ni el selector ni la lectura en `update`.
    *fake.files.open.lock().unwrap() = Some(path.clone());
    let task = run(&mut app, Action::ImportFavorites);
    assert_eq!(fake.files.calls().len(), 1);
    let arrived = run_task(task);
    let calls = fake.files.calls();
    assert_eq!(
        (calls[1].0, calls[1].1.as_str()),
        ("open", "Importar favoritos…")
    );
    assert_ne!(calls[1].2, std::thread::current().id());
    assert!(matches!(
        arrived.as_slice(),
        [Message::Files(FilesMsg::ImportFrom(Some(_)))]
    ));
    let [m] = <[Message; 1]>::try_from(arrived).unwrap();
    let read = app.update(m);
    assert!(matches!(
        run_task(read).as_slice(),
        [Message::Files(FilesMsg::Imported { result: Ok(_) })]
    ));
}

#[test]
fn export_import_disabled_while_picker_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake) = fake_app(dir.path(), sample_cfg());
    let task = run(&mut app, Action::ExportFavorites);
    assert!(app.files_open());
    assert!(!enabled(&app, Action::ExportFavorites));
    assert!(!enabled(&app, Action::ImportFavorites));
    assert!(enabled(&app, Action::CopyFavorites));
    // Un segundo pedido mientras tanto no abre otro.
    run_task(run(&mut app, Action::ImportFavorites));
    drive(&mut app, task);
    assert_eq!(fake.files.calls().len(), 1);
    assert!(!app.files_open());
    assert!(enabled(&app, Action::ExportFavorites));
    assert!(enabled(&app, Action::ImportFavorites));
}

#[test]
fn import_in_favorites_view_requeries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.json");
    std::fs::write(
        &path,
        serde_json::to_string(&export_favorites(&sample_cfg())).unwrap(),
    )
    .unwrap();
    let (mut app, fake) = fake_app(dir.path(), Config::default());
    let mut known = ServerInfo::new(addr0(), addr0());
    known.name = "Clan Nox".into();
    fake.answer_for(&addr0(), Some(known));
    let show = app.update(Message::Direct(DirectMsg::Show(DirectView::Favorites)));
    drive(&mut app, show);
    assert!(fake.queries().is_empty());
    import_from(&mut app, &fake, &path);
    assert_eq!(fake.queries(), [("cod2", addr0())]);
    assert_eq!(app.servers().len(), 1);
    let rows = visible_rows(&app.table, palette("dark"), WidgetStyle::Flat);
    let tip = rows[0]
        .cells
        .iter()
        .find(|(c, _)| *c == Column::Name)
        .and_then(|(_, cell)| cell.tooltip.clone())
        .unwrap();
    assert!(tip.contains("el del clan"), "{tip}");
}

#[test]
fn copy_favorites_as_text_round_trips_through_paste() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    let task = app.copy_favorites_as_text();
    assert!(clipboard_writes(task).is_empty());
    assert_eq!(
        app.status(),
        "No hay favoritos de Call of Duty 2 para copiar"
    );
    add_favorite(app.config_mut(), "cod2", &addr0());
    add_favorite(app.config_mut(), "cod2", &other());
    let mut known = mk("9.9.9.9", "Clan Nox", 40, 3, "1.3");
    known.connect_address = addr0();
    app.table.model.add_or_update(known);
    let task = run(&mut app, Action::CopyFavorites);
    let text = clipboard_writes(task).pop().unwrap();
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        [
            "Call of Duty 2 · JASE",
            "9.9.9.9:28960  Clan Nox",
            "5.6.7.8:28960"
        ]
    );
    assert!(text.ends_with('\n'));
    assert_eq!(app.status(), "2 favoritos copiados como texto");
    assert_eq!(find_addresses(&text), [addr0(), other()]);
    // Pegado con Ctrl+V: el diálogo trae las dos.
    let _ = app.update(Message::AddServer(AddMsg::Pasted(Some(text))));
    assert_eq!(app.add_form().unwrap().addresses(), [addr0(), other()]);
}

#[test]
fn group_combo_filters_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    add_favorite(&mut cfg, "cod2", &addr0());
    add_favorite(&mut cfg, "cod2", &other());
    set_group(&mut cfg, "cod2", &addr0().to_string(), "Clan");
    let (mut app, _) = fake_app(dir.path(), cfg);
    let show = app.update(Message::Direct(DirectMsg::Show(DirectView::Favorites)));
    drive(&mut app, show);
    let pick = app.update(Message::Direct(DirectMsg::Group(Some("Clan".into()))));
    drive(&mut app, pick);
    let text = clipboard_writes(app.copy_favorites_as_text())
        .pop()
        .unwrap();
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        ["Call of Duty 2 · Clan · JASE", "9.9.9.9:28960"]
    );
    assert_eq!(app.status(), "1 favorito copiado como texto");
}

#[test]
fn favorites_panel_context_menu_opens_as_favorite() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    assert!(sidebar::direct_menu(DirectView::Recent).is_none());
    let open = sidebar::direct_menu(DirectView::Favorites).unwrap();
    let focus = app.focus();
    let _ = app.update(open);
    assert_eq!(app.open_menu(), Some(Menu::Favorites));
    assert_eq!(app.direct_view(), None);
    assert_eq!(app.focus(), focus);
    let es = entries(&app, Menu::Favorites);
    assert_eq!(
        es.iter().map(label).collect::<Vec<_>>(),
        ["Agregar favorito…", "Copiar favoritos como texto"]
    );
    let Entry::Item { on: Some(add), .. } = &es[0] else {
        panic!("«Agregar favorito…» apagado");
    };
    let task = app.update(add.clone());
    assert_eq!(app.open_menu(), None);
    assert_eq!(clipboard_reads(task), 1);
    let task = app.update(add.clone());
    let opened = run_task_with_clipboard(task, Some("nada"));
    assert!(
        matches!(
            opened.as_slice(),
            [Message::AddServer(AddMsg::Open { favorite: true, text })] if text.is_empty()
        ),
        "{opened:?}"
    );
}

#[test]
fn copy_link_action() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), Config::default());
    assert!(!enabled(&app, Action::CopyLink));
    let mut info = mk("9.9.9.9", "Clan Nox", 40, 3, "1.3");
    info.connect_address = ServerAddress::new("9.9.9.9", 28961);
    app.table.model.add_or_update(info);
    let _ = app.update(Message::Table(table::Msg::RowClicked(addr0())));
    assert!(enabled(&app, Action::CopyLink));
    let task = run(&mut app, Action::CopyLink);
    assert_eq!(
        clipboard_writes(task),
        ["jase://connect/cod2/9.9.9.9:28960"]
    );
    assert_eq!(app.status(), "Enlace copiado");
}

#[test]
fn favorites_menu_builds_for_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fake_app(dir.path(), sample_cfg());
    let _ = app.update(sidebar::direct_menu(DirectView::Favorites).unwrap());
    for (id, _, _) in VARIANTS.iter() {
        app.theme_id = id;
        let _ = app.view();
    }
}
