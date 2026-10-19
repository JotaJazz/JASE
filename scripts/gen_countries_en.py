"""Genera src/jase/locales/countries_en.py (nombres de países en inglés) desde Qt.

Uso: .venv/bin/python scripts/gen_countries_en.py
Las claves son las de jase.geo.COUNTRY_NAMES; ``SHORT`` fuerza nombres cortos y comunes.
"""
from pathlib import Path

from PySide6.QtCore import QLocale

from jase.geo import COUNTRY_NAMES

SHORT = {
    "us": "United States", "gb": "United Kingdom", "ru": "Russia", "kr": "South Korea",
    "kp": "North Korea", "cz": "Czechia", "tw": "Taiwan", "hk": "Hong Kong",
    "mo": "Macau", "vn": "Vietnam", "ir": "Iran", "sy": "Syria", "bo": "Bolivia",
    "ve": "Venezuela", "tz": "Tanzania", "md": "Moldova", "la": "Laos", "mk": "North Macedonia",
    "ps": "Palestine", "ci": "Ivory Coast", "cd": "DR Congo", "cg": "Congo",
    "fm": "Micronesia", "bn": "Brunei", "cv": "Cape Verde", "sz": "Eswatini",
    "va": "Vatican City", "tr": "Turkey", "mm": "Myanmar", "pc": "Pacific Community", "un": "United Nations", "xx": "Unknown", "gs": "South Georgia", "io": "British Indian Ocean Territory",
}

OUT = Path(__file__).resolve().parent.parent / "src" / "jase" / "locales" / "countries_en.py"
english = QLocale("en")
names = {}
for code in COUNTRY_NAMES:
    territory = QLocale.codeToTerritory(code.upper())
    qt_name = english.territoryToString(territory)
    names[code] = SHORT.get(code) or (
        qt_name if territory != QLocale.AnyTerritory else code.upper())
lines = ['"""Nombres de países en inglés (generado por scripts/gen_countries_en.py)."""', "",
         "COUNTRY_NAMES_EN: dict[str, str] = {"]
lines += [f'    "{c}": "{n}",' for c, n in sorted(names.items())]
lines += ["}", ""]
OUT.write_text("\n".join(lines), encoding="utf-8")
