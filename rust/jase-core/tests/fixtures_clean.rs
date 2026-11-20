//! Las capturas versionadas no llevan datos de nadie (port de `tests/test_fixtures_clean.py`):
//! ni correos ni claves de Steam, en `tests/fixtures/` y en los goldens.

use std::path::{Path, PathBuf};

/// Todos los archivos bajo `dir`, con su contenido.
fn files(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.expect("entrada").path();
        if path.is_dir() {
            files(&path, out);
        } else {
            let bytes = std::fs::read(&path).expect("leer");
            out.push((path, bytes));
        }
    }
}

fn fixtures() -> Vec<(PathBuf, Vec<u8>)> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    files(
        &here.join("..").join("..").join("tests").join("fixtures"),
        &mut out,
    );
    files(&here.join("tests").join("golden"), &mut out);
    assert!(!out.is_empty());
    out
}

/// Correos de `[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}`, uno por cada `@`.
fn emails(bytes: &[u8]) -> Vec<&[u8]> {
    let local = |c: &u8| c.is_ascii_alphanumeric() || b"._%+-".contains(c);
    let domain = |c: &u8| c.is_ascii_alphanumeric() || b".-".contains(c);
    let mut out = Vec::new();
    for (at, _) in bytes.iter().enumerate().filter(|(_, c)| **c == b'@') {
        let start = at - bytes[..at].iter().rev().take_while(|c| local(c)).count();
        let run = &bytes[at + 1..];
        let run = &run[..run.iter().take_while(|c| domain(c)).count()];
        // El último punto con dos letras detrás, como el retroceso del `+` codicioso.
        let Some(dot) = (1..run.len()).rev().find(|&p| {
            run[p] == b'.'
                && run[p + 1..]
                    .iter()
                    .take(2)
                    .filter(|c| c.is_ascii_alphabetic())
                    .count()
                    == 2
        }) else {
            continue;
        };
        let tld = run[dot + 1..]
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .count();
        if start < at {
            out.push(&bytes[start..at + 2 + dot + tld]);
        }
    }
    out
}

#[test]
fn fixtures_have_no_email_addresses() {
    let found: Vec<_> = fixtures()
        .iter()
        .flat_map(|(path, bytes)| {
            emails(bytes)
                .into_iter()
                .filter(|m| !m.ends_with(b"admin@example.invalid"))
                .map(|m| format!("{}: {}", path.display(), String::from_utf8_lossy(m)))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(
        emails(b"x a.b@c.example.org y"),
        [&b"a.b@c.example.org"[..]]
    );
    assert!(emails(b"@x.org a@b.c a@.org").is_empty());
}

#[test]
fn fixtures_have_no_steam_keys() {
    let key = |b: &[u8]| {
        b.windows(4).any(|w| w == b"key=")
            || b.split(|c| !(c.is_ascii_digit() || (b'A'..=b'F').contains(c)))
                .any(|run| run.len() >= 32)
    };
    let found: Vec<_> = fixtures()
        .iter()
        .filter(|(_, bytes)| key(bytes))
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(found.is_empty(), "{found:?}");
}
