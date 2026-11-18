//! El idioma de i18n es global en cada binario de test: quien lo cambia o asierta texto
//! traducido pasa por este único candado.
use std::sync::Mutex;

use jase_core::i18n;

static LOCK: Mutex<()> = Mutex::new(());

/// Devuelve el español al soltarse, aunque `f` haga panic.
struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        let _ = i18n::set_language("es");
    }
}

/// Corre `f` con el idioma `code` y deja el español (el idioma por defecto) al salir.
pub fn with_language<T>(code: &str, f: impl FnOnce() -> T) -> T {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Se suelta antes que el candado (orden inverso a la declaración).
    let _reset = Reset;
    i18n::set_language(code).unwrap();
    f()
}
