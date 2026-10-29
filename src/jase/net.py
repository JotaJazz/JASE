"""Descargas HTTP pequeñas con plazo y tope de tamaño (sin Qt ni Pillow)."""
from __future__ import annotations

import logging
import time
import urllib.request

log = logging.getLogger(__name__)

FETCH_TIMEOUT = 5  # por operación de socket
FETCH_DEADLINE = 8  # total para leer la respuesta
FETCH_CHUNK = 64 * 1024
USER_AGENT = "JASE"
MAX_DOWNLOAD_BYTES = 2 * 1024 * 1024


def default_fetch(url: str, *, max_bytes: int = MAX_DOWNLOAD_BYTES,
                  deadline: float = FETCH_DEADLINE) -> bytes | None:
    """GET con timeout y User-Agent "JASE"; cualquier error (404, sin red…) → None.

    `deadline` es el plazo total (s) para leer la respuesta; más de `max_bytes` → None.
    """
    deadline = time.monotonic() + deadline
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    chunks: list[bytes] = []
    total = 0
    try:
        with urllib.request.urlopen(req, timeout=FETCH_TIMEOUT) as resp:
            while True:
                if time.monotonic() > deadline:
                    log.debug("Descarga demasiado lenta %s", url)
                    return None
                chunk = resp.read(FETCH_CHUNK)
                if not chunk:
                    break
                total += len(chunk)
                if total > max_bytes:
                    return None
                chunks.append(chunk)
    except Exception as exc:  # HTTPError, URLError, timeout, etc.
        log.debug("Descarga fallida %s: %s", url, exc)
        return None
    return b"".join(chunks)
