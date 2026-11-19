//! Recursos del exe de Windows (D4): icono, el mismo del instalador (`JASE.exe,0`), y
//! `VS_VERSIONINFO` con la versión del crate. Fuera de Windows no hace nada.

fn main() {
    println!("cargo:rerun-if-changed=../../src/jase/assets/app_icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../src/jase/assets/app_icon.ico")
        .set("ProductName", "Jimmy's All Seeing Eye")
        .set("FileDescription", "Jimmy's All Seeing Eye")
        .set("OriginalFilename", "JASE.exe")
        .set("LegalCopyright", "MIT License");
    // Sin `rc.exe` el build falla: un exe sin icono ni versión no se empaqueta.
    if let Err(e) = res.compile() {
        panic!("winresource: {e}");
    }
}
