import inspect

from jase.ui import main_window, window_actions, window_auto_refresh, window_background, window_favorites, window_player_search, window_settings, window_slot_watch

MIXINS = (window_actions.ActionsMixin, window_auto_refresh.AutoRefreshMixin, window_favorites.FavoritesMixin,
          window_background.BackgroundMixin, window_player_search.PlayerSearchMixin, window_settings.SettingsMixin,
          window_slot_watch.SlotWatchMixin)


def _own(cls):
    return {n for n, v in vars(cls).items() if inspect.isfunction(v)}


def test_mixins_do_not_shadow_each_other():
    seen = set()
    for cls in (*MIXINS, main_window.MainWindow):
        own = _own(cls)
        assert not (own & seen), own & seen
        seen |= own


def test_main_window_is_smaller():
    lines = inspect.getsource(main_window).count("\n")
    assert lines < 1100
