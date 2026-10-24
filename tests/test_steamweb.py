"""API web de Steam (IGameServersService/GetServerList) como fuente de servidores.

Fixture steamweb_l4d2.json: primeros 20 servidores de `\\appid\\550\\empty\\1`
(solo `response.servers`; sin URL ni clave)."""
import asyncio
import io
import threading
import time
import urllib.error
from pathlib import Path

import pytest

from jase import scanner
from jase.games import ALL_VERSIONS, GAMES, GameDef
from jase.protocols import ProtocolError, steamweb

FIXTURES = Path(__file__).parent / "fixtures"

L4D2 = GameDef(
    id="l4d2", name="Left 4 Dead 2", family="valve",
    masters=(("api.steampowered.com", 443),), protocols=(), known_versions=(),
    default_version=ALL_VERSIONS, exe_relpath="left4dead2.exe", steam_webapi=True,
    valve_appid=550, valve_gamedir="left4dead2", launch_style="valve")


def test_parse_server_list_reads_addr():
    servers = steamweb.parse_server_list((FIXTURES / "steamweb_l4d2.json").read_bytes())
    assert len(servers) == 20 and all(s.port > 0 for s in servers)


def test_parse_server_list_empty_response():
    assert steamweb.parse_server_list(b'{"response": {}}') == []


def test_parse_server_list_rejects_garbage():
    with pytest.raises(ProtocolError):
        steamweb.parse_server_list(b"<html>")


def test_parse_server_list_rejects_missing_response():
    with pytest.raises(ProtocolError):
        steamweb.parse_server_list(b'{"foo": 1}')


def test_build_url_escapes_filter():
    url = steamweb.build_url("api.steampowered.com", "K", "\\appid\\550\\empty\\1")
    assert "filter=%5Cappid%5C550%5Cempty%5C1" in url and "limit=20000" in url


def _fake_urlopen(monkeypatch, code):
    def fake(url, timeout=None):
        raise urllib.error.HTTPError(url, code, "x", {}, io.BytesIO(b""))
    monkeypatch.setattr(scanner.urllib.request, "urlopen", fake)


@pytest.mark.parametrize("code", [401, 403, 429])
def test_steamweb_http_error_message_has_no_key(monkeypatch, code):
    _fake_urlopen(monkeypatch, code)
    with pytest.raises(steamweb.SteamWebError) as err:
        asyncio.run(scanner.fetch_master_steamweb("api.steampowered.com", 443,
                                                  "\\appid\\550", "SECRETKEY123"))
    assert "SECRETKEY123" not in str(err.value) and "steampowered" not in str(err.value)


def test_steamweb_network_error_is_oserror(monkeypatch):
    def fake(url, timeout=None):
        raise urllib.error.URLError("sin red")
    monkeypatch.setattr(scanner.urllib.request, "urlopen", fake)
    with pytest.raises(OSError):
        asyncio.run(scanner.fetch_master_steamweb("api.steampowered.com", 443, "\\appid\\550", "K"))


def test_fetch_master_requires_key_for_steam_games():
    coro = scanner.fetch_master(L4D2, "api.steampowered.com", 443, 5.0, steam_key="")
    with pytest.raises(steamweb.SteamWebError):
        asyncio.run(coro)


def test_fetch_master_steam_game_adds_empty_filter():
    coro = scanner.fetch_master(L4D2, "api.steampowered.com", 443, 5.0, steam_key="K")
    assert coro.cr_code is scanner.fetch_master_steamweb.__code__
    assert coro.cr_frame.f_locals["filter"] == "\\appid\\550\\empty\\1"
    coro.close()


def test_scan_reports_steamweb_error(monkeypatch):
    _fake_urlopen(monkeypatch, 403)
    msgs = []
    result = asyncio.run(scanner.scan(L4D2, list(L4D2.masters), lambda i: None, msgs.append,
                                      steam_key="K"))
    assert result.masters_ok == 0
    assert any("clave" in m.lower() or "key" in m.lower() for m in msgs)
    assert not any("no responde" in m for m in msgs)


