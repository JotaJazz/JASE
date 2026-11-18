//! Cada literal de `tr`/`tr_fmt`/`tr_n` en `src/` está en los 5 catálogos
//! (`test_every_tr_literal_has_english` de `test_i18n.py`).

use std::path::{Path, PathBuf};

const CALLS: [(&str, usize); 3] = [("tr(", 1), ("tr_fmt(", 1), ("tr_n(", 2)];
const CATALOGS: [&str; 5] = ["en", "pt", "fr", "de", "it"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let path = e.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// Literal `"…"` al comienzo de `s` (tras espacios): su texto y el resto; `None` si no hay
/// uno o si se parte en varias líneas.
fn literal(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start().strip_prefix('"')?;
    let mut text = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((text, &s[i + 1..])),
            '\n' => return None,
            '\\' => match chars.next()?.1 {
                'n' => text.push('\n'),
                't' => text.push('\t'),
                c @ ('"' | '\\') => text.push(c),
                _ => return None,
            },
            c => text.push(c),
        }
    }
    None
}

/// (clave, dónde) de cada llamada; y los problemas (llamadas sin literal en una línea).
fn collect_keys(src: &Path) -> (Vec<(String, String)>, Vec<String>) {
    let mut files = Vec::new();
    rust_files(src, &mut files);
    files.sort();
    let (mut keys, mut problems) = (Vec::new(), Vec::new());
    for path in files {
        let code = std::fs::read_to_string(&path).unwrap();
        let rel = path.strip_prefix(src).unwrap().display().to_string();
        for (call, count) in CALLS {
            for (pos, _) in code.match_indices(call) {
                let before = code[..pos].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue; // `str(`, `attr(`…
                }
                let line_start = code[..pos].rfind('\n').map_or(0, |i| i + 1);
                if code[line_start..pos].trim_start().starts_with("//") {
                    continue;
                }
                let where_ = format!("{rel}:{}", code[..pos].matches('\n').count() + 1);
                let mut rest = &code[pos + call.len()..];
                for n in 0..count {
                    if n > 0 {
                        rest = rest.trim_start().strip_prefix(',').unwrap_or("");
                    }
                    let Some((key, tail)) = literal(rest) else {
                        problems.push(format!(
                            "{where_}: {call}) necesita {count} literal(es) en una sola línea"
                        ));
                        break;
                    };
                    keys.push((key, where_.clone()));
                    rest = tail;
                }
            }
        }
    }
    (keys, problems)
}

#[test]
fn every_tr_literal_has_english() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (keys, problems) = collect_keys(&root.join("src"));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(!keys.is_empty());
    let mut missing = Vec::new();
    for code in CATALOGS {
        let path = root.join(format!("../jase-core/locales/{code}.json"));
        let catalog: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        missing.extend(
            keys.iter()
                .filter(|(k, _)| !catalog.contains_key(k))
                .map(|(k, w)| format!("{w}: falta en locales/{code}.json: {k:?}")),
        );
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
