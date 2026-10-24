from jase.ui.occupancy_chart import OccupancyChart
from jase.ui.server_card import ServerCard


def test_chart_heights_and_empty(qtbot=None):
    chart = OccupancyChart()
    chart.set_data([None] * 24, 8, 3, "dark")
    assert not chart.has_data()
    chart.set_data([4.0] + [None] * 22 + [8.0], 8, 0, "dark")
    h = chart.bar_heights(40)
    assert h[0] == 20 and h[23] == 40 and h[5] == 0
    chart.grab()


def test_card_history_visibility():
    card = ServerCard()
    base = card.height()
    card.set_history([1.0] * 24, 8, 2, "light")
    assert not card.history_chart.isHidden() and card.height() > base
    card.set_history(None)
    assert card.history_chart.isHidden() and card.height() == base
