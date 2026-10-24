from jase.config import Config
from jase.models import ServerAddress
from jase.recent import RECENT_MAX, recent_addresses, record_recent, remove_recent

A = ServerAddress("1.1.1.1", 28960)
B = ServerAddress("2.2.2.2", 28960)


def test_record_puts_first_without_duplicates():
    cfg = Config()
    for address in (A, B, A):
        record_recent(cfg, "cod2", address)
    assert recent_addresses(cfg, "cod2") == [A, B]
    assert cfg.recent == {"cod2": ["1.1.1.1:28960", "2.2.2.2:28960"]}


def test_record_trims_to_max():
    cfg = Config()
    last = None
    for i in range(25):
        last = ServerAddress("10.0.0.1", 1000 + i)
        record_recent(cfg, "cod2", last)
    result = recent_addresses(cfg, "cod2")
    assert len(result) == RECENT_MAX == 20 and result[0] == last


def test_remove_last_entry_drops_game_key():
    cfg = Config()
    record_recent(cfg, "cod2", A)
    assert remove_recent(cfg, "cod2", A) is True
    assert "cod2" not in cfg.recent
    assert remove_recent(cfg, "cod2", A) is False


def test_garbage_entries_ignored():
    cfg = Config(recent={"cod2": ["basura", "1.1.1.1:28960"]})
    assert recent_addresses(cfg, "cod2") == [A]
