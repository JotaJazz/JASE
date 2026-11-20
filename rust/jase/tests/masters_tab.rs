//! Pestaña «Masters» de Ajustes sin ventana (como `test_masters_tab.py`).
mod support;

use std::collections::BTreeSet;

use indexmap::IndexMap;
use jase::masters_tab::{AddResult, MastersTab, Outcome, parse_master_text};
use jase_core::masters::{CustomMasters, Master, OfficialList, effective_masters, master_key};
use support::lang::with_language;

fn m(host: &str, port: u16) -> Master {
    (host.to_owned(), port)
}

fn official() -> OfficialList {
    OfficialList {
        schema: 1,
        updated: "2026-11-01".to_owned(),
        games: IndexMap::from([
            (
                "cod2".to_owned(),
                vec![m("a.example.com", 20710), m("b.example.com", 20710)],
            ),
            ("ut99".to_owned(), vec![m("u.example.com", 28900)]),
        ]),
    }
}

fn mine() -> CustomMasters {
    CustomMasters {
        added: IndexMap::from([("cod2".to_owned(), vec![m("mine.example.com", 1234)])]),
        ..CustomMasters::default()
    }
}

fn tab(custom: CustomMasters, probe: bool) -> MastersTab {
    let mut t = MastersTab::new(official(), custom, probe, false);
    t.select_game("cod2");
    t
}

fn probes(t: &MastersTab) -> Vec<String> {
    t.rows().into_iter().map(|r| r.probe).collect()
}

#[test]
fn masters_tab_lists_official_and_custom() {
    with_language("es", || {
        let mut t = tab(mine(), false);
        let rows = t.rows();
        let names: Vec<_> = rows
            .iter()
            .map(|r| format!("{}:{}", r.master.0, r.master.1))
            .collect();
        assert_eq!(
            names,
            [
                "a.example.com:20710",
                "b.example.com:20710",
                "mine.example.com:1234"
            ]
        );
        assert_eq!(
            rows.iter().map(|r| r.official).collect::<Vec<_>>(),
            [true, true, false]
        );
        assert!(rows[0].active);
        assert_eq!(t.official_label(), "Lista oficial: 2026-11-01");
        t.select_game("ut99");
        assert_eq!(t.rows().len(), 1);
    });
}

#[test]
fn disable_official_and_save() {
    let input = CustomMasters::default();
    let mut t = tab(input.clone(), false);
    t.toggle(1);
    assert_eq!(
        t.custom.disabled,
        IndexMap::from([(
            "cod2".to_owned(),
            BTreeSet::from([master_key(&m("b.example.com", 20710))])
        )])
    );
    assert!(!t.rows()[1].active);
    assert_eq!(input, CustomMasters::default()); // edita una copia
    assert_eq!(
        effective_masters("cod2", &official(), &t.custom),
        [m("a.example.com", 20710)]
    );
    t.toggle(1);
    assert!(t.custom.disabled.is_empty());
}

#[test]
fn add_custom_validates_and_rejects_duplicate() {
    let mut t = tab(CustomMasters::default(), false);
    for bad in [
        "",
        "nohost",
        "host:0",
        "host:70000",
        "ho st:80",
        "host:abc",
        ":80",
    ] {
        assert_eq!(t.add(bad), AddResult::Invalid, "{bad}");
        assert_eq!(parse_master_text(bad), None, "{bad}");
    }
    assert_eq!(t.add("A.Example.com:20710"), AddResult::Duplicate); // oficial, sin mayúsculas
    assert_eq!(t.add(" new.example.com:27950 "), AddResult::Ok);
    assert_eq!(t.add("new.example.com:27950"), AddResult::Duplicate);
    assert_eq!(
        t.custom.added,
        IndexMap::from([("cod2".to_owned(), vec![m("new.example.com", 27950)])])
    );
    assert!(!t.rows()[2].official);
    assert_eq!(t.selected, Some(2));
}

#[test]
fn remove_only_custom() {
    let mut t = tab(mine(), false);
    t.select(0);
    assert!(!t.can_remove());
    t.remove_selected();
    assert_eq!(t.rows().len(), 3);
    t.select(2);
    assert!(t.can_remove());
    t.remove_selected();
    assert_eq!(t.rows().len(), 2);
    assert!(t.custom.added.is_empty());
}

#[test]
fn restore_officials() {
    let custom = CustomMasters {
        disabled: IndexMap::from([
            (
                "cod2".to_owned(),
                BTreeSet::from([master_key(&m("a.example.com", 20710))]),
            ),
            (
                "ut99".to_owned(),
                BTreeSet::from([master_key(&m("u.example.com", 28900))]),
            ),
        ]),
        ..CustomMasters::default()
    };
    let mut t = tab(custom, false);
    assert!(!t.rows()[0].active);
    t.restore_officials();
    assert!(t.rows()[0].active);
    assert!(!t.custom.disabled.contains_key("cod2"));
    assert!(t.custom.disabled.contains_key("ut99")); // solo el juego elegido
}

