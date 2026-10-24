import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _project():
    return tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))


def test_description_does_not_enumerate_games():
    # Una lista de juegos en la descripción se queda vieja con cada juego nuevo.
    desc = _project()["project"]["description"]
    assert "(" not in desc and "Call of Duty" not in desc


def test_ruff_configured():
    data = _project()
    assert "ruff" in " ".join(data["project"]["optional-dependencies"]["dev"])
    assert data["tool"]["ruff"]["line-length"] == 100
