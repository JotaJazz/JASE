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

/// El bloque de nivel superior `key:` de un YAML (hasta la siguiente clave sin sangría).
fn yaml_block<'a>(yaml: &'a str, key: &str) -> &'a str {
    let head = format!("\n{key}:\n");
    let rest = &yaml[yaml.find(&head).unwrap_or_else(|| panic!("falta {key}:")) + head.len()..];
    let end = rest
        .match_indices('\n')
        .map(|(i, _)| i + 1)
        .find(|&i| rest[i..].starts_with(|c: char| c.is_ascii_alphabetic()))
        .unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn package_workflow_never_publishes() {
    let wf = repo(".github/workflows/package.yml").replace("\r\n", "\n");
    assert!(wf.contains("\npermissions:\n  contents: read\n"));
    for bad in [
        "contents: write",
        "tags:",
        "softprops/",
        "gh release",
        "secrets.",
    ] {
        assert!(!wf.contains(bad), "package.yml contiene {bad}");
    }
    let on = yaml_block(&wf, "on");
    assert!(on.contains("branches: [rust]"), "{on}");
    assert!(on.contains("workflow_dispatch:"), "{on}");
    assert!(!on.contains("release:"), "{on}");
    let mut uses = 0;
    for line in wf.lines() {
        let Some(at) = line
            .trim_start()
            .strip_prefix("- uses: ")
            .or_else(|| line.trim_start().strip_prefix("uses: "))
        else {
            continue;
        };
        uses += 1;
        let pin = at.split_whitespace().next().unwrap_or_default();
        let sha = pin.rsplit_once('@').map(|(_, s)| s).unwrap_or_default();
        assert!(
            sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
            "acción sin SHA fijo: {line}"
        );
    }
    assert!(uses > 0);
}

/// Las líneas de `release.yml` que fijan appimagetool y el runtime (URLs y SHA-256).
fn appimage_pins(yaml: &str) -> Vec<String> {
    yaml.lines()
        .map(str::trim)
        .filter(|l| {
            l.starts_with("curl -fsSLo ")
                || l.ends_with("  appimagetool")
                || l.ends_with("  runtime-x86_64")
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn package_workflow_names_match_release() {
    let wf = repo(".github/workflows/package.yml").replace("\r\n", "\n");
    let release = repo(".github/workflows/release.yml").replace("\r\n", "\n");
    for needle in [
        "JASE-${{ steps.ver.outputs.version }}-x86_64.AppImage",
        "name: JASE-${{ steps.ver.outputs.version }}-linux-x86_64",
        "runs-on: ubuntu-22.04",
    ] {
        assert!(wf.contains(needle), "falta {needle}");
        assert!(release.contains(needle), "release.yml cambió: {needle}");
    }
    let pins = appimage_pins(&release);
    assert_eq!(pins.len(), 4, "{pins:?}");
    assert_eq!(appimage_pins(&wf), pins);
}

#[test]
fn linux_script_ships_notices_not_pyside() {
    let sh = repo("scripts/package_linux.sh");
    for needle in [
        "set -euo pipefail",
        "JASE-LICENSE.txt",
        "DB-IP.txt",
        "flag-icons-MIT.txt",
        "THIRD-PARTY.txt",
        "scripts/build_appimage.sh",
        "--locked",
    ] {
        assert!(sh.contains(needle), "falta {needle}");
    }
    assert!(!sh.contains("PySide6"));
}