#[test]
fn probe_shows_count() {
    with_language("es", || {
        let mut t = tab(CustomMasters::default(), true);
        t.select(1);
        let (serial, game, master) = t.start_probe().unwrap();
        assert_eq!((game, &master), ("cod2", &m("b.example.com", 20710)));
        assert!(t.on_probe(serial, game, &master, Some(3)));
        assert_eq!(t.rows()[1].probe, "✓ 3 servidores");
    });
}

#[test]
fn probe_singular_and_failure() {
    with_language("es", || {
        let mut t = tab(CustomMasters::default(), true);
        t.select(0);
        for (count, text) in [
            (Some(1), "✓ 1 servidor"),
            (None, "✗ no responde"),
            (Some(0), "✓ 0 servidores"),
        ] {
            let (serial, game, master) = t.start_probe().unwrap();
            assert!(t.on_probe(serial, game, &master, count));
            assert_eq!(t.rows()[0].probe, text);
        }
    });
}

#[test]
fn probe_button_disabled_while_probing_and_reenabled() {
    let mut t = tab(CustomMasters::default(), true);
    assert!(!t.can_probe()); // sin fila elegida
    t.select(0);
    assert!(t.can_probe());
    let (serial, game, master) = t.start_probe().unwrap();
    assert!(!t.can_probe());
    assert!(t.start_probe().is_none());
    assert!(t.on_probe(serial, game, &master, Some(2)));
    assert!(t.can_probe());
    t.start_probe().unwrap();
    assert!(!t.can_probe());
    t.select(1); // cancela la prueba
    assert!(t.can_probe());
    // Sin función de prueba, nunca.
    let mut off = tab(CustomMasters::default(), false);
    off.select(0);
    assert!(!off.can_probe() && off.start_probe().is_none());
}

#[test]
fn custom_row_has_no_checkbox() {
    let mut t = tab(mine(), false);
    t.toggle(2);
    assert!(t.custom.disabled.is_empty());
    assert!(t.rows()[2].active);
}

#[test]
fn date_only_update_refreshes_footer() {
    with_language("es", || {
        let mut t = tab(CustomMasters::default(), false);
        let newer = OfficialList {
            updated: "2026-12-05".to_owned(),
            ..official()
        };
        t.show_update_result(Outcome::Updated, Some(newer), "2030-01-01");
        assert_eq!(t.official_label(), "Lista oficial: 2026-12-05");
    });
}

#[test]
fn probe_result_discarded_after_selection_change() {
    with_language("es", || {
        for change_game in [false, true] {
            let mut t = tab(CustomMasters::default(), true);
            t.select(0);
            let (serial, game, master) = t.start_probe().unwrap();
            assert_eq!(t.rows()[0].probe, "Probando…");
            if change_game {
                t.select_game("ut99");
            } else {
                t.select(1);
            }
            assert!(!t.on_probe(serial, game, &master, Some(5)));
            t.select_game("cod2");
            assert!(probes(&t).iter().all(String::is_empty), "{change_game}");
        }
    });
}

#[test]
fn check_update_outcomes() {
    with_language("es", || {
        let mut t = MastersTab::new(official(), CustomMasters::default(), false, true);
        assert!(t.has_check());
        t.select_game("cod2");
        t.show_update_result(Outcome::Unchanged, None, "2030-01-01");
        assert_eq!(t.update_status, "La lista de masters está al día");
        t.show_update_result(Outcome::Failed, None, "2030-01-01");
        assert_eq!(t.update_status, "No se pudo descargar la lista de masters");
        let new = OfficialList {
            schema: 1,
            updated: "2026-12-01".to_owned(),
            games: IndexMap::from([("cod2".to_owned(), vec![m("n.example.com", 1)])]),
        };
        t.show_update_result(Outcome::Updated, Some(new.clone()), "2030-01-01");
        assert!(t.update_status.contains("2026-12-01"));
        assert_eq!(t.official_label(), "Lista oficial: 2026-12-01");
        assert_eq!(t.rows()[0].master, m("n.example.com", 1));
        let undated = OfficialList {
            updated: String::new(),
            ..new
        };
        t.show_update_result(Outcome::Updated, Some(undated), "2030-01-01");
        assert_eq!(t.update_status, "Lista de masters actualizada (2030-01-01)");
        assert_eq!(t.official_label(), "Lista oficial: —");
    });
}
