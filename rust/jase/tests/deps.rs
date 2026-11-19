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
            ![
                "iced",
                "keyring",
                "image",
                "rfd",
                "keyring-core",
                "windows-native-keyring-store",
                "zbus-secret-service-keyring-store",
            ]
            .contains(&name),
            "{line}"
        );
    }
}

/// El llavero va por D-Bus en Rust puro y cifra sin OpenSSL, y el selector de archivos sin
/// GTK: nada de C que enlazar.
#[test]
fn jase_has_no_c_dbus_or_openssl() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let Ok(out) = std::process::Command::new("cargo")
        .args(["tree", "-p", "jase", "--target", "all", "-e", "normal"])
        .args(["--prefix", "none", "--manifest-path", manifest])
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
    assert!(tree.contains("zbus-secret-service-keyring-store"));
    assert!(tree.contains("ksni")); // la bandeja de Linux, también sin GTK ni libappindicator
    for line in tree.lines() {
        let name = line.split(' ').next().unwrap_or("");
        assert!(
            ![
                "libdbus-sys",
                "dbus",
                "openssl-sys",
                "openssl",
                "gtk",
                "gtk-sys",
                "glib",
                "glib-sys",
                "gobject-sys",
                "libappindicator",
                "libappindicator-sys",
            ]
            .contains(&name),
            "{line}"
        );
    }
}
