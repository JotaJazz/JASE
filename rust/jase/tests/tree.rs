//! Árbol de juegos (`test_game_tree.py`): carpetas Mods/Mapas/Países, marcas y expansión
//! sobre `GameTree::rows()`, sin widgets.

mod support;

use jase::exe_icon::{bgra_to_rgba, platform_icon};
use jase::theme::{VARIANTS, hex, palette, widget_style};
use jase::tree::{
    Event, GameTree, Item, Kind, Msg, Nav, Node, TreeRow, branch_svg, row_height, update, view,
};
use jase_core::browse::{BASE_MOD, Breakdown};
use support::lang::with_language;

fn pairs(v: &[(&str, usize)]) -> Vec<(String, usize)> {
    v.iter().map(|(k, n)| ((*k).to_owned(), *n)).collect()
}

fn bd(mods: &[(&str, usize)], maps: &[(&str, usize)], other: usize) -> Breakdown {
    Breakdown {
        mods: pairs(mods),
        maps: pairs(maps),
        other_maps: other,
        countries: Vec::new(),
    }
}

const MAPS: &[(&str, usize)] = &[("dm1", 5), ("dm2", 3)];

fn tree() -> GameTree {
    let mut t = GameTree::default();
    t.set_games(vec!["a", "b"]);
    t
}

/// Abre el juego y sus carpetas: `rows()` solo trae lo visible.
fn open(t: &mut GameTree, id: &'static str) {
    t.set_expanded(id, Node::Game, true);
    for k in [Kind::Mods, Kind::Maps, Kind::Countries] {
        t.set_expanded(id, Node::Folder(k), true);
    }
}

fn game_row(t: &GameTree, id: &str) -> TreeRow {
    t.rows()
        .into_iter()
        .find(|r| r.game == id && r.item == Item::Game)
        .unwrap()
}

fn folder(t: &GameTree, id: &str, kind: Kind) -> Option<TreeRow> {
    t.rows()
        .into_iter()
        .find(|r| r.game == id && r.item == Item::Folder(kind))
}

/// Hijos visibles de la carpeta (hojas y «otros…»).
fn children(t: &GameTree, id: &str, kind: Kind) -> Vec<TreeRow> {
    let rows = t.rows();
    let Some(at) = rows
        .iter()
        .position(|r| r.game == id && r.item == Item::Folder(kind))
    else {
        return Vec::new();
    };
    rows[at + 1..]
        .iter()
        .take_while(|r| r.depth == 2)
        .cloned()
        .collect()
}

fn leaf(t: &GameTree, id: &str, kind: Kind, value: &str) -> Option<TreeRow> {
    children(t, id, kind)
        .into_iter()
        .find(|r| r.item == Item::Leaf(kind, value.to_owned()))
}

fn mark(t: &mut GameTree, id: &'static str, kind: Kind, value: &str) -> Option<Event> {
    update(t, Msg::Toggle(id, kind, value.to_owned()))
}

fn set(v: &[&str]) -> std::collections::HashSet<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn set_games_order_and_labels() {
    let mut t = tree();
    t.set_label("a", "Juego A", "Juego A completo");
    assert_eq!(t.game_ids(), ["a", "b"]);
    assert_eq!(t.rows().len(), 2);
    let a = game_row(&t, "a");
    assert_eq!(a.text, "Juego A");
    assert_eq!(a.tooltip, "Juego A completo");
    assert!(!a.expandable);
}

#[test]
fn breakdown_creates_checkable_leaves() {
    with_language("es", || {
        let mut t = tree();
        open(&mut t, "a");
        t.set_breakdown("a", Some(bd(&[("rpg", 2), (BASE_MOD, 6)], MAPS, 0)), true);
        let mods = children(&t, "a", Kind::Mods);
        let maps = children(&t, "a", Kind::Maps);
        assert_eq!((mods.len(), maps.len()), (2, 2));
        assert_eq!(mods[1].text, "(base) · 6");
        assert_eq!(maps[0].text, "dm1 · 5");
        assert_eq!(maps[0].item, Item::Leaf(Kind::Maps, "dm1".into()));
        assert_eq!(maps[0].checked, Some(false));
        assert_eq!(folder(&t, "a", Kind::Maps).unwrap().text, "Mapas");
        assert!(game_row(&t, "a").expandable);
        t.set_breakdown("a", None, true);
        assert_eq!(t.rows().len(), 2);
        assert!(!game_row(&t, "a").expandable);
    });
}

