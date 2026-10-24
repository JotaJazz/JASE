import io
import json
import urllib.error

import pytest
from PIL import Image

from jase.config import Config, load_config, save_config
from jase.games import GAMES
from jase.maps import cod, iwi, service, unreal
from jase.maps.service import MapImageService, gametracker_map_name, gametracker_url, safe_map_filename

COD2 = GAMES["cod2"]
UT99 = GAMES["ut99"]


def png_bytes(color=(255, 0, 0), size=(8, 8), fmt="PNG") -> bytes:
    buf = io.BytesIO()
    Image.new("RGB", size, color).save(buf, fmt)
    return buf.getvalue()


def load(path) -> Image.Image:
    with Image.open(path) as img:
        img.load()
        return img


class FakeFetch:
    def __init__(self, result=None, error=None):
        self.result = result
        self.error = error
        self.urls = []

    def __call__(self, url):
        self.urls.append(url)
        if self.error:
            raise self.error
        return self.result


@pytest.fixture
def local_cod(monkeypatch):
    """Extractor de CoD falso con contador; `images` = mapa -> imagen PIL."""
    calls = []
    images = {}

    def fake_find(install_dir, map_name):
        calls.append((install_dir, map_name))
        return b"IWI:" + map_name.encode() if map_name in images else None

    def fake_decode(data):
        return images[data[4:].decode()]

    monkeypatch.setattr(cod, "find_loadscreen", fake_find)
    monkeypatch.setattr(iwi, "decode_iwi", fake_decode)
    return calls, images


def test_gametracker_ids():
    assert GAMES["cod2"].gametracker_id == "cod2"
    assert GAMES["cod1"].gametracker_id == "cod"
    assert GAMES["ut99"].gametracker_id == ""


def test_local_extraction_is_cached_as_png(tmp_path, local_cod):
    calls, images = local_cod
    images["mp_toujane"] = Image.new("RGB", (16, 16), (0, 255, 0))
    svc = MapImageService(tmp_path / "cache", fetch=FakeFetch())
    path = svc.get(COD2, "D:/CoD2", "mp_toujane", allow_download=False)
    assert path == tmp_path / "cache" / "cod2" / "mp_toujane.png"
    assert load(path).format == "PNG"
    assert len(calls) == 1
    # Caché positiva en disco: ni otra instancia vuelve a extraer.
    svc2 = MapImageService(tmp_path / "cache", fetch=FakeFetch())
    assert svc2.get(COD2, "D:/CoD2", "mp_toujane", allow_download=False) == path
    assert svc.get(COD2, "D:/CoD2", "mp_toujane", allow_download=False) == path
    assert len(calls) == 1


def test_cod_loadscreen_is_stored_as_4_3(tmp_path, local_cod):
    # Las pantallas de carga son cuadradas pero el juego las estira a la pantalla (4:3).
    _, images = local_cod
    images["mp_x"] = Image.new("RGB", (1024, 1024), (1, 2, 3))
    path = MapImageService(tmp_path, fetch=FakeFetch()).get(COD2, "d", "mp_x", False)
    w, h = load(path).size
    assert w * 3 == h * 4 and w <= 640


def test_local_has_priority_over_download(tmp_path, local_cod):
    _, images = local_cod
    images["mp_toujane"] = Image.new("RGB", (4, 4), (0, 0, 255))
    fetch = FakeFetch(png_bytes())
    path = MapImageService(tmp_path, fetch=fetch).get(COD2, "d", "mp_toujane", True)
    assert path is not None and fetch.urls == []
    assert load(path).convert("RGB").getpixel((0, 0)) == (0, 0, 255)


def test_download_only_when_allowed(tmp_path, local_cod):
    fetch = FakeFetch(png_bytes((10, 20, 30), fmt="JPEG"))
    svc = MapImageService(tmp_path, fetch=fetch)
    assert svc.get(COD2, "d", "mp_custom", allow_download=False) is None
    assert fetch.urls == []
    path = svc.get(COD2, "d", "mp_custom", allow_download=True)
    assert fetch.urls == ["https://image.gametracker.com/images/maps/160x120/cod2/mp_custom.jpg"]
    assert path == tmp_path / "cod2" / "mp_custom.png"
    assert load(path).format == "PNG"


def test_no_download_without_gametracker_id(tmp_path, monkeypatch):
    monkeypatch.setattr(unreal, "find_map_file", lambda d, m: None)
    fetch = FakeFetch(png_bytes())
    assert MapImageService(tmp_path, fetch=fetch).get(UT99, "d", "DM-X", True) is None
    assert fetch.urls == []


