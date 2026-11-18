mod support;

use iced::font::Weight;
use iced::theme::Mode;
use iced::widget::container;
use iced::widget::scrollable::{self, Status};
use iced::{Background, Color, Font, Gradient};
use jase::theme::{
    Edges, PING_GOOD_MAX, PING_MID_MAX, Palette, SELECTION_COLOR_MIN_CONTRAST, TERMINAL_COLORS,
    THEME_IDS, VARIANTS, WidgetStyle, contrast_ratio, hex, iced_theme, legible, palette,
    ping_color, resolve_theme, style, terminal_color_name, theme_display_name, widget_style,
};
use support::lang::with_language;

#[path = "../../jase-core/tests/support/python.rs"]
mod python;

const VARIANT_IDS: [&str; 14] = [
    "dark",
    "light",
    "neon",
    "brown",
    "green",
    "classic",
    "xp",
    "terminal_green",
    "terminal_white",
    "terminal_sepia",
    "terminal_amber",
    "synthwave",
    "rust",
    "goldsrc",
];

fn rgb(s: &str) -> Color {
    let v = u32::from_str_radix(&s[1..], 16).unwrap();
    Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn q3_hex(p: &Palette) -> Vec<Option<String>> {
    p.q3.iter().map(|c| c.map(hex)).collect()
}

#[test]
fn ping_thresholds() {
    let p = palette("dark");
    assert_eq!((PING_GOOD_MAX, PING_MID_MAX), (80, 150));
    assert_eq!(ping_color(79, p), p.ping_good);
    assert_eq!(ping_color(80, p), p.ping_mid);
    assert_eq!(ping_color(149, p), p.ping_mid);
    assert_eq!(ping_color(150, p), p.ping_bad);
}

#[test]
fn theme_ids_order() {
    assert_eq!(
        THEME_IDS,
        [
            "dark",
            "light",
            "neon",
            "brown",
            "green",
            "classic",
            "xp",
            "terminal",
            "synthwave",
            "rust",
            "goldsrc"
        ]
    );
    assert_eq!(TERMINAL_COLORS, ["green", "white", "sepia", "amber"]);
    let ids: Vec<&str> = VARIANTS.iter().map(|v| v.0).collect();
    assert_eq!(ids, VARIANT_IDS);
    assert_eq!(palette("bogus"), palette("dark"));
}

#[test]
fn all_themes_have_q3_colors() {
    for (id, p, _) in &VARIANTS {
        assert_eq!(p.q3.len(), 10, "{id}");
        if id.starts_with("terminal_") {
            assert!(p.q3.iter().all(Option::is_none), "{id} es monocromo");
        } else {
            assert!(p.q3[7].is_none() && p.q3[1].is_some(), "{id}");
        }
    }
}

#[test]
fn dark_and_light_tokens_unchanged() {
    let dark = palette("dark");
    let light = palette("light");
    let before = [
        (dark, ["#16181d", "#1d2027", "#1a1d23", "#2a2e37"]),
        (light, ["#f6f7f9", "#ffffff", "#eef0f3", "#e5e7eb"]),
    ];
    for (p, [bg, surface, sidebar, border]) in before {
        assert_eq!(
            [p.bg, p.surface, p.sidebar, p.border].map(hex),
            [bg, surface, sidebar, border]
        );
    }
    let rest = |p: &Palette| {
        [
            p.text,
            p.muted,
            p.accent,
            p.selection,
            p.ping_good,
            p.ping_mid,
            p.ping_bad,
            p.occupancy,
            p.favorite,
        ]
        .map(hex)
    };
    assert_eq!(
        rest(dark),
        [
            "#d7dbe2", "#7c8494", "#3b82f6", "#24324a", "#3ecf6e", "#e5b93a", "#ef5a5a", "#3b82f6",
            "#f5c542"
        ]
    );
    assert_eq!(
        rest(light),
        [
            "#1f2328", "#6b7280", "#2563eb", "#dbeafe", "#15803d", "#a16207", "#b91c1c", "#60a5fa",
            "#b7791f"
        ]
    );
    let badges = |p: &Palette| {
        [
            p.badge_cod1,
            p.badge_coduo,
            p.badge_cod2,
            p.badge_cod4,
            p.badge_q3,
            p.badge_rtcw,
            p.badge_et,
            p.badge_jka,
            p.badge_sof2,
            p.badge_cs16,
            p.badge_ut99,
            p.badge_mohaa,
            p.badge_mohaas,
            p.badge_mohaab,
            p.badge_bf1942,
        ]
        .map(hex)
    };
    assert_eq!(
        badges(dark),
        [
            "#936b09", "#8a6d3b", "#c0562b", "#4b7a3c", "#c54c1b", "#7a5c8a", "#3b6ea5", "#3b78a8",
            "#a04040", "#a65a17", "#8b701b", "#5a7a4a", "#8a4b3c", "#3c7a7a", "#5e7a2c"
        ]
    );
    assert_eq!(
        badges(light),
        [
            "#8a6408", "#67512c", "#904020", "#385b2d", "#a23e16", "#5b4567", "#2c527b", "#2c5b81",
            "#8a3333", "#8a4d17", "#8a6f1b", "#435b37", "#67382d", "#2d5b5b", "#475c21"
        ]
    );
    let some = |v: [&str; 10]| v.map(|s| (s != "-").then(|| s.to_owned())).to_vec();
    assert_eq!(
        q3_hex(dark),
        some([
            "#6b7280", "#ef5a5a", "#3ecf6e", "#e5c07b", "#5b8def", "#56c8d8", "#c678dd", "-",
            "#f0913a", "#8a919e"
        ])
    );
    assert_eq!(
        q3_hex(light),
        some([
            "#111827", "#c62828", "#15803d", "#a16207", "#1d4ed8", "#0e7490", "#9333ea", "-",
            "#c2410c", "#6b7280"
        ])
    );
}

#[test]
fn divider_and_outline() {
    for (id, p, _) in &VARIANTS {
        if !["neon", "synthwave"].contains(id) {
            assert_eq!(p.divider, p.border, "{id}");
            assert_eq!(p.selection_outline, p.selection, "{id}");
        }
    }
    for (id, border, divider) in [
        ("neon", "#19f5e1", "#0f4d49"),
        ("synthwave", "#ff3fa4", "#4a1f5e"),
    ] {
        let p = palette(id);
        assert_eq!(hex(p.border), border);
        assert_eq!(hex(p.divider), divider);
        assert_eq!(hex(p.selection_outline), border);
        assert_eq!(p.bevel_light, p.border);
        assert_eq!(p.bevel_dark, p.border);
    }
}

#[test]
fn contrast_ratio_helper() {
    let (black, white) = (rgb("#000000"), rgb("#ffffff"));
    assert!((contrast_ratio(black, white) - 21.0).abs() < 1e-3);
    assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-6);
}

