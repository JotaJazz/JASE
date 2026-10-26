"""Genera los iconos de la barra para el tema Clásico: pixel art de 16×16 con la paleta de 98.

Uso: .venv/bin/python scripts/make_win98_icons.py [--preview PATH]
Escribe src/jase/assets/icons/win98/<nombre>.svg (un <rect> por píxel, sin suavizado).
Todo va dibujado como texto; a las siluetas se les calcula contorno, luz y sombra.
"""
from __future__ import annotations

import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "jase" / "assets" / "icons" / "win98"
N = 16

# Paleta VGA de 16 colores de Windows 98.
PALETTE = {
    "K": "#000000", "W": "#ffffff", "S": "#c0c0c0", "D": "#808080",
    "N": "#000080", "B": "#0000ff", "T": "#008080", "C": "#00ffff",
    "G": "#008000", "L": "#00ff00", "O": "#808000", "Y": "#ffff00",
    "M": "#800000", "R": "#ff0000",
}

Grid = list[list[str]]


def parse(art: str) -> Grid:
    rows = [line.strip() for line in art.strip().splitlines()]
    assert len(rows) == N, len(rows)
    for row in rows:
        assert len(row) == N, row
        assert all(ch == "." or ch in PALETTE for ch in row), row
    return [list(row) for row in rows]


def blank() -> Grid:
    return [["."] * N for _ in range(N)]


def outline(grid: Grid, color: str = "K") -> None:
    """Contorno de 1 px (vecinos en cruz) alrededor de lo pintado."""
    filled = {(x, y) for y in range(N) for x in range(N) if grid[y][x] != "."}
    for x, y in filled:
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < N and 0 <= ny < N and (nx, ny) not in filled:
                grid[ny][nx] = color


def highlight(grid: Grid, fill: str, light: str) -> None:
    """Luz arriba/izquierda: el píxel de relleno junto al contorno por ese lado."""
    for y in range(N):
        for x in range(N):
            if grid[y][x] != fill:
                continue
            up = grid[y - 1][x] if y else "."
            left = grid[y][x - 1] if x else "."
            if up in "K." or left in "K.":
                grid[y][x] = light


def shaded(art: str, fill: str, light: str, dark: str | None = None) -> Grid:
    """Silueta de relleno → contorno negro, luz arriba/izquierda y sombra abajo/derecha."""
    g = parse(art)
    if dark:
        for y in range(N):
            for x in range(N):
                if g[y][x] == fill and x + y > 15:
                    g[y][x] = dark
    outline(g)
    highlight(g, fill, light)
    return g


REFRESH = """
    ................
    ................
    ......GGGG......
    ....GGGGGGGG....
    ...GGG....GGG...
    ..GGG......GG...
    .........GGGGGG.
    ...GG.....GGGG..
    ..GGGG.....GG...
    .GGGGGG.........
    ...GG......GGG..
    ...GGG....GGG...
    ....GGGGGGGG....
    ......GGGG......
    ................
    ................
"""

REFRESH_ONE = """
    ................
    ................
    ......GGGG......
    ....GGGGGGGG....
    ...GGG....GGG...
    ..GGG......GG...
    ..GG.....GGGGGG.
    ..GG......GGGG..
    ..GG.......GG...
    ..GG............
    ..GGG......GG...
    ...GGG....GGG...
    ....GGGGGGGG....
    ......GGGG......
    ................
    ................
"""

GEAR = """
    ................
    ......SSSS......
    ..SS..SSSS..SS..
    ..SSSSSSSSSSSS..
    ...SSSSSSSSSS...
    ..SSSSSSSSSSSS..
    .SSSSS....SSSSS.
    .SSSSS....SSSSS.
    .SSSSS....SSSSS.
    .SSSSS....SSSSS.
    ..SSSSSSSSSSSS..
    ...SSSSSSSSSS...
    ..SSSSSSSSSSSS..
    ..SS..SSSS..SS..
    ......SSSS......
    ................
"""


def refresh() -> Grid:
    return shaded(REFRESH, "G", "L")


def refresh_one() -> Grid:
    return shaded(REFRESH_ONE, "G", "L")


def settings() -> Grid:
    return shaded(GEAR, "S", "W", "D")


def _paste(dst: Grid, src: list[str], ox: int, oy: int) -> None:
    for y, row in enumerate(src):
        for x, ch in enumerate(row):
            if ch != ".":
                dst[oy + y][ox + x] = ch


