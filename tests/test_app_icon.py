import tomllib
from fnmatch import fnmatch
from pathlib import Path

from PIL import Image
from PySide6.QtCore import QSize

from jase import __main__ as entry
from jase.config import Config
from jase.ui import icons

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "src" / "jase" / "assets"
SIZES = (16, 24, 32, 48, 64, 128, 256)


def test_app_icon_loads(qapp):
    icon = icons.app_icon()
    assert not icon.isNull()
    sizes = {(s.width(), s.height()) for s in icon.availableSizes()}
    assert (16, 16) in sizes and (256, 256) in sizes


def test_app_icon_small_variant_differs(qapp):
    icon = icons.app_icon()
    small = icon.pixmap(QSize(16, 16)).toImage()
    big_scaled = icon.pixmap(QSize(32, 32)).toImage().scaled(16, 16)
    assert small != big_scaled


def test_app_icon_in_package_data():
    cfg = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    patterns = cfg["tool"]["setuptools"]["package-data"]["jase"]
    for name in ("app_icon.svg", "app_icon_small.svg", "app_icon.ico"):
        assert any(fnmatch(f"assets/{name}", p) for p in patterns), name
        assert (ASSETS / name).is_file()


def test_ico_has_all_sizes():
    with Image.open(ASSETS / "app_icon.ico") as im:
        assert {(s, s) for s in SIZES} <= set(im.info["sizes"])


def test_main_sets_window_icon(monkeypatch, tmp_path):
    seen = {}

    class Stop(Exception):
        pass

    class App:
        def setApplicationName(self, n):
            pass

        def setWindowIcon(self, icon):
            seen["icon"] = icon

    def fake_window(*a, **kw):
        raise Stop

    sentinel = object()
    monkeypatch.setattr(entry.icons, "app_icon", lambda: sentinel)
    monkeypatch.setattr(entry, "default_config_path", lambda: tmp_path / "c.json")
    monkeypatch.setattr(entry, "load_config", lambda p: Config(language="es"))
    monkeypatch.setattr(entry, "MainWindow", fake_window)
    monkeypatch.setattr(entry, "CountrySource", lambda: None)
    monkeypatch.setattr(entry, "QApplication", lambda argv: App())
    try:
        entry.main()
    except Stop:
        pass
    assert seen["icon"] is sentinel


def test_license_is_mit():
    text = (ROOT / "LICENSE").read_text(encoding="utf-8")
    assert text.startswith("MIT License")
    assert "Copyright (c) 2026 JotaJazz" in text