#[test]
fn contrast_text() {
    for (id, p, _) in &VARIANTS {
        for surface in [p.bg, p.surface, p.sidebar] {
            assert!(contrast_ratio(p.text, surface) >= 4.5, "{id}");
        }
    }
}

#[test]
fn contrast_selection() {
    for (id, p, _) in &VARIANTS {
        assert!(contrast_ratio(p.selection_text, p.selection) >= 4.5, "{id}");
    }
}

#[test]
fn badge_tokens_have_wcag_contrast_with_white() {
    for (id, p, _) in &VARIANTS {
        for badge in [
            p.badge_cod1,
            p.badge_coduo,
            p.badge_cod2,
            p.badge_cod4,
            p.badge_q3,
            p.badge_rtcw,
            p.badge_et,
            p.badge_jka,
            p.badge_sof2,
            p.badge_cs16,
            p.badge_ut99,
            p.badge_mohaa,
            p.badge_mohaas,
            p.badge_mohaab,
            p.badge_bf1942,
            p.badge_tacops,
            p.badge_urt,
            p.badge_oa,
            p.badge_q2,
            p.badge_ut2004,
            p.badge_zandronum,
            p.badge_l4d,
            p.badge_l4d2,
            p.badge_css,
            p.badge_tf2,
            p.badge_hldm,
            p.badge_dod,
            p.badge_tfc,
            p.badge_dday,
            p.badge_gmod,
            p.badge_jk2,
            p.badge_smokinguns,
            p.badge_tremulous,
            p.badge_warfork,
            p.badge_warsow,
            p.badge_xonotic,
        ] {
            assert!(
                contrast_ratio(Color::WHITE, badge) >= 4.5,
                "{id} {}",
                hex(badge)
            );
        }
    }
}