#[test]
fn mods_folder_hidden_without_mods() {
    let mut t = tree();
    open(&mut t, "a");
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    assert!(folder(&t, "a", Kind::Mods).is_none());
    assert!(folder(&t, "a", Kind::Maps).is_some());
}

#[test]
fn other_maps_leaf_not_checkable() {
    with_language("es", || {
        let mut t = tree();
        open(&mut t, "a");
        t.set_breakdown("a", Some(bd(&[], MAPS, 7)), true);
        let maps = children(&t, "a", Kind::Maps);
        let other = maps.last().unwrap();
        assert_eq!(other.item, Item::Other);
        assert_eq!(other.text, "otros… · 7");
        assert_eq!(other.checked, None);
        t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
        assert_eq!(children(&t, "a", Kind::Maps).len(), 2);
    });
}

#[test]
fn check_leaf_emits_marks_changed() {
    let mut t = tree();
    t.set_current(Some("a"));
    open(&mut t, "a");
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    assert_eq!(
        mark(&mut t, "a", Kind::Maps, "dm2"),
        Some(Event::MarksChanged("a"))
    );
    assert_eq!(t.marks("a").maps, set(&["dm2"]));
    assert!(t.marks("a").mods.is_empty() && t.marks("a").countries.is_empty());
    assert_eq!(
        leaf(&t, "a", Kind::Maps, "dm2").unwrap().checked,
        Some(true)
    );
    assert_eq!(
        mark(&mut t, "a", Kind::Maps, "dm2"),
        Some(Event::MarksChanged("a"))
    );
    assert!(t.marks("a").is_empty());
    // Solo cambian las cuentas: nada que avisar.
    assert!(!t.set_breakdown("a", Some(bd(&[], &[("dm1", 9), ("dm2", 3)], 0)), true));
}

#[test]
fn update_breakdown_keeps_marks_and_expansion() {
    let mut t = tree();
    t.set_current(Some("a"));
    t.set_breakdown("a", Some(bd(&[("rpg", 2)], MAPS, 0)), true);
    t.set_expanded("a", Node::Game, true);
    t.set_expanded("a", Node::Folder(Kind::Maps), true);
    mark(&mut t, "a", Kind::Maps, "dm1");
    t.set_breakdown(
        "a",
        Some(bd(&[("rpg", 4)], &[("dm1", 8), ("dm2", 1)], 0)),
        true,
    );
    let dm1 = leaf(&t, "a", Kind::Maps, "dm1").unwrap();
    assert_eq!(dm1.text, "dm1 · 8");
    assert_eq!(dm1.checked, Some(true));
    assert!(folder(&t, "a", Kind::Maps).unwrap().expanded);
    assert!(game_row(&t, "a").expanded);
    assert!(!folder(&t, "a", Kind::Mods).unwrap().expanded);
    // Cambian las claves: conserva expansión y marcas.
    t.set_breakdown(
        "a",
        Some(bd(&[("rpg", 4)], &[("dm1", 8), ("dm3", 1)], 0)),
        true,
    );
    assert!(folder(&t, "a", Kind::Maps).unwrap().expanded);
    assert_eq!(
        leaf(&t, "a", Kind::Maps, "dm1").unwrap().checked,
        Some(true)
    );
}

#[test]
fn vanished_leaf_drops_mark() {
    let mut t = tree();
    t.set_current(Some("a"));
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    mark(&mut t, "a", Kind::Maps, "dm2");
    assert!(t.set_breakdown("a", Some(bd(&[], &[("dm1", 5)], 0)), true));
    assert!(t.marks("a").is_empty());
}

#[test]
fn check_leaf_of_other_game_activates_it_first() {
    let mut t = tree();
    t.set_current(Some("a"));
    t.set_breakdown("b", Some(bd(&[], MAPS, 0)), true);
    // El juego se activa con la marca ya puesta: al elegirlo, la app aplica sus marcas.
    assert_eq!(
        mark(&mut t, "b", Kind::Maps, "dm1"),
        Some(Event::GameActivated("b"))
    );
    assert_eq!(t.marks("b").maps, set(&["dm1"]));
    assert_eq!(t.current(), Some("b"));
}

