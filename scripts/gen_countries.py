"""Genera src/jase/locales/countries_<idioma>.py (nombres de países) desde CLDR (Babel).

Uso: python scripts/gen_countries.py [pt fr de it]
Necesita `babel` (solo para generar; la app no lo usa): `pip install babel`. Las claves son
las de jase.geo.COUNTRY_NAMES; los códigos que CLDR no tiene o que no son países van en
``SPECIAL``. El inglés sale de Qt: scripts/gen_countries_en.py.
"""
from __future__ import annotations

import ast
import sys
from pathlib import Path

from babel import Locale

ROOT = Path(__file__).resolve().parent.parent
LOCALES = {"pt": Locale("pt", "BR"), "fr": Locale("fr"), "de": Locale("de"),
           "it": Locale("it")}
SPECIAL = {
    "pt": {"eu": "União Europeia", "pc": "Comunidade do Pacífico", "un": "Nações Unidas",
           "xx": "Desconhecido"},
    "fr": {"eu": "Union européenne", "pc": "Communauté du Pacifique", "un": "Nations Unies",
           "xx": "Inconnu"},
    "de": {"eu": "Europäische Union", "pc": "Pazifische Gemeinschaft",
           "un": "Vereinte Nationen", "xx": "Unbekannt"},
    "it": {"eu": "Unione europea", "pc": "Comunità del Pacifico", "un": "Nazioni Unite",
           "xx": "Sconosciuto"},
}
LANGUAGE_NAMES = {"pt": "portugués (Brasil)", "fr": "francés", "de": "alemán",
                  "it": "italiano"}


def spanish_codes() -> list[str]:
    """Claves de COUNTRY_NAMES leídas del fuente (sin importar jase: otro entorno)."""
    tree = ast.parse((ROOT / "src" / "jase" / "geo.py").read_text(encoding="utf-8"))
    for node in ast.walk(tree):
        if (isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name)
                and node.targets[0].id == "_NAMES_ES"):
            text = ast.literal_eval(node.value)
            return [e.split(" ", 1)[0] for line in text.splitlines() for e in line.split("|")]
    raise SystemExit("No se encontró _NAMES_ES en geo.py")


def main(langs: list[str]) -> None:
    codes = spanish_codes()
    for lang in langs:
        territories = LOCALES[lang].territories
        names = {}
        for code in codes:
            name = SPECIAL[lang].get(code) or territories.get(code.upper())
            if not name:
                raise SystemExit(f"{lang}: sin nombre para {code!r}; agrégalo a SPECIAL")
            names[code] = name
        var = f"COUNTRY_NAMES_{lang.upper()}"
        lines = [f'"""Nombres de países en {LANGUAGE_NAMES[lang]} '
                 '(generado por scripts/gen_countries.py)."""', "",
                 f"{var}: dict[str, str] = {{"]
        lines += [f'    "{c}": "{n}",' for c, n in sorted(names.items())]
        lines += ["}", ""]
        out = ROOT / "src" / "jase" / "locales" / f"countries_{lang}.py"
        out.write_text("\n".join(lines), encoding="utf-8")
        print(f"{out.relative_to(ROOT)}: {len(names)} países")


if __name__ == "__main__":
    main(sys.argv[1:] or list(LOCALES))