#[test]
fn ping_and_q3_contrast_min_3() {
    for (id, p, _) in &VARIANTS {
        for c in [p.ping_good, p.ping_mid, p.ping_bad]
            .into_iter()
            .chain(p.q3.iter().flatten().copied())
        {
            assert!(contrast_ratio(c, p.surface) >= 3.0, "{id} {}", hex(c));
        }
    }
}

#[test]
fn resolve_theme_cases() {
    assert_eq!(resolve_theme("system", Mode::Light, "green"), "light");
    assert_eq!(resolve_theme("system", Mode::Dark, "green"), "dark");
    assert_eq!(resolve_theme("system", Mode::None, "green"), "dark");
    assert_eq!(resolve_theme("neon", Mode::Light, "green"), "neon");
    assert_eq!(resolve_theme("bogus", Mode::Light, "green"), "light");
    assert_eq!(
        resolve_theme("terminal", Mode::Dark, "amber"),
        "terminal_amber"
    );
    assert_eq!(resolve_theme("terminal", Mode::Dark, "x"), "terminal_green");
}

#[test]
fn theme_display_names_both_languages() {
    for lang in ["es", "en"] {
        with_language(lang, || {
            let names: Vec<String> = THEME_IDS.iter().map(|t| theme_display_name(t)).collect();
            let mut unique = names.clone();
            unique.sort();
            unique.dedup();
            assert!(names.iter().all(|n| !n.is_empty()) && unique.len() == THEME_IDS.len());
            for c in TERMINAL_COLORS {
                assert_eq!(
                    theme_display_name(&format!("terminal_{c}")),
                    theme_display_name("terminal")
                );
            }
            let colors: Vec<String> = TERMINAL_COLORS
                .iter()
                .map(|c| terminal_color_name(c))
                .collect();
            assert!(colors.iter().all(|c| !c.is_empty()));
        });
    }
    with_language("en", || {
        assert_eq!(theme_display_name("xp"), "Windows XP");
        assert_eq!(terminal_color_name("white"), "White");
        assert_eq!(theme_display_name("rust"), "Rust");
    });
    with_language("es", || assert_eq!(theme_display_name("rust"), "Óxido"));
}

#[test]
fn legible_color_on_selection() {
    let (navy, white) = (rgb("#000080"), Color::WHITE);
    assert_eq!(SELECTION_COLOR_MIN_CONTRAST, 3.5);
    assert_eq!(legible(rgb("#006400"), navy, white), white);
    assert_eq!(legible(rgb("#ffd23f"), navy, white), rgb("#ffd23f"));
    assert_eq!(
        legible(palette("classic").q3[2].unwrap(), navy, white),
        white
    );
    let dark = palette("dark");
    assert_eq!(legible(dark.ping_bad, dark.selection, white), dark.ping_bad);
}

#[test]
fn styles() {
    for (id, _, style) in &VARIANTS {
        let expected = match *id {
            "classic" => WidgetStyle::Win98,
            "xp" => WidgetStyle::Luna,
            "goldsrc" => WidgetStyle::Vgui,
            t if t.starts_with("terminal_") => WidgetStyle::Terminal,
            _ => WidgetStyle::Flat,
        };
        assert_eq!(*style, expected, "{id}");
        assert_eq!(widget_style(id), expected);
    }
    assert_eq!(widget_style("bogus"), WidgetStyle::Flat);
}

#[test]
fn palette_matches_python() {
    let Some(mut cmd) = python::python() else {
        return;
    };
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/gen_palette_rs.py"
    );
    let out = cmd.arg(script).arg("--stdout").output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/src/theme/palette.rs");
    // En Windows git puede convertir los saltos de línea al sacar el archivo.
    let on_disk = std::fs::read_to_string(file).unwrap().replace("\r\n", "\n");
    assert!(
        on_disk == String::from_utf8(out.stdout).unwrap(),
        "palette.rs desactualizado: corre scripts/gen_palette_rs.py"
    );
}

#[test]
fn iced_theme_uses_palette() {
    for (id, p, _) in &VARIANTS {
        let t = iced_theme(id).palette();
        assert_eq!(
            [
                t.background,
                t.text,
                t.primary,
                t.success,
                t.warning,
                t.danger
            ],
            [p.bg, p.text, p.accent, p.ping_good, p.ping_mid, p.ping_bad],
            "{id}"
        );
    }
}

const ACTIVE: Status = Status::Active {
    is_horizontal_scrollbar_disabled: false,
    is_vertical_scrollbar_disabled: false,
};

