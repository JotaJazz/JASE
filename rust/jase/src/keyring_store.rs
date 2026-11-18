//! El llavero real: el Administrador de credenciales en Windows y el Secret Service (D-Bus)
//! en los demás, con los mismos destinos y atributos que `keyring` de Python (`passwords`).

pub use imp::system_keyring;

#[cfg(not(windows))]
mod imp {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, PoisonError};

    use keyring_core::api::CredentialStoreApi;
    use keyring_core::{Entry, Error};
    use zbus_secret_service_keyring_store::Store;

    use crate::passwords::{Keyring, KeyringFn, PY_APPLICATION, SERVICE, ss_label};

    pub fn system_keyring() -> KeyringFn {
        Arc::new(SecretService(Mutex::new(None)))
    }

    /// El store se crea en el primer uso (ya en un hilo bloqueante); si falla (sin D-Bus o sin
    /// Secret Service) no se guarda el error: se reintenta en la próxima llamada.
    struct SecretService(Mutex<Option<Arc<Store>>>);

    impl SecretService {
        /// Busca `{service, username}` en todas las colecciones (también lo de 1.3.0) y crea
        /// en la por defecto con la etiqueta de Python.
        fn entry(&self, user: &str) -> Result<Entry, Error> {
            let mut guard = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            let store = match guard.clone() {
                Some(store) => store,
                None => {
                    let store = Store::new()?;
                    *guard = Some(store.clone());
                    store
                }
            };
            drop(guard);
            let label = ss_label(user);
            store.build(
                SERVICE,
                user,
                Some(&HashMap::from([("label", label.as_str())])),
            )
        }
    }

    impl Keyring for SecretService {
        fn get(&self, user: &str) -> Option<String> {
            match self.entry(user).ok()?.get_password() {
                Ok(password) => Some(password),
                // Python toma la primera que encuentra.
                Err(Error::Ambiguous(entries)) => entries.first()?.get_password().ok(),
                Err(_) => None,
            }
        }

        fn set(&self, user: &str, value: &str) -> Result<(), String> {
            if value.is_empty() {
                if let Ok(entry) = self.entry(user)
                    && let Err(Error::Ambiguous(entries)) = entry.delete_credential()
                {
                    for entry in entries {
                        let _ = entry.delete_credential();
                    }
                }
                return Ok(());
            }
            let entry = self.entry(user).map_err(|e| e.to_string())?;
            let entries = match entry.set_password(value) {
                Ok(()) => vec![entry],
                Err(Error::Ambiguous(entries)) => {
                    for entry in &entries {
                        entry.set_password(value).map_err(|e| e.to_string())?;
                    }
                    entries
                }
                Err(e) => return Err(e.to_string()),
            };
            // Con los atributos completos, el `replace=True` de 1.3.0 reemplaza esta entrada
            // en vez de crear otra al lado.
            let application = HashMap::from([("application", PY_APPLICATION)]);
            for entry in entries {
                let _ = entry.update_attributes(&application);
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::sync::Arc;

    use keyring_core::api::CredentialStoreApi;
    use keyring_core::{Entry, Error};
    use windows_native_keyring_store::Store;

    use crate::passwords::{KeyringFn, SERVICE, WinCreds, WinVault};

    pub fn system_keyring() -> KeyringFn {
        Arc::new(WinVault(Credman(Store::new().map_err(|e| e.to_string()))))
    }

    /// Credenciales genéricas por destino explícito (UTF-16LE, `Persist` Enterprise, como
    /// Python).
    struct Credman(Result<Arc<Store>, String>);

    impl Credman {
        fn entry(&self, target: &str) -> Result<Entry, String> {
            let store = self.0.as_ref().map_err(Clone::clone)?;
            // Con destino explícito el usuario no cuenta: va en `UserName` al escribir.
            store
                .build(SERVICE, "", Some(&HashMap::from([("target", target)])))
                .map_err(|e| e.to_string())
        }
    }

    impl WinCreds for Credman {
        fn read(&self, target: &str) -> Result<Option<(String, String)>, String> {
            let entry = self.entry(target)?;
            let password = match entry.get_password() {
                Ok(password) => password,
                Err(Error::NoEntry) => return Ok(None),
                Err(e) => return Err(e.to_string()),
            };
            let attributes = entry.get_attributes().map_err(|e| e.to_string())?;
            let user = attributes.get("username").cloned().unwrap_or_default();
            Ok(Some((user, password)))
        }

        fn write(&self, target: &str, user: &str, password: &str) -> Result<(), String> {
            let entry = self.entry(target)?;
            entry.set_password(password).map_err(|e| e.to_string())?;
            // Una credencial nueva con destino explícito queda con `UserName` vacío.
            entry
                .update_attributes(&HashMap::from([("username", user)]))
                .map_err(|e| e.to_string())
        }

        fn delete(&self, target: &str) -> Result<(), String> {
            match self.entry(target)?.delete_credential() {
                Ok(()) | Err(Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}
