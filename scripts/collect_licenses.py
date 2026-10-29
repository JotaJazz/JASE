"""Genera los avisos de terceros para el zip: `licenses/THIRD-PARTY.txt` + LICENSE de CPython.

Recorre la clausura de dependencias de ejecución de `jase` (metadata instalada) y escribe
nombre, versión, licencia y texto completo de cada distribución. Falla si falta algo.
Uso: python scripts/collect_licenses.py <directorio-de-salida>
"""
from __future__ import annotations

import re
import shutil
import sys
from importlib import metadata
from pathlib import Path

from packaging.requirements import Requirement

LICENSE_FILE = re.compile(r"(^|/)(LICEN[CS]E|COPYING|NOTICE|AUTHORS)[^/]*$", re.I)


# Las ruedas de Qt for Python no traen archivo de licencia: el texto LGPL/GPL va aparte.
QT_FAMILY = ("pyside6", "shiboken6")
QT_NOTE = ("Licensed under LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only (distributed here under\n"
           "the LGPL-3.0). The full license texts are in PySide6-LGPL-3.0.txt in this folder.")


# Código portado dentro de JASE (no es una distribución instalada).
VENDORED = (
    ("Skulltag/Zandronum launcher Huffman tree (jase/protocols/huffman.py)", "MIT", """\
Copyright 2009 Timothy Landers
email: code.vortexcortex@gmail.com

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE."""),
)


def _norm(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def runtime_closure(root: str = "jase",
                    root_extras: tuple[str, ...] = ("gui",)) -> list[metadata.Distribution]:
    """Distribuciones requeridas por `root` con sus `root_extras` (la app empaquetada es
    `jase[gui]`); las dependencias de las dependencias, sin extras. Los marcadores se
    evalúan para el entorno actual."""
    seen: dict[str, metadata.Distribution] = {}
    todo: list[tuple[str, tuple[str, ...]]] = [(root, ("", *root_extras))]
    while todo:
        raw, extras = todo.pop()
        name = _norm(raw)
        if name in seen:
            continue
        try:
            dist = metadata.distribution(name)
        except metadata.PackageNotFoundError:
            continue  # marcador no aplicable o dependencia opcional ausente
        seen[name] = dist
        for line in dist.requires or []:
            req = Requirement(line)
            if req.marker is None or any(req.marker.evaluate({"extra": e}) for e in extras):
                todo.append((req.name, ("",)))
    seen.pop(_norm(root), None)
    return sorted(seen.values(), key=lambda d: _norm(d.metadata["Name"]))


def _license_label(dist: metadata.Distribution) -> str:
    md = dist.metadata
    label = md.get("License-Expression") or md.get("License") or ""
    if not label or len(label) > 100 or "\n" in label:
        classes = [c.split("::")[-1].strip() for c in md.get_all("Classifier") or []
                   if c.startswith("License ::")]
        label = ", ".join(classes) or (label.splitlines()[0] if label else "desconocida")
    return label


def _license_texts(dist: metadata.Distribution) -> list[tuple[str, str]]:
    out = []
    for f in dist.files or []:
        if f.parent.name.endswith(".dist-info") or ".dist-info/" in f.as_posix():
            if LICENSE_FILE.search(f.as_posix()):
                try:
                    data = Path(str(f.locate())).read_bytes()
                except OSError:
                    continue
                out.append((f.name, data.decode("utf-8", errors="replace")))
    return out


def build_notice() -> str:
    parts = ["Third-party software bundled with JASE\n"
             "======================================\n"]
    for dist in runtime_closure():
        name, version = dist.metadata["Name"], dist.version
        texts = _license_texts(dist)
        if not texts and _norm(name).startswith(QT_FAMILY):
            texts = [("PySide6-LGPL-3.0.txt", QT_NOTE)]
        if not texts:
            raise SystemExit(f"{name} {version}: no se encontró texto de licencia")
        parts.append(
            f"\n{'=' * 78}\n{name} {version}\nLicense: {_license_label(dist)}\n{'=' * 78}\n")
        for fname, text in texts:
            parts.append(f"\n--- {fname} ---\n{text.strip()}\n")
    for title, label, text in VENDORED:
        parts.append(f"\n{'=' * 78}\n{title}\nLicense: {label}\n{'=' * 78}\n\n{text}\n")
    return "".join(parts)


def find_cpython_license() -> Path:
    base = Path(sys.base_prefix)
    pyver = f"python{sys.version_info.major}.{sys.version_info.minor}"
    candidates = [base / "LICENSE.txt", base / "lib" / pyver / "LICENSE.txt"]
    for c in candidates:
        if c.is_file():
            return c
    raise SystemExit("No se encontró LICENSE.txt de CPython en " + ", ".join(map(str, candidates)))


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print(__doc__)
        return 2
    out = Path(argv[0])
    out.mkdir(parents=True, exist_ok=True)
    (out / "THIRD-PARTY.txt").write_text(build_notice(), encoding="utf-8")
    shutil.copyfile(find_cpython_license(), out / "CPython-LICENSE.txt")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
