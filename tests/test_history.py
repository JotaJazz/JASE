import json

from jase.history import KEEP_DAYS, MAX_FILE_BYTES, MAX_SAMPLES, MIN_GAP_S, History

NOW = 1_700_000_000.0


def test_min_gap(tmp_path):
    h = History(tmp_path / "h.json")
    h.record("q3", "1.2.3.4:27960", 3, 8, NOW)
    h.record("q3", "1.2.3.4:27960", 4, 8, NOW + MIN_GAP_S - 1)
    h.record("q3", "1.2.3.4:27960", 5, 8, NOW + MIN_GAP_S)
    assert [s[1] for s in h.samples("q3", "1.2.3.4:27960")] == [3, 5]


def test_prune_by_days_and_cap(tmp_path):
    h = History(tmp_path / "h.json")
    h.record("q3", "a:1", 1, 8, NOW)
    later = NOW + (KEEP_DAYS * 86400) + 10
    h.record("q3", "a:1", 2, 8, later)
    assert [s[1] for s in h.samples("q3", "a:1")] == [2]
    h2 = History(tmp_path / "h2.json")
    for i in range(MAX_SAMPLES + 5):
        h2.record("q3", "a:1", i % 7, 8, NOW + i * MIN_GAP_S)
    assert len(h2.samples("q3", "a:1")) <= MAX_SAMPLES


def test_hourly_average_timezone(tmp_path):
    h = History(tmp_path / "h.json")
    base = 1_700_006_400.0  # 00:00 UTC
    h.record("q3", "a:1", 4, 8, base)
    h.record("q3", "a:1", 6, 8, base + 600)
    h.record("q3", "a:1", 2, 8, base + 3 * 3600)
    utc = h.hourly_average("q3", "a:1", base + 4000, 0)
    assert utc[0] == 5.0 and utc[3] == 2.0 and utc[1] is None and len(utc) == 24
    shifted = h.hourly_average("q3", "a:1", base + 4000, -3 * 3600)
    assert shifted[21] == 5.0 and shifted[0] == 2.0


def test_roundtrip_and_prune(tmp_path):
    path = tmp_path / "cache" / "h.json"
    h = History(path)
    h.record("q3", "a:1", 1, 8, NOW)
    h.record("q3", "b:2", 1, 8, NOW)
    h.record("cod", "c:3", 1, 8, NOW)
    h.prune({("q3", "a:1")})
    h.save()
    h2 = History(path)
    h2.load()
    assert h2.samples("q3", "a:1") and not h2.samples("q3", "b:2") and not h2.samples("cod", "c:3")


def test_corrupt_or_huge_is_empty(tmp_path):
    path = tmp_path / "h.json"
    path.write_text("{no json")
    h = History(path)
    h.load()
    assert h.samples("q3", "a:1") == []
    path.write_text(json.dumps({"q3": {"a:1": [[1, 2, 3]]}}))
    h.load()
    assert h.samples("q3", "a:1") == [[1, 2, 3]]
    path.write_text(json.dumps({"q3": {"a:1": [["x", 2, 3], [1, 2]]}}))
    h.load()
    assert h.samples("q3", "a:1") == []
    path.write_bytes(b" " * (MAX_FILE_BYTES + 1))
    h.load()
    assert h.samples("q3", "a:1") == []


def test_hostile_content(tmp_path):
    path = tmp_path / "h.json"
    h = History(path)
    path.write_bytes(b"[" * 100000)
    h.load(now=NOW)
    assert h.samples("q3", "a:1") == []
    path.write_text("[1, 2]")
    h.load(now=NOW)
    assert h._data == {}
    path.write_text('{"q3": {"a:1": [[NaN, 1, 2], [Infinity, 1, 2], [true, 1, 2]]}}')
    h.load(now=NOW)
    assert h.samples("q3", "a:1") == []
    bad = [[-5, 1, 8], [0, 1, 8], [int(NOW) + 3 * 86400, 1, 8], [int(NOW) - 10, 2, 8],
           [int(NOW) - 100, 3, 8]]
    path.write_text(json.dumps({"q3": {"a:1": bad}}))
    h.load(now=NOW)
    assert [s[1] for s in h.samples("q3", "a:1")] == [3, 2]
