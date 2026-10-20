import pytest
from PySide6.QtCore import Qt
from jase.ui import theme


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
BADGES_BEFORE = {
    "dark": {
        "cod1": "#936b09", "coduo": "#8a6d3b", "cod2": "#c0562b", "cod4": "#4b7a3c",
        "q3": "#c54c1b", "rtcw": "#7a5c8a", "et": "#3b6ea5", "jka": "#3b78a8",
        "sof2": "#a04040", "cs16": "#a65a17", "ut99": "#8b701b", "mohaa": "#5a7a4a",
        "mohaas": "#8a4b3c", "mohaab": "#3c7a7a", "bf1942": "#5e7a2c",
    },
    "light": {
        "cod1": "#8a6408", "coduo": "#67512c", "cod2": "#904020", "cod4": "#385b2d",
        "q3": "#a23e16", "rtcw": "#5b4567", "et": "#2c527b", "jka": "#2c5b81",
        "sof2": "#8a3333", "cs16": "#8a4d17", "ut99": "#8a6f1b", "mohaa": "#435b37",
        "mohaas": "#67382d", "mohaab": "#2d5b5b", "bf1942": "#475c21",
    },
}
Q3_BEFORE = {
    "dark": {"0": "#6b7280", "1": "#ef5a5a", "2": "#3ecf6e", "3": "#e5c07b", "4": "#5b8def",
             "5": "#56c8d8", "6": "#c678dd", "7": None, "8": "#f0913a", "9": "#8a919e"},
    "light": {"0": "#111827", "1": "#c62828", "2": "#15803d", "3": "#a16207", "4": "#1d4ed8",
              "5": "#0e7490", "6": "#9333ea", "7": None, "8": "#c2410c", "9": "#6b7280"},
}
ALL = ["dark", "light", "neon", "brown", "green", "classic", "xp", "amber", "synthwave", "rust"]
NEW_KEYS = {"divider", "selection_outline", "selection_text", "bevel_light", "bevel_dark", "button_top", "button_bottom"}


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
        for g, v in BADGES_BEFORE[tid].items():
            assert theme.TOKENS[tid][f"badge_{g}"] == v, f"{tid}/badge_{g}"
        assert theme.Q3_COLORS[tid] == Q3_BEFORE[tid]


def test_divider_and_outline():
    for tid in ALL:
        tk = theme.TOKENS[tid]
        if tid not in ("neon", "synthwave"):
            assert tk["divider"] == tk["border"] and tk["selection_outline"] == tk["selection"]
    assert theme.TOKENS["neon"]["border"] == "#19f5e1"
    assert theme.TOKENS["neon"]["divider"] == "#0f4d49"
    assert theme.TOKENS["neon"]["selection_outline"] == "#19f5e1"
    assert theme.TOKENS["synthwave"]["border"] == "#ff3fa4"
    assert theme.TOKENS["synthwave"]["divider"] == "#4a1f5e"
    assert theme.TOKENS["synthwave"]["selection_outline"] == "#ff3fa4"
    for tid in ("neon", "synthwave"):
        assert theme.TOKENS[tid]["bevel_light"] == theme.TOKENS[tid]["bevel_dark"] == theme.TOKENS[tid]["border"]


def test_font_fallbacks():
    assert theme.FONTS["classic"] == theme.FONTS["xp"] == \
        'Tahoma, "MS Sans Serif", "Segoe UI", sans-serif'
    assert theme.FONTS["amber"] == '"Lucida Console", Consolas, "DejaVu Sans Mono", monospace'


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


# ------------------------------------------------------------ hojas de estilo
def _rules(css: str) -> list[tuple[str, str]]:
    """(selector, cuerpo) de cada regla de la hoja."""
    out = []
    for chunk in css.split("}"):
        if "{" in chunk:
            sel, body = chunk.split("{", 1)
            out.append((sel.strip(), body.strip()))
    return out


@pytest.mark.parametrize("tid", ALL)
def test_stylesheet_builds_for_every_theme(tid):
    css = theme.build_stylesheet(tid)
    t = theme.TOKENS[tid]
    assert css.strip()
    for key in ("bg", "surface", "text", "border", "divider", "selection", "selection_text",
                "accent"):
        assert t[key] in css, (tid, key)
    assert css.count("{") == css.count("}")
    if t["selection_outline"] != t["selection"]:
        assert any(":selected" in sel and t["selection_outline"] in body
                   for sel, body in _rules(css)), tid


