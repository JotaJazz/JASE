//! El idioma de i18n es global en cada binario de test: quien lo cambia o asierta texto
//! traducido pasa por este único candado.
use std::sync::Mutex;

use jase_core::i18n;

static LOCK: Mutex<()> = Mutex::new(());

/// Corre `f` con el idioma `code` y deja el español (el idioma por defecto) al salir.
pub fn with_language<T>(code: &str, f: impl FnOnce() -> T) -> T {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_language(code).unwrap();
    let out = f();
    i18n::set_language("es").unwrap();
    out
}
