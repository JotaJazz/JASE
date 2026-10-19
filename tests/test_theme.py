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


def _luminance(hex_color: str) -> float:
    def lin(c: int) -> float:
        c /= 255
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (int(hex_color[i:i + 2], 16) for i in (1, 3, 5))
    return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)


def test_badge_tokens_have_wcag_contrast_with_white():
    for mode, tokens in theme.TOKENS.items():
        for name, value in tokens.items():
            if name.startswith("badge_"):
                ratio = 1.05 / (_luminance(value) + 0.05)
                assert ratio >= 4.5, f"{mode}/{name} {value}: {ratio:.2f}"
