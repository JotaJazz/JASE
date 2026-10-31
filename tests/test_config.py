import json
from jase.config import load_config, save_config, version_for
from jase.games import GAMES

def test_defaults_when_missing(tmp_path):
    cfg = load_config(tmp_path / "nope.json")
    assert cfg.game_paths == {"cod2": r"D:\Juegos\CoD2", "ut99": r"D:\Juegos\UnrealTournament",
                              "mohaa": r"D:\Juegos\MOHAA_STOCK"}
    assert cfg.theme == "goldsrc"
    assert version_for(GAMES["cod2"], cfg) == "1.3" and version_for(GAMES["ut99"], cfg) == "Todas"

def test_corrupt_file_gives_defaults(tmp_path):
    p = tmp_path / "c.json"; p.write_text("{not json")
    assert load_config(p).theme == "goldsrc"

def test_roundtrip(tmp_path):
    p = tmp_path / "sub" / "c.json"
    cfg = load_config(p); cfg.theme = "light"; cfg.version_filter["cod2"] = "1.0"
    cfg.masters["ut99"] = [("localhost", 28900)]
    save_config(cfg, p)
    cfg2 = load_config(p)
    assert cfg2.theme == "light" and version_for(GAMES["cod2"], cfg2) == "1.0"
    assert cfg2.masters == {"ut99": [("localhost", 28900)]}

def test_bad_masters_keeps_other_fields(tmp_path):
    import json
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"theme": "dark", "game_paths": {"cod2": "X"},
                             "version_filter": {"cod2": "1.0"}, "masters": {"ut99": [["only-host"]]}}))
    cfg = load_config(p)
    assert cfg.theme == "dark" and cfg.game_paths["cod2"] == "X"
    assert cfg.version_filter == {"cod2": "1.0"} and cfg.masters == {}

def test_toplevel_list_gives_defaults(tmp_path):
    p = tmp_path / "c.json"; p.write_text("[1, 2]")
    cfg = load_config(p)
    assert cfg.theme == "goldsrc" and "cod2" in cfg.game_paths

def test_invalid_theme_gives_default(tmp_path):
    p = tmp_path / "c.json"; p.write_text('{"theme": "disco"}')
    assert load_config(p).theme == "goldsrc"

def test_config_theme_tolerant(tmp_path):
    from jase.ui.theme import THEME_IDS
    p = tmp_path / "c.json"
    for tid in ("system", *THEME_IDS):
        p.write_text(json.dumps({"theme": tid}))
        assert load_config(p).theme == tid
    for bad in (None, 3, ["neon"], {"a": 1}, "NEON"):
        p.write_text(json.dumps({"theme": bad}))
        assert load_config(p).theme == "goldsrc"

def test_unhashable_theme_and_non_dict_fields_do_not_crash(tmp_path):
    import json
    for theme in ([], {}, 5, None):
        p = tmp_path / "c.json"
        p.write_text(json.dumps({"theme": theme, "game_paths": [1], "version_filter": "x", "masters": 3}))
        cfg = load_config(p)
        assert cfg.theme == "goldsrc"
        assert "cod2" in cfg.game_paths and cfg.version_filter == {} and cfg.masters == {}


def test_save_config_is_atomic(tmp_path, monkeypatch):
    import os
    from jase import config as config_mod
    p = tmp_path / "c.json"
    cfg = load_config(p); cfg.theme = "dark"
    save_config(cfg, p)
    replaced = []
    real_replace = os.replace

    def spy(src, dst):
        replaced.append((os.path.dirname(os.fspath(src)), os.fspath(dst)))
        real_replace(src, dst)

    monkeypatch.setattr(config_mod.os, "replace", spy)
    cfg.theme = "light"
    save_config(cfg, p)
    assert replaced == [(str(tmp_path), str(p))]  # temporal en el mismo directorio
    assert load_config(p).theme == "light"

    def boom(src, dst):
        raise OSError("disco lleno")

    monkeypatch.setattr(config_mod.os, "replace", boom)
    cfg.theme = "dark"
    try:
        save_config(cfg, p)
    except OSError:
        pass
    else:
        raise AssertionError("el OSError debe propagarse")
    assert load_config(p).theme == "light"  # el fichero previo queda intacto
    assert sorted(f.name for f in tmp_path.iterdir()) == ["c.json"]  # sin temporales


def test_quick_filters_roundtrip(tmp_path):
    from jase.ui.server_model import QuickFilters
    p = tmp_path / "c.json"
    cfg = load_config(p)
    assert cfg.quick_filters == QuickFilters()
    cfg.quick_filters = QuickFilters(hide_empty=True, hide_full=False, hide_password=True,
                                     max_ping=150)
    save_config(cfg, p)
    assert load_config(p).quick_filters == cfg.quick_filters


