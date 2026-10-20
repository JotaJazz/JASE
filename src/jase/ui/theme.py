from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor

from jase.i18n import tr

THEME_IDS: tuple[str, ...] = (
    "dark", "light", "neon", "brown", "green", "classic", "xp", "amber", "synthwave", "rust",
)

_BADGES_DARK = {
    "badge_cod1": "#936b09", "badge_coduo": "#8a6d3b", "badge_cod2": "#c0562b",
    "badge_cod4": "#4b7a3c", "badge_q3": "#c54c1b", "badge_rtcw": "#7a5c8a",
    "badge_et": "#3b6ea5", "badge_jka": "#3b78a8", "badge_sof2": "#a04040",
    "badge_cs16": "#a65a17", "badge_ut99": "#8b701b", "badge_mohaa": "#5a7a4a",
    "badge_mohaas": "#8a4b3c", "badge_mohaab": "#3c7a7a", "badge_bf1942": "#5e7a2c",
}
_BADGES_LIGHT = {
    "badge_cod1": "#8a6408", "badge_coduo": "#67512c", "badge_cod2": "#904020",
    "badge_cod4": "#385b2d", "badge_q3": "#a23e16", "badge_rtcw": "#5b4567",
    "badge_et": "#2c527b", "badge_jka": "#2c5b81", "badge_sof2": "#8a3333",
    "badge_cs16": "#8a4d17", "badge_ut99": "#8a6f1b", "badge_mohaa": "#435b37",
    "badge_mohaas": "#67382d", "badge_mohaab": "#2d5b5b", "badge_bf1942": "#475c21",
}

