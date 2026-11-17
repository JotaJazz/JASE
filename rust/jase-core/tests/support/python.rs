use std::path::PathBuf;
use std::process::Command;

/// Un `Command` del Python del `.venv` del repo con `PYTHONPATH` al `src/` del repo (prueba
/// el código del árbol, no uno instalado), o `None` (con aviso) si no hay `.venv`.
pub fn python() -> Option<Command> {
    let python = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.venv/bin/python"
    ));
    if python.exists() {
        let mut cmd = Command::new(python);
        cmd.env(
            "PYTHONPATH",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../src"),
        );
        Some(cmd)
    } else {
        eprintln!("sin .venv: se salta la lectura desde Python");
        None
    }
}
