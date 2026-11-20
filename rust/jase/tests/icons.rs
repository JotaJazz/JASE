use std::path::PathBuf;

use iced::widget::svg::Handle;
use iced::{Color, Size};
use jase::icons::{
    Arrow, ICON_SIZE, TOOL_ICONS, WIN98_ICON_SIZE, app_icon, arrow_svg, render_rgba, sort_arrow,
    tool_icon, tool_icon_size, tool_icon_svg,
};
use jase::theme::{VARIANTS, WidgetStyle, hex, palette};
use jase_core::games::GAMES;

fn assets() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "src",
        "jase",
        "assets",
    ]
    .iter()
    .collect()
}

fn svg_names(dir: PathBuf) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".svg").map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

fn close(px: &[u8], want: Color) -> bool {
    let want = hex(want);
    let want = u32::from_str_radix(&want[1..], 16).unwrap();
    let rgb = [(want >> 16) as u8, (want >> 8) as u8, want as u8];
    px.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 4)
}

#[test]
fn tool_icon_renders_every_svg() {
    for name in TOOL_ICONS {
        for (id, _, _) in &VARIANTS {
            for disabled in [false, true] {
                let svg = tool_icon_svg(name, id, disabled).unwrap();
                let side = tool_icon_size(id) as u32;
                assert!(render_rgba(&svg, side, side).is_some(), "{name} {id}");
                assert!(tool_icon(name, id, disabled).is_some());
            }
        }
    }
}

#[test]
fn tool_icon_tints_with_theme_text_and_muted() {
    let p = palette("dark");
    for (disabled, want) in [(false, p.text), (true, p.muted)] {
        let svg = tool_icon_svg("stop", "dark", disabled).unwrap();
        let rgba = render_rgba(&svg, 20, 20).unwrap();
        // Trazo hueco: se toman los píxeles más opacos de toda la imagen.
        let opaque: Vec<&[u8]> = rgba.chunks(4).filter(|px| px[3] >= 250).collect();
        assert!(!opaque.is_empty());
        assert!(close(opaque[0], want), "{:?} vs {}", opaque[0], hex(want));
    }
}

#[test]
fn badge_icon_every_game() {
    for game in GAMES {
        assert_eq!(game.badge.len(), 2, "{}", game.id);
        for (id, p, _) in &VARIANTS {
            assert!(p.badge(game.id).is_some(), "{} {id}", game.id);
            let _ = jase::icons::badge::<()>(game, p, 16.0);
        }
    }
    assert!(palette("dark").badge("no_such_game").is_none());
}

#[test]
fn classic_uses_pixel_art_icons() {
    let names = svg_names(assets().join("icons"));
    let mut tool: Vec<String> = TOOL_ICONS.iter().map(|s| (*s).to_owned()).collect();
    tool.sort();
    assert_eq!(names, tool);
    for folder in ["win98", "xp"] {
        assert_eq!(
            svg_names(assets().join("icons").join(folder)),
            names,
            "{folder}"
        );
    }
    let want = std::fs::read(assets().join("icons").join("win98").join("stop.svg")).unwrap();
    assert_eq!(
        &*tool_icon_svg("stop", "classic", false).unwrap(),
        &want[..]
    );
    assert_eq!(tool_icon_size("classic"), WIN98_ICON_SIZE);
    assert_ne!(
        tool_icon_svg("stop", "classic", false),
        tool_icon_svg("stop", "xp", false)
    );
}

#[test]
fn xp_uses_luna_icons() {
    let want = std::fs::read(assets().join("icons").join("xp").join("stop.svg")).unwrap();
    assert_eq!(&*tool_icon_svg("stop", "xp", false).unwrap(), &want[..]);
    assert_eq!(tool_icon_size("xp"), ICON_SIZE);
    assert_ne!(
        tool_icon_svg("stop", "xp", false),
        tool_icon_svg("stop", "light", false)
    );
}

