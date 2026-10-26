from jase.ui import icons, theme


def test_tool_icon_renders_every_svg(qtbot):
    names = sorted(p.stem for p in icons.ICONS_DIR.glob("*.svg"))
    assert names  # los SVG viajan con el paquete
    for name in names:
        for mode in theme.TOKENS:
            assert not icons.tool_icon(name, mode).isNull(), (name, mode)


def test_tool_icon_is_cached_and_missing_is_null(qtbot):
    assert icons.tool_icon("refresh", "dark") is icons.tool_icon("refresh", "dark")
    assert icons.tool_icon("no_such_icon", "dark").isNull()


def test_tool_icon_tints_with_theme_text_and_muted(qtbot):
    from PySide6.QtGui import QColor, QIcon
    icon = icons.tool_icon("stop", "dark")
    for state, token in ((QIcon.Normal, "text"), (QIcon.Disabled, "muted")):
        image = icon.pixmap(icons.ICON_SIZE, state).toImage()
        # Trazo hueco: se toman los píxeles más opacos de toda la imagen.
        pixels = [image.pixelColor(x, y) for x in range(image.width())
                  for y in range(image.height())]
        opaque = [c for c in pixels if c.alpha() >= 250]
        assert opaque
        want = QColor(theme.TOKENS["dark"][token])
        got = opaque[0]  # premultiplicado con alfa ~253: tolerancia de unos niveles
        assert max(abs(got.red() - want.red()), abs(got.green() - want.green()),
                   abs(got.blue() - want.blue())) <= 4


def test_badge_icon_every_game(qtbot):
    from jase.games import GAMES
    for game in GAMES.values():
        assert len(game.badge) == 2, game.id
        for mode in theme.TOKENS:
            assert f"badge_{game.id}" in theme.TOKENS[mode], (game.id, mode)
            assert not icons.badge_icon(game.id, mode).isNull(), (game.id, mode)


def test_classic_uses_pixel_art_icons(qtbot):
    names = sorted(p.stem for p in icons.ICONS_DIR.glob("*.svg"))
    for folder, _size in icons.STYLE_ICONS.values():
        assert names == sorted(p.stem for p in folder.glob("*.svg")), folder
    icon = icons.tool_icon("stop", "classic")
    assert icon.availableSizes()[0] == icons.WIN98_ICON_SIZE
    assert icon is not icons.tool_icon("stop", "xp")


def test_xp_uses_luna_icons(qtbot):
    icon = icons.tool_icon("stop", "xp")
    assert icon.availableSizes()[0] == icons.ICON_SIZE
    assert icon is not icons.tool_icon("stop", "light")
