//! El llavero (`passwords.py` y `tests/test_passwords.py`), su compatibilidad con lo que
//! guarda 1.3.0 y la clave de Steam que la app lee al arrancar.

mod support;

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use jase::app::Message;
use jase::passwords::{
    Keyring, NoKeyring, STEAM_KEY_USER, WinCreds, WinVault, server_user, ss_attributes, ss_label,
    win_targets,
};
use jase_core::config::Config;
use jase_core::models::ServerAddress;
use support::{fake_app_task, fake_app_with_key, run_task};

/// Destino → (`UserName`, contraseña).
type CredMap = BTreeMap<String, (String, String)>;

/// El Administrador de credenciales en memoria; con `fail` toda llamada falla.
#[derive(Default)]
struct MemCreds {
    map: Mutex<CredMap>,
    fail: AtomicBool,
}

impl MemCreds {
    fn with(entries: &[(&str, &str, &str)]) -> MemCreds {
        let map = entries
            .iter()
            .map(|(t, u, p)| (t.to_string(), (u.to_string(), p.to_string())))
            .collect();
        MemCreds {
            map: Mutex::new(map),
            ..MemCreds::default()
        }
    }

    fn check(&self) -> Result<(), String> {
        if self.fail.load(Ordering::SeqCst) {
            Err("boom".into())
        } else {
            Ok(())
        }
    }
}

impl WinCreds for MemCreds {
    fn read(&self, target: &str) -> Result<Option<(String, String)>, String> {
        self.check()?;
        Ok(self.map.lock().unwrap().get(target).cloned())
    }

    fn write(&self, target: &str, user: &str, password: &str) -> Result<(), String> {
        self.check()?;
        let entry = (user.to_owned(), password.to_owned());
        self.map.lock().unwrap().insert(target.to_owned(), entry);
        Ok(())
    }

    fn delete(&self, target: &str) -> Result<(), String> {
        self.check()?;
        self.map.lock().unwrap().remove(target);
        Ok(())
    }
}

fn map(vault: &WinVault<MemCreds>) -> CredMap {
    vault.0.map.lock().unwrap().clone()
}

/// `WinVaultKeyring._resolve_credential` de keyring 25.7, copiado a mano.
fn python_get(m: &CredMap, user: &str) -> Option<String> {
    let mut res = m.get("JimmysAllSeeingEye");
    if res.is_none_or(|(u, _)| u != user) {
        res = m.get(&format!("{user}@JimmysAllSeeingEye"));
    }
    res.map(|(_, pw)| pw.clone())
}

fn cod2() -> String {
    server_user("cod2", &ServerAddress::new("1.2.3.4", 28960))
}

#[test]
fn set_get_forget() {
    let kr = WinVault(MemCreds::default());
    let user = cod2();
    assert_eq!(user, "cod2:1.2.3.4:28960");
    assert_eq!(kr.get(&user), None);
    kr.set(&user, "pw").unwrap();
    assert_eq!(kr.get(&user).as_deref(), Some("pw"));
    kr.set(&user, "").unwrap();
    kr.set(&user, "").unwrap(); // la 2ª vez no falla
    assert_eq!(kr.get(&user), None);
    assert!(map(&kr).is_empty());
}

#[test]
fn backend_errors_are_tolerated() {
    let kr = WinVault(MemCreds::default());
    kr.0.fail.store(true, Ordering::SeqCst);
    assert_eq!(kr.get(&cod2()), None);
    assert_eq!(kr.set(&cod2(), ""), Ok(()));
    assert!(kr.set(&cod2(), "pw").is_err()); // guardar sí informa
}

#[test]
fn steam_key_roundtrip_and_clear() {
    let kr = WinVault(MemCreds::default());
    assert_eq!(STEAM_KEY_USER, "steam:webapi");
    assert_eq!(kr.get(STEAM_KEY_USER), None);
    kr.set(STEAM_KEY_USER, "ABC").unwrap();
    assert_eq!(kr.get(STEAM_KEY_USER).as_deref(), Some("ABC"));
    kr.set(STEAM_KEY_USER, "").unwrap();
    assert_eq!(kr.get(STEAM_KEY_USER), None);
    kr.set(STEAM_KEY_USER, "").unwrap(); // borrar de nuevo no falla
}