#[test]
fn app_icon_loads() {
    let svg = std::fs::read(assets().join("app_icon.svg")).unwrap();
    let rgba = render_rgba(&svg, 64, 64).unwrap();
    assert_eq!(rgba.len(), 64 * 64 * 4);
    assert!(rgba.chunks(4).any(|px| px[3] == 255));
    assert!(app_icon().is_some());
}

#[test]
fn arrow_svg_known_answers() {
    let c = Color::from_rgb8(0xa1, 0xb2, 0xc3);
    let svg = |w, h, d| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
             viewBox=\"0 0 {w} {h}\"><path d=\"{d}\" fill=\"#a1b2c3\"/></svg>"
        )
    };
    assert_eq!(arrow_svg(Arrow::Up, c), svg(9, 5, "M0 5L4.5 0L9 5Z"));
    assert_eq!(arrow_svg(Arrow::Down, c), svg(9, 5, "M0 0H9L4.5 5Z"));
    assert_eq!(arrow_svg(Arrow::Right, c), svg(5, 9, "M0 0L5 4.5L0 9Z"));
}

#[test]
fn sort_arrow_per_style() {
    let p = palette("dark");
    let file = |dir: &str, name: &str| {
        Handle::from_memory(std::fs::read(assets().join(dir).join(name)).unwrap())
    };
    let tinted = |d, c| Handle::from_memory(arrow_svg(d, c).into_bytes());
    let flat = Size::new(9.0, 5.0);
    let small = Size::new(7.0, 4.0);
    // Como la captura de 1.3.0: ascendente dibuja la flecha hacia arriba.
    let cases = [
        (WidgetStyle::Flat, true, tinted(Arrow::Up, p.accent), flat),
        (
            WidgetStyle::Flat,
            false,
            tinted(Arrow::Down, p.accent),
            flat,
        ),
        (
            WidgetStyle::Win98,
            true,
            file("win98", "arrow_up.svg"),
            small,
        ),
        (
            WidgetStyle::Win98,
            false,
            file("win98", "arrow_down.svg"),
            small,
        ),
        (WidgetStyle::Luna, true, file("luna", "sort_up.svg"), small),
        (
            WidgetStyle::Luna,
            false,
            file("luna", "sort_down.svg"),
            small,
        ),
        (WidgetStyle::Vgui, true, tinted(Arrow::Up, p.text), flat),
        (
            WidgetStyle::Terminal,
            false,
            tinted(Arrow::Down, p.text),
            flat,
        ),
    ];
    for (s, asc, handle, size) in cases {
        let (h, sz) = sort_arrow(s, p, asc);
        assert_eq!(h.id(), handle.id(), "{s:?} {asc}");
        assert_eq!(sz, size, "{s:?}");
    }
}

/// Apagado en 98 y Luna, el icono a color sale gris y aclarado hacia el fondo, como el
/// `QIcon::Disabled` que genera Qt (`generatedIconPixmap`); el encendido sigue a color.
#[test]
fn disabled_color_icons_are_grayed_like_qt() {
    let side = WIN98_ICON_SIZE as u32;
    let on = tool_icon_svg("stop", "classic", false).unwrap();
    let off = tool_icon_svg("stop", "classic", true).unwrap();
    let saturated = |px: &[u8]| {
        px.chunks(4).any(|c| {
            let (hi, lo) = (c[..3].iter().max().unwrap(), c[..3].iter().min().unwrap());
            c[3] > 0 && hi - lo > 8
        })
    };
    assert!(saturated(&render_rgba(&on, side, side).unwrap()));
    let px = render_rgba(&off, side, side).unwrap();
    assert!(!saturated(&px));
    // Lo que da Qt offscreen sobre `#efefef`: negro → 95, rojo → 149, blanco → 255.
    let has = |v: u8| px.chunks(4).any(|c| c[3] == 255 && c[0].abs_diff(v) <= 2);
    assert!(has(95) && has(149) && has(255));
    let off = tool_icon_svg("stop", "xp", true).unwrap();
    assert!(!saturated(&render_rgba(&off, 20, 20).unwrap()));
}
