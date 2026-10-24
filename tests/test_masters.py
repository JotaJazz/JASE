import json
from pathlib import Path

import pytest

from jase import masters
from jase.config import Config
from jase.games import GAMES
from jase.masters import (CustomMasters, effective_masters, load_custom,
                          load_official, master_key, migrate_config_masters, official_for,
                          parse_official, save_custom)

ROOT = Path(__file__).resolve().parents[1]


def doc(games, schema=1, updated="2026-10-20"):
    return json.dumps({"schema": schema, "updated": updated, "games": games}).encode()


def entry(host="m.example.com", port=1234):
    return {"host": host, "port": port}


def test_repo_masters_json_matches_bundled_and_gamedefs():
    root_bytes = (ROOT / "masters.json").read_bytes()
    assert root_bytes == (ROOT / "src/jase/assets/masters.json").read_bytes()
    official = parse_official(root_bytes)
    assert official.schema == 1 and official.updated == "2026-10-20"
    assert set(official.games) == set(GAMES)
    for gid, game in GAMES.items():
        assert official.games[gid] == tuple(game.masters)
    assert masters.bundled_official() == official


def test_parse_official_ok():
    o = parse_official(doc({"cod2": [entry(), entry("M.Example.com", 1234)]}))
    assert o.games == {"cod2": (("m.example.com", 1234),)}  # dedupe por master_key
    assert o.updated == "2026-10-20"


@pytest.mark.parametrize("data", [b"{nope", b"[]", b"\xff\xfe", doc({}, updated=3)])
def test_parse_official_rejects_bad_json(data):
    with pytest.raises(ValueError):
        parse_official(data)


@pytest.mark.parametrize("schema", [2, 0, "1", True, None])
def test_parse_official_rejects_unknown_schema(schema):
    with pytest.raises(ValueError):
        parse_official(doc({"cod2": [entry()]}, schema=schema))


@pytest.mark.parametrize("host", ["http://x", "a b", "", "a/b", "u@h", "x" * 254, 5, None])
def test_parse_official_rejects_bad_host(host):
    with pytest.raises(ValueError):
        parse_official(doc({"cod2": [entry(host)]}))


@pytest.mark.parametrize("port", [0, 70000, "x", True, 1.5, None, -1])
def test_parse_official_rejects_bad_port(port):
    with pytest.raises(ValueError):
        parse_official(doc({"cod2": [entry(port=port)]}))


def test_parse_official_rejects_all_games_empty():
    with pytest.raises(ValueError):
        parse_official(doc({gid: [] for gid in GAMES}))


def test_parse_official_rejects_too_big():
    pad = "x" * (masters.MAX_OFFICIAL_BYTES + 1)
    with pytest.raises(ValueError):
        parse_official(doc({"cod2": [entry()]}, updated=pad))


def test_parse_official_ignores_unknown_game():
    o = parse_official(doc({"cod2": [entry()], "zzz": [entry("http://bad", 0)]}))
    assert "zzz" not in o.games


def test_official_for_absent_vs_empty():
    o = parse_official(doc({"cod2": [], "ut99": [entry()]}))
    assert official_for("cod2", o) == ()
    assert official_for("ut99", o) == (("m.example.com", 1234),)
    assert official_for("cod1", o) == GAMES["cod1"].masters
    assert official_for("nope", o) == ()


def test_load_official_fallback_order(tmp_path):
    cache = tmp_path / "cache"
    assert load_official(cache) == masters.bundled_official()  # sin caché
    cache.mkdir()
    (cache / "masters.json").write_bytes(b"{garbage")
    assert load_official(cache) == masters.bundled_official()  # caché inválida
    (cache / "masters.json").write_bytes(doc({"cod2": [entry()]}, updated="2030-01-01"))
    assert load_official(cache).updated == "2030-01-01"  # caché válida


def test_custom_tolerant_load(tmp_path):
    p = tmp_path / "c.json"
    assert load_custom(p) == CustomMasters()
    p.write_text("{bad")
    assert load_custom(p) == CustomMasters()
    p.write_text("[1]")
    assert load_custom(p) == CustomMasters()
    p.write_text(json.dumps({
        "added": {"cod2": [entry(), {"host": "x y", "port": 1}, {"host": "ok", "port": 0}, 3,
                           entry("M.EXAMPLE.com", 1234)], "ut99": "x", 5: []},
        "disabled": {"cod2": ["a:1", 7, ""], "ut99": "x"}}))
    c = load_custom(p)
    assert c.added == {"cod2": [("m.example.com", 1234)]}
    assert c.disabled == {"cod2": {"a:1"}}
    p.write_text(json.dumps({"added": 3, "disabled": []}))
    assert load_custom(p) == CustomMasters()


def test_custom_roundtrip(tmp_path):
    p = tmp_path / "sub" / "c.json"
    c = CustomMasters({"cod2": [("h.example", 1)]}, {"cod2": {"b:2", "a:1"}})
    save_custom(c, p)
    assert load_custom(p) == c
    assert json.loads(p.read_text())["schema"] == 1
    assert [f.name for f in p.parent.iterdir()] == ["c.json"]


