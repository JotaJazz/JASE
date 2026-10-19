"""Captura (textura Screenshot) de mapas de UT99 desde los paquetes .unr."""
from __future__ import annotations

import logging
import mmap
import os
import struct

from PIL import Image

from jase.paths import _child, resolve_game_dir

log = logging.getLogger(__name__)

PACKAGE_TAG = 0x9E2A83C1
MAX_DIMENSION = 2048
MAX_TABLE_COUNT = 1_000_000
RF_HAS_STACK = 0x02000000

# Tipos de propiedad (EPropertyType) que nos interesan.
PROP_BYTE, PROP_INT, PROP_BOOL, PROP_OBJECT, PROP_STRUCT = 1, 2, 3, 5, 10
PROP_SIZES = {0: 1, 1: 2, 2: 4, 3: 12, 4: 16}

# ETextureFormat
TEXF_P8, TEXF_DXT1, TEXF_RGBA8 = 0, 3, 5


class _FormatError(Exception):
    pass


class _Reader:
    """Lectura secuencial acotada sobre los bytes del paquete."""

    def __init__(self, data, pos: int = 0):
        self.data = data
        self.pos = pos

    def take(self, n: int) -> bytes:
        if n < 0 or self.pos + n > len(self.data):
            raise _FormatError("lectura fuera del archivo")
        chunk = self.data[self.pos : self.pos + n]
        self.pos += n
        return chunk

    def u8(self) -> int:
        return self.take(1)[0]

    def i32(self) -> int:
        return struct.unpack("<i", self.take(4))[0]

    def u32(self) -> int:
        return struct.unpack("<I", self.take(4))[0]

    def compact(self) -> int:
        """FCompactIndex: bit 7 signo, bit 6 continúa, luego grupos de 7 bits."""
        first = self.u8()
        value = first & 0x3F
        more = first & 0x40
        shift = 6
        count = 1
        while more:
            if count >= 5:
                raise _FormatError("índice compacto inválido")
            b = self.u8()
            value |= (b & 0x7F) << shift
            shift += 7
            more = b & 0x80
            count += 1
        return -value if first & 0x80 else value


class _Package:
    def __init__(self, data):
        self.data = data
        r = _Reader(data)
        if r.u32() != PACKAGE_TAG:
            raise _FormatError("no es un paquete Unreal")
        self.version = r.u32() & 0xFFFF
        r.u32()  # flags
        name_count, name_off = r.u32(), r.u32()
        export_count, export_off = r.u32(), r.u32()
        import_count, import_off = r.u32(), r.u32()
        for count in (name_count, export_count, import_count):
            if count > MAX_TABLE_COUNT:
                raise _FormatError("tabla demasiado grande")

        self.names: list[str] = []
        r.pos = name_off
        for _ in range(name_count):
            if self.version >= 64:
                length = r.compact()
                raw = r.take(length) if length > 0 else b""
            else:
                end = data.find(b"\0", r.pos)
                if end < 0:
                    raise _FormatError("nombre sin terminar")
                raw = r.take(end - r.pos + 1)
            r.u32()  # flags del nombre
            self.names.append(bytes(raw).rstrip(b"\0").decode("latin-1"))

        self.imports: list[str] = []  # nombre de objeto de cada import
        r.pos = import_off
        for _ in range(import_count):
            r.compact()  # class package
            r.compact()  # class name
            r.i32()  # package
            self.imports.append(self.name(r.compact()))

        # (class_ref, object_name, flags, serial_size, serial_offset)
        self.exports: list[tuple[int, str, int, int, int]] = []
        r.pos = export_off
        for _ in range(export_count):
            class_ref = r.compact()
            r.compact()  # super
            r.i32()  # package
            obj_name = self.name(r.compact())
            flags = r.u32()
            size = r.compact()
            offset = r.compact() if size > 0 else 0
            self.exports.append((class_ref, obj_name, flags, size, offset))

    def name(self, index: int) -> str:
        if not 0 <= index < len(self.names):
            raise _FormatError("nombre fuera de rango")
        return self.names[index]

    def class_name(self, export: tuple) -> str:
        ref = export[0]
        if ref < 0 and -ref - 1 < len(self.imports):
            return self.imports[-ref - 1]
        if 0 < ref <= len(self.exports):
            return self.exports[ref - 1][1]
        return "Class" if ref == 0 else ""

    def export(self, ref: int) -> tuple:
        if not 0 < ref <= len(self.exports):
            raise _FormatError("referencia de objeto inválida")
        return self.exports[ref - 1]

    def open_object(self, export: tuple) -> _Reader:
        _, _, flags, size, offset = export
        if size <= 0 or offset < 0 or offset + size > len(self.data):
            raise _FormatError("objeto fuera del archivo")
        r = _Reader(self.data[offset : offset + size])
        if flags & RF_HAS_STACK:
            node = r.compact()
            r.compact()  # state node
            r.take(8)  # probe mask
            r.take(4)  # latent action
            if node != 0:
                r.compact()  # offset
        return r

    def properties(self, r: _Reader) -> dict[str, tuple[int, bytes]]:
        """Propiedades etiquetadas (solo índice de arreglo 0) hasta "None"."""
        props: dict[str, tuple[int, bytes]] = {}
        while True:
            name = self.name(r.compact())
            if name == "None":
                return props
            info = r.u8()
            ptype = info & 0x0F
            size_code = (info >> 4) & 0x07
            is_array = info & 0x80
            if ptype == PROP_STRUCT:
                r.compact()  # nombre del struct
            if size_code in PROP_SIZES:
                size = PROP_SIZES[size_code]
            elif size_code == 5:
                size = r.u8()
            elif size_code == 6:
                size = struct.unpack("<H", r.take(2))[0]
            else:
                size = r.u32()
            index = 0
            if is_array and ptype != PROP_BOOL:
                b = r.u8()
                if b & 0x80 == 0:
                    index = b
                elif b & 0xC0 == 0x80:
                    index = ((b & 0x7F) << 8) | r.u8()
                else:
                    index = ((b & 0x3F) << 24) | int.from_bytes(r.take(3), "big")
            payload = r.take(size)
            if ptype == PROP_BOOL:
                payload = b"\x01" if is_array else b"\x00"
            if index == 0 and name not in props:
                props[name] = (ptype, payload)

    @staticmethod
    def object_ref(prop: tuple[int, bytes] | None) -> int:
        if prop is None or prop[0] != PROP_OBJECT:
            return 0
        return _Reader(prop[1]).compact()


