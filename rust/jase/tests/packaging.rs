//! Archivos del empaquetado (fase 6): lo que el paquete promete se lee del repo.

use std::path::Path;

fn repo(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()))
}

#[test]
fn about_accepts_no_copyleft() {
    let about = repo("rust/about.toml");
    let start = about.find("accepted = [").expect("accepted") + "accepted = [".len();
    let end = start + about[start..].find(']').expect("cierre de accepted");
    let accepted: Vec<&str> = about[start..end]
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .flat_map(|l| l.split(','))
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .collect();
    assert!(!accepted.is_empty());
    for lic in &accepted {
        for bad in ["GPL", "EUPL", "SSPL"] {
            assert!(!lic.contains(bad), "licencia copyleft aceptada: {lic}");
        }
    }
    assert!(about.contains("x86_64-pc-windows-msvc"));
    assert!(about.contains("x86_64-unknown-linux-gnu"));
    assert!(about.contains(r#"workarounds = ["ring"]"#));
}

#[test]
fn about_template_credits_iced_and_vendored_code() {
    let hbs = repo("rust/about.hbs");
    for needle in ["Iced", "Timothy Landers", "huffman.rs", "{{{"] {
        assert!(hbs.contains(needle), "falta {needle}");
    }
    assert!(!hbs.contains("PySide6"));
}

#[test]
fn workspace_crates_are_not_published() {
    for krate in ["jase", "jase-core", "jase-cli"] {
        let toml = repo(&format!("rust/{krate}/Cargo.toml"));
        assert!(
            toml.contains("publish = false"),
            "{krate} sin publish = false"
        );
    }
}