#[test]
fn set_games_reorder_keeps_marks() {
    let mut t = tree();
    t.set_label("a", "A", "tip");
    t.set_current(Some("a"));
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    open(&mut t, "a");
    mark(&mut t, "a", Kind::Maps, "dm1");
    t.set_games(vec!["b", "a"]);
    assert_eq!(t.game_ids(), ["b", "a"]);
    let a = game_row(&t, "a");
    assert_eq!(a.text, "A");
    assert!(a.expanded);
    assert_eq!(t.marks("a").maps, set(&["dm1"]));
    assert_eq!(
        leaf(&t, "a", Kind::Maps, "dm1").unwrap().checked,
        Some(true)
    );
}

#[test]
fn breakdown_none_keeps_marks() {
    let mut t = tree();
    t.set_current(Some("a"));
    open(&mut t, "a");
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    mark(&mut t, "a", Kind::Maps, "dm1");
    assert!(!t.set_breakdown("a", None, true));
    assert_eq!(t.marks("a").maps, set(&["dm1"]));
    assert!(folder(&t, "a", Kind::Maps).is_none());
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    assert_eq!(
        leaf(&t, "a", Kind::Maps, "dm1").unwrap().checked,
        Some(true)
    );
}

#[test]
fn no_prune_keeps_missing_marked_leaf_with_zero() {
    let mut t = tree();
    t.set_current(Some("a"));
    open(&mut t, "a");
    t.set_breakdown("a", Some(bd(&[("rpg", 2)], MAPS, 0)), true);
    mark(&mut t, "a", Kind::Mods, "rpg");
    mark(&mut t, "a", Kind::Maps, "dm2");
    assert!(!t.set_breakdown("a", Some(bd(&[], &[("dm1", 5)], 0)), false));
    assert_eq!(t.marks("a").mods, set(&["rpg"]));
    assert_eq!(t.marks("a").maps, set(&["dm2"]));
    let rpg = leaf(&t, "a", Kind::Mods, "rpg").unwrap();
    assert_eq!((rpg.text.as_str(), rpg.checked), ("rpg · 0", Some(true)));
    let maps = children(&t, "a", Kind::Maps);
    assert_eq!(
        (maps[1].text.as_str(), maps[1].checked),
        ("dm2 · 0", Some(true))
    );
    // Con poda se descartan.
    assert!(t.set_breakdown("a", Some(bd(&[], &[("dm1", 5)], 0)), true));
    assert!(t.marks("a").is_empty());
}

#[test]
fn label_and_icon_keep_state_and_emit_nothing() {
    let mut t = tree();
    t.set_current(Some("a"));
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    open(&mut t, "a");
    mark(&mut t, "a", Kind::Maps, "dm1");
    t.set_label("a", "Nuevo", "tip");
    assert!(game_row(&t, "a").expanded);
    assert_eq!(t.marks("a").maps, set(&["dm1"]));
    assert_eq!(
        leaf(&t, "a", Kind::Maps, "dm1").unwrap().checked,
        Some(true)
    );
}

#[test]
fn unchanged_breakdown_emits_nothing() {
    let mut t = tree();
    t.set_current(Some("a"));
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    mark(&mut t, "a", Kind::Maps, "dm1");
    assert!(!t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true));
}

#[test]
fn clear_marks_unchecks_and_emits_once() {
    let mut t = tree();
    t.set_current(Some("a"));
    open(&mut t, "a");
    t.set_breakdown("a", Some(bd(&[("rpg", 2)], MAPS, 0)), true);
    mark(&mut t, "a", Kind::Mods, "rpg");
    mark(&mut t, "a", Kind::Maps, "dm1");
    t.clear_marks("a");
    assert!(t.marks("a").is_empty());
    for kind in [Kind::Mods, Kind::Maps] {
        assert!(
            children(&t, "a", kind)
                .iter()
                .all(|r| r.checked == Some(false))
        );
    }
}

#[test]
fn set_current_game_does_not_activate() {
    // `set_current` no devuelve evento: solo mueve la selección.
    let mut t = tree();
    t.set_current(Some("b"));
    assert!(game_row(&t, "b").selected && !game_row(&t, "a").selected);
    t.set_current(None);
    assert!(t.rows().iter().all(|r| !r.selected));
}

#[test]
fn click_and_activate_game_node_emit() {
    let mut t = tree();
    assert_eq!(
        update(&mut t, Msg::Activate("a")),
        Some(Event::GameActivated("a"))
    );
    assert_eq!(
        update(&mut t, Msg::Activate("b")),
        Some(Event::GameActivated("b"))
    );
    assert!(game_row(&t, "b").selected);
}

