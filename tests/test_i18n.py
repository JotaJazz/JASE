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
    ("fr_FR", "en"), ("est_EE", "en"), ("ess", "en"),
])
def test_system_language_rule(name, expected):
    assert i18n.system_language(name) == expected


def test_resolve_language():
    assert i18n.resolve_language("auto", "es_CL") == "es"
    assert i18n.resolve_language("auto", "de_DE") == "en"
    assert i18n.resolve_language("es", "en_US") == "es"
    assert i18n.resolve_language("en", "es_CL") == "en"


def test_set_language_rejects_unknown():
    with pytest.raises(ValueError):
        i18n.set_language("fr")
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
    return keys, problems


def test_every_tr_literal_has_english():
    keys, problems = _collect_keys()
    assert not problems, "\n".join(problems)
    missing = [f"{where}: falta en locales/en.py: {k!r}"
               for k, where in keys.items() if k not in en.MESSAGES]
    assert not missing, "\n".join(missing)
    orphans = [f"clave huérfana en locales/en.py: {k!r}"
               for k in en.MESSAGES if k not in keys]
    assert not orphans, "\n".join(orphans)
