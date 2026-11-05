"""Listas de servidores guardadas en disco entre sesiones (núcleo sin Qt)."""
import json

from jase import server_cache
from jase.models import Player, ServerAddress, ServerInfo


def info(host="1.2.3.4", port=28960, **over):
    a = ServerAddress(host, port)
    base = dict(address=a, connect_address=ServerAddress(host, port - 1), name="Srv",
                map="mp_toujane", gametype="tdm", num_players=3, max_players=20,
                password=True, version="1.3", ping_ms=42,
                players=[Player("Jimmy", 10, 50, raw_name="^1Jimmy")],
                rules={"g_gametype": "tdm"}, raw_name="^2Srv", bots=1,
                connect_port_deduced=True)
    base.update(over)
    return ServerInfo(**base)


def test_roundtrip_keeps_every_field(tmp_path):
    servers = [info(), info("5.6.7.8", 27015, players=[], rules={})]
    server_cache.save(tmp_path, "cod2", servers, now=1000.0)
    saved_at, loaded = server_cache.load(tmp_path, "cod2")
    assert saved_at == 1000.0 and loaded == servers
    assert loaded[0].raw_name == "^2Srv" and loaded[0].bots == 1
    assert loaded[0].connect_port_deduced and loaded[0].players[0].raw_name == "^1Jimmy"


def test_missing_corrupt_or_foreign_files_give_none(tmp_path):
    assert server_cache.load(tmp_path, "cod2") is None
    path = tmp_path / "cod2.json"
    path.write_bytes(b"{roto")
    assert server_cache.load(tmp_path, "cod2") is None
    path.write_text(json.dumps({"schema": 99, "saved_at": 1, "servers": []}))
    assert server_cache.load(tmp_path, "cod2") is None
    path.write_text("[" * 100000)
    assert server_cache.load(tmp_path, "cod2") is None


def test_invalid_entries_are_dropped_one_by_one(tmp_path):
    server_cache.save(tmp_path, "cod2", [info(), info("5.6.7.8")], now=5.0)
    path = tmp_path / "cod2.json"
    data = json.loads(path.read_text())
    data["servers"][0]["address"] = "no es una dirección"
    data["servers"].append({"basura": True})
    data["servers"].append(None)
    path.write_text(json.dumps(data))
    _, loaded = server_cache.load(tmp_path, "cod2")
    assert [s.address.host for s in loaded] == ["5.6.7.8"]


def test_oversized_file_is_ignored(tmp_path, monkeypatch):
    server_cache.save(tmp_path, "cod2", [info()], now=5.0)
    monkeypatch.setattr(server_cache, "MAX_BYTES", 10)
    assert server_cache.load(tmp_path, "cod2") is None


def test_index_counts_and_forget(tmp_path):
    server_cache.save(tmp_path, "cod2", [info(), info("5.6.7.8")], now=5.0)
    server_cache.save(tmp_path, "ut99", [info()], now=7.0)
    assert server_cache.read_index(tmp_path) == {"cod2": (2, 5.0), "ut99": (1, 7.0)}
    server_cache.forget(tmp_path, "cod2")
    assert server_cache.read_index(tmp_path) == {"ut99": (1, 7.0)}
    assert server_cache.load(tmp_path, "cod2") is None


def test_unknown_game_id_is_rejected(tmp_path):
    server_cache.save(tmp_path, "../fuera", [info()], now=1.0)
    assert not any(tmp_path.parent.glob("fuera*"))
    assert server_cache.load(tmp_path, "../fuera") is None


def test_corrupt_index_reads_as_empty(tmp_path):
    (tmp_path / "index.json").write_text("{roto")
    assert server_cache.read_index(tmp_path) == {}


def test_non_finite_or_huge_date_is_not_a_list(tmp_path):
    server_cache.save(tmp_path, "cod2", [info()], now=5.0)
    path = tmp_path / "cod2.json"
    for bad in ("NaN", "Infinity", "1e300", "-5"):
        raw = json.loads(path.read_text())
        raw["saved_at"] = float(bad)
        path.write_text(json.dumps(raw))  # json escribe NaN/Infinity y Python los lee
        assert server_cache.load(tmp_path, "cod2") is None, bad
