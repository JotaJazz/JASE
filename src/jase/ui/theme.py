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
    },
}

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
QListWidget {{
    background: {t['sidebar']}; border: 1px solid {t['border']}; border-radius: 6px;
    outline: none;
}}
QListWidget::item {{ padding: 6px 8px; border-radius: 6px; }}
QListWidget::item:selected {{ background: {t['selection']}; color: {t['text']}; }}
QLineEdit, QComboBox {{
    background: {t['surface']}; color: {t['text']};
    border: 1px solid {t['border']}; border-radius: 6px; padding: 5px 8px;
}}
QLineEdit:focus, QComboBox:focus {{ border-color: {t['accent']}; }}
QComboBox QAbstractItemView {{
    background: {t['surface']}; color: {t['text']};
    selection-background-color: {t['selection']};
}}
QPushButton {{
    background: {t['surface']}; color: {t['text']};
    border: 1px solid {t['border']}; border-radius: 6px; padding: 6px 14px;
}}
QPushButton#connect {{
    background: {t['accent']}; color: #ffffff; border: none; font-weight: 600;
}}
QPushButton#connect:disabled {{ background: {t['border']}; color: {t['muted']}; }}
"""
