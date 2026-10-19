import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _load():
    spec = importlib.util.spec_from_file_location("collect_licenses", ROOT / "scripts" / "collect_licenses.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_collect_licenses_writes_third_party(tmp_path):
    mod = _load()
    assert mod.main([str(tmp_path)]) == 0
    text = (tmp_path / "THIRD-PARTY.txt").read_text(encoding="utf-8")
    for name in ("keyring", "PySide6", "shiboken6", "jaraco.classes", "more-itertools"):
        assert name.lower() in text.lower(), name
    assert "pillow" in text.lower()
    # Texto completo de licencia, no solo el nombre.
    assert "Permission is hereby granted" in text
    assert "LGPL-3.0" in text
    cp = (tmp_path / "CPython-LICENSE.txt").read_text(encoding="utf-8")
    assert "PYTHON SOFTWARE FOUNDATION" in cp.upper()


def test_collect_licenses_fails_without_cpython_license(monkeypatch, tmp_path):
    import pytest

    mod = _load()
    monkeypatch.setattr(sys, "base_prefix", str(tmp_path))
    with pytest.raises(SystemExit):
        mod.find_cpython_license()
