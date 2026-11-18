use iced::Color;
use jase::colored::{cell_text_color, color_runs};
use jase::theme::{hex, palette};
use jase_core::protocols::quake3::strip_colors;
use proptest::prelude::*;

fn texts(runs: &[(String, Option<Color>)]) -> Vec<&str> {
    runs.iter().map(|(t, _)| t.as_str()).collect()
}

fn joined(raw: &str) -> String {
    color_runs(raw, palette("dark"))
        .into_iter()
        .map(|(t, _)| t)
        .collect()
}

#[test]
fn runs_split_by_color_code() {
    let p = palette("dark");
    let runs = color_runs("^1Ti^2tan", p);
    assert_eq!(texts(&runs), ["Ti", "tan"]);
    assert_eq!(runs[0].1, p.q3[1]);
    assert_eq!(runs[1].1, p.q3[2]);
    assert!(runs[0].1.is_some());
}

#[test]
fn plain_text_has_default_color() {
    assert_eq!(
        color_runs("Plain", palette("dark")),
        [("Plain".to_owned(), None)]
    );
}

#[test]
fn white_uses_default_text_color() {
    for id in ["light", "dark"] {
        assert_eq!(
            color_runs("^7Hola", palette(id)),
            [("Hola".to_owned(), None)]
        );
    }
}

#[test]
fn black_is_readable_on_dark() {
    let runs = color_runs("^0Negro", palette("dark"));
    assert_eq!(runs.len(), 1);
    let c = runs[0].1.expect("^0 tiene color en dark");
    // Luminosidad HSL de Qt (0-255): (máx + mín) / 2.
    let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    assert!((max + min) / 2.0 * 255.0 > 60.0);
}

#[test]
fn text_always_matches_strip_colors() {
    for raw in [
        "^1Jimmy^7 TDM",
        "^aX^",
        "a^^1b",
        "^\x011Nombre",
        "^9^8^0z",
        "",
    ] {
        assert_eq!(joined(raw), strip_colors(raw), "{raw:?}");
    }
}

#[test]
fn letter_codes_are_removed_without_color() {
    assert_eq!(color_runs("^aX", palette("dark")), [("X".to_owned(), None)]);
}

#[test]
fn double_caret_codes_render_clean() {
    let p = palette("dark");
    assert_eq!(color_runs("^^7Sugus", p), [("Sugus".to_owned(), None)]);
    let runs = color_runs("^^00B E N D E R", p);
    assert_eq!(texts(&runs), ["B E N D E R"]);
    assert_eq!(runs[0].1, color_runs("^0x", p)[0].1);
    for raw in ["^^7Sugus", "^^00B E N D E R", "^^71337", "x^^1y"] {
        assert_eq!(joined(raw), strip_colors(raw), "{raw:?}");
    }
}

#[test]
fn cell_text_color_stays_legible_on_selection() {
    let p = palette("classic");
    assert_eq!(hex(p.text), "#000000");
    assert_eq!(hex(p.selection), "#000080");
    assert_eq!(hex(p.selection_text), "#ffffff");
    let green = Color::from_rgb8(0x00, 0x64, 0x00);
    let yellow = Color::from_rgb8(0xff, 0xd2, 0x3f);
    assert_eq!(cell_text_color(Some(green), false, p), green);
    assert_eq!(cell_text_color(None, false, p), p.text);
    assert_eq!(cell_text_color(Some(green), true, p), p.selection_text); // verde sobre azul marino
    assert_eq!(cell_text_color(None, true, p), p.selection_text);
    assert_eq!(cell_text_color(Some(yellow), true, p), yellow);
}

proptest! {
    #[test]
    fn color_runs_never_panic(raw in "[\\^0-9a-zA-Z \\x00-\\x1f\\x7fñé漢😀]{0,40}") {
        let runs = color_runs(&raw, palette("dark"));
        let text: String = runs.iter().map(|(t, _)| t.as_str()).collect();
        prop_assert_eq!(text, strip_colors(&raw));
        prop_assert!(runs.iter().all(|(t, _)| !t.is_empty()));
        prop_assert!(runs.windows(2).all(|w| w[0].1 != w[1].1));
    }
}
