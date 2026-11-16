"""Exporta los catálogos de src/jase/locales/ a JSON para el núcleo en Rust.

Uso: python scripts/export_locales.py  (escribe rust/jase-core/locales/<código>.json)

Python está congelado en 1.3.0: solo hace falta si cambia un catálogo antes de retirarlo.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase.locales import de, en, fr, it, pt  # noqa: E402

OUT = ROOT / "rust" / "jase-core" / "locales"


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    for code, module in (("en", en), ("pt", pt), ("fr", fr), ("de", de), ("it", it)):
        text = json.dumps(module.MESSAGES, ensure_ascii=False, indent=1) + "\n"
        (OUT / f"{code}.json").write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
