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