#[test]
fn set_password_empty_deletes() {
    let kr = WinVault(MemCreds::default());
    kr.set(&cod2(), "pw").unwrap();
    kr.set(&cod2(), "").unwrap();
    assert_eq!(kr.get(&cod2()), None);
}

#[test]
fn no_keyring_reads_nothing_and_cannot_save() {
    assert_eq!(NoKeyring.get(STEAM_KEY_USER), None);
    assert_eq!(NoKeyring.set(STEAM_KEY_USER, ""), Ok(()));
    assert!(NoKeyring.set(STEAM_KEY_USER, "x").is_err());
}

#[test]
fn win_targets_match_python() {
    assert_eq!(
        win_targets(&cod2()),
        [
            "JimmysAllSeeingEye".to_owned(),
            "cod2:1.2.3.4:28960@JimmysAllSeeingEye".to_owned(),
        ]
    );
    assert_eq!(
        win_targets(STEAM_KEY_USER)[1],
        "steam:webapi@JimmysAllSeeingEye"
    );
}

/// Lo que deja 1.3.0 tras guardar cod2=`a` y luego q3=`b`.
fn python_state() -> WinVault<MemCreds> {
    WinVault(MemCreds::with(&[
        ("JimmysAllSeeingEye", "q3:2.2.2.2:27960", "b"),
        (
            "cod2:1.1.1.1:28960@JimmysAllSeeingEye",
            "cod2:1.1.1.1:28960",
            "a",
        ),
    ]))
}

#[test]
fn vault_reads_python_entries() {
    let kr = python_state();
    assert_eq!(kr.get("cod2:1.1.1.1:28960").as_deref(), Some("a"));
    assert_eq!(kr.get("q3:2.2.2.2:27960").as_deref(), Some("b"));
    assert_eq!(kr.get("q3:3.3.3.3:27960"), None);
}

#[test]
fn python_reads_rust_entries() {
    let kr = python_state();
    kr.set("q3:2.2.2.2:27960", "c").unwrap();
    kr.set("et:4.4.4.4:27960", "d").unwrap();
    let m = map(&kr);
    assert!(!m.contains_key("JimmysAllSeeingEye"));
    assert_eq!(
        m["q3:2.2.2.2:27960@JimmysAllSeeingEye"],
        ("q3:2.2.2.2:27960".to_owned(), "c".to_owned())
    );
    assert_eq!(python_get(&m, "q3:2.2.2.2:27960").as_deref(), Some("c"));
    assert_eq!(python_get(&m, "et:4.4.4.4:27960").as_deref(), Some("d"));
    assert_eq!(python_get(&m, "cod2:1.1.1.1:28960").as_deref(), Some("a"));
    // Y Rust lee lo mismo.
    assert_eq!(kr.get("q3:2.2.2.2:27960").as_deref(), Some("c"));
}

#[test]
fn forget_clears_both_targets() {
    let kr = WinVault(MemCreds::with(&[
        ("JimmysAllSeeingEye", "q3:2.2.2.2:27960", "new"),
        (
            "q3:2.2.2.2:27960@JimmysAllSeeingEye",
            "q3:2.2.2.2:27960",
            "old",
        ),
        (
            "cod2:1.1.1.1:28960@JimmysAllSeeingEye",
            "cod2:1.1.1.1:28960",
            "a",
        ),
    ]));
    kr.set("q3:2.2.2.2:27960", "").unwrap();
    let m = map(&kr);
    assert_eq!(
        m.keys().collect::<Vec<_>>(),
        ["cod2:1.1.1.1:28960@JimmysAllSeeingEye"]
    );
    // El destino del servicio de otro usuario no se borra.
    let kr = python_state();
    kr.set("cod2:1.1.1.1:28960", "").unwrap();
    assert_eq!(map(&kr).keys().collect::<Vec<_>>(), ["JimmysAllSeeingEye"]);
}

#[test]
fn secret_service_attributes_match_python() {
    assert_eq!(
        ss_attributes("cod2:1.2.3.4:28960"),
        [
            ("username", "cod2:1.2.3.4:28960".to_owned()),
            ("service", "JimmysAllSeeingEye".to_owned()),
            ("application", "Python keyring library".to_owned()),
        ]
    );
    assert_eq!(
        ss_label("cod2:1.2.3.4:28960"),
        "Password for 'cod2:1.2.3.4:28960' on 'JimmysAllSeeingEye'"
    );
}

