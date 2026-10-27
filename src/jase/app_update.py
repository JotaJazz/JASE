"""Aviso de versión nueva de JASE desde GitHub Releases. Núcleo sin Qt.

Se usa el listado y no `/releases/latest`, que ignora las prerelease (hasta la 0.12 todas
lo eran): en el listado cuentan las releases y las prerelease, y los borradores no.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import urllib.error
import urllib.request
from collections.abc import Callable
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

DOWNLOAD_PREFIX = "https://github.com/JotaJazz/JASE/releases/download/"
MAX_INSTALLER_BYTES = 300 * 1024 * 1024
CHUNK_BYTES = 64 * 1024

_VERSION_RE = re.compile(r"v?(\d+(?:\.\d+)*)")


_SHA256_RE = re.compile(r"sha256:([0-9a-fA-F]{64})")


@dataclass(frozen=True)
class Asset:
    name: str
    url: str
    size: int
    sha256: str


@dataclass(frozen=True)
class Release:
    version: str
    url: str
    installer: Asset | None = None
    portable: Asset | None = None  # JASE-<versión>-win64.zip (actualizar el portátil)


# Códigos de error de la descarga del instalador (la UI los traduce; nunca texto aquí).
DOWNLOAD_SIZE = "size"  # el archivo terminó antes del tamaño declarado
DOWNLOAD_TOO_MUCH = "too_much"  # llegaron más bytes de los declarados
DOWNLOAD_HASH = "hash"  # el SHA-256 no coincide
DOWNLOAD_INSECURE = "insecure"  # URL o redirección que no es https
DOWNLOAD_CANCELLED = "cancelled"
DOWNLOAD_NETWORK = "network"  # red o disco


class DownloadError(ValueError):
    """Fallo de la descarga del instalador; `code` es uno de los DOWNLOAD_*."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(detail or code)
        self.code = code


class DownloadCancelled(DownloadError):
    """El usuario canceló la descarga."""

    def __init__(self) -> None:
        super().__init__(DOWNLOAD_CANCELLED)


class InsecureRedirect(urllib.error.URLError):
    """Redirección a una URL que no es https."""


def _installer_asset(item: dict, version: str) -> Asset | None:
    """El asset `JASE-<versión>-setup.exe` si todos sus campos son válidos; si no, None."""
    return _asset(item, f"JASE-{version}-setup.exe")


def _portable_asset(item: dict, version: str) -> Asset | None:
    return _asset(item, f"JASE-{version}-win64.zip")


def _asset(item: dict, wanted: str) -> Asset | None:
    assets = item.get("assets")
    if not isinstance(assets, list):
        return None
    for a in assets:
        if not isinstance(a, dict) or a.get("name") != wanted:
            continue
        url, size, digest = a.get("browser_download_url"), a.get("size"), a.get("digest")
        if not isinstance(url, str) or not url.startswith(DOWNLOAD_PREFIX):
            return None
        if type(size) is not int or not 0 < size <= MAX_INSTALLER_BYTES:
            return None
        m = _SHA256_RE.fullmatch(digest) if isinstance(digest, str) else None
        if m is None:
            return None
        return Asset(wanted, url, size, m.group(1).lower())
    return None


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
            ver = ".".join(map(str, version))
            best = (version, Release(ver, url, _installer_asset(item, ver),
                                     _portable_asset(item, ver)))
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
    return check_detailed(cache_dir, now, current, fetch)[0]


def check_detailed(cache_dir: Path, now: float, current: str,
                   fetch=default_fetch) -> tuple[Release | None, bool]:
    """(release más nueva o None, ¿se pudo consultar?). Nunca lanza."""
    cache_dir = Path(cache_dir)
    try:
        data = fetch(RELEASES_URL)
        if len(data) > MAX_BYTES:
            raise ValueError("respuesta demasiado grande")
        release = newest_release(data)
    except Exception:  # red, tamaño o contenido inválido: se reintenta tras 1 h
        _write_meta(cache_dir, now, False)
        return None, False
    _write_meta(cache_dir, now, True)
    mine = parse_version(current)
    if release is None or mine is None:
        return None, True
    theirs = parse_version(release.version)
    return (release if theirs is not None and theirs > mine else None), True


class _HttpsOnly(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if not newurl.lower().startswith("https://"):
            raise InsecureRedirect("redirección no https")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


class _Stream:
    """Respuesta HTTP con `read(n)` y `close()`."""

    def __init__(self, resp) -> None:
        self._resp = resp

    def read(self, n: int) -> bytes:
        return self._resp.read(n)

    def close(self) -> None:
        self._resp.close()


def default_fetch_stream(url: str):
    """Abre `url` (solo https, también en cada redirección) para leerla por trozos."""
    if not url.lower().startswith("https://"):
        raise DownloadError(DOWNLOAD_INSECURE, "solo https")
    opener = urllib.request.build_opener(_HttpsOnly)
    req = urllib.request.Request(url, headers={"User-Agent": "JASE"})
    return _Stream(opener.open(req, timeout=FETCH_TIMEOUT_S))


def download_installer(asset: Asset, dest_dir: Path, fetch_stream=default_fetch_stream,
                       progress: Callable[[int, int], None] = lambda done, total: None,
                       cancelled: Callable[[], bool] = lambda: False) -> Path:
    """Descarga el instalador, verifica tamaño y SHA-256 y lo deja en `dest_dir`.

    Ante cualquier fallo (red, tamaño, hash, cancelación) borra el temporal y lanza
    DownloadError con su código.
    """
    dest_dir = Path(dest_dir)
    dest_dir.mkdir(parents=True, exist_ok=True)
    final = dest_dir / asset.name
    tmp = dest_dir / (asset.name + ".part")
    try:
        digest = hashlib.sha256()
        done = 0
        stream = fetch_stream(asset.url)
        try:
            with open(tmp, "wb") as out:
                while True:
                    if cancelled():
                        raise DownloadCancelled()
                    chunk = stream.read(CHUNK_BYTES)
                    if not chunk:
                        break
                    done += len(chunk)
                    if done > asset.size:
                        raise DownloadError(DOWNLOAD_TOO_MUCH)
                    digest.update(chunk)
                    out.write(chunk)
                    progress(done, asset.size)
        finally:
            stream.close()
        if done != asset.size:
            raise DownloadError(DOWNLOAD_SIZE)
        if digest.hexdigest() != asset.sha256:
            raise DownloadError(DOWNLOAD_HASH)
        os.replace(tmp, final)
        return final
    except DownloadError:
        tmp.unlink(missing_ok=True)
        raise
    except InsecureRedirect as e:
        tmp.unlink(missing_ok=True)
        raise DownloadError(DOWNLOAD_INSECURE, str(e)) from e
    except Exception as e:  # red, disco…
        tmp.unlink(missing_ok=True)
        raise DownloadError(DOWNLOAD_NETWORK, str(e)) from e
