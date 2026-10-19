"""Iconos de la barra de herramientas: SVG propios con trazo `currentColor`, teñidos por tema."""
from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QByteArray, QSize
from PySide6.QtGui import QIcon
from PySide6.QtSvg import QSvgRenderer

from jase.ui import theme
from jase.ui.flags import _SCALES, _render

ICONS_DIR = Path(__file__).resolve().parent.parent / "assets" / "icons"
ICON_SIZE = QSize(20, 20)

_cache: dict[tuple[str, str], QIcon] = {}


def tool_icon(name: str, mode: str) -> QIcon:
    """Icono `name` con el color de texto del tema `mode` (gris «muted» si está deshabilitado).

    Llamar desde el hilo de la UI (usa QPixmap). Icono vacío si falta el SVG.
    """
    key = (name, mode)
    if key in _cache:
        return _cache[key]
    icon = QIcon()
    path = ICONS_DIR / f"{name}.svg"
    if path.is_file():
        tokens = theme.TOKENS[mode]
        svg = path.read_text(encoding="utf-8")
        for state, color in ((QIcon.Normal, tokens["text"]), (QIcon.Disabled, tokens["muted"])):
            renderer = QSvgRenderer(QByteArray(svg.replace("currentColor", color).encode()))
            if not renderer.isValid():
                continue
            for scale in _SCALES:
                icon.addPixmap(_render(renderer, ICON_SIZE, scale), state)
    _cache[key] = icon
    return icon
