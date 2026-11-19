//! Contraseñas en el llavero del sistema (`passwords.py`), compatibles con las que guardó
//! 1.3.0 con `keyring` 25.7. Todo bloquea: solo se llama dentro de `app::blocking`.

use std::sync::Arc;

use jase_core::models::ServerAddress;

pub const SERVICE: &str = "JimmysAllSeeingEye";
pub const STEAM_KEY_USER: &str = "steam:webapi";
/// El atributo `application` que pone `keyring.backends.SecretService`.
pub const PY_APPLICATION: &str = "Python keyring library";

pub fn server_user(game_id: &str, address: &ServerAddress) -> String {
    format!("{game_id}:{address}")
}

pub fn rcon_user(game_id: &str, address: &ServerAddress) -> String {
    format!("rcon:{game_id}:{address}")
}

/// Una clave que nunca sale en un `Debug` (mensajes, logs): imprime `***`.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

/// Destinos de `WinVaultKeyring`: el del servicio (el último guardado por Python) y el
/// compuesto `{user}@{SERVICE}`.
pub fn win_targets(user: &str) -> [String; 2] {
    [SERVICE.to_owned(), format!("{user}@{SERVICE}")]
}

/// Los atributos con que Python crea la entrada del Secret Service.
pub fn ss_attributes(user: &str) -> [(&'static str, String); 3] {
    [
        ("username", user.to_owned()),
        ("service", SERVICE.to_owned()),
        ("application", PY_APPLICATION.to_owned()),
    ]
}

pub fn ss_label(user: &str) -> String {
    format!("Password for '{user}' on '{SERVICE}'")
}

/// `_get`/`_set` de Python: un error al leer es `None`; `set(user, "")` borra y nunca falla;
/// guardar sí devuelve el error (la UI lo muestra).
pub trait Keyring: Send + Sync {
    fn get(&self, user: &str) -> Option<String>;
    fn set(&self, user: &str, value: &str) -> Result<(), String>;
}

pub type KeyringFn = Arc<dyn Keyring>;

/// El Administrador de credenciales de Windows: destino → (`UserName`, contraseña).
pub trait WinCreds: Send + Sync {
    fn read(&self, target: &str) -> Result<Option<(String, String)>, String>;
    fn write(&self, target: &str, user: &str, password: &str) -> Result<(), String>;
    fn delete(&self, target: &str) -> Result<(), String>;
}

/// Lee y borra como `WinVaultKeyring`; escribe siempre en el destino compuesto y, si el del
/// servicio era de este usuario, lo borra: así 1.3.0 y Rust leen el valor nuevo sin que Rust
/// mueva credenciales ajenas.
pub struct WinVault<C>(pub C);

impl<C: WinCreds> WinVault<C> {
    /// ¿El destino existe y es de este usuario?
    fn owned(&self, target: &str, user: &str) -> Result<bool, String> {
        Ok(self.0.read(target)?.is_some_and(|(u, _)| u == user))
    }
}

impl<C: WinCreds> Keyring for WinVault<C> {
    fn get(&self, user: &str) -> Option<String> {
        let [service, compound] = win_targets(user);
        match self.0.read(&service).ok()? {
            Some((u, password)) if u == user => Some(password),
            _ => self.0.read(&compound).ok()?.map(|(_, password)| password),
        }
    }

    fn set(&self, user: &str, value: &str) -> Result<(), String> {
        let [service, compound] = win_targets(user);
        if value.is_empty() {
            for target in [service, compound] {
                if self.owned(&target, user).unwrap_or(false) {
                    let _ = self.0.delete(&target);
                }
            }
            return Ok(());
        }
        self.0.write(&compound, user, value)?;
        // Si quedara, Python y Rust seguirían leyendo el valor viejo del servicio.
        if self.owned(&service, user)? {
            self.0.delete(&service)?;
        }
        Ok(())
    }
}

/// Sin llavero (`--smoke`, `--screenshot`): nada guardado y no se puede guardar.
pub struct NoKeyring;

impl Keyring for NoKeyring {
    fn get(&self, _user: &str) -> Option<String> {
        None
    }

    fn set(&self, _user: &str, value: &str) -> Result<(), String> {
        if value.is_empty() {
            Ok(())
        } else {
            Err("no keyring".to_owned())
        }
    }
}