def test_old_config_without_quick_filters_loads_defaults(tmp_path):
    import json
    from jase.ui.server_model import QuickFilters
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"theme": "dark", "game_paths": {"cod2": "X"},
                             "version_filter": {"cod2": "1.0"}}))
    cfg = load_config(p)
    assert cfg.quick_filters == QuickFilters()
    assert cfg.theme == "dark" and cfg.game_paths["cod2"] == "X"


def test_invalid_quick_filter_fields_fall_back_per_field(tmp_path):
    import json
    from jase.ui.server_model import QuickFilters
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"quick_filters": {"hide_empty": True, "hide_full": "si",
                                               "hide_password": 1, "max_ping": -5}}))
    assert load_config(p).quick_filters == QuickFilters(hide_empty=True)
    p.write_text(json.dumps({"quick_filters": {"max_ping": True}}))
    assert load_config(p).quick_filters == QuickFilters()
    p.write_text(json.dumps({"quick_filters": [1, 2]}))
    assert load_config(p).quick_filters == QuickFilters()


def test_ui_state_roundtrip(tmp_path):
    p = tmp_path / "c.json"
    cfg = load_config(p)
    assert cfg.ui_state == {}
    cfg.ui_state = {"geometry": "AAEC", "last_game": "ut99"}
    save_config(cfg, p)
    assert load_config(p).ui_state == {"geometry": "AAEC", "last_game": "ut99"}


def test_v1_config_without_ui_state_loads(tmp_path):
    import json
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"theme": "dark", "game_paths": {"cod2": "X"}}))
    cfg = load_config(p)
    assert cfg.ui_state == {} and cfg.theme == "dark"


def test_invalid_ui_state_is_ignored_per_entry(tmp_path):
    import json
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"ui_state": {"geometry": 5, "last_game": "cod2", "x": None}}))
    assert load_config(p).ui_state == {"last_game": "cod2"}
    p.write_text(json.dumps({"ui_state": ["nope"], "theme": "light"}))
    cfg = load_config(p)
    assert cfg.ui_state == {} and cfg.theme == "light"


def test_saved_game_paths_gain_new_defaults_without_overwriting(tmp_path):
    """Una config vieja (sin «mohaa») recibe las rutas por defecto que le faltan."""
    p = tmp_path / "c.json"
    p.write_text('{"game_paths": {"cod2": "E:\\\\CoD2", "cod1": "C:\\\\CoD", "ut99": ""}}',
                 encoding="utf-8")
    cfg = load_config(p)
    assert cfg.game_paths == {"cod2": "E:\\CoD2", "cod1": "C:\\CoD", "ut99": "",
                              "mohaa": r"D:\Juegos\MOHAA_STOCK"}


def test_config_find_games_offered_tolerant(tmp_path):
    import json

    from jase.config import load_config, save_config

    path = tmp_path / "c.json"
    assert load_config(path).find_games_offered is False
    path.write_text(json.dumps({"find_games_offered": "si", "theme": "dark"}))
    cfg = load_config(path)
    assert cfg.find_games_offered is False and cfg.theme == "dark"
    cfg.find_games_offered = True
    save_config(cfg, path)
    assert load_config(path).find_games_offered is True


def test_config_language_tolerant(tmp_path):
    p = tmp_path / "c.json"
    assert load_config(p).language == "auto"
    for raw in ('{}', '{"language": "xx"}', '{"language": 3}', '{"language": [1]}'):
        p.write_text(raw)
        assert load_config(p).language == "auto"
    p.write_text('{"language": "en"}')
    cfg = load_config(p)
    assert cfg.language == "en"
    save_config(cfg, p)
    assert load_config(p).language == "en"
    assert '"language": "en"' in p.read_text()

def test_config_themes_match_registry():
    from jase.config import THEMES
    from jase.ui.theme import THEME_IDS
    assert THEMES == {"system", *THEME_IDS}


def test_recent_roundtrip_and_tolerance(tmp_path):
    p = tmp_path / "c.json"
    p.write_text(json.dumps({"recent": {"cod2": ["1.1.1.1:28960", 5], "ut99": "x"}}))
    assert load_config(p).recent == {"cod2": ["1.1.1.1:28960"]}
    p.write_text(json.dumps({"recent": [1, 2]}))
    assert load_config(p).recent == {}
    cfg = load_config(p)
    cfg.recent = {"cod2": ["1.1.1.1:28960"]}
    save_config(cfg, p)
    assert load_config(p).recent == {"cod2": ["1.1.1.1:28960"]}


