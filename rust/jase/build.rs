//! Recursos del exe de Windows (D4): icono, el mismo del instalador (`JASE.exe,0`), y
//! `VS_VERSIONINFO` con la versión del crate. Fuera de Windows no hace nada.

fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/app_icon.ico")
        .set("ProductName", "Jimmy's All Seeing Eye")
        .set("FileDescription", "Jimmy's All Seeing Eye")
        .set("OriginalFilename", "JASE.exe")
        .set("LegalCopyright", "MIT License");
    // winresource escribe el apóstrofo como `\'`, que `rc.exe` deja con la barra: se quita.
    let rc = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("jase.rc");
    res.write_resource_file(&rc).unwrap();
    let text = std::fs::read_to_string(&rc).unwrap().replace("\\'", "'");
    std::fs::write(&rc, text).unwrap();
    res.set_resource_file(rc.to_str().unwrap());
    // Sin `rc.exe` el build falla: un exe sin icono ni versión no se empaqueta.
    if let Err(e) = res.compile() {
        panic!("winresource: {e}");
    }
}