/// `cargo test -p jase --test passwords -- --ignored` en un escritorio con Secret Service o
/// en Windows: toca el llavero real del usuario (con un usuario propio que borra al final).
#[test]
#[ignore]
fn real_keyring_round_trip() {
    let kr = jase::keyring_store::system_keyring();
    let user = format!("jase-test:{}", std::process::id());
    kr.set(&user, "secreto").unwrap();
    assert_eq!(kr.get(&user).as_deref(), Some("secreto"));
    kr.set(&user, "otro").unwrap();
    assert_eq!(kr.get(&user).as_deref(), Some("otro"));
    kr.set(&user, "").unwrap();
    assert_eq!(kr.get(&user), None);
}

// ------------------------------------------------------------ la clave de Steam en la app

fn starting_with(game: &str) -> Config {
    let mut cfg = Config::default();
    cfg.ui_state.insert("last_game".into(), game.into());
    cfg
}

fn steam_keys(msgs: Vec<Message>) -> Vec<String> {
    msgs.into_iter()
        .filter_map(|m| match m {
            Message::SteamKey(k) => Some(k),
            _ => None,
        })
        .collect()
}

#[test]
fn steam_key_loads_from_keyring_off_ui_thread() {
    let dir = tempfile::tempdir().unwrap();
    let (app, fake, task) = fake_app_with_key(dir.path(), Config::default(), "FROMRING");
    assert_eq!(app.steam_key(), None);
    assert_eq!(steam_keys(run_task(task)), ["FROMRING"]);
    let calls = fake.keyring.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "steam:webapi");
    assert_ne!(calls[0].1, std::thread::current().id());
}

#[test]
fn keyring_never_runs_in_update() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _task) = fake_app_with_key(dir.path(), starting_with("l4d"), "K");
    let _ = app.update(Message::SteamKey("K".into()));
    let _ = app.update(Message::Refresh);
    let _ = app.update(Message::SelectGame("cod2"));
    assert!(fake.keyring.calls().is_empty());
    assert_eq!(app.steam_key(), Some("K"));
}

#[test]
fn steam_scan_waits_for_key() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _task) = fake_app_task(dir.path(), starting_with("l4d"));
    assert!(fake.calls().is_empty());
    assert_eq!(app.status(), "");
    let _ = app.update(Message::SteamKey("K".into()));
    assert_eq!(fake.calls(), ["l4d"]);
    assert_eq!(fake.steam_keys(), ["K"]);
    assert!(app.is_scanning());

    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _task) = fake_app_task(dir.path(), starting_with("l4d"));
    let _ = app.update(Message::SteamKey(String::new()));
    assert!(fake.calls().is_empty());
    assert_eq!(
        app.status(),
        "Left 4 Dead: falta la clave de API de Steam; ponla en Ajustes → General"
    );
}

#[test]
fn steam_key_for_a_game_left_behind_does_not_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _task) = fake_app_task(dir.path(), starting_with("l4d"));
    let _ = app.update(Message::SelectGame("cod2"));
    let _ = app.update(Message::SteamKey("K".into()));
    assert_eq!(fake.calls(), ["cod2"]);
}

#[test]
fn scan_receives_steam_key() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _task) = fake_app_task(dir.path(), Config::default());
    let _ = app.update(Message::SteamKey("K".into()));
    let _ = app.update(Message::SelectGame("l4d"));
    let _ = app.update(Message::Refresh);
    assert_eq!(fake.calls(), ["cod2", "l4d", "l4d"]);
    assert_eq!(fake.steam_keys(), ["", "K", "K"]);
}

#[test]
fn non_steam_scan_gets_no_key() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, fake, _task) = fake_app_task(dir.path(), starting_with("l4d"));
    let _ = app.update(Message::SteamKey("K".into()));
    let _ = app.update(Message::SelectGame("cod2"));
    assert_eq!(fake.calls(), ["l4d", "cod2"]);
    assert_eq!(fake.steam_keys(), ["K", ""]);
}
