import ast
import string
from pathlib import Path

import pytest

from jase import i18n
from jase.locales import en

SRC = Path(__file__).resolve().parent.parent / "src" / "jase"


@pytest.fixture
def english(monkeypatch):
    monkeypatch.setattr(en, "MESSAGES", {
        "Hola {name}": "Hello {name}",
        "{n} servidor": "{n} server",
        "{n} servidores": "{n} servers",
    })
    i18n.set_language("en")


def test_spanish_is_identity():
    assert i18n.tr("Hola") == "Hola"
    assert i18n.tr("Hola {name}", name="Ana") == "Hola Ana"


def test_english_lookup_and_format(english):
    assert i18n.current_language() == "en"
    assert i18n.tr("Hola {name}", name="Ana") == "Hello Ana"


def test_missing_english_falls_back_to_spanish(english):
    assert i18n.tr("Sin traducir") == "Sin traducir"
    assert i18n.tr("Falta {x}", x=1) == "Falta 1"


def test_tr_n_singular_plural_both_languages(english):
    assert i18n.tr_n("{n} servidor", "{n} servidores", 1) == "1 server"
    assert i18n.tr_n("{n} servidor", "{n} servidores", 0) == "0 servers"
    assert i18n.tr_n("{n} servidor", "{n} servidores", 5) == "5 servers"
    i18n.set_language("es")
    assert i18n.tr_n("{n} servidor", "{n} servidores", 1) == "1 servidor"
    assert i18n.tr_n("{n} servidor", "{n} servidores", 2) == "2 servidores"


@pytest.mark.parametrize("name,expected", [
    ("es", "es"), ("es_CL", "es"), ("es-419", "es"), ("es_ES.UTF-8", "es"),
    ("en", "en"), ("en_US", "en"), ("", "en"), ("C", "en"),
    ("pt_BR", "pt"), ("pt-PT", "pt"), ("fr_FR", "fr"), ("de_DE.UTF-8", "de"), ("it", "it"),
    ("ru_RU", "en"), ("est_EE", "en"), ("ess", "en"), ("french", "en"),
])
def test_system_language_rule(name, expected):
    assert i18n.system_language(name) == expected


def test_resolve_language():
    assert i18n.resolve_language("auto", "es_CL") == "es"
    assert i18n.resolve_language("auto", "de_DE") == "de"
    assert i18n.resolve_language("auto", "ru_RU") == "en"
    assert i18n.resolve_language("es", "en_US") == "es"
    assert i18n.resolve_language("en", "es_CL") == "en"


def test_set_language_rejects_unknown():
    with pytest.raises(ValueError):
        i18n.set_language("ru")
    assert i18n.current_language() == "es"


def _fields(text):
    return {f for _, f, _, _ in string.Formatter().parse(text) if f is not None}


def test_catalog_format_fields_match():
    for es, english_text in en.MESSAGES.items():
        assert _fields(es) == _fields(english_text), es


def _call_name(node):
    f = node.func
    if isinstance(f, ast.Name):
        return f.id
    if isinstance(f, ast.Attribute):
        return f.attr
    return None


def _collect_keys():
    keys, problems = {}, []
    for path in sorted(SRC.rglob("*.py")):
        rel = path.relative_to(SRC.parent)
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call):
                continue
            name = _call_name(node)
            if name not in ("tr", "tr_n"):
                continue
            count = 1 if name == "tr" else 2
            args = node.args[:count]
            if len(args) < count or not all(
                    isinstance(a, ast.Constant) and isinstance(a.value, str) for a in args):
                problems.append(f"{rel}:{node.lineno}: {name}() necesita literales str "
                                f"como primer(os) {count} argumento(s)")
                continue
            for a in args:
                keys.setdefault(a.value, f"{rel}:{node.lineno}")
            if any(k.arg is None for k in node.keywords):
                continue  # **kwargs: no se puede comprobar de forma estática
            given = {k.arg for k in node.keywords}
            for a in args:
                expected = _fields(a.value) - ({"n"} if name == "tr_n" else set())
                if given != expected:
                    problems.append(f"{rel}:{node.lineno}: {name}() pasa {sorted(given)} "
                                    f"pero {a.value!r} usa {sorted(expected)}")
    return keys, problems


def test_i18n_imports_are_not_aliased():
    bad = []
    for path in sorted(SRC.rglob("*.py")):
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        for node in ast.walk(tree):
            if isinstance(node, ast.ImportFrom) and (node.module or "").endswith("i18n"):
                bad += [f"{path.relative_to(SRC.parent)}:{node.lineno}: import con alias de {a.name}"
                        for a in node.names if a.name in ("tr", "tr_n") and a.asname]
            elif isinstance(node, ast.Import):
                bad += [f"{path.relative_to(SRC.parent)}:{node.lineno}: alias de {a.name}"
                        for a in node.names if a.name.endswith("i18n") and a.asname]
    assert not bad, "\n".join(bad)


def test_every_tr_literal_has_english():
    keys, problems = _collect_keys()
    assert not problems, "\n".join(problems)
    missing = [f"{where}: falta en locales/en.py: {k!r}"
               for k, where in keys.items() if k not in en.MESSAGES]
    assert not missing, "\n".join(missing)
    orphans = [f"clave huérfana en locales/en.py: {k!r}"
               for k in en.MESSAGES if k not in keys]
    assert not orphans, "\n".join(orphans)


OTHER_CATALOGS = [c for c in i18n.LANGUAGES if c not in ("es", "en")]


@pytest.mark.parametrize("code", OTHER_CATALOGS)
def test_every_language_translates_every_english_key(code):
    """Cada idioma tiene las mismas claves que el inglés y con los mismos campos."""
    messages = i18n._CATALOGS[code].MESSAGES
    assert set(messages) == set(en.MESSAGES), set(messages) ^ set(en.MESSAGES)
    bad = [k for k, v in messages.items() if _fields(k) != _fields(v) or not v.strip()]
    assert not bad, bad


def test_other_language_falls_back_to_english_then_spanish(monkeypatch):
    from jase.locales import fr
    monkeypatch.setattr(fr, "MESSAGES", {"Hola": "Bonjour"})
    monkeypatch.setattr(en, "MESSAGES", {"Hola": "Hello", "Adiós": "Bye"})
    i18n.set_language("fr")
    try:
        assert (i18n.tr("Hola"), i18n.tr("Adiós"), i18n.tr("Nada")) == ("Bonjour", "Bye", "Nada")
    finally:
        i18n.set_language("es")


@pytest.mark.parametrize("code", OTHER_CATALOGS + ["en"])
def test_country_names_exist_for_every_language(code):
    from jase import geo
    i18n.set_language(code)
    try:
        assert set(geo._LOCALIZED[code]) == set(geo.COUNTRY_NAMES)
        assert geo.country_name("cl") != "CL"
    finally:
        i18n.set_language("es")
