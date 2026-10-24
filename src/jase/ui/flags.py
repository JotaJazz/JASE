"""Banderas (flag-icons 4x3, MIT) renderizadas desde SVG, en caché por código y tamaño."""
from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QRectF, QSize, Qt
from PySide6.QtGui import QIcon, QImage, QPainter, QPixmap
from PySide6.QtSvg import QSvgRenderer

FLAGS_DIR = Path(__file__).resolve().parent.parent / "assets" / "flags"
# Escalas pre-renderizadas: QIcon elige la adecuada para pantallas HiDPI.
_SCALES = (1, 2)

_cache: dict[tuple[str, int, int], QIcon | None] = {}


def flag_path(code: str) -> Path | None:
    code = (code or "").strip().lower()
    if len(code) != 2 or not code.isalpha() or not code.isascii():
        return None
    path = FLAGS_DIR / f"{code}.svg"
    return path if path.is_file() else None


def _render(renderer: QSvgRenderer, size: QSize, scale: int) -> QPixmap:
    image = QImage(size.width() * scale, size.height() * scale,
                   QImage.Format.Format_ARGB32_Premultiplied)
    image.fill(Qt.GlobalColor.transparent)
    painter = QPainter(image)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    renderer.render(painter, QRectF(0, 0, image.width(), image.height()))
    painter.end()
    pixmap = QPixmap.fromImage(image)
    pixmap.setDevicePixelRatio(scale)
    return pixmap


def flag_icon(code: str | None, size: QSize) -> QIcon | None:
    """Icono de la bandera del código ISO alfa-2, o None si no hay SVG (o es inválido).

    Llamar desde el hilo de la UI (usa QPixmap).
    """
    key = ((code or "").lower(), size.width(), size.height())
    if key in _cache:
        return _cache[key]
    icon = None
    path = flag_path(key[0])
    if path is not None:
        renderer = QSvgRenderer(str(path))
        if renderer.isValid():
            icon = QIcon()
            for scale in _SCALES:
                pixmap = _render(renderer, size, scale)
                # También en modo Selected: si no, Qt tiñe la bandera en la fila elegida.
                for mode in (QIcon.Mode.Normal, QIcon.Mode.Selected, QIcon.Mode.Active):
                    icon.addPixmap(pixmap, mode)
    _cache[key] = icon
    return icon


def flag_pixmap(code: str | None, size: QSize) -> QPixmap | None:
    icon = flag_icon(code, size)
    return icon.pixmap(size) if icon is not None else None
