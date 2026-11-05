"""Verificador de la documentación: enlaces rotos, rutas inexistentes, datos personales
y etapas de la memoria. Lo corren los tests (tests/test_check_docs.py).

Uso: python scripts/check_docs.py  (sale con 1 si encuentra algo)

- Enlaces relativos y rutas del repo entre comillas invertidas: README*.md, AGENTS.md y
  docs/ (salvo docs/superpowers/, notas locales). Solo se revisan las rutas cuyo primer
  segmento es una carpeta del repo (`src/…`, `docs/…`): `System\\X.exe`, `code/…` o las que
  llevan un marcador `<…>` no.
- Datos personales (rutas de unidad, /home, /mnt, IPv4 fuera de los rangos de
  documentación, claves hex): solo AGENTS.md y docs/. Los README citan masters reales.
- docs/ai/PROJECT_MEMORY.md: etapas `## AAAA-MM-DD — tema` con 3 viñetas como mucho.
- En un repo git, «existe» = está versionado (`build/` o `dist/` locales no cuentan). Se
  revisan también enlaces con título y de referencia (`[x]: ruta`), y las rutas
  relativas al paquete (`ui/x.py` → `src/jase/ui/x.py`).
"""
from __future__ import annotations

import fnmatch
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXCLUDED = ("docs/superpowers/",)
MEMORY = "docs/ai/PROJECT_MEMORY.md"

_LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
_REF_LINK = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)")
_TICKS = re.compile(r"`([^`\n]+)`")
_SCHEME = re.compile(r"^[a-z][a-z0-9+.-]*:|^#")
_LINE_SUFFIX = re.compile(r":\d+(-\d+)?$")
_PERSONAL = (
    (re.compile(r"(?<![%\w>])[A-Za-z]:[\\/]"), "ruta de unidad"),
    (re.compile(r"/home/"), "ruta /home"),
    (re.compile(r"/mnt/"), "ruta /mnt"),
    (re.compile(r"(?<![0-9a-zA-Z])[0-9a-fA-F]{32,}(?![0-9a-zA-Z])"), "posible clave"),
)
_IPV4 = re.compile(r"(?<![\d.])(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?!\d|\.\d)")
_DOC_RANGES = ("192.0.2.", "198.51.100.", "203.0.113.")
_SPECIAL_IPS = {"127.0.0.1", "0.0.0.0", "255.255.255.255"}
_STAGE = re.compile(r"^## \d{4}-\d{2}-\d{2} — \S")


def _docs(root: Path) -> list[Path]:
    files = [root / n for n in ("README.md", "README.es.md", "AGENTS.md")]
    files += sorted((root / "docs").rglob("*.md")) if (root / "docs").is_dir() else []
    return [f for f in files if f.is_file() and not _excluded(root, f)]


def _excluded(root: Path, path: Path) -> bool:
    rel = path.relative_to(root).as_posix()
    return any(rel.startswith(e) for e in EXCLUDED)


def _personal_scope(root: Path, path: Path) -> bool:
    rel = path.relative_to(root).as_posix()
    return rel == "AGENTS.md" or rel.startswith("docs/")


class _Repo:
    """Qué existe en el repo: los archivos versionados (git ls-files) si `root` es la raíz
    de un repo git, para que `build/` o `dist/` locales no cuenten; si no, el disco."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.tracked = _tracked(root)
        # Raíces del disco (también build/, dist/): una ruta ahí se revisa, y sin
        # versionar no existe.
        names = {p.name for p in root.iterdir()}
        self.roots = {n for n in names if not n.startswith(".") or n == ".github"}
        package = root / "src" / "jase"
        self.package_dirs = ({p.name for p in package.iterdir() if p.is_dir()}
                             if package.is_dir() else set())

    def exists(self, rel: str) -> bool:
        if self.tracked is None:
            return any(self.root.glob(rel)) if "*" in rel else (self.root / rel).exists()
        if "*" in rel:
            return any(fnmatch.fnmatch(f, rel) for f in self.tracked)
        return rel in self.tracked or any(f.startswith(rel + "/") for f in self.tracked)


def _tracked(root: Path) -> set[str] | None:
    try:
        top = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=root,
                             capture_output=True, text=True, check=True).stdout.strip()
        if Path(top).resolve() != root.resolve():
            return None
        out = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True,
                             check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    return set(out.splitlines())


def _check_links(path: Path, line: str) -> list[str]:
    out = []
    for target in _LINK.findall(line) + _REF_LINK.findall(line):
        if _SCHEME.match(target):
            continue
        file_part = target.split("#", 1)[0]
        if file_part and not (path.parent / file_part).exists():
            out.append(f"enlace roto: {target}")
    return out


def _check_paths(repo: _Repo, line: str) -> list[str]:
    out = []
    for token in _TICKS.findall(line):
        token = _LINE_SUFFIX.sub("", token.strip()).rstrip("/")
        if (" " in token or "<" in token or "/" not in token
                or any((token + "/").startswith(e) for e in EXCLUDED)):
            continue
        first = token.split("/", 1)[0]
        if first in repo.roots:
            ok = repo.exists(token)
        elif first in repo.package_dirs:  # `ui/x.py`: relativa a src/jase/
            ok = repo.exists(f"src/jase/{token}")
        else:
            continue
        if not ok:
            out.append(f"ruta inexistente: {token}")
    return out


def _check_personal(line: str) -> list[str]:
    out = [f"dato personal ({what})" for rx, what in _PERSONAL if rx.search(line)]
    for m in _IPV4.finditer(line):
        ip = m.group(0)
        if all(int(o) <= 255 for o in m.groups()) and ip not in _SPECIAL_IPS \
                and not ip.startswith(_DOC_RANGES):
            out.append(f"dato personal (IPv4 {ip}; usar 192.0.2.x, 198.51.100.x o 203.0.113.x)")
    return out


def _check_memory(text: str) -> list[tuple[int, str]]:
    out: list[tuple[int, str]] = []
    stage, start, bullets = None, 0, 0

    def close() -> None:
        if stage is not None and bullets > 3:
            out.append((start, f"etapa con {bullets} viñetas (máximo 3): {stage}"))

    for lineno, line in enumerate(text.splitlines(), 1):
        if line.startswith("## "):
            close()
            stage, start, bullets = line[3:].strip(), lineno, 0
            if not _STAGE.match(line):
                out.append((lineno, "etapa sin el formato «## AAAA-MM-DD — tema»"))
        elif stage is not None and line[:2] in ("- ", "* "):
            bullets += 1
    close()
    return out


def check(root: Path = ROOT) -> list[str]:
    """Problemas encontrados, como «ruta:línea: mensaje»; vacío = todo bien."""
    root = Path(root)
    repo = _Repo(root)
    problems = []
    for path in _docs(root):
        rel = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        personal = _personal_scope(root, path)
        for lineno, line in enumerate(text.splitlines(), 1):
            found = _check_links(path, line) + _check_paths(repo, line)
            if personal:
                found += _check_personal(line)
            problems += [f"{rel}:{lineno}: {msg}" for msg in found]
        if rel == MEMORY:
            problems += [f"{rel}:{n}: {msg}" for n, msg in _check_memory(text)]
    return problems


def main() -> int:
    problems = check()
    for problem in problems:
        print(problem)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
