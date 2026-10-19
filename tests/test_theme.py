from PySide6.QtCore import Qt
from jase.ui import theme


def test_resolve_mode():
    assert theme.resolve_mode("system", Qt.ColorScheme.Light) == "light"
    assert theme.resolve_mode("system", Qt.ColorScheme.Unknown) == "dark"
    assert theme.resolve_mode("dark", Qt.ColorScheme.Light) == "dark"


def test_stylesheet_uses_tokens():
    assert "#16181d" in theme.build_stylesheet("dark")
    assert "#f6f7f9" in theme.build_stylesheet("light")


def test_ping_thresholds():
    t = theme.TOKENS["dark"]
    assert theme.ping_color(79, "dark").name() == t["ping_good"]
    assert theme.ping_color(80, "dark").name() == t["ping_mid"]
    assert theme.ping_color(150, "dark").name() == t["ping_bad"]
