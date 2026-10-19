from jase.config import load_config, save_config, masters_for, version_for
from jase.games import GAMES

def test_defaults_when_missing(tmp_path):
    cfg = load_config(tmp_path / "nope.json")
    assert cfg.game_paths == {"cod2": r"D:\Juegos\CoD2", "ut99": r"D:\Juegos\UnrealTournament",
                              "mohaa": r"D:\Juegos\MOHAA_STOCK"}
    assert cfg.theme == "system"
    assert version_for(GAMES["cod2"], cfg) == "1.3" and version_for(GAMES["ut99"], cfg) == "Todas"

def test_corrupt_file_gives_defaults(tmp_path):
    p = tmp_path / "c.json"; p.write_text("{not json")
    assert load_config(p).theme == "system"

def test_roundtrip(tmp_path):
    p = tmp_path / "sub" / "c.json"
    cfg = load_config(p); cfg.theme = "light"; cfg.version_filter["cod2"] = "1.0"
    cfg.masters["ut99"] = [("localhost", 28900)]
    save_config(cfg, p)
    cfg2 = load_config(p)
    assert cfg2.theme == "light" and version_for(GAMES["cod2"], cfg2) == "1.0"
    assert masters_for(GAMES["ut99"], cfg2) == [("localhost", 28900)]
    assert masters_for(GAMES["cod2"], cfg2) == [("cod2master.activision.com", 20710)]

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
    assert cfg.theme == "system" and "cod2" in cfg.game_paths

def test_invalid_theme_gives_system(tmp_path):
    p = tmp_path / "c.json"; p.write_text('{"theme": "neon"}')
    assert load_config(p).theme == "system"

def test_unhashable_theme_and_non_dict_fields_do_not_crash(tmp_path):
    import json
    for theme in ([], {}, 5, None):
        p = tmp_path / "c.json"
        p.write_text(json.dumps({"theme": theme, "game_paths": [1], "version_filter": "x", "masters": 3}))
        cfg = load_config(p)
        assert cfg.theme == "system"
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
