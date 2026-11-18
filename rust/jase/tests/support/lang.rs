//! El idioma de i18n es global en cada binario de test: quien lo cambia o asierta texto
//! traducido pasa por este único candado.
use std::cell::Cell;
use std::sync::{Mutex, MutexGuard};

use jase_core::i18n;

static LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    /// Este hilo ya tiene el candado (un test no lo toma dos veces: se bloquearía solo).
    static HELD: Cell<bool> = const { Cell::new(false) };
}

/// Mantiene el candado con el idioma en español mientras viva (el `App` de prueba).
pub struct Spanish(Option<MutexGuard<'static, ()>>);

impl Spanish {
    pub fn hold() -> Spanish {
        if HELD.get() {
            return Spanish(None);
        }
        let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        HELD.set(true);
        Spanish(Some(guard))
    }
}

impl Drop for Spanish {
    fn drop(&mut self) {
        if self.0.is_some() {
            HELD.set(false);
        }
    }
}

/// Devuelve el español al soltarse, aunque `f` haga panic.
struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        let _ = i18n::set_language("es");
    }
}

struct Release;

impl Drop for Release {
    fn drop(&mut self) {
        HELD.set(false);
    }
}

/// Corre `f` con el idioma `code` y deja el español (el idioma por defecto) al salir.
pub fn with_language<T>(code: &str, f: impl FnOnce() -> T) -> T {
    assert!(!HELD.get(), "with_language con un App de prueba vivo");
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    HELD.set(true);
    let _held = Release;
    // Se suelta antes que el candado (orden inverso a la declaración).
    let _reset = Reset;
    i18n::set_language(code).unwrap();
    f()
}
