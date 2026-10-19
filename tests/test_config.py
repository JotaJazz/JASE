from jase.config import load_config, save_config, masters_for, version_for
from jase.games import GAMES

def test_defaults_when_missing(tmp_path):
    cfg = load_config(tmp_path / "nope.json")
    assert cfg.game_paths == {"cod2": r"D:\Juegos\CoD2", "ut99": r"D:\Juegos\UnrealTournament"}
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
    assert cfg.theme == "dark" and cfg.game_paths == {"cod2": "X"}
    assert cfg.version_filter == {"cod2": "1.0"} and cfg.masters == {}

def test_toplevel_list_gives_defaults(tmp_path):
    p = tmp_path / "c.json"; p.write_text("[1, 2]")
    cfg = load_config(p)
    assert cfg.theme == "system" and "cod2" in cfg.game_paths

def test_invalid_theme_gives_system(tmp_path):
    p = tmp_path / "c.json"; p.write_text('{"theme": "neon"}')
    assert load_config(p).theme == "system"
