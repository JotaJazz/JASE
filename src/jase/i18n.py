"""Traducción sin dependencias de Qt.

El español es el idioma de origen: las claves son los propios textos en español.
En otro idioma se busca en ``locales.<idioma>.MESSAGES``; si falta, en inglés y, si
tampoco está, se devuelve el español.

Llaves: solo se interpretan (y hay que duplicarlas como ``{{``/``}}``) cuando la
llamada formatea, es decir, cuando pasa argumentos con nombre o es ``tr_n``.
Importar siempre como ``from jase.i18n import tr, tr_n`` (sin alias).
"""
from __future__ import annotations

from collections.abc import Mapping
from types import ModuleType

from jase.locales import de, en, fr, it, pt

LANGUAGES = ("es", "en", "pt", "fr", "de", "it")
# Cada idioma con su propio nombre (así se muestra en Ajustes).
LANGUAGE_NAMES = {"es": "Español", "en": "English", "pt": "Português (Brasil)",
                  "fr": "Français", "de": "Deutsch", "it": "Italiano"}
# Módulos (no sus diccionarios): los tests reemplazan ``MESSAGES``.
_CATALOGS: dict[str, ModuleType] = {"en": en, "pt": pt, "fr": fr, "de": de, "it": it}

_language = "es"


def set_language(code: str) -> None:
    global _language
    if code not in LANGUAGES:
        raise ValueError(f"idioma no soportado: {code!r}")
    _language = code


def current_language() -> str:
    return _language


def system_language(locale_name: str) -> str:
    """Idioma soportado del nombre de locale (es, es_CL, pt-BR, fr.UTF-8...); si no, "en"."""
    code = locale_name[:2].lower()
    if code in LANGUAGES and (len(locale_name) == 2 or locale_name[2] in "_-."):
        return code
    return "en"


def environment_language(environ: Mapping[str, str]) -> str:
    """Idioma sin Qt (consola): JASE_LANG o LC_ALL/LC_MESSAGES/LANG; si no, "en"."""
    if environ.get("JASE_LANG") in LANGUAGES:
        return environ["JASE_LANG"]
    for name in ("LC_ALL", "LC_MESSAGES", "LANG"):
        if environ.get(name):
            return system_language(environ[name])
    return "en"


def resolve_language(setting: str, locale_name: str) -> str:
    if setting == "auto":
        return system_language(locale_name)
    return setting


def _render(text: str, fmt: dict) -> str:
    return text.format(**fmt) if fmt else text


def _lookup(text: str) -> str:
    if _language == "es":
        return text
    found = _CATALOGS[_language].MESSAGES.get(text)
    return found if found is not None else en.MESSAGES.get(text, text)


def tr(text: str, **fmt) -> str:
    return _render(_lookup(text), fmt)


def tr_n(singular: str, plural: str, n: int, **fmt) -> str:
    return _lookup(singular if n == 1 else plural).format(n=n, **fmt)