@pytest.mark.parametrize("fetch", [
    FakeFetch(None),  # 404 / error de red (el fetch por defecto devuelve None)
    FakeFetch(error=OSError("sin red")),
    FakeFetch(b"<html>not an image</html>"),
])
def test_download_failure_is_remembered_in_memory(tmp_path, local_cod, fetch):
    calls, _ = local_cod
    svc = MapImageService(tmp_path, fetch=fetch)
    assert svc.get(COD2, "d", "mp_missing", True) is None
    assert svc.get(COD2, "d", "mp_missing", True) is None
    assert len(fetch.urls) == 1
    assert len(calls) == 1  # la extracción local fallida también se recuerda
    # Nada se guarda en disco: otra sesión vuelve a intentarlo.
    assert not (tmp_path / "cod2" / "mp_missing.png").exists()
    svc2 = MapImageService(tmp_path, fetch=fetch)
    assert svc2.get(COD2, "d", "mp_missing", True) is None
    assert len(fetch.urls) == 2


def test_local_miss_then_enabling_download_tries_download(tmp_path, local_cod):
    fetch = FakeFetch(png_bytes())
    svc = MapImageService(tmp_path, fetch=fetch)
    assert svc.get(COD2, "d", "mp_late", False) is None
    assert svc.get(COD2, "d", "mp_late", True) is not None
    assert len(fetch.urls) == 1


def test_changed_install_dir_retries_local(tmp_path, local_cod):
    calls, _ = local_cod
    svc = MapImageService(tmp_path, fetch=FakeFetch())
    svc.get(COD2, "a", "mp_x", False)
    svc.get(COD2, "b", "mp_x", False)
    assert [c[0] for c in calls] == ["a", "b"]


def test_corrupt_local_image_gives_none(tmp_path, monkeypatch):
    monkeypatch.setattr(cod, "find_loadscreen", lambda d, m: b"garbage")
    svc = MapImageService(tmp_path, fetch=FakeFetch())
    assert svc.get(COD2, "d", "mp_x", False) is None  # IwiError real, sin excepción


def test_extractor_exception_gives_none(tmp_path, monkeypatch):
    def boom(d, m):
        raise RuntimeError("inesperado")

    monkeypatch.setattr(cod, "find_loadscreen", boom)
    assert MapImageService(tmp_path, fetch=FakeFetch()).get(COD2, "d", "mp_x", False) is None


def test_ut99_uses_unreal_screenshot(tmp_path, monkeypatch):
    monkeypatch.setattr(unreal, "find_map_file", lambda d, m: f"{d}/Maps/{m}.unr")
    monkeypatch.setattr(unreal, "extract_screenshot",
                        lambda p: Image.new("RGB", (8, 8), (9, 9, 9)))
    path = MapImageService(tmp_path, fetch=FakeFetch()).get(UT99, "d", "DM-Deck16][", False)
    assert path is not None and path.parent == tmp_path / "ut99"
    assert load(path).size == (8, 8)


def test_unsafe_map_name_gives_valid_filename(tmp_path, monkeypatch):
    monkeypatch.setattr(unreal, "find_map_file", lambda d, m: "x.unr")
    monkeypatch.setattr(unreal, "extract_screenshot", lambda p: Image.new("RGB", (2, 2)))
    path = MapImageService(tmp_path, fetch=FakeFetch()).get(UT99, "d", 'DM-a][b:c/d\\e*?"<>|', False)
    assert path is not None and path.exists() and path.parent == tmp_path / "ut99"
    assert not set(path.name) & set('\\/:*?"<>|')
    assert path.name == "dm-a][bcde.png"


def test_safe_map_filename():
    assert safe_map_filename("DM-Deck16][") == "dm-deck16]["
    assert safe_map_filename("a:b") == "ab"
    assert safe_map_filename("con") == "con_"  # nombre reservado de Windows
    assert safe_map_filename(" x. ") == "x"
    assert safe_map_filename(':/*') is None
    assert safe_map_filename("") is None


def test_empty_map_name_gives_none(tmp_path):
    fetch = FakeFetch(png_bytes())
    assert MapImageService(tmp_path, fetch=fetch).get(COD2, "d", "", True) is None
    assert fetch.urls == []


