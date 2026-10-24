import io
import zipfile

import pytest
from PIL import Image

from jase.games import GAMES
from jase.maps import levelshots
from jase.maps.levelshots import find_levelshot, valid_map_path
from jase.maps.service import MapImageService


def img_bytes(color=(255, 0, 0), fmt="PNG") -> bytes:
    buf = io.BytesIO()
    Image.new("RGB", (8, 8), color).save(buf, fmt)
    return buf.getvalue()


def make_pk3(path, entries: dict[str, bytes]):
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as z:
        for name, data in entries.items():
            z.writestr(name, data)


def pixel(data: bytes):
    with Image.open(io.BytesIO(data)) as im:
        return im.convert("RGB").getpixel((0, 0))


def test_finds_levelshot_case_insensitive(tmp_path):
    make_pk3(tmp_path / "baseq3" / "pak0.pk3", {"levelshots/q3dm17.jpg": img_bytes(fmt="JPEG")})
    data = find_levelshot(str(tmp_path), "baseq3", "Q3DM17")
    assert data is not None
    Image.open(io.BytesIO(data)).load()


def test_last_pk3_wins(tmp_path):
    make_pk3(tmp_path / "baseq3" / "pak0.pk3", {"levelshots/m.png": img_bytes((255, 0, 0))})
    make_pk3(tmp_path / "baseq3" / "zz.pk3", {"levelshots/m.png": img_bytes((0, 0, 255))})
    assert pixel(find_levelshot(str(tmp_path), "baseq3", "m")) == (0, 0, 255)


def test_extension_order_within_pk3(tmp_path):
    make_pk3(tmp_path / "baseq3" / "a.pk3", {
        "levelshots/m.png": img_bytes((0, 255, 0)),
        "levelshots/m.jpg": img_bytes((255, 0, 0), "JPEG"),
    })
    px = pixel(find_levelshot(str(tmp_path), "baseq3", "m"))
    assert px[0] > 200  # .jpg antes que .png


def test_jka_nested_game_data(tmp_path):
    make_pk3(tmp_path / "GameData" / "base" / "assets1.pk3",
             {"levelshots/mp/ffa1.tga": img_bytes(fmt="TGA")})
    assert find_levelshot(str(tmp_path), "base", "mp/ffa1") is not None


@pytest.mark.parametrize("name", ["../x", "a\\b", "", "c:/x", "a//b", "./x", "a/.."])
def test_invalid_map_names(tmp_path, monkeypatch, name):
    assert valid_map_path(name) is False
    monkeypatch.setattr(levelshots, "read_entry",
                        lambda *a: pytest.fail("no debe abrir archivos"))
    assert find_levelshot(str(tmp_path), "baseq3", name) is None


def test_garbage_pk3_ignored(tmp_path):
    (tmp_path / "baseq3").mkdir()
    (tmp_path / "baseq3" / "zzz.pk3").write_text("basura")
    make_pk3(tmp_path / "baseq3" / "pak0.pk3", {"levelshots/m.png": img_bytes()})
    assert find_levelshot(str(tmp_path), "baseq3", "m") is not None


def test_missing_install_dir():
    assert find_levelshot(None, "baseq3", "m") is None


def test_service_uses_levelshots_for_oa(tmp_path):
    inst = tmp_path / "oa"
    make_pk3(inst / "baseoa" / "pak0.pk3", {"levelshots/oa_dm1.jpg": img_bytes(fmt="JPEG")})
    svc = MapImageService(tmp_path / "cache")
    path = svc.get(GAMES["oa"], str(inst), "oa_dm1", allow_download=False)
    assert path is not None and path.suffix == ".png"
    with Image.open(path) as im:
        assert im.format == "PNG"


def test_levelshot_dirs_defined():
    assert GAMES["jka"].levelshot_dir == "base"
    assert GAMES["cod2"].levelshot_dir == ""