fn bg(s: &container::Style) -> Option<Color> {
    match s.background {
        Some(Background::Color(c)) => Some(c),
        _ => None,
    }
}

fn gradient(b: Option<Background>) -> Vec<Color> {
    match b {
        Some(Background::Gradient(Gradient::Linear(l))) => {
            l.stops.iter().flatten().map(|s| s.color).collect()
        }
        _ => vec![],
    }
}

fn bevel(w: f32, top_left: Color, bottom_right: Color) -> Edges {
    Edges {
        top: Some((w, top_left)),
        right: Some((w, bottom_right)),
        bottom: Some((w, bottom_right)),
        left: Some((w, top_left)),
    }
}

#[test]
fn dark_keeps_flat_look() {
    let (p, s) = (palette("dark"), widget_style("dark"));
    assert_eq!(s.radius(), 6.0);
    assert_eq!((s.font(), s.text_size()), (Font::DEFAULT, 13.0));
    let (frame, edges) = style::table_frame(p, s);
    assert_eq!(edges, None);
    assert_eq!(bg(&frame), Some(p.surface));
    assert_eq!(
        (frame.border.width, frame.border.color, frame.border.radius),
        (1.0, p.border, 6.0.into())
    );
    // La única línea es la de abajo del encabezado (`border-bottom: 1px solid divider`).
    let (header, edges) = style::table_header(p, s, "dark");
    assert_eq!(edges, Some(Edges::bottom(1.0, p.divider)));
    assert_eq!(
        (bg(&header), header.text_color),
        (Some(p.sidebar), Some(p.muted))
    );
    assert_eq!(style::status_label(p, s), None);
    assert_eq!(style::status_bar(p, s).1, None);
    assert_eq!(style::sidebar(p, s).border.radius, 6.0.into());
}

#[test]
fn classic_uses_bevels() {
    let (p, s) = (palette("classic"), widget_style("classic"));
    assert_eq!(Edges::raised(p), bevel(2.0, p.bevel_light, p.bevel_dark));
    assert_eq!(Edges::sunken(p), bevel(2.0, p.bevel_dark, p.bevel_light));
    let (header, edges) = style::table_header(p, s, "classic");
    assert_eq!(edges, Some(Edges::raised(p)));
    assert_eq!(
        (bg(&header), header.text_color),
        (Some(p.button_top), Some(p.text))
    );
    let (frame, edges) = style::table_frame(p, s);
    assert_eq!(edges, Some(Edges::sunken(p)));
    assert_eq!((frame.border.width, frame.border.radius), (0.0, 0.0.into()));
    assert_eq!(s.radius(), 0.0);
    let row = style::table_row(p, s, true, true);
    assert_eq!(
        (bg(&row), row.text_color),
        (Some(p.selection), Some(p.selection_text))
    );
    assert_eq!(style::status_label(p, s), Some(Edges::thin(p, true)));
    assert_eq!(Edges::thin(p, true), bevel(1.0, p.border, p.bevel_light));
}

#[test]
fn xp_uses_luna_gradient() {
    let (p, s) = (palette("xp"), widget_style("xp"));
    let thumb = style::scrollable(p, s, ACTIVE).vertical_rail.scroller;
    assert_eq!(
        gradient(Some(thumb.background)),
        [p.scroll_top, p.scroll_bottom]
    );
    assert_eq!(
        (thumb.border.color, thumb.border.width, thumb.border.radius),
        (p.scroll_border, 1.0, 3.0.into())
    );
    let (header, edges) = style::table_header(p, s, "xp");
    assert_eq!(bg(&header), Some(p.header_bg));
    let edges = edges.unwrap();
    assert_eq!(edges, Edges::right_bottom(1.0, 3.0, p.header_line));
    assert_eq!(edges.bottom, Some((3.0, p.header_line)));
    assert_eq!(s.radius(), 3.0);
}

#[test]
fn terminal_uses_inverse_video() {
    for id in VARIANT_IDS.iter().filter(|id| id.starts_with("terminal_")) {
        let (p, s) = (palette(id), widget_style(id));
        assert_eq!((s.font(), s.text_size()), (Font::MONOSPACE, 12.0), "{id}");
        let tip = style::tooltip(p, s);
        assert_eq!(
            (bg(&tip), tip.text_color),
            (Some(p.text), Some(p.bg)),
            "{id}"
        );
        let bar = style::status_bar(p, s).0;
        assert_eq!(
            (bg(&bar), bar.text_color),
            (Some(p.text), Some(p.bg)),
            "{id}"
        );
        let row = style::table_row(p, s, true, true);
        assert_eq!(bg(&row), Some(p.text), "{id}: la selección es la tinta");
        assert!([p.bg, p.surface].contains(&row.text_color.unwrap()), "{id}");
    }
}

