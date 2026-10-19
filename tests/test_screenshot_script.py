import importlib.util
import ipaddress
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC_NETS = [ipaddress.ip_network(n) for n in
            ("192.0.2.0/24", "198.51.100.0/24", "203.0.113.0/24")]


def load_script():
    spec = importlib.util.spec_from_file_location(
        "screenshot_script", ROOT / "scripts" / "screenshot.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_screenshot_fixture_uses_documentation_ips():
    servers = load_script().sample_servers()
    assert len(servers) >= 10
    for s in servers:
        for addr in (s.address, s.connect_address):
            ip = ipaddress.ip_address(addr.host)
            assert any(ip in net for net in DOC_NETS), addr.host


def test_screenshot_script_writes_png(tmp_path):
    from PySide6.QtGui import QImage
    out = tmp_path / "shot.png"
    assert load_script().main(["--lang", "en", "--out", str(out)]) == 0
    assert QImage(str(out)).width() == 1280


def test_banner_svg_is_self_contained():
    root = ET.parse(ROOT / "docs" / "banner.svg").getroot()
    assert root.get("viewBox") == "0 0 1280 320"
    texts = {"".join(t.itertext()) for t in root.iter("{http://www.w3.org/2000/svg}text")}
    for wanted in ("JASE", "Jimmy's All Seeing Eye",
                   "The server browser your classic shooters still deserve"):
        assert wanted in texts
    for el in root.iter():
        for key, value in el.attrib.items():
            if key.endswith("href"):
                assert value.startswith("#"), value
        assert not el.tag.endswith("}use") or el.get("href", "").startswith("#")
