//! Port de tests/test_i18n.py (los que no leen el código Python ni usan Qt).
use jase_core::i18n::{self, Catalog};
use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;

/// El idioma es global: los tests que lo cambian van de a uno.
static LOCK: Mutex<()> = Mutex::new(());

fn with_language<T>(code: &str, f: impl FnOnce() -> T) -> T {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_language(code).unwrap();
    let out = f();
    i18n::set_language("es").unwrap();
    out
}

#[test]
fn spanish_is_identity() {
    with_language("es", || {
        assert_eq!(i18n::tr("Hola"), "Hola");
        assert_eq!(i18n::tr_fmt("Hola {name}", &[("name", &"Ana")]), "Hola Ana");
    });
}

#[test]
fn english_lookup_and_format() {
    with_language("en", || {
        assert_eq!(i18n::current_language(), "en");
        assert_eq!(
            i18n::tr_fmt("{name}: el favorito no respondió", &[("name", &"Ana")]),
            "Ana: the favorite did not respond"
        );
        assert_eq!(i18n::tr("Servidor"), "Server");
    });
}

#[test]
fn missing_english_falls_back_to_spanish() {
    with_language("en", || {
        assert_eq!(i18n::tr("Sin traducir"), "Sin traducir");
        assert_eq!(i18n::tr_fmt("Falta {x}", &[("x", &1)]), "Falta 1");
    });
}

#[test]
fn tr_n_singular_plural_both_languages() {
    with_language("en", || {
        assert_eq!(
            i18n::tr_n("{n} servidor", "{n} servidores", 1, &[]),
            "1 server"
        );
        assert_eq!(
            i18n::tr_n("{n} servidor", "{n} servidores", 0, &[]),
            "0 servers"
        );
        assert_eq!(
            i18n::tr_n("{n} servidor", "{n} servidores", 5, &[]),
            "5 servers"
        );
    });
    with_language("es", || {
        assert_eq!(
            i18n::tr_n("{n} servidor", "{n} servidores", 1, &[]),
            "1 servidor"
        );
        assert_eq!(
            i18n::tr_n("{n} servidor", "{n} servidores", 2, &[]),
            "2 servidores"
        );
    });
}

#[test]
fn tr_without_args_keeps_braces() {
    with_language("es", || {
        assert_eq!(i18n::tr("{x} y {{y}}"), "{x} y {{y}}");
        assert_eq!(i18n::tr_fmt("{{lit}} {a}", &[("a", &"b")]), "{lit} b");
        assert_eq!(i18n::tr_fmt("{falta} {a}", &[("a", &"b")]), "{falta} b"); // Python: KeyError
    });
}

#[test]
fn system_language_rule() {
    let cases = [
        ("es", "es"),
        ("es_CL", "es"),
        ("es-419", "es"),
        ("es_ES.UTF-8", "es"),
        ("en", "en"),
        ("en_US", "en"),
        ("", "en"),
        ("C", "en"),
        ("pt_BR", "pt"),
        ("pt-PT", "pt"),
        ("fr_FR", "fr"),
        ("de_DE.UTF-8", "de"),
        ("it", "it"),
        ("ru_RU", "en"),
        ("est_EE", "en"),
        ("ess", "en"),
        ("french", "en"),
        ("ES_cl", "es"),
        ("é", "en"),
    ];
    for (name, expected) in cases {
        assert_eq!(i18n::system_language(name), expected, "{name:?}");
    }
}

#[test]
fn environment_language_cases() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |k: &str| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| (*v).to_owned())
        }
    };
    assert_eq!(
        i18n::environment_language(&env(&[("JASE_LANG", "fr"), ("LANG", "de_DE")])),
        "fr"
    );
    assert_eq!(
        i18n::environment_language(&env(&[("JASE_LANG", "ru"), ("LANG", "de_DE")])),
        "de"
    );
    assert_eq!(
        i18n::environment_language(&env(&[
            ("LC_ALL", ""),
            ("LC_MESSAGES", "pt_BR"),
            ("LANG", "de")
        ])),
        "pt"
    );
    assert_eq!(
        i18n::environment_language(&env(&[("LANG", "C.UTF-8")])),
        "en"
    );
    assert_eq!(i18n::environment_language(&env(&[])), "en");
}

#[test]
fn resolve_language() {
    assert_eq!(i18n::resolve_language("auto", "es_CL"), "es");
    assert_eq!(i18n::resolve_language("auto", "de_DE"), "de");
    assert_eq!(i18n::resolve_language("auto", "ru_RU"), "en");
    assert_eq!(i18n::resolve_language("es", "en_US"), "es");
    assert_eq!(i18n::resolve_language("en", "es_CL"), "en");
}

#[test]
fn set_language_rejects_unknown() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    assert!(i18n::set_language("ru").is_err());
    assert_eq!(i18n::current_language(), "es");
}

/// Campos `{x}` de una cadena de formato (`{{` no cuenta), como string.Formatter.
fn fields(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text.replace("{{", "").replace("}}", "");
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            break;
        };
        out.insert(rest[start + 1..start + len].to_owned());
        rest = rest[start + len + 1..].to_owned();
    }
    out
}

#[test]
fn catalog_format_fields_match() {
    for code in ["en", "pt", "fr", "de", "it"] {
        for (es, text) in i18n::catalog(code).unwrap() {
            assert_eq!(fields(es), fields(text), "{code}: {es}");
        }
    }
}

#[test]
fn every_language_translates_every_english_key() {
    let en: BTreeSet<&String> = i18n::catalog("en").unwrap().keys().collect();
    for code in ["pt", "fr", "de", "it"] {
        let cat = i18n::catalog(code).unwrap();
        assert_eq!(cat.keys().collect::<BTreeSet<_>>(), en, "{code}");
        assert!(cat.values().all(|v| !v.trim().is_empty()), "{code}");
    }
}

#[test]
fn other_language_falls_back_to_english_then_spanish() {
    let fr: Catalog = HashMap::from([("Hola".to_owned(), "Bonjour".to_owned())]);
    let en: Catalog = HashMap::from([
        ("Hola".to_owned(), "Hello".to_owned()),
        ("Adiós".to_owned(), "Bye".to_owned()),
    ]);
    let look = |t| i18n::lookup_in(t, Some(&fr), &en);
    assert_eq!(
        (look("Hola"), look("Adiós"), look("Nada")),
        ("Bonjour", "Bye", "Nada")
    );
}

#[test]
fn language_names() {
    assert_eq!(i18n::LANGUAGES, ["es", "en", "pt", "fr", "de", "it"]);
    assert_eq!(i18n::language_name("pt"), "Português (Brasil)");
}
