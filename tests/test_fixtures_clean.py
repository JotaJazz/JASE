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
