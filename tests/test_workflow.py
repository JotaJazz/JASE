import re
from pathlib import Path

from jase.games import GAMES

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
NOTES = ROOT / "packaging" / "release-notes.md"


def _wf() -> str:
    return WORKFLOW.read_text(encoding="utf-8")


def test_workflow_triggers():
    text = _wf()
    assert re.search(r"^on:\s*\n\s+push:\s*\n\s+tags:\s*\n\s+- ['\"]?v\*['\"]?\s*$", text, re.M)
    assert re.search(r"^\s+workflow_dispatch:", text, re.M)
    assert re.search(r"^permissions:\s*\n\s+contents: read\s*$", text, re.M)
    assert text.count("contents: write") == 1
    assert "windows-latest" in text


def test_workflow_runs_tests_and_smoke():
    text = _wf()
    assert "pytest -q -W error" in text
    assert "QT_QPA_PLATFORM: offscreen" in text
    assert "pyinstaller jase.spec" in text
    assert r"dist\JASE\JASE.exe --smoke" in text and "SMOKE OK" in text
    assert "packaging/licenses" in text and r"dist\JASE\licenses" in text
    assert "Compress-Archive" in text and "JASE-${{ steps.ver.outputs.version }}-win64.zip" in text
    assert "pip install -e" not in text  # metadata real: instalación no editable
    # Orden: pruebas, build, smoke, licencias, zip.
    order = ["pytest -q -W error", "pyinstaller jase.spec", "--smoke", "packaging/licenses", "Compress-Archive"]
    pos = [text.index(s) for s in order]
    assert pos == sorted(pos)


def test_workflow_release_is_draft_prerelease():
    text = _wf()
    assert re.search(r"^\s+draft: true\s*$", text, re.M)
    assert re.search(r"^\s+prerelease: true\s*$", text, re.M)
    assert "body_path: packaging/release-notes.md" in text
    assert "softprops/action-gh-release@" in text
    assert "startsWith(github.ref, 'refs/tags/v')" in text
    assert "upload-artifact@" in text


def test_workflow_pins_versions():
    text = _wf()
    assert 'python-version: "3.13"' in text
    assert "PySide6==6.12.0" in text
    assert re.search(r"\.\[build,dev\]", text)
    pyproject = (ROOT / "pyproject.toml").read_text(encoding="utf-8")
    assert re.search(r"pyinstaller==\d+\.\d+\.\d+", pyproject)
    uses = re.findall(r"^\s+(?:-\s+)?uses:\s*(\S+)", text, re.M)
    assert uses
    for ref in uses:
        assert re.fullmatch(r"[\w./-]+@[0-9a-f]{40}", ref), f"acción sin SHA fijo: {ref}"


def test_release_notes_bilingual_and_lists_games():
    text = NOTES.read_text(encoding="utf-8")
    assert text.count("<!-- ES -->") == 1
    en, es = text.split("<!-- ES -->")
    for half in (en, es):
        for game in GAMES.values():
            assert game.name in half, game.name
        assert "SmartScreen" in half
        assert "%APPDATA%\\JimmysASE" in half
        assert "Issues" in half
    assert "More info" in en and "Run anyway" in en
    assert "Más información" in es and "Ejecutar de todas formas" in es
    for needle in ("BF1942", "CS 1.6", "SoF2", "JKA"):
        assert needle in en