def test_dark_stylesheet_keeps_flat_look():
    css = theme.build_stylesheet("dark")
    assert "border-radius: 6px" in css
    assert "outset" not in css and "inset" not in css and "Tahoma" not in css
    assert "font-family" not in css and "::indicator" not in css


def test_classic_uses_bevels_and_tahoma():
    css = theme.build_stylesheet("classic")
    t = theme.TOKENS["classic"]
    rules = _rules(css)
    assert "Tahoma" in css and '"MS Sans Serif"' in css
    assert any(sel.startswith("QWidget") and "font-family" in body for sel, body in rules)
    buttons = " ".join(body for sel, body in rules if sel == "QPushButton")
    assert "outset" in buttons and t["bevel_light"] in buttons and t["bevel_dark"] in buttons
    pressed = " ".join(body for sel, body in rules if "QPushButton:pressed" in sel)
    assert "inset" in pressed
    headers = " ".join(body for sel, body in rules if sel == "QHeaderView::section")
    assert "outset" in headers
    views = " ".join(body for sel, body in rules if sel.startswith("QTableView"))
    assert "inset" in views
    assert "border-radius: 6px" not in css and "border-radius: 3px" not in css
    assert "2px" in buttons and "border-radius: 0" in css
    assert t["selection_text"] in css


def test_xp_uses_luna_gradient():
    css = theme.build_stylesheet("xp")
    t = theme.TOKENS["xp"]
    rules = _rules(css)
    buttons = " ".join(body for sel, body in rules if sel == "QPushButton")
    assert "qlineargradient" in buttons
    assert t["button_top"] in buttons and t["button_bottom"] in buttons
    assert "#7f9db9" in buttons and "border-radius: 3px" in buttons
    assert "Tahoma" in css and "#316ac5" in css
    assert "outset" not in css


def test_amber_monospace_only_in_views():
    css = theme.build_stylesheet("amber")
    mono = [sel for sel, body in _rules(css) if "font-family" in body]
    assert mono, "sin fuente monoespaciada"
    for sel in mono:
        parts = [p.strip() for p in sel.split(",")]
        assert "QWidget" not in parts and "QMainWindow" not in parts, sel
        assert any(p.startswith(("QTableView", "QTreeWidget", "QTreeView")) for p in parts)
    assert "Lucida Console" in css


def test_legible_color_on_selection():
    from PySide6.QtGui import QColor
    navy, white = QColor("#000080"), QColor("#ffffff")
    assert theme.legible(QColor("#006400"), navy, white) == white
    assert theme.legible(QColor("#ffd23f"), navy, white) == QColor("#ffd23f")
    # ^2 de Clásico sobre azul marino (3.2) pasa a blanco
    assert theme.legible(QColor(theme.Q3_COLORS["classic"]["2"]), navy, white) == white
    # Oscuro: el ping malo sigue rojo sobre su selección
    dark = theme.TOKENS["dark"]
    bad = QColor(dark["ping_bad"])
    assert theme.legible(bad, QColor(dark["selection"]), white) == bad


@pytest.mark.parametrize("tid", [t for t in ALL if t not in ("dark", "light")])
def test_new_themes_style_checkbox_indicators(tid):
    css = theme.build_stylesheet(tid)
    assert "QCheckBox::indicator" in css and "::indicator:checked" in css


def test_classic_sunken_on_white_keeps_bottom_right_edge():
    css = theme.build_stylesheet("classic")
    t = theme.TOKENS["classic"]
    assert t["bevel_inner"] != t["surface"]
    rules = _rules(css)
    edge = f"{t['bevel_dark']} {t['bevel_inner']} {t['bevel_inner']} {t['bevel_dark']}"
    indicator = next(b for sel, b in rules if sel.startswith("QCheckBox::indicator,"))
    map_image = [b for sel, b in rules if sel == "QFrame#server_card QLabel#map_image"]
    assert edge in indicator and edge in map_image[-1]