def _screenshot_export(pkg: _Package) -> tuple | None:
    for exp in pkg.exports:
        if pkg.class_name(exp) != "LevelInfo":
            continue
        try:
            ref = pkg.object_ref(pkg.properties(pkg.open_object(exp)).get("Screenshot"))
            if ref > 0:
                return pkg.export(ref)
        except _FormatError:
            continue
    for exp in pkg.exports:
        if exp[1].casefold() == "screenshot" and pkg.class_name(exp) == "Texture":
            return exp
    return None


def _int_prop(props: dict, name: str, default: int) -> int:
    prop = props.get(name)
    if prop is None:
        return default
    ptype, payload = prop
    if ptype == PROP_BYTE and len(payload) >= 1:
        return payload[0]
    if ptype == PROP_INT and len(payload) == 4:
        return struct.unpack("<i", payload)[0]
    return default


def _palette(pkg: _Package, ref: int) -> bytes:
    exp = pkg.export(ref)
    if pkg.class_name(exp) != "Palette":
        raise _FormatError("la paleta no es un Palette")
    r = pkg.open_object(exp)
    pkg.properties(r)
    count = r.compact()
    if not 0 < count <= 256:
        raise _FormatError("paleta inválida")
    rgba = r.take(count * 4)
    rgb = b"".join(rgba[i : i + 3] for i in range(0, len(rgba), 4))
    return rgb + b"\0" * (768 - len(rgb))


def _decode_texture(pkg: _Package, exp: tuple) -> Image.Image:
    if pkg.class_name(exp) != "Texture":
        raise _FormatError("la captura no es una Texture")
    r = pkg.open_object(exp)
    props = pkg.properties(r)
    fmt = _int_prop(props, "Format", TEXF_P8)
    if r.compact() < 1:
        raise _FormatError("textura sin mips")
    if pkg.version >= 63:
        r.i32()  # skip offset del lazy array
    size = r.compact()
    if size < 0:
        raise _FormatError("tamaño de mip inválido")
    data = r.take(size)
    width, height = r.i32(), r.i32()
    if not (0 < width <= MAX_DIMENSION and 0 < height <= MAX_DIMENSION):
        raise _FormatError("dimensiones absurdas")

    if fmt == TEXF_P8:
        if size < width * height:
            raise _FormatError("mip P8 incompleto")
        ref = pkg.object_ref(props.get("Palette"))
        img = Image.frombytes("P", (width, height), bytes(data[: width * height]))
        img.putpalette(_palette(pkg, ref))
        return img.convert("RGB")
    if fmt == TEXF_DXT1:
        expected = ((width + 3) // 4) * ((height + 3) // 4) * 8
        if size < expected:
            raise _FormatError("mip DXT1 incompleto")
        img = Image.frombytes("RGBA", (width, height), bytes(data[:expected]), "bcn", 1)
        return img.convert("RGB")
    if fmt == TEXF_RGBA8:
        expected = width * height * 4
        if size < expected:
            raise _FormatError("mip RGBA8 incompleto")
        img = Image.frombytes("RGBA", (width, height), bytes(data[:expected]), "raw", "BGRA")
        return img.convert("RGB")
    raise _FormatError(f"formato de textura no soportado: {fmt}")


def extract_screenshot(unr_path: str) -> Image.Image | None:
    """Imagen RGB de la propiedad Screenshot del LevelInfo del mapa, o None.

    Nunca lanza excepciones: archivo ausente, truncado o con formato
    inesperado devuelve None.
    """
    try:
        with open(unr_path, "rb") as f, mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ) as data:
            pkg = _Package(data)
            exp = _screenshot_export(pkg)
            if exp is None:
                return None
            return _decode_texture(pkg, exp)
    except Exception as exc:  # cualquier problema de formato o E/S: sin imagen
        log.debug("Sin captura para %s: %s", unr_path, exc)
        return None


def find_map_file(install_dir: str | None, map_name: str) -> str | None:
    """Ruta de Maps/<map_name>.unr (sin distinguir mayúsculas), o None.

    La caché (Cache/, archivos con nombre GUID) se ignora.
    """
    if not map_name or "/" in map_name or "\\" in map_name:
        return None
    game_dir = resolve_game_dir(install_dir, "Maps")
    if game_dir is None:
        return None
    maps_dir = _child(game_dir, "Maps")
    if maps_dir is None:
        return None
    base = map_name[:-4] if map_name.casefold().endswith(".unr") else map_name
    found = _child(maps_dir, base + ".unr")
    return found if found and os.path.isfile(found) else None