#[test]
fn arrow_keys_activate_game_but_programmatic_does_not() {
    let mut t = tree();
    t.set_current(Some("a"));
    assert_eq!(
        update(&mut t, Msg::Nav(Nav::Down)),
        Some(Event::GameActivated("b"))
    );
    assert!(game_row(&t, "b").selected);
    assert_eq!(
        update(&mut t, Msg::Nav(Nav::Up)),
        Some(Event::GameActivated("a"))
    );
    t.set_games(vec!["b", "a"]); // reordenar tampoco activa (no devuelve evento)
    assert_eq!(t.current(), Some("a"));
}

#[test]
fn arrows_open_folders_and_space_marks_leaf() {
    let mut t = tree();
    t.set_current(Some("a"));
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    assert_eq!(update(&mut t, Msg::Nav(Nav::Right)), None); // abre «a»
    assert!(game_row(&t, "a").expanded);
    update(&mut t, Msg::Nav(Nav::Down)); // carpeta Mapas
    assert_eq!(update(&mut t, Msg::Nav(Nav::Right)), None);
    update(&mut t, Msg::Nav(Nav::Down)); // dm1
    assert_eq!(
        update(&mut t, Msg::Nav(Nav::Space)),
        Some(Event::MarksChanged("a"))
    );
    assert_eq!(t.marks("a").maps, set(&["dm1"]));
    update(&mut t, Msg::Nav(Nav::Left)); // a la carpeta
    update(&mut t, Msg::Nav(Nav::Left)); // la cierra
    assert!(!folder(&t, "a", Kind::Maps).unwrap().expanded);
}

#[test]
fn classic_mode_draws_dotted_branches() {
    let mut t = tree();
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    t.set_expanded("a", Node::Game, true);
    let rows = t.rows();
    let (a, maps, b) = (&rows[0], &rows[1], &rows[2]);
    assert_eq!((a.item.clone(), maps.depth), (Item::Game, 1));
    for id in ["classic", "xp"] {
        let (p, s) = (palette(id), widget_style(id));
        let open = branch_svg(a, p, s).unwrap();
        assert!(open.contains(&hex(p.tree_line)) && open.contains(&hex(p.tree_box)));
        // Cerrado: + (con la barra vertical); abierto: −.
        let closed = branch_svg(maps, p, s).unwrap();
        assert_ne!(open, closed);
        assert!(closed.contains(&hex(p.tree_box)));
        // Sin hijos: solo puntos, sin cuadro (`tree_box` puede ser el gris de los puntos).
        let plain = branch_svg(b, p, s).unwrap();
        assert!(plain.contains(&hex(p.tree_line)) && !plain.contains("width=\"9\""));
    }
    let (p, s) = (palette("dark"), widget_style("dark"));
    let arrow = branch_svg(a, p, s).unwrap();
    assert!(arrow.contains(&hex(p.arrow)) && !arrow.contains(&hex(p.tree_line)));
    assert_ne!(arrow, branch_svg(maps, p, s).unwrap()); // ▾ abierto, ▸ cerrado
    assert_eq!(branch_svg(b, p, s), None); // plano y sin hijos: nada
}

/// Alto de cada fila como `visualItemRect` en 1.3.0: el juego con su insignia, la carpeta, la
/// hoja con casilla y la de país con bandera.
#[test]
fn row_heights_match_qt() {
    let mut t = tree();
    open(&mut t, "a");
    let mut b = bd(&[], &[("dm1", 5)], 0);
    b.countries = pairs(&[("cl", 3)]);
    t.set_breakdown("a", Some(b), true);
    let rows = t.rows();
    let game = game_row(&t, "a");
    let maps = folder(&t, "a", Kind::Maps).unwrap();
    let leaf = rows
        .iter()
        .find(|r| r.item == Item::Leaf(Kind::Maps, "dm1".into()))
        .unwrap();
    let country = &children(&t, "a", Kind::Countries)[0];
    for (id, want) in [
        ("dark", [30, 26, 26, 28]),
        ("classic", [22, 17, 18, 20]),
        ("xp", [22, 17, 18, 20]),
        ("goldsrc", [22, 17, 18, 20]),
        ("terminal_green", [20, 16, 16, 18]),
    ] {
        let s = widget_style(id);
        let got = [&game, &maps, leaf, country].map(|r| row_height(r, s));
        assert_eq!(got, want, "{id}");
    }
}

