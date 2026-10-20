import pytest
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


DARK_BEFORE = {
    "bg": "#16181d", "surface": "#1d2027", "sidebar": "#1a1d23", "border": "#2a2e37",
    "text": "#d7dbe2", "muted": "#7c8494", "accent": "#3b82f6", "selection": "#24324a",
    "ping_good": "#3ecf6e", "ping_mid": "#e5b93a", "ping_bad": "#ef5a5a",
    "occupancy": "#3b82f6", "favorite": "#f5c542",
}
LIGHT_BEFORE = {
    "bg": "#f6f7f9", "surface": "#ffffff", "sidebar": "#eef0f3", "border": "#e5e7eb",
    "text": "#1f2328", "muted": "#6b7280", "accent": "#2563eb", "selection": "#dbeafe",
    "ping_good": "#15803d", "ping_mid": "#a16207", "ping_bad": "#b91c1c",
    "occupancy": "#60a5fa", "favorite": "#b7791f",
}
ALL = ["dark", "light", "neon", "brown", "green", "classic", "xp", "amber", "synthwave", "rust"]
NEW_KEYS = {"selection_text", "bevel_light", "bevel_dark", "button_top", "button_bottom"}


def test_theme_ids_order():
    assert theme.THEME_IDS == tuple(ALL)
    assert set(theme.TOKENS) == set(ALL) == set(theme.Q3_COLORS)
    assert set(theme.STYLE) == set(ALL) == set(theme.FONTS)


def test_all_themes_have_same_token_keys():
    keys = set(theme.TOKENS["dark"])
    assert NEW_KEYS <= keys
    for tid in ALL:
        assert set(theme.TOKENS[tid]) == keys, tid


def test_all_themes_have_q3_colors():
    keys = set(theme.Q3_COLORS["dark"])
    for tid in ALL:
        assert set(theme.Q3_COLORS[tid]) == keys, tid


def test_dark_and_light_tokens_unchanged():
    for tid, before in (("dark", DARK_BEFORE), ("light", LIGHT_BEFORE)):
        for k, v in before.items():
            assert theme.TOKENS[tid][k] == v, f"{tid}/{k}"


def test_styles_and_fonts():
    for tid in ALL:
        assert theme.STYLE[tid] == {"classic": "win98", "xp": "luna"}.get(tid, "flat")
    assert theme.FONTS["classic"] and "Tahoma" in theme.FONTS["classic"]
    assert theme.FONTS["xp"] and "Tahoma" in theme.FONTS["xp"]
    assert theme.FONTS["amber"] and "mono" in theme.FONTS["amber"].lower()
    assert theme.FONTS["dark"] is None


def test_contrast_ratio_helper():
    assert theme.contrast_ratio("#000000", "#ffffff") == pytest.approx(21.0)
    assert theme.contrast_ratio("#ffffff", "#ffffff") == pytest.approx(1.0)


@pytest.mark.parametrize("tid", ALL)
@pytest.mark.parametrize("surface", ["bg", "surface", "sidebar"])
def test_contrast_text(tid, surface):
    t = theme.TOKENS[tid]
    assert theme.contrast_ratio(t["text"], t[surface]) >= 4.5


@pytest.mark.parametrize("tid", ALL)
def test_contrast_selection(tid):
    t = theme.TOKENS[tid]
    assert theme.contrast_ratio(t["selection_text"], t["selection"]) >= 4.5


@pytest.mark.parametrize("tid", ALL)
def test_badge_tokens_have_wcag_contrast_with_white(tid):
    for name, value in theme.TOKENS[tid].items():
        if name.startswith("badge_"):
            assert theme.contrast_ratio("#ffffff", value) >= 4.5, f"{tid}/{name} {value}"


@pytest.mark.parametrize("tid", ALL)
def test_ping_and_q3_contrast_min_3(tid):
    t = theme.TOKENS[tid]
    for k in ("ping_good", "ping_mid", "ping_bad"):
        assert theme.contrast_ratio(t[k], t["surface"]) >= 3.0, f"{tid}/{k}"
    for k, v in theme.Q3_COLORS[tid].items():
        if v is not None:
            assert theme.contrast_ratio(v, t["surface"]) >= 3.0, f"{tid}/^{k}"


def test_resolve_theme():
    assert theme.resolve_theme("system", Qt.ColorScheme.Light) == "light"
    assert theme.resolve_theme("system", Qt.ColorScheme.Dark) == "dark"
    assert theme.resolve_theme("system", Qt.ColorScheme.Unknown) == "dark"
    assert theme.resolve_theme("neon", Qt.ColorScheme.Light) == "neon"
    assert theme.resolve_theme("bogus", Qt.ColorScheme.Light) == "light"


def test_theme_display_names_both_languages():
    from jase import i18n
    try:
        for lang in ("es", "en"):
            i18n.set_language(lang)
            names = [theme.theme_display_name(t) for t in ALL]
            assert all(names) and len(set(names)) == len(ALL)
        assert theme.theme_display_name("xp") == "Windows XP"
        assert theme.theme_display_name("rust") == "Rust"
        i18n.set_language("es")
        assert theme.theme_display_name("rust") == "Óxido"
    finally:
        i18n.set_language("es")