TOKENS: dict[str, dict[str, str]] = {
    "dark": {
        "bg": "#16181d", "surface": "#1d2027", "sidebar": "#1a1d23", "border": "#2a2e37",
        "text": "#d7dbe2", "muted": "#7c8494", "accent": "#3b82f6", "selection": "#24324a",
        "selection_text": "#d7dbe2",
        "ping_good": "#3ecf6e", "ping_mid": "#e5b93a", "ping_bad": "#ef5a5a",
        "occupancy": "#3b82f6", "favorite": "#f5c542",
        "bevel_light": "#2a2e37", "bevel_dark": "#2a2e37",
        "button_top": "#1d2027", "button_bottom": "#1d2027",
        **_BADGES_DARK,
    },
    "light": {
        "bg": "#f6f7f9", "surface": "#ffffff", "sidebar": "#eef0f3", "border": "#e5e7eb",
        "text": "#1f2328", "muted": "#6b7280", "accent": "#2563eb", "selection": "#dbeafe",
        "selection_text": "#1f2328",
        "ping_good": "#15803d", "ping_mid": "#a16207", "ping_bad": "#b91c1c",
        "occupancy": "#60a5fa", "favorite": "#b7791f",
        "bevel_light": "#e5e7eb", "bevel_dark": "#e5e7eb",
        "button_top": "#ffffff", "button_bottom": "#ffffff",
        **_BADGES_LIGHT,
    },
    "neon": {
        "bg": "#0b0f14", "surface": "#0f161d", "sidebar": "#0d131a", "border": "#137c75",
        "text": "#c8fff9", "muted": "#6fa8a3", "accent": "#19f5e1", "selection": "#0e2f33",
        "selection_text": "#19f5e1",
        "ping_good": "#19f5e1", "ping_mid": "#ffd23f", "ping_bad": "#ff5c7a",
        "occupancy": "#19f5e1", "favorite": "#ffd23f",
        "bevel_light": "#137c75", "bevel_dark": "#137c75",
        "button_top": "#0f161d", "button_bottom": "#0f161d",
        **_BADGES_DARK,
    },
    "brown": {
        "bg": "#2b211a", "surface": "#342820", "sidebar": "#32261d", "border": "#4a3a2c",
        "text": "#ecdcc8", "muted": "#a8927a", "accent": "#c08a4a", "selection": "#5a3f28",
        "selection_text": "#fff3e0",
        "ping_good": "#8fcf6a", "ping_mid": "#e0b060", "ping_bad": "#ef6a5a",
        "occupancy": "#c08a4a", "favorite": "#f0c060",
        "bevel_light": "#4a3a2c", "bevel_dark": "#4a3a2c",
        "button_top": "#3a2c21", "button_bottom": "#3a2c21",
        **_BADGES_DARK,
    },
    "green": {
        "bg": "#151c14", "surface": "#1c2619", "sidebar": "#182116", "border": "#2c3a28",
        "text": "#cfe8c4", "muted": "#86a07c", "accent": "#6aa84f", "selection": "#2f4a26",
        "selection_text": "#e6f8de",
        "ping_good": "#8fd16a", "ping_mid": "#e0c050", "ping_bad": "#ef6a5a",
        "occupancy": "#6aa84f", "favorite": "#e0c050",
        "bevel_light": "#2c3a28", "bevel_dark": "#2c3a28",
        "button_top": "#1c2619", "button_bottom": "#1c2619",
        **_BADGES_DARK,
    },
    "classic": {
        "bg": "#c0c0c0", "surface": "#ffffff", "sidebar": "#c0c0c0", "border": "#808080",
        "text": "#000000", "muted": "#404040", "accent": "#000080", "selection": "#000080",
        "selection_text": "#ffffff",
        "ping_good": "#006400", "ping_mid": "#8a5a00", "ping_bad": "#b00000",
        "occupancy": "#000080", "favorite": "#8a5a00",
        "bevel_light": "#ffffff", "bevel_dark": "#404040",
        "button_top": "#c0c0c0", "button_bottom": "#c0c0c0",
        **_BADGES_LIGHT,
    },
    "xp": {
        "bg": "#ece9d8", "surface": "#ffffff", "sidebar": "#ece9d8", "border": "#aca899",
        "text": "#000000", "muted": "#555555", "accent": "#316ac5", "selection": "#316ac5",
        "selection_text": "#ffffff",
        "ping_good": "#007000", "ping_mid": "#8a5a00", "ping_bad": "#b00000",
        "occupancy": "#316ac5", "favorite": "#8a5a00",
        "bevel_light": "#ffffff", "bevel_dark": "#aca899",
        "button_top": "#ffffff", "button_bottom": "#d6d0c0",
        **_BADGES_LIGHT,
    },
    "amber": {
        "bg": "#120c02", "surface": "#160f03", "sidebar": "#1a1104", "border": "#5a3a00",
        "text": "#ffb000", "muted": "#b07a10", "accent": "#ffb000", "selection": "#ffb000",
        "selection_text": "#120c02",
        "ping_good": "#b8e060", "ping_mid": "#ffd060", "ping_bad": "#ff6a3a",
        "occupancy": "#ffb000", "favorite": "#ffd060",
        "bevel_light": "#5a3a00", "bevel_dark": "#5a3a00",
        "button_top": "#1a1104", "button_bottom": "#1a1104",
        **_BADGES_DARK,
    },
    "synthwave": {
        "bg": "#1a1033", "surface": "#211442", "sidebar": "#1f1240", "border": "#6b2a7a",
        "text": "#f3e6ff", "muted": "#a58bd0", "accent": "#ff3fa4", "selection": "#4a1f5e",
        "selection_text": "#ffffff",
        "ping_good": "#3ff7ff", "ping_mid": "#ffe14a", "ping_bad": "#ff5c8a",
        "occupancy": "#ff3fa4", "favorite": "#ffe14a",
        "bevel_light": "#6b2a7a", "bevel_dark": "#6b2a7a",
        "button_top": "#2b1055", "button_bottom": "#2b1055",
        **_BADGES_DARK,
    },
    "rust": {
        "bg": "#1c1410", "surface": "#22170f", "sidebar": "#2a1a12", "border": "#6b2e1a",
        "text": "#e6d2c0", "muted": "#a08670", "accent": "#b5451b", "selection": "#7a2a14",
        "selection_text": "#fff0e6",
        "ping_good": "#8fc860", "ping_mid": "#e88a3a", "ping_bad": "#ef5a4a",
        "occupancy": "#b5451b", "favorite": "#e8a43a",
        "bevel_light": "#6b2e1a", "bevel_dark": "#6b2e1a",
        "button_top": "#2a1a12", "button_bottom": "#2a1a12",
        **_BADGES_DARK,
    },
}

