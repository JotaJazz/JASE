"""Genera `src/jase/assets/app_icon.ico` desde los SVG del icono (Qt offscreen + Pillow).

Uso: .venv/bin/python scripts/make_icon.py [--previews DIR]
La variante pequeña se usa para 16 y 24 px; la completa para el resto.
"""
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PIL import Image
from PySide6.QtCore import QRectF, Qt
from PySide6.QtGui import QGuiApplication, QImage, QPainter
from PySide6.QtSvg import QSvgRenderer

ASSETS = Path(__file__).resolve().parent.parent / "src" / "jase" / "assets"
SIZES = (16, 24, 32, 48, 64, 128, 256)
SMALL_SIZES = (16, 24)


def render(svg: Path, side: int) -> Image.Image:
    renderer = QSvgRenderer(str(svg))
    image = QImage(side, side, QImage.Format_ARGB32)
    image.fill(Qt.transparent)
    painter = QPainter(image)
    painter.setRenderHint(QPainter.Antialiasing)
    renderer.render(painter, QRectF(0, 0, side, side))
    painter.end()
    return Image.frombuffer("RGBA", (side, side), bytes(image.constBits()), "raw", "BGRA", 0, 1)


def frames() -> dict[int, Image.Image]:
    return {s: render(ASSETS / ("app_icon_small.svg" if s in SMALL_SIZES else "app_icon.svg"), s)
            for s in SIZES}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--previews", type=Path, help="carpeta donde guardar PNG de 16, 32 y 256 px")
    args = parser.parse_args(argv)
    _app = QGuiApplication(sys.argv[:1])
    imgs = frames()
    imgs[256].save(ASSETS / "app_icon.ico", format="ICO", sizes=[(s, s) for s in SIZES],
                   append_images=[imgs[s] for s in SIZES if s != 256])
    if args.previews:
        args.previews.mkdir(parents=True, exist_ok=True)
        for s in (16, 32, 256):
            imgs[s].save(args.previews / f"icon_{s}.png")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
