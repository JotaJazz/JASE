import re
from pathlib import Path

FIXTURES = Path(__file__).resolve().parent / "fixtures"
EMAIL = re.compile(rb"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")


def test_fixtures_have_no_email_addresses():
    found = {}
    for f in FIXTURES.rglob("*"):
        if f.is_file():
            for m in EMAIL.findall(f.read_bytes()):
                if not m.endswith(b"admin@example.invalid"):
                    found.setdefault(f.name, []).append(m)
    assert not found, found


def test_fixtures_have_no_steam_keys():
    found = [f.name for f in FIXTURES.rglob("*")
             if f.is_file() and (re.search(rb"[0-9A-F]{32}", f.read_bytes())
                                 or b"key=" in f.read_bytes())]
    assert not found, found