_PAGE = [
    "KKKKKK...",
    "KWWWWKK..",
    "KWWWWKWK.",
    "KWWWWKKKK",
    "KWWWWWWWK",
    "KWDDDDDWK",
    "KWWWWWWWK",
    "KWDDDDDWK",
    "KWWWWWWWK",
    "KWDDDDDWK",
    "KWWWWWWWK",
    "KKKKKKKKK",
]


def copy() -> Grid:
    g = blank()
    _paste(g, _PAGE, 1, 0)
    _paste(g, _PAGE, 6, 4)
    return g


ART = {
    "stop": """
        ................
        .....KKKKKK.....
        ....KRRRRRRK....
        ...KRRRRRRRRK...
        ..KRRWRRRRWRMK..
        .KRRWWWRRWWWRMK.
        .KRRRWWWWWWRRMK.
        .KRRRRWWWWRRRMK.
        .KRRRRWWWWRRRMK.
        .KRRRWWWWWWRRMK.
        .KRRWWWRRWWWRMK.
        ..KRRWRRRRWRMK..
        ...KMMMMMMMMK...
        ....KMMMMMMK....
        .....KKKKKK.....
        ................
    """,
    "connect": """
        ................
        ....K...........
        ....KK..........
        ....KLK.........
        ....KLGK........
        ....KLGGK.......
        ....KLGGGK......
        ....KLGGGGK.....
        ....KLGGGGGK....
        ....KLGGGGK.....
        ....KLGGGK......
        ....KLGGK.......
        ....KLGK........
        ....KGK.........
        ....KK..........
        ....K...........
    """,
    "favorite": """
        ................
        .......K........
        ......KYK.......
        ......KYK.......
        .....KYYYK......
        KKKKKKYYYKKKKKK.
        .KYYYYYYYYYYYK..
        ..KYYYYYYYYYK...
        ...KYYYYYYYK....
        ....KYYYYYK.....
        ....KYYYOYK.....
        ...KYYYKOYYK....
        ...KYYK.KOYK....
        ..KYK.....KOK...
        ..KK.......KK...
        ................
    """,
    "theme": """
        ................
        .KKKKKKKKKKKKKK.
        .KWSSSSSSSSSSDK.
        .KWSKKKKKKKKSDK.
        .KWSKTTTTTTKSDK.
        .KWSKTNNNNTKSDK.
        .KWSKTWWWWTKSDK.
        .KWSKTTTTTTKSDK.
        .KWSKKKKKKKKSDK.
        .KWSSSSSSSSGSDK.
        .KDDDDDDDDDDDDK.
        .KKKKKKKKKKKKKK.
        .....KSSSSK.....
        ...KKKKKKKKKK...
        ...KWSSSSSSDK...
        ...KKKKKKKKKK...
    """,
}

BUILT = {"refresh": refresh, "refresh_one": refresh_one, "settings": settings, "copy": copy}


def icons() -> dict[str, Grid]:
    out = {name: parse(art) for name, art in ART.items()}
    out.update({name: build() for name, build in BUILT.items()})
    return out


def to_svg(grid: Grid) -> str:
    rects = [f'<rect x="{x}" y="{y}" width="1" height="1" fill="{PALETTE[ch]}"/>'
             for y, row in enumerate(grid) for x, ch in enumerate(row) if ch != "."]
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{N}" height="{N}" '
            f'viewBox="0 0 {N} {N}" shape-rendering="crispEdges">' + "".join(rects) + "</svg>\n")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--preview", type=Path, help="PNG con los iconos ampliados ×8")
    args = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    grids = icons()
    for name, grid in grids.items():
        (OUT / f"{name}.svg").write_text(to_svg(grid), encoding="utf-8")
    print(f"{len(grids)} iconos en {OUT}")
    if args.preview:
        from PySide6.QtGui import QColor, QGuiApplication, QImage
        _app = QGuiApplication.instance() or QGuiApplication([])
        scale, gap = 8, 8
        image = QImage(len(grids) * (N * scale + gap), N * scale, QImage.Format.Format_RGB32)
        image.fill(QColor("#c0c0c0"))
        for i, grid in enumerate(grids.values()):
            for y, row in enumerate(grid):
                for x, ch in enumerate(row):
                    if ch == ".":
                        continue
                    for py in range(scale):
                        for px in range(scale):
                            image.setPixelColor(i * (N * scale + gap) + x * scale + px,
                                                y * scale + py, QColor(PALETTE[ch]))
        image.save(str(args.preview))
        print(args.preview)


if __name__ == "__main__":
    main()