# Estilo de widgets por tema: "flat" (redondeado), "win98" (biselado) o "luna" (Windows XP).
STYLE: dict[str, str] = {tid: "flat" for tid in THEME_IDS}
STYLE["classic"] = "win98"
STYLE["xp"] = "luna"

# Fuente por tema; None = la de siempre.
FONTS: dict[str, str | None] = {tid: None for tid in THEME_IDS}
FONTS["classic"] = 'Tahoma, "MS Sans Serif", sans-serif'
FONTS["xp"] = 'Tahoma, "Segoe UI", sans-serif'
FONTS["amber"] = '"Lucida Console", "DejaVu Sans Mono", monospace'

# Colores ^0-^9 de Quake 3 / CoD ajustados por tema para que se lean sobre el fondo.
# None = color de texto normal (el blanco ^7 en ambos temas).
_Q3_DARK: dict[str, str | None] = {
    "0": "#6b7280", "1": "#ef5a5a", "2": "#3ecf6e", "3": "#e5c07b", "4": "#5b8def",
    "5": "#56c8d8", "6": "#c678dd", "7": None, "8": "#f0913a", "9": "#8a919e",
}
_Q3_LIGHT: dict[str, str | None] = {
    "0": "#111827", "1": "#c62828", "2": "#15803d", "3": "#a16207", "4": "#1d4ed8",
    "5": "#0e7490", "6": "#9333ea", "7": None, "8": "#c2410c", "9": "#6b7280",
}
Q3_COLORS: dict[str, dict[str, str | None]] = {
    "dark": _Q3_DARK,
    "light": _Q3_LIGHT,
    "neon": dict(_Q3_DARK),
    "brown": {**_Q3_DARK, "0": "#7c8494"},
    "green": dict(_Q3_DARK),
    "classic": dict(_Q3_LIGHT),
    "xp": dict(_Q3_LIGHT),
    "amber": dict(_Q3_DARK),
    "synthwave": dict(_Q3_DARK),
    "rust": dict(_Q3_DARK),
}

# Opacidad de la barra de ocupación (columna Jugadores) sobre el fondo de la celda.
OCCUPANCY_ALPHA = 0.35

PING_GOOD_MAX = 80
PING_MID_MAX = 150


def resolve_mode(pref: str, scheme: Qt.ColorScheme) -> str:
    if pref in ("dark", "light"):
        return pref
    return "light" if scheme == Qt.ColorScheme.Light else "dark"


def resolve_theme(pref: str, scheme: Qt.ColorScheme) -> str:
    """Id de tema concreto: "system" (o un id inválido) sigue el esquema del sistema."""
    if pref in TOKENS:
        return pref
    return "light" if scheme == Qt.ColorScheme.Light else "dark"


def theme_display_name(theme_id: str) -> str:
    names = {
        "system": tr("Sistema"),
        "dark": tr("Oscuro"),
        "light": tr("Claro"),
        "neon": tr("Neón"),
        "brown": tr("Marrón"),
        "green": tr("Verde"),
        "classic": tr("Clásico"),
        "xp": tr("Windows XP"),
        "amber": tr("Ámbar"),
        "synthwave": tr("Synthwave"),
        "rust": tr("Óxido"),
    }
    return names.get(theme_id, theme_id)


def _luminance(hex_color: str) -> float:
    def lin(c: int) -> float:
        c /= 255
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (int(hex_color[i:i + 2], 16) for i in (1, 3, 5))
    return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)


def contrast_ratio(fg: str, bg: str) -> float:
    """Contraste WCAG entre dos colores "#rrggbb" (1.0 a 21.0)."""
    a, b = _luminance(fg), _luminance(bg)
    hi, lo = max(a, b), min(a, b)
    return (hi + 0.05) / (lo + 0.05)


