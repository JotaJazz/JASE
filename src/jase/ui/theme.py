from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor

TOKENS: dict[str, dict[str, str]] = {
    "dark": {
        "bg": "#16181d",
        "surface": "#1d2027",
        "sidebar": "#1a1d23",
        "border": "#2a2e37",
        "text": "#d7dbe2",
        "muted": "#7c8494",
        "accent": "#3b82f6",
        "selection": "#24324a",
        "ping_good": "#3ecf6e",
        "ping_mid": "#e5b93a",
        "ping_bad": "#ef5a5a",
        "occupancy": "#3b82f6",
        "favorite": "#f5c542",
        "badge_cod1": "#936b09",
        "badge_coduo": "#8a6d3b",
        "badge_cod2": "#c0562b",
        "badge_cod4": "#4b7a3c",
        "badge_q3": "#c54c1b",
        "badge_rtcw": "#7a5c8a",
        "badge_et": "#3b6ea5",
        "badge_jka": "#3b78a8",
        "badge_sof2": "#a04040",
        "badge_cs16": "#a65a17",
        "badge_ut99": "#8b701b",
        "badge_mohaa": "#5a7a4a",
        "badge_mohaas": "#8a4b3c",
        "badge_mohaab": "#3c7a7a",
    },
    "light": {
        "bg": "#f6f7f9",
        "surface": "#ffffff",
        "sidebar": "#eef0f3",
        "border": "#e5e7eb",
        "text": "#1f2328",
        "muted": "#6b7280",
        "accent": "#2563eb",
        "selection": "#dbeafe",
        "ping_good": "#15803d",
        "ping_mid": "#a16207",
        "ping_bad": "#b91c1c",
        "occupancy": "#60a5fa",
        "favorite": "#b7791f",
        "badge_cod1": "#8a6408",
        "badge_coduo": "#67512c",
        "badge_cod2": "#904020",
        "badge_cod4": "#385b2d",
        "badge_q3": "#a23e16",
        "badge_rtcw": "#5b4567",
        "badge_et": "#2c527b",
        "badge_jka": "#2c5b81",
        "badge_sof2": "#8a3333",
        "badge_cs16": "#8a4d17",
        "badge_ut99": "#8a6f1b",
        "badge_mohaa": "#435b37",
        "badge_mohaas": "#67382d",
        "badge_mohaab": "#2d5b5b",
    },
}

# Colores ^0-^9 de Quake 3 / CoD ajustados por tema para que se lean sobre el fondo.
# None = color de texto normal (el blanco ^7 en ambos temas).
Q3_COLORS: dict[str, dict[str, str | None]] = {
    "dark": {
        "0": "#6b7280", "1": "#ef5a5a", "2": "#3ecf6e", "3": "#e5c07b", "4": "#5b8def",
        "5": "#56c8d8", "6": "#c678dd", "7": None, "8": "#f0913a", "9": "#8a919e",
    },
    "light": {
        "0": "#111827", "1": "#c62828", "2": "#15803d", "3": "#a16207", "4": "#1d4ed8",
        "5": "#0e7490", "6": "#9333ea", "7": None, "8": "#c2410c", "9": "#6b7280",
    },
}

# Opacidad de la barra de ocupación (columna Jugadores) sobre el fondo de la celda.
OCCUPANCY_ALPHA = 0.35

PING_GOOD_MAX = 80
PING_MID_MAX = 150


def resolve_mode(pref: str, scheme: Qt.ColorScheme) -> str:
    if pref in ("dark", "light"):
        return pref
    return "light" if scheme == Qt.ColorScheme.Light else "dark"


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
