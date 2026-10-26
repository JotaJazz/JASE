"""Iconos de la barra de herramientas: SVG propios con trazo `currentColor`, teñidos por tema."""
from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QByteArray, QRectF, QSize, Qt
from PySide6.QtGui import QColor, QFont, QIcon, QImage, QPainter, QPixmap
from PySide6.QtSvg import QSvgRenderer

from jase.games import GAMES
from jase.ui import theme
from jase.ui.flags import _SCALES, _render

ICONS_DIR = Path(__file__).resolve().parent.parent / "assets" / "icons"
ICON_SIZE = QSize(20, 20)
# Variantes a color por estilo: pixel art de 98 (scripts/make_win98_icons.py) y Luna de XP.
WIN98_ICONS_DIR = ICONS_DIR / "win98"
WIN98_ICON_SIZE = QSize(16, 16)
STYLE_ICONS = {"win98": (WIN98_ICONS_DIR, WIN98_ICON_SIZE), "luna": (ICONS_DIR / "xp", ICON_SIZE)}
APP_ICON_DIR = Path(__file__).resolve().parent.parent / "assets"
APP_ICON_SMALL_SIZES = (16, 24)
APP_ICON_LARGE_SIZES = (32, 48, 64, 128, 256)

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
    styled = STYLE_ICONS.get(theme.STYLE[mode])
    if styled and (styled[0] / f"{name}.svg").is_file():
        # Icono a color del estilo, sin teñir (98: 16 px exactos, sin escalar a medias).
        renderer = QSvgRenderer(str(styled[0] / f"{name}.svg"))
        for scale in _SCALES:
            icon.addPixmap(_render(renderer, styled[1], scale))
    elif path.is_file():
        tokens = theme.TOKENS[mode]
        svg = path.read_text(encoding="utf-8")
        for state, color in ((QIcon.Mode.Normal, tokens["text"]),
                             (QIcon.Mode.Disabled, tokens["muted"])):
            renderer = QSvgRenderer(QByteArray(svg.replace("currentColor", color).encode()))
            if not renderer.isValid():
                continue
            for scale in _SCALES:
                icon.addPixmap(_render(renderer, ICON_SIZE, scale), state)
    _cache[key] = icon
    return icon


_badge_cache: dict[tuple[str, str, int], QIcon] = {}


def badge_icon(game_id: str, mode: str, size: int = 16) -> QIcon:
    """Icono dibujado del juego: cuadrado redondeado `badge_<id>` con sus iniciales en blanco.

    Llamar desde el hilo de la UI (usa QPixmap). Icono vacío si el juego no existe.
    """
    key = (game_id, mode, size)
    if key in _badge_cache:
        return _badge_cache[key]
    icon = QIcon()
    game = GAMES.get(game_id)
    color = theme.TOKENS[mode].get(f"badge_{game_id}")
    if game is not None and color:
        for scale in _SCALES:
            side = size * scale
            image = QImage(side, side, QImage.Format.Format_ARGB32_Premultiplied)
            image.fill(Qt.GlobalColor.transparent)
            painter = QPainter(image)
            painter.setRenderHint(QPainter.RenderHint.Antialiasing)
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(QColor(color))
            painter.drawRoundedRect(QRectF(0, 0, side, side), side * 0.22, side * 0.22)
            font = QFont()
            font.setBold(True)
            font.setPixelSize(max(1, round(side * 0.55)))
            painter.setFont(font)
            painter.setPen(QColor("#ffffff"))
            painter.drawText(QRectF(0, 0, side, side), Qt.AlignmentFlag.AlignCenter, game.badge)
            painter.end()
            icon.addPixmap(QPixmap.fromImage(image))
    _badge_cache[key] = icon
    return icon


_app_icon: QIcon | None = None


def app_icon() -> QIcon:
    """Icono de la aplicación: variante simplificada para 16 y 24 px, completa desde 32 px.

    Llamar desde el hilo de la UI (usa QPixmap). Icono vacío si faltan los SVG.
    """
    global _app_icon
    if _app_icon is not None:
        return _app_icon
    icon = QIcon()
    for filename, sizes in (("app_icon_small.svg", APP_ICON_SMALL_SIZES),
                            ("app_icon.svg", APP_ICON_LARGE_SIZES)):
        path = APP_ICON_DIR / filename
        if not path.is_file():
            continue
        renderer = QSvgRenderer(str(path))
        if not renderer.isValid():
            continue
        for side in sizes:
            for scale in _SCALES:
                icon.addPixmap(_render(renderer, QSize(side, side), scale))
    _app_icon = icon
    return icon