def ping_color(ping_ms: int, mode: str) -> QColor:
    t = TOKENS[mode]
    if ping_ms < PING_GOOD_MAX:
        return QColor(t["ping_good"])
    if ping_ms < PING_MID_MAX:
        return QColor(t["ping_mid"])
    return QColor(t["ping_bad"])


def build_stylesheet(mode: str) -> str:
    t = TOKENS[mode]
    return f"""
QMainWindow, QWidget {{ background: {t['bg']}; color: {t['text']}; }}
QTableView {{
    background: {t['surface']}; alternate-background-color: {t['surface']};
    border: 1px solid {t['border']}; border-radius: 6px;
    gridline-color: {t['border']};
    selection-background-color: {t['selection']}; selection-color: {t['text']};
}}
QHeaderView::section {{
    background: {t['sidebar']}; color: {t['muted']};
    border: none; border-bottom: 1px solid {t['border']}; padding: 6px 8px;
}}
QListWidget, QTreeWidget#game_tree {{
    background: {t['sidebar']}; border: 1px solid {t['border']}; border-radius: 6px;
    outline: none;
}}
QListWidget::item, QTreeWidget#game_tree::item {{ padding: 6px 8px; border-radius: 6px; }}
QListWidget::item:selected, QTreeWidget#game_tree::item:selected {{ background: {t['selection']}; color: {t['text']}; }}
QLineEdit, QComboBox {{
    background: {t['surface']}; color: {t['text']};
    border: 1px solid {t['border']}; border-radius: 6px; padding: 5px 8px;
}}
QLineEdit:focus, QComboBox:focus {{ border-color: {t['accent']}; }}
QComboBox:disabled, QCheckBox:disabled {{ color: {t['muted']}; }}
QComboBox QAbstractItemView {{
    background: {t['surface']}; color: {t['text']};
    selection-background-color: {t['selection']};
}}
QPushButton {{
    background: {t['surface']}; color: {t['text']};
    border: 1px solid {t['border']}; border-radius: 6px; padding: 6px 14px;
}}
QToolBar#toolbar {{ background: {t['bg']}; border: none; spacing: 4px; padding: 4px 8px; }}
QToolBar#toolbar::separator {{ background: {t['border']}; width: 1px; margin: 4px 6px; }}
QToolButton {{ border: 1px solid transparent; border-radius: 6px; padding: 4px; }}
QToolButton:hover {{ background: {t['surface']}; border-color: {t['border']}; }}
QToolButton:pressed {{ background: {t['selection']}; }}
QMenuBar {{ background: {t['bg']}; color: {t['text']}; }}
QMenuBar::item:selected {{ background: {t['selection']}; }}
QMenu {{ background: {t['surface']}; color: {t['text']}; border: 1px solid {t['border']}; }}
QMenu::item {{ padding: 5px 24px 5px 24px; }}
QMenu::item:selected {{ background: {t['selection']}; }}
QMenu::item:disabled {{ color: {t['muted']}; }}
QMenu::separator {{ height: 1px; background: {t['border']}; margin: 4px 8px; }}
QFrame#server_card {{
    background: {t['surface']}; border: 1px solid {t['border']}; border-radius: 6px;
}}
QFrame#server_card QLabel, QWidget#card_details {{ background: transparent; }}
QFrame#server_card QLabel[role="caption"], QLabel#card_empty, QLabel#card_lock {{
    color: {t['muted']};
}}
QFrame#server_card QLabel#map_image {{
    background: {t['surface']}; color: {t['muted']};
    border: 1px solid {t['border']}; border-radius: 4px;
}}
QFrame#server_card QLabel#map_image[placeholder="false"] {{ border: none; }}
QPushButton#copy_address {{ padding: 2px 10px; }}
QPushButton#favorite_toggle {{ padding: 0px 6px; color: {t['muted']}; }}
QPushButton#favorite_toggle[favorite="true"] {{ color: {t['favorite']}; }}
"""
