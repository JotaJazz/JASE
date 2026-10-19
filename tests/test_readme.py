import re
from pathlib import Path

import pytest

from jase.games import GAMES

ROOT = Path(__file__).resolve().parent.parent
READMES = ("README.md", "README.es.md")
LINK = re.compile(r"!?\[[^\]]*\]\(([^)\s]+)\)")


def _text(name):
    return (ROOT / name).read_text(encoding="utf-8")


@pytest.mark.parametrize("name", READMES)
def test_readme_links_exist(name):
    for target in LINK.findall(_text(name)):
        if re.match(r"^([a-z]+:|#)", target):
            continue
        path = target.split("#")[0]
        assert (ROOT / path).exists(), f"{name}: {target} no existe"


def test_readmes_cross_link():
    assert "(README.es.md)" in _text("README.md")
    assert "(README.md)" in _text("README.es.md")


@pytest.mark.parametrize("name", READMES)
def test_readmes_list_all_games(name):
    text = _text(name)
    for game in GAMES.values():
        assert game.name in text, f"{name}: falta {game.name}"


@pytest.mark.parametrize("name", READMES)
def test_readme_no_stale_ui_symbols(name):
    text = _text(name)
    assert "⟳" not in text and "⚙" not in text


@pytest.mark.parametrize(
    "name,pattern",
    [("README.md", r"(\d+) supported games"), ("README.es.md", r"(\d+) juegos soportados")],
)
def test_readme_game_count(name, pattern):
    match = re.search(pattern, _text(name))
    assert match and int(match.group(1)) == len(GAMES)


@pytest.mark.parametrize("name", READMES)
def test_readmes_list_all_exes(name):
    text = _text(name)
    for game in GAMES.values():
        exe = game.exe_relpath.replace("\\", "/").rsplit("/", 1)[-1]
        assert exe in text, f"{name}: falta {exe}"


@pytest.mark.parametrize("name", READMES)
def test_readme_download_link_lists_all_releases(name):
    text = _text(name)
    assert "https://github.com/JotaJazz/JASE/releases)" in text
    assert "/releases/latest" not in text
