"""Traducción español/inglés sin dependencias de Qt.

El español es el idioma de origen: las claves son los propios textos en español.
En inglés se busca en ``locales.en.MESSAGES``; si falta, se devuelve el español.

Llaves: solo se interpretan (y hay que duplicarlas como ``{{``/``}}``) cuando la
llamada formatea, es decir, cuando pasa argumentos con nombre o es ``tr_n``.
Importar siempre como ``from jase.i18n import tr, tr_n`` (sin alias).
"""
from __future__ import annotations

from jase.locales import en

LANGUAGES = ("es", "en")

_language = "es"


def set_language(code: str) -> None:
    global _language
    if code not in LANGUAGES:
        raise ValueError(f"idioma no soportado: {code!r}")
    _language = code


def current_language() -> str:
    return _language


def system_language(locale_name: str) -> str:
    """"es" si el nombre de locale empieza por es (es, es_CL, es-419, es.UTF-8)."""
    if locale_name.startswith("es") and (
            len(locale_name) == 2 or locale_name[2] in "_-."):
        return "es"
    return "en"


def resolve_language(setting: str, locale_name: str) -> str:
    if setting == "auto":
        return system_language(locale_name)
    return setting


def _render(text: str, fmt: dict) -> str:
    return text.format(**fmt) if fmt else text


def tr(text: str, **fmt) -> str:
    if _language == "en":
        text = en.MESSAGES.get(text, text)
    return _render(text, fmt)


def tr_n(singular: str, plural: str, n: int, **fmt) -> str:
    key = singular if n == 1 else plural
    if _language == "en":
        key = en.MESSAGES.get(key, key)
    return key.format(n=n, **fmt)
