import struct
import zipfile
from pathlib import Path

import pytest

from jase.maps import cod, iwi

RED_DXT1 = struct.pack("<HHI", 0xF800, 0x0000, 0)  # 4x4 todo color0 = rojo
GREEN_DXT1 = struct.pack("<HHI", 0x07E0, 0x0000, 0)


def make_iwi(fmt: int, w: int, h: int, mips: list[bytes]) -> bytes:
    """IWI v5: mips del más chico al más grande; offsets = fin de cada mip."""
    header_len = 28
    body = b"".join(mips)
    ends = []
    pos = header_len
    for m in mips:
        pos += len(m)
        ends.append(pos)
    ends = list(reversed(ends))  # offsets[0] = fin del mip más grande
    ends += [ends[-1]] * (4 - len(ends))
    header = struct.pack("<3sBBBHHH", b"IWi", 5, fmt, 0, w, h, 1)
    header += struct.pack("<4I", *ends[:4])
    return header + body


def test_decode_dxt1_takes_largest_mip():
    data = make_iwi(0x0B, 4, 4, [GREEN_DXT1, RED_DXT1])
    img = iwi.decode_iwi(data)
    assert img.size == (4, 4)
    assert img.convert("RGBA").getpixel((0, 0)) == (255, 0, 0, 255)
    assert img.convert("RGBA").getpixel((3, 3)) == (255, 0, 0, 255)


def test_decode_dxt5_single_mip():
    alpha = bytes([255, 255]) + bytes(6)  # alpha0=255, indices 0
    color = struct.pack("<HHI", 0x001F, 0x0000, 0)  # azul
    img = iwi.decode_iwi(make_iwi(0x0D, 4, 4, [alpha + color]))
    assert img.convert("RGBA").getpixel((1, 2)) == (0, 0, 255, 255)


def test_decode_argb32_bgra_order():
    pixel = bytes([255, 0, 0, 255])  # B, G, R, A → azul opaco
    img = iwi.decode_iwi(make_iwi(0x01, 4, 4, [pixel * 16]))
    assert img.size == (4, 4)
    assert img.convert("RGBA").getpixel((2, 1)) == (0, 0, 255, 255)


def test_decode_rgb24_bgr_order():
    pixel = bytes([0, 255, 0])  # B, G, R → verde
    img = iwi.decode_iwi(make_iwi(0x02, 4, 4, [pixel * 16]))
    assert img.convert("RGB").getpixel((0, 3)) == (0, 255, 0)


def test_unknown_format_raises():
    with pytest.raises(iwi.IwiError):
        iwi.decode_iwi(make_iwi(0x7F, 4, 4, [bytes(64)]))


@pytest.mark.parametrize(
    "data",
    [
        b"",
        b"IWi",
        b"XYZ" + bytes(40),
        make_iwi(0x0B, 4, 4, [RED_DXT1])[:-3],  # truncado
        make_iwi(0x0B, 0, 4, [RED_DXT1]),  # ancho cero
        make_iwi(0x0B, 60000, 60000, [RED_DXT1]),  # absurdo
    ],
)
def test_malformed_raises_iwi_error(data):
    with pytest.raises(iwi.IwiError):
        iwi.decode_iwi(data)


def make_zip(path: Path, entries: dict[str, bytes]) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as z:
        for name, data in entries.items():
            z.writestr(name, data)
    return path


def test_find_loadscreen_last_alphabetical_wins(tmp_path):
    make_zip(tmp_path / "main" / "iw_00.iwd", {"images/loadscreen_mp_toujane.iwi": b"A"})
    make_zip(tmp_path / "main" / "iw_07.iwd", {"images/loadscreen_mp_toujane.iwi": b"B"})
    make_zip(tmp_path / "main" / "iw_03.iwd", {"images/loadscreen_mp_toujane.iwi": b"C"})
    assert cod.find_loadscreen(str(tmp_path), "mp_toujane") == b"B"


def test_find_loadscreen_case_insensitive_and_pk3(tmp_path):
    make_zip(tmp_path / "Main" / "pak0.PK3", {"Images/LoadScreen_MP_Carentan.IWI": b"X"})
    assert cod.find_loadscreen(str(tmp_path), "mp_carentan") == b"X"
    assert cod.find_loadscreen(str(tmp_path), "MP_CARENTAN") == b"X"


def test_find_loadscreen_in_mods_overrides_main(tmp_path):
    make_zip(tmp_path / "main" / "iw_00.iwd", {"images/loadscreen_mp_foo.iwi": b"MAIN"})
    make_zip(tmp_path / "mods" / "x" / "z.iwd", {"images/loadscreen_mp_foo.iwi": b"MOD"})
    make_zip(tmp_path / "mods" / "x" / "y.iwd", {"other.txt": b""})
    make_zip(tmp_path / "mods" / "y" / "only.iwd", {"images/loadscreen_mp_bar.iwi": b"BAR"})
    assert cod.find_loadscreen(str(tmp_path), "mp_foo") == b"MOD"
    assert cod.find_loadscreen(str(tmp_path), "mp_bar") == b"BAR"


def test_find_loadscreen_ignores_corrupt_and_dirs(tmp_path):
    make_zip(tmp_path / "main" / "iw_00.iwd", {"images/loadscreen_mp_foo.iwi": b"OK"})
    (tmp_path / "main" / "zz_corrupt.iwd").write_bytes(b"esto no es un zip")
    (tmp_path / "main" / "zz_truncated.iwd").write_bytes(
        (tmp_path / "main" / "iw_00.iwd").read_bytes()[:40]
    )
    (tmp_path / "main" / "zzz_dir.iwd").mkdir()
    assert cod.find_loadscreen(str(tmp_path), "mp_foo") == b"OK"


def test_find_loadscreen_resolves_nested_and_parent(tmp_path):
    make_zip(tmp_path / "CoD2" / "main" / "iw_00.iwd", {"images/loadscreen_mp_foo.iwi": b"N"})
    assert cod.find_loadscreen(str(tmp_path), "mp_foo") == b"N"
    assert cod.find_loadscreen(str(tmp_path / "CoD2" / "main"), "mp_foo") == b"N"


@pytest.mark.parametrize("install", [None, "", "/no/existe/jase"])
def test_find_loadscreen_missing_install(install):
    assert cod.find_loadscreen(install, "mp_foo") is None


def test_find_loadscreen_not_found_and_bad_names(tmp_path):
    make_zip(tmp_path / "main" / "iw_00.iwd", {"images/loadscreen_mp_foo.iwi": b"OK"})
    assert cod.find_loadscreen(str(tmp_path), "mp_nope") is None
    assert cod.find_loadscreen(str(tmp_path), "") is None
    assert cod.find_loadscreen(str(tmp_path), "../mp_foo") is None