def test_effective_union_disabled_dedupe():
    o = parse_official(doc({"cod2": [entry("a.com", 1), entry("b.com", 2)]}))
    c = CustomMasters({"cod2": [("B.com", 2), ("c.com", 3)]}, {"cod2": {master_key(("a.com", 1))}})
    assert effective_masters("cod2", o, c) == [("b.com", 2), ("c.com", 3)]
    assert effective_masters("cod1", o, CustomMasters()) == list(GAMES["cod1"].masters)


def test_migration_preserves_effective_list_and_runs_once():
    o = parse_official(doc({"cod2": [entry("a.com", 1), entry("b.com", 2)]}))
    cfg = Config()
    cfg.masters = {"cod2": [("b.com", 2), ("mine.lan", 9)], "ut99": []}
    custom = CustomMasters()
    assert migrate_config_masters(cfg, custom, o) is True
    assert cfg.masters == {}
    assert custom.added == {"cod2": [("mine.lan", 9)]}
    assert custom.disabled == {"cod2": {"a.com:1"}}
    assert sorted(effective_masters("cod2", o, custom)) == [("b.com", 2), ("mine.lan", 9)]
    assert effective_masters("ut99", o, custom) == list(GAMES["ut99"].masters)
    assert migrate_config_masters(cfg, custom, o) is False


def test_migration_drops_invalid_entries():
    o = parse_official(doc({"cod2": [entry("a.com", 1)]}))
    cfg = Config()
    cfg.masters = {"cod2": [("a.com", 1), ("bad host!", 5), ("ok.lan", 70000), ("ok.lan", 7)]}
    custom = CustomMasters()
    assert migrate_config_masters(cfg, custom, o) is True
    assert custom.added == {"cod2": [("ok.lan", 7)]}


def test_main_window_scan_uses_effective_masters(qtbot, tmp_path):
    from jase.ui.main_window import MainWindow
    from test_main_window import FakeLaunch, FakeMaps
    from jase.scanner import ScanResult

    got = {}

    async def scan(game, masters_, on_server, on_status, **kw):
        got[game.id] = list(masters_)
        return ScanResult(masters_ok=1, servers_listed=0, servers_responded=0)

    cfg = Config()
    cfg.masters = {"cod2": [("mine.lan", 9)]}
    w = MainWindow(cfg, tmp_path / "config.json", scan_fn=scan, launch_fn=FakeLaunch(),
                   map_service=FakeMaps())
    qtbot.addWidget(w)
    assert cfg.masters == {}
    assert load_custom(tmp_path / "masters.custom.json").added == {"cod2": [("mine.lan", 9)]}
    w.select_game("cod2")
    qtbot.waitUntil(lambda: "cod2" in got and not w.is_scanning() and not w._workers)
    assert got["cod2"] == [("mine.lan", 9)]
    w.close()


def test_load_official_deeply_nested_cache_falls_back(tmp_path):
    cache = tmp_path / "cache"
    cache.mkdir()
    (cache / "masters.json").write_bytes(b"[" * 100000)
    assert load_official(cache) == masters.bundled_official()
    with pytest.raises(ValueError):
        parse_official(b"[" * 100000)


def test_load_custom_nested_or_oversize_is_empty(tmp_path):
    p = tmp_path / "c.json"
    p.write_bytes(b"[" * 100000)
    assert load_custom(p) == CustomMasters()
    pad = "x" * (masters.MAX_OFFICIAL_BYTES + 1)
    p.write_text(json.dumps({"added": {"cod2": [entry()]}, "pad": pad}))
    assert load_custom(p) == CustomMasters()


def test_migration_save_failure_keeps_config_masters(qtbot, tmp_path, monkeypatch):
    from jase.config import load_config
    from jase.ui.main_window import MainWindow
    from test_main_window import FakeLaunch, FakeMaps

    def boom(*a, **kw):
        raise OSError("disco lleno")

    monkeypatch.setattr(masters, "save_custom", boom)
    cfg = Config()
    cfg.masters = {"cod2": [("mine.lan", 9)]}
    got = {}

    async def scan(game, masters_, on_server, on_status, **kw):
        got[game.id] = list(masters_)
        from jase.scanner import ScanResult
        return ScanResult(masters_ok=1, servers_listed=0, servers_responded=0)

    w = MainWindow(cfg, tmp_path / "config.json", scan_fn=scan, launch_fn=FakeLaunch(),
                   map_service=FakeMaps())
    qtbot.addWidget(w)
    assert cfg.masters == {"cod2": [("mine.lan", 9)]}
    assert w._custom.added == {"cod2": [("mine.lan", 9)]}
    w.select_game("cod2")
    qtbot.waitUntil(lambda: "cod2" in got and not w.is_scanning() and not w._workers)
    assert got["cod2"] == [("mine.lan", 9)]
    w._save_config()  # guardado posterior no relacionado
    assert load_config(tmp_path / "config.json").masters == {"cod2": [("mine.lan", 9)]}
    w.close()