#[test]
fn period_themes_font_is_8pt_and_others_keep_size() {
    for (id, _, s) in &VARIANTS {
        let period = ["classic", "xp", "goldsrc"].contains(id);
        let font = match s {
            _ if period => Font::with_name("Tahoma"),
            WidgetStyle::Terminal => Font::MONOSPACE,
            _ => Font::DEFAULT,
        };
        let size = match s {
            _ if period => 8.0 * 96.0 / 72.0,
            WidgetStyle::Terminal => 12.0,
            _ => 13.0,
        };
        assert_eq!((s.font(), s.text_size()), (font, size), "{id}");
    }
    let widths = [
        WidgetStyle::Flat,
        WidgetStyle::Win98,
        WidgetStyle::Luna,
        WidgetStyle::Vgui,
        WidgetStyle::Terminal,
    ]
    .map(style::scrollbar_width);
    assert_eq!(widths, [10.0, 16.0, 17.0, 16.0, 14.0]);
}

#[test]
fn flat_themes_have_their_own_flavor() {
    let header = |id| style::table_header(palette(id), widget_style(id), id);
    let p = palette("synthwave");
    let (h, edges) = header("synthwave");
    assert_eq!(gradient(h.background), [p.bar_top, p.sidebar]);
    assert_eq!(h.text_color, Some(p.header_text));
    assert_eq!(edges, Some(Edges::bottom(2.0, p.sunset)));
    for (id, w) in [("neon", 1.0), ("green", 1.0), ("rust", 2.0)] {
        assert_eq!(
            header(id).1,
            Some(Edges::bottom(w, palette(id).accent)),
            "{id}"
        );
    }
    for id in ["dark", "light"] {
        let p = palette(id);
        assert_eq!(header(id).1, Some(Edges::bottom(1.0, p.divider)), "{id}");
        assert_eq!(bg(&header(id).0), Some(p.sidebar), "{id}");
    }
}

#[test]
fn classic_sunken_on_white_keeps_bottom_right_edge() {
    let p = palette("classic");
    assert_ne!(p.bevel_inner, p.surface);
    assert_eq!(
        Edges::sunken_on_surface(p),
        bevel(2.0, p.bevel_dark, p.bevel_inner)
    );
}

#[test]
fn unfocused_selection_uses_face_color() {
    for (id, face) in [
        ("classic", palette("classic").button_top),
        ("xp", palette("xp").bg),
    ] {
        let (p, s) = (palette(id), widget_style(id));
        for st in [
            style::table_row(p, s, true, false),
            style::list_item(p, s, true, false),
        ] {
            assert_eq!((bg(&st), st.text_color), (Some(face), Some(p.text)), "{id}");
        }
    }
    let (p, s) = (palette("dark"), widget_style("dark"));
    assert_eq!(
        style::table_row(p, s, true, false),
        style::table_row(p, s, true, true)
    );
    assert_eq!(
        style::list_item(p, s, true, false),
        style::list_item(p, s, true, true)
    );
}

fn container_colors(s: &container::Style) -> Vec<Color> {
    let mut out: Vec<Color> = s.text_color.into_iter().collect();
    match s.background {
        Some(Background::Color(c)) => out.push(c),
        b => out.extend(gradient(b)),
    }
    if s.border.width > 0.0 {
        out.push(s.border.color);
    }
    out
}

fn edge_colors(e: Option<Edges>) -> Vec<Color> {
    e.map_or(vec![], |e| {
        [e.top, e.right, e.bottom, e.left]
            .into_iter()
            .flatten()
            .map(|(_, c)| c)
            .collect()
    })
}

fn scroll_colors(s: &scrollable::Style) -> Vec<Color> {
    let mut out = container_colors(&s.container);
    for rail in [s.vertical_rail, s.horizontal_rail] {
        match rail.background {
            Some(Background::Color(c)) => out.push(c),
            b => out.extend(gradient(b)),
        }
        let thumb = rail.scroller;
        out.extend(gradient(Some(thumb.background)));
        if let Background::Color(c) = thumb.background {
            out.push(c);
        }
        for b in [rail.border, thumb.border] {
            if b.width > 0.0 {
                out.push(b.color);
            }
        }
    }
    out
}

