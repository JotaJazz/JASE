import struct
from pathlib import Path

from jase.maps import unreal

RF_HAS_STACK = 0x02000000


def compact(value: int) -> bytes:
    """FCompactIndex de Unreal: signo + 6 bits, luego grupos de 7 bits."""
    neg = value < 0
    v = abs(value)
    first = (0x80 if neg else 0) | (v & 0x3F)
    v >>= 6
    if v:
        first |= 0x40
    out = bytearray([first])
    while v:
        b = v & 0x7F
        v >>= 7
        out.append(b | (0x80 if v else 0))
    return bytes(out)


class PackageBuilder:
    """Paquete Unreal mínimo (versión 68) para tests."""

    def __init__(self):
        self.names: list[str] = ["None"]
        self.imports: list[tuple[str, str, int, str]] = []
        self.exports: list[dict] = []

    def name(self, text: str) -> int:
        if text not in self.names:
            self.names.append(text)
        return self.names.index(text)

    def class_ref(self, class_name: str) -> int:
        for key in ("Core", "Class", class_name):
            self.name(key)
        entry = ("Core", "Class", 0, class_name)
        if entry not in self.imports:
            self.imports.append(entry)
        return -(self.imports.index(entry) + 1)

    def add_export(self, class_name: str, obj_name: str, body: bytes, flags: int = 0) -> int:
        """Agrega un export y devuelve su referencia (índice + 1)."""
        self.exports.append(
            {
                "class": self.class_ref(class_name),
                "name": self.name(obj_name),
                "flags": flags,
                "body": body,
            }
        )
        return len(self.exports)

    def prop(self, name: str, ptype: int, payload: bytes) -> bytes:
        size_codes = {1: 0, 2: 1, 4: 2, 12: 3, 16: 4}
        if len(payload) in size_codes:
            info = ptype | (size_codes[len(payload)] << 4)
            size = b""
        else:
            info = ptype | (5 << 4)
            size = bytes([len(payload)])
        return compact(self.name(name)) + bytes([info]) + size + payload

    def none(self) -> bytes:
        return compact(self.name("None"))

    def build(self) -> bytes:
        header_size = 64
        names = b"".join(
            compact(len(n) + 1) + n.encode("latin-1") + b"\0" + struct.pack("<I", 0)
            for n in self.names
        )
        imports = b"".join(
            compact(self.names.index(cp))
            + compact(self.names.index(cn))
            + struct.pack("<i", pkg)
            + compact(self.names.index(on))
            for cp, cn, pkg, on in self.imports
        )
        name_off = header_size
        import_off = name_off + len(names)
        bodies_off = import_off + len(imports)
        bodies = b""
        export_table = b""
        for exp in self.exports:
            offset = bodies_off + len(bodies)
            bodies += exp["body"]
            export_table += (
                compact(exp["class"])
                + compact(0)
                + struct.pack("<i", 0)
                + compact(exp["name"])
                + struct.pack("<I", exp["flags"])
                + compact(len(exp["body"]))
                + compact(offset)
            )
        export_off = bodies_off + len(bodies)
        header = struct.pack(
            "<IHHIIIIIII",
            0x9E2A83C1,
            68,
            0,
            0,
            len(self.names),
            name_off,
            len(self.exports),
            export_off,
            len(self.imports),
            import_off,
        )
        header = header.ljust(header_size, b"\0")
        return header + names + imports + bodies + export_table


def palette_body(b: PackageBuilder, colors: list[tuple[int, int, int]]) -> bytes:
    entries = list(colors) + [(0, 0, 0)] * (256 - len(colors))
    data = b"".join(bytes([r, g, bl, 0]) for r, g, bl in entries)
    return b.none() + compact(256) + data


def texture_body(
    b: PackageBuilder,
    width: int,
    height: int,
    pixels: bytes,
    palette_ref: int | None,
    fmt: int | None = None,
    body_offset: int = 0,
) -> bytes:
    props = b""
    if palette_ref is not None:
        props += b.prop("Palette", 5, compact(palette_ref))
    if fmt is not None:
        props += b.prop("Format", 1, bytes([fmt]))
    props += b.prop("USize", 2, struct.pack("<i", width))
    props += b.prop("VSize", 2, struct.pack("<i", height))
    props += b.none()
    mip_tail = struct.pack("<ii", width, height) + bytes([1, 1])
    # skip offset: posición absoluta al final de los datos del mip
    head = props + compact(1)
    data = compact(len(pixels)) + pixels
    skip = body_offset + len(head) + 4 + len(data)
    return head + struct.pack("<i", skip) + data + mip_tail


RED, GREEN, BLUE, WHITE = (255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255)


