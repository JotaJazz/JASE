from PySide6.QtGui import QColor

from jase.protocols.quake3 import strip_colors
from jase.ui import theme
from jase.ui.colored_text import color_runs


def test_runs_split_by_color_code():
    runs = color_runs("^1Ti^2tan", "dark")
    assert [t for t, _ in runs] == ["Ti", "tan"]
    assert runs[0][1] == QColor(theme.Q3_COLORS["dark"]["1"])
    assert runs[1][1] == QColor(theme.Q3_COLORS["dark"]["2"])


def test_plain_text_has_default_color():
    assert color_runs("Plain", "dark") == [("Plain", None)]


def test_white_uses_default_text_color():
    assert color_runs("^7Hola", "light") == [("Hola", None)]
    assert color_runs("^7Hola", "dark") == [("Hola", None)]


def test_black_is_readable_on_dark():
    (_, c), = color_runs("^0Negro", "dark")
    assert c is not None and c.lightness() > 60


def test_text_always_matches_strip_colors():
    for raw in ["^1Jimmy^7 TDM", "^aX^", "a^^1b", "^\x011Nombre", "^9^8^0z", ""]:
        assert "".join(t for t, _ in color_runs(raw, "dark")) == strip_colors(raw)


def test_letter_codes_are_removed_without_color():
    assert color_runs("^aX", "dark") == [("X", None)]


def test_double_caret_codes_render_clean():
    assert color_runs("^^7Sugus", "dark") == [("Sugus", None)]
    (text, color), = color_runs("^^00B E N D E R", "dark")
    assert text == "B E N D E R"
    assert color == color_runs("^0x", "dark")[0][1]
    for raw in ["^^7Sugus", "^^00B E N D E R", "^^71337", "x^^1y"]:
        assert "".join(t for t, _ in color_runs(raw, "dark")) == strip_colors(raw)