@pytest.mark.parametrize("raw,expected", [
    ("dm-deck16][", "dm_deck16__"),
    ("DM-Rankin", "dm_rankin"),
    ("obj/obj_team2", "obj_obj_team2"),
    ("mp_a b", "mp_a_b"),
    ("de_dust2", "de_dust2"),
])
def test_gametracker_map_name(raw, expected):
    assert gametracker_map_name(raw) == expected


def test_gametracker_url_normalizes_name():
    assert gametracker_url("ut2k4", "DM-Rankin") == (
        "https://image.gametracker.com/images/maps/160x120/ut2k4/dm_rankin.jpg")


def test_default_fetch_errors_give_none(monkeypatch):
    def fail(req, timeout):
        assert timeout == 5
        assert req.get_header("User-agent") == "JASE"
        raise urllib.error.HTTPError(req.full_url, 404, "Not Found", {}, None)

    monkeypatch.setattr(service.urllib.request, "urlopen", fail)
    assert service.default_fetch("https://example.invalid/x.jpg") is None

    def timeout(req, timeout):
        raise TimeoutError()

    monkeypatch.setattr(service.urllib.request, "urlopen", timeout)
    assert service.default_fetch("https://example.invalid/x.jpg") is None


def test_unwritable_cache_gives_none(tmp_path, local_cod):
    _, images = local_cod
    images["mp_x"] = Image.new("RGB", (4, 4))
    blocker = tmp_path / "file"
    blocker.write_text("no soy carpeta")
    assert MapImageService(blocker, fetch=FakeFetch()).get(COD2, "d", "mp_x", False) is None


def test_config_download_map_images_roundtrip(tmp_path):
    p = tmp_path / "c.json"
    cfg = load_config(p)
    assert cfg.download_map_images is False and Config().download_map_images is False
    cfg.download_map_images = True
    save_config(cfg, p)
    assert load_config(p).download_map_images is True
    p.write_text(json.dumps({"download_map_images": "si", "theme": "dark"}))
    cfg = load_config(p)
    assert cfg.download_map_images is False and cfg.theme == "dark"


def test_settings_dialog_download_checkbox(qtbot):
    from PySide6.QtWidgets import QCheckBox

    from jase.ui.dialogs import SettingsDialog

    cfg = Config()
    d = SettingsDialog(cfg)
    qtbot.addWidget(d)
    check = d.findChild(QCheckBox, "download_map_images")
    assert check.text() == "Descargar imágenes de mapas desde internet (GameTracker)"
    assert not check.isChecked()
    check.setChecked(True)
    d.apply_to(cfg)
    assert cfg.download_map_images is True
    d2 = SettingsDialog(cfg)
    qtbot.addWidget(d2)
    assert d2.findChild(QCheckBox, "download_map_images").isChecked()


def test_store_returns_existing_file_when_replace_fails(tmp_path, monkeypatch):
    path = tmp_path / "cod2" / "mp_x.png"
    path.parent.mkdir()
    path.write_bytes(png_bytes())

    def locked(src, dst):  # Windows: destino abierto por otro proceso/hilo
        raise PermissionError("en uso")

    monkeypatch.setattr(service.os, "replace", locked)
    assert MapImageService._store(Image.new("RGB", (4, 4)), path) == path
    assert sorted(p.name for p in path.parent.iterdir()) == ["mp_x.png"]  # sin temporales
    path.unlink()
    assert MapImageService._store(Image.new("RGB", (4, 4)), path) is None


class FakeResponse:
    def __init__(self, chunks):
        self.chunks = list(chunks)

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False

    def read(self, n=-1):
        return self.chunks.pop(0) if self.chunks else b""


def test_default_fetch_reads_in_chunks(monkeypatch):
    monkeypatch.setattr(service.urllib.request, "urlopen",
                        lambda req, timeout: FakeResponse([b"ab", b"cd"]))
    assert service.default_fetch("https://x.invalid/a.jpg") == b"abcd"


def test_default_fetch_overall_deadline(monkeypatch):
    clock = [0.0]

    def fake_monotonic():
        clock[0] += 1.0  # cada lectura "tarda" 1 s
        return clock[0]

    class Trickle(FakeResponse):
        def read(self, n=-1):
            return b"x"  # nunca termina

    monkeypatch.setattr(service.time, "monotonic", fake_monotonic)
    monkeypatch.setattr(service.urllib.request, "urlopen", lambda req, timeout: Trickle([]))
    assert service.default_fetch("https://x.invalid/a.jpg") is None
    assert clock[0] <= service.FETCH_DEADLINE + 3