#[test]
fn stylesheet_uses_tokens() {
    let statuses = [
        ACTIVE,
        Status::Active {
            is_horizontal_scrollbar_disabled: true,
            is_vertical_scrollbar_disabled: true,
        },
        Status::Hovered {
            is_horizontal_scrollbar_hovered: false,
            is_vertical_scrollbar_hovered: true,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        },
        Status::Dragged {
            is_horizontal_scrollbar_dragged: false,
            is_vertical_scrollbar_dragged: true,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        },
    ];
    for (id, p, s) in &VARIANTS {
        let s = *s;
        // Cada color en el `Debug` de la paleta, como un token de `TOKENS` en el QSS.
        let tokens = format!("{p:?}");
        let mut colors = container_colors(&style::window(p));
        colors.extend(container_colors(&style::sidebar(p, s)));
        let (frame, e) = style::table_frame(p, s);
        colors.extend(container_colors(&frame).into_iter().chain(edge_colors(e)));
        let (header, e) = style::table_header(p, s, id);
        colors.extend(container_colors(&header).into_iter().chain(edge_colors(e)));
        let (bar, e) = style::status_bar(p, s);
        colors.extend(container_colors(&bar).into_iter().chain(edge_colors(e)));
        colors.extend(edge_colors(style::status_label(p, s)));
        colors.extend(container_colors(&style::tooltip(p, s)));
        colors.extend(container_colors(&style::occupancy_block(p)));
        colors.extend(edge_colors(Some(style::occupancy_frame(p, s))));
        // La barra plana es `occupancy` con alfa (`occupancy_tokens_in_both_themes`).
        colors.push(Color {
            a: 1.0,
            ..style::occupancy_bar(p)
        });
        for (sel, focus) in [(false, true), (true, true), (true, false)] {
            colors.extend(container_colors(&style::table_row(p, s, sel, focus)));
            colors.extend(container_colors(&style::list_item(p, s, sel, focus)));
        }
        for st in statuses {
            colors.extend(scroll_colors(&style::scrollable(p, s, st)));
        }
        for c in colors {
            assert!(
                c == Color::TRANSPARENT || tokens.contains(&format!("{c:?}")),
                "{id}: {} no es un token",
                hex(c)
            );
        }
    }
}

#[test]
fn outline_is_top_and_bottom_only() {
    // `_outline_rules`: solo `border-top`/`border-bottom` de la celda.
    for id in ["neon", "synthwave"] {
        let p = palette(id);
        let line = Some((1.0, p.selection_outline));
        let want = Edges {
            top: line,
            bottom: line,
            ..Edges::default()
        };
        assert_eq!(style::row_outline(p), Some(want), "{id}");
        assert_eq!(
            style::table_row(p, widget_style(id), true, true)
                .border
                .width,
            0.0
        );
    }
    assert_eq!(style::row_outline(palette("dark")), None);
}

#[test]
fn green_marks_selected_game() {
    let p = palette("green");
    let mark = Edges {
        left: Some((3.0, p.accent)),
        ..Edges::default()
    };
    assert_eq!(style::list_item_edges(p, "green", true), Some(mark));
    assert_eq!(style::list_item_edges(p, "green", false), None);
    assert_eq!(style::list_item_edges(palette("dark"), "dark", true), None);
}

#[test]
fn flat_header_is_bold() {
    // El estilo plano no pone `font-weight` y Qt deja la sección en negrita; los de época,
    // `font-weight: normal`.
    for (id, _, s) in &VARIANTS {
        let want = if *s == WidgetStyle::Flat {
            Weight::Bold
        } else {
            Weight::Normal
        };
        let f = style::header_font(*s);
        assert_eq!((f.weight, f.family), (want, s.font().family), "{id}");
    }
}

#[test]
fn status_label_padding_per_style() {
    // `QStatusBar QLabel { padding }`: 0 4 en 98/VGUI, 0 6 en Luna/Terminal.
    let pad = |s| style::status_label_padding(s).left;
    assert_eq!(pad(WidgetStyle::Win98), 4.0);
    assert_eq!(pad(WidgetStyle::Vgui), 4.0);
    assert_eq!(pad(WidgetStyle::Luna), 6.0);
    assert_eq!(pad(WidgetStyle::Terminal), 6.0);
    assert_eq!(pad(WidgetStyle::Flat), 0.0);
}
