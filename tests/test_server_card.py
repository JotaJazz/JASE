import pytest
from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QPixmap
from PySide6.QtWidgets import QApplication, QLabel, QPushButton

from jase.models import ServerAddress, ServerInfo
from jase.ui import theme
from jase.ui.server_card import MAP_IMAGE_SIZE, ServerCard


def info(**kw) -> ServerInfo:
    values = dict(address=ServerAddress("1.2.3.4", 7778), connect_address=ServerAddress("1.2.3.4", 7777),
                  name="Jimmy TDM", map="mp_toujane", gametype="tdm", num_players=3,
                  max_players=20, password=False, version="1.3", ping_ms=42)
    values.update(kw)
    return ServerInfo(**values)


@pytest.fixture
def card(qtbot):
    c = ServerCard()
    qtbot.addWidget(c)
    c.resize(700, 150)
    c.show()
    return c


def label(card, name) -> QLabel:
    w = card.findChild(QLabel, name)
    assert w is not None, name
    return w


def test_empty_state(card):
    card.set_server(None, "dark")
    assert label(card, "card_empty").isVisible()
    assert label(card, "card_empty").text() == "Selecciona un servidor"
    assert not label(card, "card_name").isVisible()


def test_shows_server_details(card):
    card.set_server(info(), "dark")
    assert not label(card, "card_empty").isVisible()
    assert label(card, "card_name").isVisible()
    assert "Jimmy TDM" in label(card, "card_name").text()
    assert label(card, "card_address").text() == "1.2.3.4:7777"
    assert label(card, "card_map").text() == "mp_toujane"
    assert label(card, "card_mode").text() == "tdm"
    assert label(card, "card_players").text() == "3 / 20"
    assert label(card, "card_ping").text() == "42 ms"
    assert label(card, "card_version").text() == "1.3"
    assert not label(card, "card_lock").isVisible()
    card.set_server(info(password=True), "dark")
    assert label(card, "card_lock").isVisible() and "🔒" in label(card, "card_lock").text()


def test_ping_is_colored_by_theme(card):
    for mode in ("dark", "light"):
        card.set_server(info(ping_ms=200), mode)
        css = label(card, "card_ping").styleSheet()
        assert theme.ping_color(200, mode).name() in css


def test_name_uses_color_codes_and_escapes_html(card):
    card.set_server(info(name="<b>Jimmy</b>", raw_name="^1<b>Jimmy</b>"), "dark")
    text = label(card, "card_name").text()
    assert "&lt;b&gt;" in text and "<b>" not in text
    assert theme.Q3_COLORS["dark"]["1"] in text


def test_copy_button_copies_connect_address(card, qtbot):
    card.set_server(info(), "dark")
    button = card.findChild(QPushButton, "copy_address")
    assert button.text() == "Copiar"
    with qtbot.waitSignal(card.copy_requested) as sig:
        qtbot.mouseClick(button, Qt.LeftButton)
    assert sig.args == ["1.2.3.4:7777"]
    assert QApplication.clipboard().text() == "1.2.3.4:7777"


def test_placeholder_shows_map_name(card):
    card.set_server(info(map="mp_carentan"), "dark")
    card.set_map_image(None)
    image = label(card, "map_image")
    assert image.pixmap().isNull()
    assert image.text() == "mp_carentan"
    assert image.size() == MAP_IMAGE_SIZE


def test_map_image_scaled_keeping_aspect(card):
    card.set_server(info(), "dark")
    pix = QPixmap(1024, 1024)
    pix.fill(QColor("red"))
    card.set_map_image(pix)
    shown = label(card, "map_image").pixmap()
    assert not shown.isNull()
    assert (shown.width(), shown.height()) == (120, 120)
    pix = QPixmap(480, 360)
    pix.fill(QColor("blue"))
    card.set_map_image(pix)
    shown = label(card, "map_image").pixmap()
    assert (shown.width(), shown.height()) == (160, 120)
    assert label(card, "map_image").text() == ""


def test_new_server_resets_image_but_same_server_keeps_it(card):
    card.set_server(info(), "dark")
    pix = QPixmap(160, 120)
    pix.fill(QColor("red"))
    card.set_map_image(pix)
    card.set_server(info(num_players=5), "light")  # refresco del mismo servidor
    assert not label(card, "map_image").pixmap().isNull()
    card.set_server(info(map="mp_other"), "light")  # cambió el mapa
    assert label(card, "map_image").pixmap().isNull()
    assert label(card, "map_image").text() == "mp_other"


def test_country_hidden_by_default(card):
    card.set_server(info(), "dark")
    assert not label(card, "card_country").isVisible()
    assert not label(card, "card_country_flag").isVisible()
    card.set_country("AR", "Argentina")
    assert label(card, "card_country").isVisible()
    assert "Argentina" in label(card, "card_country").text()
    flag = label(card, "card_country_flag")
    assert flag.isVisible() and not flag.pixmap().isNull()
    card.set_country(None, None)
    assert not label(card, "card_country").isVisible()
    assert not label(card, "card_country_flag").isVisible()


def test_country_without_flag_shows_name_only(card):
    card.set_server(info(), "dark")
    card.set_country("qq", "QQ")
    assert label(card, "card_country").isVisible()
    assert not label(card, "card_country_flag").isVisible()


HOSTILE = '<img src="file:///x.png"><h1>VIP</h1>'


def test_remote_text_is_plain(card):
    from jase.ui.dialogs import PasswordDialog
    card.set_server(info(map=HOSTILE, gametype=HOSTILE), "dark")
    for lab in (card.map_value, card.mode_value, card.map_label, card.note_label):
        assert lab.textFormat() == Qt.TextFormat.PlainText
    assert "<img" not in card.name_label.toolTip()
    assert PasswordDialog("<b>x</b>", None).findChild(QLabel).textFormat() == Qt.TextFormat.PlainText


def test_plain_tip_escapes_and_keeps_line_breaks():
    from jase.ui.colored_text import plain_tip
    tip = plain_tip("<b>a</b>\nnota")
    assert "<b>" not in tip and "&lt;b&gt;" in tip
    assert "white-space: pre-wrap" in tip and "\n" in tip


def test_plain_tip_empty_stays_empty():
    from jase.ui.colored_text import plain_tip
    assert plain_tip("") == ""


def test_confirm_plain_uses_plain_text(qtbot, monkeypatch):
    from PySide6.QtWidgets import QMessageBox
    from jase.ui.dialogs import confirm_plain
    seen = []

    def fake(box):
        seen.append(box.textFormat())
        return QMessageBox.StandardButton.Yes
    monkeypatch.setattr(QMessageBox, "exec", fake)
    assert confirm_plain(None, "t", HOSTILE)
    assert seen == [Qt.TextFormat.PlainText]


def test_model_tooltips_are_escaped():
    from jase.ui.server_model import COL_MAP, COL_NAME, ServerTableModel
    m = ServerTableModel()
    m.add_or_update(info(name=HOSTILE, map=HOSTILE))
    for col in (COL_NAME, COL_MAP):
        assert "<img" not in m.data(m.index(0, col), Qt.ItemDataRole.ToolTipRole)
