"""Meta de comprobaciones periódicas (último intento/éxito) y escritura atómica. Sin Qt."""
from __future__ import annotations

import contextlib
import json
import os
import tempfile
from pathlib import Path

MAX_META_BYTES = 256 * 1024 + 1


def _num(value: object) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return value


def read_meta(path: Path) -> dict:
    """Meta tolerante: {"attempt": float|None, "ok": bool, "success": float|None}."""
    result: dict = {"attempt": None, "ok": True, "success": None}
    try:
        with open(path, "rb") as fh:
            meta = json.loads(fh.read(MAX_META_BYTES).decode("utf-8"))
    except (OSError, ValueError, RecursionError):
        return result
    if not isinstance(meta, dict):
        return result
    legacy = _num(meta.get("last_check"))  # formato anterior: un solo instante
    success = _num(meta.get("last_success"))
    result["success"] = success if success is not None else legacy
    attempt = _num(meta.get("last_attempt"))
    result["attempt"] = attempt if attempt is not None else legacy
    result["ok"] = meta.get("last_attempt_ok") is not False
    return result


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as fh:
            fh.write(data)
        os.replace(tmp, path)
    except BaseException:
        with contextlib.suppress(OSError):
            os.unlink(tmp)
        raise


def write_meta(path: Path, now: float, ok: bool) -> None:
    previous = read_meta(path)
    data = {"last_attempt": now, "last_attempt_ok": ok,
            "last_success": now if ok else previous["success"]}
    with contextlib.suppress(OSError):
        atomic_write(path, json.dumps(data).encode())


def should_check(path: Path, now: float, interval_s: float, retry_s: float) -> bool:
    meta = read_meta(path)
    attempt, success = meta["attempt"], meta["success"]
    if not meta["ok"] and attempt is not None:
        return not (0 <= now - attempt < retry_s)  # reintento tras un fallo
    if success is None:
        return True
    return not (0 <= now - success < interval_s)  # reloj retrasado → revisar