def test_steamweb_custom_host_never_sends_key(monkeypatch):
    calls = []
    monkeypatch.setattr(scanner.urllib.request, "urlopen",
                        lambda *a, **k: calls.append(a) or None)
    with pytest.raises(steamweb.SteamWebError) as err:
        asyncio.run(scanner.fetch_master_steamweb("evil.example.com", 443,
                                                  "\\appid\\550", "SECRETKEY123"))
    assert calls == []
    msg = str(err.value)
    assert "SECRETKEY123" not in msg and "evil.example.com" not in msg
    assert steamweb.OFFICIAL_HOST in msg


def test_build_url_rejects_custom_host():
    with pytest.raises(ValueError):
        steamweb.build_url("evil.example.com", "K", "\\appid\\550")


def test_scan_reports_custom_steam_host(monkeypatch):
    calls = []
    monkeypatch.setattr(scanner.urllib.request, "urlopen",
                        lambda *a, **k: calls.append(a) or None)
    msgs = []
    result = asyncio.run(scanner.scan(L4D2, [("evil.example.com", 443)], lambda i: None,
                                      msgs.append, steam_key="K"))
    assert result.masters_ok == 0 and calls == []
    assert any(steamweb.OFFICIAL_HOST in m for m in msgs)


def test_steamweb_cancel_returns_while_download_blocks(monkeypatch):
    started = threading.Event()
    release = threading.Event()
    threads = []

    def fake(url, timeout=None):
        threads.append(threading.current_thread())
        started.set()
        release.wait(5)
        raise urllib.error.URLError("tarde")
    monkeypatch.setattr(scanner.urllib.request, "urlopen", fake)

    async def main():
        task = asyncio.create_task(scanner.fetch_master_steamweb(
            steamweb.OFFICIAL_HOST, 443, "\\appid\\550", "K"))
        while not started.is_set():
            await asyncio.sleep(0.01)
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task

    t0 = time.monotonic()
    asyncio.run(main())
    elapsed = time.monotonic() - t0
    try:
        assert elapsed < 2.0
        assert threads and threads[0].daemon
    finally:
        release.set()
        for t in threads:
            t.join(5)


def test_steamweb_result_from_thread(monkeypatch):
    data = (FIXTURES / "steamweb_l4d2.json").read_bytes()

    class Resp(io.BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, *exc):
            return False
    monkeypatch.setattr(scanner.urllib.request, "urlopen",
                        lambda url, timeout=None: Resp(data))
    servers = asyncio.run(scanner.fetch_master_steamweb(
        steamweb.OFFICIAL_HOST, 443, "\\appid\\550", "K"))
    assert len(servers) == 20


def test_fetch_master_filter_with_and_without_empty(monkeypatch):
    seen = []

    async def fake(host, port, filter, key, timeout=5.0):
        seen.append(filter)
        return []

    monkeypatch.setattr(scanner, "fetch_master_steamweb", fake)
    tf2 = GAMES["tf2"]
    asyncio.run(scanner.fetch_master(tf2, "api.steampowered.com", 443, 5.0, steam_key="K"))
    asyncio.run(scanner.fetch_master(tf2, "api.steampowered.com", 443, 5.0, steam_key="K",
                                     steam_include_empty=True))
    assert seen == ["\\appid\\440\\empty\\1", "\\appid\\440"]


def test_scan_passes_include_empty_to_fetch_master(monkeypatch):
    seen = []

    async def fake_fetch(game, host, port, timeout, steam_key="", steam_include_empty=False):
        seen.append(steam_include_empty)
        return []

    monkeypatch.setattr(scanner, "fetch_master", fake_fetch)
    asyncio.run(scanner.scan(GAMES["tf2"], [("h", 1)], lambda i: None, lambda m: None,
                             steam_key="K", steam_include_empty=True))
    assert seen == [True]


def test_parse_server_list_deeply_nested_is_protocol_error():
    with pytest.raises(ProtocolError):
        steamweb.parse_server_list(b"[" * 100000)
