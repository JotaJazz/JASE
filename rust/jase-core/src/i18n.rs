//! Traducción sin UI. Traducción de `src/jase/i18n.py`.
//!
//! El español es el idioma de origen: las claves son los propios textos en español. En
//! otro idioma se busca en su catálogo; si falta, en inglés y, si tampoco está, se devuelve
//! el español. Las llaves solo se interpretan cuando se formatea (`tr_fmt`, `tr_n`).
//! Catálogos: `locales/<código>.json`, exportados de Python con `scripts/export_locales.py`.
use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const LANGUAGES: [&str; 6] = ["es", "en", "pt", "fr", "de", "it"];

pub type Catalog = HashMap<String, String>;

/// Argumentos con nombre para formatear (`{name}`).
pub type Args<'a> = &'a [(&'a str, &'a dyn Display)];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("idioma no soportado: {0:?}")]
pub struct UnknownLanguage(pub String);

/// Índice en LANGUAGES del idioma actual.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

static CATALOGS: LazyLock<HashMap<&'static str, Catalog>> = LazyLock::new(|| {
    let raw = [
        ("en", include_str!("../locales/en.json")),
        ("pt", include_str!("../locales/pt.json")),
        ("fr", include_str!("../locales/fr.json")),
        ("de", include_str!("../locales/de.json")),
        ("it", include_str!("../locales/it.json")),
    ];
    // Los catálogos son parte del binario y los valida el test de catálogos.
    raw.into_iter()
        .map(|(code, text)| (code, serde_json::from_str(text).unwrap_or_default()))
        .collect()
});

/// Cada idioma con su propio nombre (así se muestra en Ajustes).
pub fn language_name(code: &str) -> &'static str {
    match code {
        "es" => "Español",
        "en" => "English",
        "pt" => "Português (Brasil)",
        "fr" => "Français",
        "de" => "Deutsch",
        "it" => "Italiano",
        _ => "",
    }
}

pub fn set_language(code: &str) -> Result<(), UnknownLanguage> {
    let index = LANGUAGES
        .iter()
        .position(|l| *l == code)
        .ok_or_else(|| UnknownLanguage(code.to_owned()))?;
    CURRENT.store(index, Ordering::Relaxed);
    Ok(())
}

pub fn current_language() -> &'static str {
    LANGUAGES[CURRENT.load(Ordering::Relaxed)]
}

fn supported(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|l| **l == code).copied()
}

/// Idioma soportado del nombre de locale (es, es_CL, pt-BR, fr.UTF-8...); si no, "en".
pub fn system_language(locale_name: &str) -> &'static str {
    let chars: Vec<char> = locale_name.chars().collect();
    let code: String = chars.iter().take(2).collect::<String>().to_lowercase();
    match supported(&code) {
        Some(lang) if chars.len() == 2 || chars.get(2).is_some_and(|c| "_-.".contains(*c)) => lang,
        _ => "en",
    }
}

/// Idioma sin UI (consola): JASE_LANG o LC_ALL/LC_MESSAGES/LANG; si no, "en".
pub fn environment_language(environ: &impl Fn(&str) -> Option<String>) -> &'static str {
    if let Some(lang) = environ("JASE_LANG").as_deref().and_then(supported) {
        return lang;
    }
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(value) = environ(name).filter(|v| !v.is_empty()) {
            return system_language(&value);
        }
    }
    "en"
}

/// `setting` es "auto" o un código de LANGUAGES (la config ya lo validó).
pub fn resolve_language(setting: &str, locale_name: &str) -> &'static str {
    if setting == "auto" {
        return system_language(locale_name);
    }
    supported(setting).unwrap_or("es")
}

/// El catálogo de un idioma (no hay para el español, que es la clave).
pub fn catalog(code: &str) -> Option<&'static Catalog> {
    CATALOGS.get(code)
}

/// Búsqueda con respaldo: catálogo del idioma → inglés → la clave en español.
pub fn lookup_in<'a>(text: &'a str, catalog: Option<&'a Catalog>, english: &'a Catalog) -> &'a str {
    catalog
        .and_then(|c| c.get(text))
        .or_else(|| english.get(text))
        .map_or(text, String::as_str)
}

fn lookup(text: &str) -> &str {
    let lang = current_language();
    if lang == "es" {
        return text;
    }
    match catalog("en") {
        Some(en) => lookup_in(text, catalog(lang), en),
        None => text,
    }
}

/// `str.format` con argumentos con nombre: `{name}` → valor y `{{`/`}}` → llave. Un campo
/// sin argumento queda tal cual (Python lanzaría KeyError; nunca se tumba la UI por esto).
fn render(text: &str, args: Args) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        if tail.starts_with('{')
            && let Some(end) = tail.find('}')
            && let Some((_, value)) = args.iter().find(|(k, _)| *k == &tail[1..end])
        {
            let _ = write!(out, "{value}");
            rest = &tail[end + 1..];
            continue;
        }
        out.push_str(&tail[..1]);
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

pub fn tr(text: &str) -> String {
    lookup(text).to_owned()
}

pub fn tr_fmt(text: &str, args: Args) -> String {
    if args.is_empty() {
        tr(text)
    } else {
        render(lookup(text), args)
    }
}

pub fn tr_n(singular: &str, plural: &str, n: i64, args: Args) -> String {
    let mut all: Vec<(&str, &dyn Display)> = vec![("n", &n)];
    all.extend_from_slice(args);
    render(lookup(if n == 1 { singular } else { plural }), &all)
}
