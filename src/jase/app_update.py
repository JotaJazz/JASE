"""Aviso de versión nueva de JASE desde GitHub Releases. Núcleo sin Qt.

Todas las releases del proyecto se publican como prerelease, así que `/releases/latest`
devuelve 404: se usa el listado, donde las prerelease cuentan y los borradores no.
"""
from __future__ import annotations

import json
import re
import urllib.request
from dataclasses import dataclass
from pathlib import Path

from jase import fetch_meta
from jase.masters import FETCH_TIMEOUT_S

RELEASES_URL = "https://api.github.com/repos/JotaJazz/JASE/releases?per_page=10"
RELEASE_PAGE_PREFIX = "https://github.com/JotaJazz/JASE/releases/"
CHECK_INTERVAL_S = 86400
RETRY_AFTER_FAILURE_S = 3600
META_FILENAME = "app_update.meta.json"
MAX_BYTES = 512 * 1024

_VERSION_RE = re.compile(r"v?(\d+(?:\.\d+)*)")


@dataclass(frozen=True)
class Release:
    version: str
    url: str


def parse_version(text: str) -> tuple[int, ...] | None:
    """«v0.10.0» o «0.10.0» → (0, 10, 0); cualquier otra cosa → None."""
    if not isinstance(text, str):
        return None
    m = _VERSION_RE.fullmatch(text)
    return tuple(int(p) for p in m.group(1).split(".")) if m else None


def newest_release(data: bytes) -> Release | None:
    """La release publicada (no borrador) de mayor versión con enlace a nuestra página."""
    try:
        raw = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, ValueError, RecursionError):
        return None
    if not isinstance(raw, list):
        return None
    best: tuple[tuple[int, ...], Release] | None = None
    for item in raw:
        if not isinstance(item, dict) or item.get("draft") is not False:
            continue
        tag, url = item.get("tag_name"), item.get("html_url")
        version = parse_version(tag) if isinstance(tag, str) else None
        if version is None or not isinstance(url, str) \
                or not url.startswith(RELEASE_PAGE_PREFIX):
            continue
        if best is None or version > best[0]:
            best = (version, Release(".".join(map(str, version)), url))
    return best[1] if best else None


def should_check(meta_path: Path, now: float) -> bool:
    return fetch_meta.should_check(meta_path, now, CHECK_INTERVAL_S, RETRY_AFTER_FAILURE_S)


def _write_meta(cache_dir: Path, now: float, ok: bool) -> None:
    fetch_meta.write_meta(cache_dir / META_FILENAME, now, ok)


def default_fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={
        "User-Agent": "JASE", "Accept": "application/vnd.github+json"})
    with urllib.request.urlopen(req, timeout=FETCH_TIMEOUT_S) as resp:
        return resp.read(MAX_BYTES + 1)


def check(cache_dir: Path, now: float, current: str, fetch=default_fetch) -> Release | None:
    """Devuelve la release solo si es más nueva que `current`. Nunca lanza."""
    cache_dir = Path(cache_dir)
    try:
        data = fetch(RELEASES_URL)
        if len(data) > MAX_BYTES:
            raise ValueError("respuesta demasiado grande")
        release = newest_release(data)
    except Exception:  # red, tamaño o contenido inválido: se reintenta tras 1 h
        _write_meta(cache_dir, now, False)
        return None
    _write_meta(cache_dir, now, True)
    mine = parse_version(current)
    if release is None or mine is None:
        return None
    theirs = parse_version(release.version)
    return release if theirs is not None and theirs > mine else None