def make_map(tmp_path: Path, with_levelinfo: bool = True, name: str = "Shot") -> Path:
    b = PackageBuilder()
    pal = b.add_export("Palette", "Palette0", palette_body(b, [RED, GREEN, BLUE, WHITE]))
    tex_name = name if with_levelinfo else "Screenshot"
    tex = b.add_export("Texture", tex_name, texture_body(b, 2, 2, bytes([0, 1, 2, 3]), pal))
    if with_levelinfo:
        # LevelInfo con stack (Node != 0 → offset extra) como en los mapas reales
        stack = compact(-1) + compact(-1) + struct.pack("<qi", -1, 0) + compact(-1)
        props = b.prop("Title", 13, compact(5) + b"Test\0")
        props += b.prop("Screenshot", 5, compact(tex))
        props += b.none()
        b.add_export("LevelInfo", "LevelInfo0", stack + props, flags=RF_HAS_STACK)
    path = tmp_path / "DM-Test.unr"
    path.write_bytes(b.build())
    return path


def assert_rgb_2x2(img):
    assert img is not None
    assert img.size == (2, 2)
    rgb = img.convert("RGB")
    assert rgb.getpixel((0, 0)) == RED
    assert rgb.getpixel((1, 0)) == GREEN
    assert rgb.getpixel((0, 1)) == BLUE
    assert rgb.getpixel((1, 1)) == WHITE


def test_extracts_p8_screenshot_via_levelinfo(tmp_path):
    assert_rgb_2x2(unreal.extract_screenshot(str(make_map(tmp_path))))


def test_falls_back_to_texture_named_screenshot(tmp_path):
    assert_rgb_2x2(unreal.extract_screenshot(str(make_map(tmp_path, with_levelinfo=False))))


def test_dxt1_texture(tmp_path):
    b = PackageBuilder()
    block = struct.pack("<HHI", 0xF800, 0x0000, 0)  # 4x4 todo rojo
    b.add_export("Texture", "Screenshot", texture_body(b, 4, 4, block, None, fmt=3))
    path = tmp_path / "DM-Dxt.unr"
    path.write_bytes(b.build())
    img = unreal.extract_screenshot(str(path))
    assert img is not None
    assert img.size == (4, 4)
    assert img.convert("RGB").getpixel((3, 3)) == RED


def test_truncated_file_returns_none(tmp_path):
    data = make_map(tmp_path).read_bytes()
    for cut in (0, 3, 20, 64, len(data) // 2, len(data) - 1):
        bad = tmp_path / f"cut{cut}.unr"
        bad.write_bytes(data[:cut])
        assert unreal.extract_screenshot(str(bad)) is None


def test_garbage_and_missing_return_none(tmp_path):
    junk = tmp_path / "junk.unr"
    junk.write_bytes(b"\x00\x01garbage" * 100)
    assert unreal.extract_screenshot(str(junk)) is None
    assert unreal.extract_screenshot(str(tmp_path / "nope.unr")) is None
    assert unreal.extract_screenshot(str(tmp_path)) is None


def test_absurd_dimensions_return_none(tmp_path):
    b = PackageBuilder()
    pal = b.add_export("Palette", "Palette0", palette_body(b, [RED]))
    b.add_export("Texture", "Screenshot", texture_body(b, 100000, 100000, bytes(4), pal))
    path = tmp_path / "DM-Huge.unr"
    path.write_bytes(b.build())
    assert unreal.extract_screenshot(str(path)) is None


def test_no_screenshot_returns_none(tmp_path):
    b = PackageBuilder()
    b.add_export("Palette", "Palette0", palette_body(b, [RED]))
    path = tmp_path / "DM-None.unr"
    path.write_bytes(b.build())
    assert unreal.extract_screenshot(str(path)) is None


def test_find_map_file_nested_and_case_insensitive(tmp_path):
    install = tmp_path / "UnrealTournament"
    maps = install / "UnrealTournament" / "Maps"
    maps.mkdir(parents=True)
    target = maps / "CTF-Face.unr"
    target.write_bytes(b"x")
    (install / "UnrealTournament" / "Cache").mkdir()
    assert unreal.find_map_file(str(install), "CTF-Face") == str(target)
    assert unreal.find_map_file(str(install), "ctf-face") == str(target)
    assert unreal.find_map_file(str(install), "CTF-Face.unr") == str(target)
    assert unreal.find_map_file(str(install / "UnrealTournament"), "CTF-Face") == str(target)


def test_find_map_file_missing(tmp_path):
    (tmp_path / "Maps").mkdir()
    assert unreal.find_map_file(str(tmp_path), "DM-Nope") is None
    assert unreal.find_map_file(str(tmp_path), "") is None
    assert unreal.find_map_file(str(tmp_path), "../x") is None
    assert unreal.find_map_file(None, "DM-Deck16][") is None
    assert unreal.find_map_file(str(tmp_path / "missing"), "DM-Deck16][") is None
