"""Decodificador de texturas IWI (versión 5: CoD1/CoD2).

Cabecera (28 bytes, little-endian):
    0  char[3] "IWi"
    3  u8      versión (5)
    4  u8      formato
    5  u8      uso/flags
    6  u16     ancho
    8  u16     alto
    10 u16     profundidad
    12 u32[4]  fin de cada mip (offsets[0] = fin del mip más grande)
Los mips se guardan del más chico al más grande, así que el mip principal
ocupa los últimos bytes antes de offsets[0].
"""
from __future__ import annotations

import struct

from PIL import Image

HEADER = struct.Struct("<3sBBBHHH4I")
MAX_SIDE = 8192

FMT_ARGB32 = 0x01
FMT_RGB24 = 0x02
FMT_LA88 = 0x03
FMT_A8 = 0x04
FMT_DXT1 = 0x0B
FMT_DXT3 = 0x0C
FMT_DXT5 = 0x0D

# formato -> (n de BCn de Pillow, bytes por bloque 4x4)
_BCN = {FMT_DXT1: (1, 8), FMT_DXT3: (2, 16), FMT_DXT5: (3, 16)}
# formato -> (modo de salida, rawmode de Pillow, bytes por píxel)
_RAW = {
    FMT_ARGB32: ("RGBA", "BGRA", 4),
    FMT_RGB24: ("RGB", "BGR", 3),
    FMT_LA88: ("LA", "LA", 2),
    FMT_A8: ("L", "L", 1),
}


class IwiError(ValueError):
    """El archivo IWI está dañado o usa un formato no soportado."""


def decode_iwi(data: bytes) -> Image.Image:
    """Imagen del mip más grande del IWI. Lanza IwiError si no se reconoce."""
    if len(data) < HEADER.size:
        raise IwiError("IWI demasiado corto")
    magic, version, fmt, _usage, width, height, _depth, end0, *_ = HEADER.unpack_from(data)
    if magic != b"IWi":
        raise IwiError("no es un archivo IWI")
    if version != 5:
        raise IwiError(f"versión de IWI no soportada: {version}")
    if not (0 < width <= MAX_SIDE and 0 < height <= MAX_SIDE):
        raise IwiError(f"dimensiones inválidas: {width}x{height}")

    if fmt in _BCN:
        n, block = _BCN[fmt]
        size = ((width + 3) // 4) * ((height + 3) // 4) * block
    elif fmt in _RAW:
        size = width * height * _RAW[fmt][2]
    else:
        raise IwiError(f"formato de IWI desconocido: 0x{fmt:02X}")

    end = end0 if HEADER.size + size <= end0 <= len(data) else len(data)
    start = end - size
    if start < HEADER.size:
        raise IwiError("IWI truncado")
    payload = data[start:end]

    try:
        if fmt in _BCN:
            return Image.frombytes("RGBA", (width, height), payload, "bcn", n)
        mode, rawmode, _ = _RAW[fmt]
        return Image.frombytes(mode, (width, height), payload, "raw", rawmode)
    except (ValueError, OSError) as exc:
        raise IwiError(f"no se pudo decodificar el IWI: {exc}") from exc
