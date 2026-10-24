from jase.config import Config, load_config, save_config
from jase.filter_presets import MAX_PRESETS, FilterPreset, parse, remove, to_json, upsert
from jase.models import QuickFilters


def test_parse_tolerant():
    raw = [
        {"name": "A", "quick": {"hide_empty": True, "max_ping": "x"}, "text": "dm"},
        "basura", {"name": ""}, {"name": 3}, {"name": "a"},  # duplicado casefold
        {"name": "B", "quick": 5, "text": 7},
    ]
    out = parse(raw)
    assert [p.name for p in out] == ["A", "B"]
    assert out[0].quick == QuickFilters(hide_empty=True)
    assert out[0].text == "dm"
    assert out[1].quick == QuickFilters() and out[1].text == ""
    assert parse("no") == [] and parse(None) == []


def test_parse_cap():
    raw = [{"name": f"p{i}"} for i in range(50)]
    assert len(parse(raw)) == MAX_PRESETS


def test_upsert_replaces_keeping_order_and_remove():
    a, b = FilterPreset("A", QuickFilters(), ""), FilterPreset("B", QuickFilters(), "")
    a2 = FilterPreset("a", QuickFilters(hide_full=True), "x")
    out = upsert([a, b], a2)
    assert out == [a2, b]
    assert remove(out, "B") == [a2]
    full = [FilterPreset(f"p{i}", QuickFilters(), "") for i in range(MAX_PRESETS)]
    assert len(upsert(full, FilterPreset("new", QuickFilters(), ""))) == MAX_PRESETS


def test_config_roundtrip(tmp_path):
    cfg = Config()
    cfg.filter_presets = [FilterPreset("A", QuickFilters(max_ping=80), "t")]
    save_config(cfg, tmp_path / "c.json")
    assert load_config(tmp_path / "c.json").filter_presets == cfg.filter_presets
    assert to_json(cfg.filter_presets)[0]["quick"]["max_ping"] == 80
