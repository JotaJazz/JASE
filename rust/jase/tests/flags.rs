use std::path::PathBuf;

use iced::Size;
use jase::flags::{FLAG_SIZE, codes, flag, flag_svg};
use jase::icons::{render_rgba, tool_icon, tool_icon_svg};

fn flags_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "src",
        "jase",
        "assets",
        "flags",
    ]
    .iter()
    .collect()
}

#[test]
fn missing_icon_and_bad_flag_codes_are_none() {
    assert!(tool_icon_svg("no_such_icon", "dark", false).is_none());
    assert!(tool_icon("no_such_icon", "classic", false).is_none());
    assert!(render_rgba(b"not svg", 8, 8).is_none());
    assert!(render_rgba(b"", 0, 0).is_none());
    for code in ["", "c", "chl", "1a", "ñe", "../", "x/", "zz"] {
        assert!(flag_svg(code).is_none(), "{code:?}");
        assert!(flag(code).is_none(), "{code:?}");
    }
    let cl = std::fs::read(flags_dir().join("cl.svg")).unwrap();
    assert_eq!(flag_svg("CL"), Some(&cl[..]));
    assert_eq!(flag_svg(" cl "), Some(&cl[..]));
    assert!(flag("cl").is_some());
    assert_eq!(FLAG_SIZE, Size::new(20.0, 15.0));
}

#[test]
fn flag_list_matches_assets_dir() {
    let mut want: Vec<String> = std::fs::read_dir(flags_dir())
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".svg").map(str::to_owned)
        })
        .collect();
    want.sort();
    assert_eq!(codes().collect::<Vec<_>>(), want);
    for code in codes() {
        let disk = std::fs::read(flags_dir().join(format!("{code}.svg"))).unwrap();
        assert_eq!(flag_svg(code), Some(&disk[..]), "{code}");
    }
}

#[test]
fn every_flag_parses() {
    for code in codes() {
        assert!(
            render_rgba(flag_svg(code).unwrap(), 20, 15).is_some(),
            "{code}"
        );
    }
}
