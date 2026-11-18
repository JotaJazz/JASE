use iced::Color;
use iced::theme::Mode;
use jase::theme::{
    PING_GOOD_MAX, PING_MID_MAX, Palette, SELECTION_COLOR_MIN_CONTRAST, TERMINAL_COLORS, THEME_IDS,
    VARIANTS, WidgetStyle, contrast_ratio, hex, iced_theme, legible, palette, ping_color,
    resolve_theme, terminal_color_name, theme_display_name, widget_style,
};
use jase_core::i18n::set_language;

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
        set_language(lang).unwrap();
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
    }
    set_language("en").unwrap();
    assert_eq!(theme_display_name("xp"), "Windows XP");
    assert_eq!(terminal_color_name("white"), "White");
    assert_eq!(theme_display_name("rust"), "Rust");
    set_language("es").unwrap();
    assert_eq!(theme_display_name("rust"), "Óxido");
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