def test_auto_refresh_minutes_tolerant(tmp_path):
    path = tmp_path / "c.json"
    for raw, expected in ((7, 0), (True, 0), ("5", 0), (5, 5), (15, 15), (0, 0)):
        path.write_text(json.dumps({"auto_refresh_minutes": raw}))
        assert load_config(path).auto_refresh_minutes == expected
    cfg = load_config(path)
    cfg.auto_refresh_minutes = 10
    save_config(cfg, path)
    assert load_config(path).auto_refresh_minutes == 10


def test_check_app_updates_tolerant(tmp_path):
    path = tmp_path / "c.json"
    assert load_config(path).check_app_updates is True  # ausente
    for raw, expected in ((False, False), (True, True), (0, True), ("no", True), (None, True)):
        path.write_text(json.dumps({"check_app_updates": raw}))
        assert load_config(path).check_app_updates is expected
    cfg = load_config(path)
    cfg.check_app_updates = False
    save_config(cfg, path)
    assert load_config(path).check_app_updates is False


def test_steam_include_empty_tolerant(tmp_path):
    path = tmp_path / "c.json"
    assert load_config(path).steam_include_empty is False  # ausente
    for raw, expected in ((True, True), (False, False), (1, False), ("yes", False), (None, False)):
        path.write_text(json.dumps({"steam_include_empty": raw}))
        assert load_config(path).steam_include_empty is expected
    cfg = load_config(path)
    cfg.steam_include_empty = True
    save_config(cfg, path)
    assert load_config(path).steam_include_empty is True


def test_minimize_to_tray_tolerant(tmp_path):
    path = tmp_path / "c.json"
    assert load_config(path).minimize_to_tray is False  # ausente
    for raw, expected in ((True, True), (False, False), (1, False), ("yes", False), (None, False)):
        path.write_text(json.dumps({"minimize_to_tray": raw}))
        assert load_config(path).minimize_to_tray is expected
    cfg = load_config(path)
    cfg.minimize_to_tray = True
    save_config(cfg, path)
    assert load_config(path).minimize_to_tray is True


def test_deeply_nested_config_gives_defaults(tmp_path):
    from jase.config import Config
    p = tmp_path / "c.json"
    p.write_bytes(b"[" * 100000)
    assert load_config(p) == Config()


def test_amber_theme_migrates_to_terminal_amber(tmp_path):
    p = tmp_path / "config.json"
    p.write_text(json.dumps({"theme": "amber"}), encoding="utf-8")
    cfg = load_config(p)
    assert (cfg.theme, cfg.terminal_color) == ("terminal", "amber")


def test_terminal_color_loaded_saved_and_invalid_ignored(tmp_path):
    p = tmp_path / "config.json"
    p.write_text(json.dumps({"theme": "terminal", "terminal_color": "sepia"}), encoding="utf-8")
    cfg = load_config(p)
    assert (cfg.theme, cfg.terminal_color) == ("terminal", "sepia")
    save_config(cfg, p)
    assert json.loads(p.read_text(encoding="utf-8"))["terminal_color"] == "sepia"
    p.write_text(json.dumps({"theme": "dark", "terminal_color": "magenta"}), encoding="utf-8")
    cfg = load_config(p)
    assert (cfg.theme, cfg.terminal_color) == ("dark", "green")


def test_bots_and_country_quick_filters_roundtrip(tmp_path):
    import json
    from jase.ui.server_model import QuickFilters
    p = tmp_path / "c.json"
    cfg = load_config(p)
    cfg.quick_filters = QuickFilters(hide_bots=True, country="cl")
    save_config(cfg, p)
    assert load_config(p).quick_filters == QuickFilters(hide_bots=True, country="cl")
    for bad in ("chile", "c1", 5, "", "ñu"):
        p.write_text(json.dumps({"quick_filters": {"country": bad, "hide_bots": "si"}}))
        assert load_config(p).quick_filters == QuickFilters()
    p.write_text(json.dumps({"quick_filters": {"country": "CL"}}))
    assert load_config(p).quick_filters.country == "cl"


def test_damaged_config_is_kept(tmp_path):
    p = tmp_path / "config.json"; p.write_text('{"favorites": {"cod2": []},}')
    save_config(load_config(p), p)
    assert (tmp_path / "config.json.bad").read_text() == '{"favorites": {"cod2": []},}'


def test_non_dict_config_is_kept(tmp_path):
    p = tmp_path / "config.json"; p.write_text("[1, 2]")
    load_config(p)
    assert (tmp_path / "config.json.bad").read_text() == "[1, 2]"


def test_first_run_creates_no_backup(tmp_path):
    p = tmp_path / "config.json"
    load_config(p)
    assert not (tmp_path / "config.json.bad").exists()
