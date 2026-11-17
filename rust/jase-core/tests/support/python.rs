use std::path::PathBuf;

/// El Python del `.venv` del repo, o `None` (con aviso) si no está.
pub fn python() -> Option<PathBuf> {
    let python = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.venv/bin/python"
    ));
    if python.exists() {
        Some(python)
    } else {
        eprintln!("sin .venv: se salta la lectura desde Python");
        None
    }
}
