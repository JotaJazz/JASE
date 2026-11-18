#[test]
fn core_does_not_depend_on_gui_crates() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let Ok(out) = std::process::Command::new("cargo")
        .args([
            "tree",
            "-p",
            "jase-core",
            "-e",
            "normal",
            "--prefix",
            "none",
        ])
        .args(["--manifest-path", manifest])
        .output()
    else {
        return; // sin cargo en el PATH
    };
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8(out.stdout).unwrap();
    for line in tree.lines() {
        let name = line.split(' ').next().unwrap_or("");
        assert!(
            !["iced", "keyring", "image", "rfd"].contains(&name),
            "{line}"
        );
    }
}