#[test]
fn countries_folder_with_flags_and_marks() {
    let mut t = tree();
    t.set_current(Some("a"));
    open(&mut t, "a");
    let mut b = bd(&[], &[("dm1", 5)], 0);
    b.countries = pairs(&[("cl", 3), ("ar", 1)]);
    t.set_breakdown("a", Some(b), true);
    let leaves = children(&t, "a", Kind::Countries);
    assert_eq!(leaves.len(), 2);
    assert_eq!(leaves[0].item, Item::Leaf(Kind::Countries, "cl".into()));
    assert!(leaves[0].text.ends_with(" · 3"));
    assert_eq!(leaves[0].flag.as_deref(), Some("cl"));
    mark(&mut t, "a", Kind::Countries, "cl");
    assert_eq!(t.marks("a").countries, set(&["cl"]));
    // Sin países (base aún no cargada) y sin poda: la marca no se pierde.
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), false);
    assert_eq!(t.marks("a").countries, set(&["cl"]));
    t.clear_marks("a");
    assert!(t.marks("a").is_empty());
}

#[test]
fn countries_folder_hidden_without_countries() {
    let mut t = tree();
    open(&mut t, "a");
    t.set_breakdown("a", Some(bd(&[], MAPS, 0)), true);
    assert!(folder(&t, "a", Kind::Countries).is_none());
}

#[test]
fn tree_view_builds_for_every_theme() {
    let mut t = GameTree::default();
    t.set_games(vec!["cod2", "ut99", "zz"]);
    t.set_current(Some("cod2"));
    let mut b = bd(&[("pam", 2), (BASE_MOD, 1)], MAPS, 4);
    b.countries = pairs(&[("cl", 2), ("xx", 1), ("??", 1)]);
    t.set_breakdown("cod2", Some(b), true);
    open(&mut t, "cod2");
    mark(&mut t, "cod2", Kind::Maps, "dm1");
    mark(&mut t, "cod2", Kind::Countries, "cl");
    for (id, _, _) in VARIANTS.iter() {
        for focused in [true, false] {
            let _ = view(&t, id, focused);
        }
        let (p, s) = (palette(id), widget_style(id));
        for r in t.rows() {
            let _ = branch_svg(&r, p, s);
        }
    }
}

#[test]
fn tree_view_builds_with_exe_icons() {
    let mut t = GameTree::default();
    t.set_games(vec!["q3", "cod2"]);
    t.set_current(Some("q3"));
    t.exe_icons.insert("q3", support::exe_icon());
    for (id, _, _) in VARIANTS.iter() {
        for focused in [true, false] {
            let _ = view(&t, id, focused);
        }
    }
}

#[test]
fn bgra_to_rgba_swaps_channels_and_uses_mask() {
    // Con alfa propio: solo se cambia el orden de los canales.
    let bgra = [1, 2, 3, 255, 4, 5, 6, 128];
    assert_eq!(bgra_to_rgba(&bgra, None, 2), [3, 2, 1, 255, 6, 5, 4, 128]);
    // Todo el alfa en 0: la máscara de 1 bit (filas de 4 bytes) dice qué es opaco.
    let bgra = [1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9, 0, 10, 11, 12, 0];
    let mask = [0b0100_0000, 0, 0, 0, 0b1000_0000, 0, 0, 0];
    let rgba = bgra_to_rgba(&bgra, Some(&mask), 2);
    let alpha: Vec<u8> = rgba.chunks(4).map(|p| p[3]).collect();
    assert_eq!(alpha, [255, 0, 0, 255]);
    assert_eq!(&rgba[..3], [3, 2, 1]);
    // Sin máscara, opaco; una máscara corta no hace pánico.
    assert!(bgra_to_rgba(&bgra, None, 2).chunks(4).all(|p| p[3] == 255));
    assert_eq!(bgra_to_rgba(&bgra, Some(&[]), 2).len(), 16);
    assert_eq!(bgra_to_rgba(&bgra, Some(&mask), 0).len(), 16);
}

#[cfg(windows)]
#[test]
fn explorer_has_an_icon() {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let exe = std::path::Path::new(&root).join("explorer.exe");
    assert!(platform_icon(&exe.display().to_string()).is_some());
}

#[cfg(not(windows))]
#[test]
fn no_platform_icon_off_windows() {
    assert!(platform_icon("/usr/bin/env").is_none());
}
