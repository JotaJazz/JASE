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
    assert coro.cr_frame.f_locals["filter"] == "\\appid\\550\\gamedir\\left4dead2\\empty\\1"
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
    assert seen == ["\\appid\\440\\gamedir\\tf\\empty\\1", "\\appid\\440\\gamedir\\tf"]


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


@pytest.mark.parametrize("code, kind", [(401, "invalid_key"), (403, "invalid_key"),
                                        (429, "rate_limited")])
def test_scan_error_kinds_for_steam_http(monkeypatch, code, kind):
    _fake_urlopen(monkeypatch, code)
    result = asyncio.run(scanner.scan(L4D2, list(L4D2.masters), lambda i: None,
                                      lambda m: None, steam_key="K"))
    assert [(e.master, e.kind) for e in result.errors] == [("api.steampowered.com:443", kind)]
    assert "K" not in result.errors[0].message.split()


def test_scan_error_kind_missing_key():
    result = asyncio.run(scanner.scan(L4D2, list(L4D2.masters), lambda i: None,
                                      lambda m: None, steam_key=""))
    assert [e.kind for e in result.errors] == ["missing_key"]
    assert result.masters_ok == 0


def test_scan_error_kind_unsupported_host(monkeypatch):
    monkeypatch.setattr(scanner.urllib.request, "urlopen", lambda *a, **k: None)
    result = asyncio.run(scanner.scan(L4D2, [("evil.example.com", 443)], lambda i: None,
                                      lambda m: None, steam_key="K"))
    assert [e.kind for e in result.errors] == ["unsupported_host"]


# --- spam en la API (sondeo 2026-10-29, CS 1.6): IPs con 200 servidores falsos cada una
# («This is a dummy CS 1.6 server!» o nombres copiados) llenan el tope de 10 000 entradas.
def _api_json(addrs):
    import json
    return json.dumps({"response": {"servers": [{"addr": a} for a in addrs]}}).encode()


def _fake_steam(monkeypatch, pages):
    """Cada llamada devuelve la siguiente página; guarda los filtros pedidos."""
    import urllib.parse
    filters = []

    class Resp(io.BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, *a):
            return False

    def fake(url, timeout=None):
        filters.append(urllib.parse.parse_qs(urllib.parse.urlsplit(url).query)["filter"][0])
        return Resp(_api_json(pages[min(len(filters), len(pages)) - 1]))
    monkeypatch.setattr(scanner.urllib.request, "urlopen", fake)
    return filters


def _spam(ip, n):
    return [f"{ip}:{27000 + i}" for i in range(n)]


def test_steamweb_requeries_without_spam_ips_while_truncated(monkeypatch):
    monkeypatch.setattr(scanner, "STEAMWEB_LIMIT", 300)
    real = ["1.1.1.1:27015", "2.2.2.2:27015", "2.2.2.2:27016"]
    pages = [_spam("9.9.9.9", 150) + _spam("8.8.8.8", 150),  # tope: puede faltar gente
             _spam("7.7.7.7", 120) + real + _spam("6.6.6.6", 180)[:177],  # tope otra vez
             real + _spam("5.5.5.5", 100)]  # bajo el tope: completa
    filters = _fake_steam(monkeypatch, pages)
    got = asyncio.run(scanner.fetch_master_steamweb("api.steampowered.com", 443,
                                                    "\\appid\\10\\gamedir\\cstrike", "K"))
    assert sorted(map(str, got)) == sorted(real)
    assert len(filters) == 3
    assert filters[0].startswith("\\appid\\10\\gamedir\\cstrike\\nand\\1\\name_match\\")
    assert "\\nor\\2\\gameaddr\\8.8.8.8\\gameaddr\\9.9.9.9" in filters[1]
    assert "\\nor\\4\\gameaddr\\6.6.6.6\\gameaddr\\7.7.7.7\\gameaddr\\8.8.8.8" in filters[2]


def test_steamweb_single_query_when_not_truncated(monkeypatch):
    filters = _fake_steam(monkeypatch, [["1.1.1.1:27015"] + _spam("9.9.9.9", 100)])
    got = asyncio.run(scanner.fetch_master_steamweb("api.steampowered.com", 443,
                                                    "\\appid\\10", "K"))
    assert [str(a) for a in got] == ["1.1.1.1:27015"] and len(filters) == 1


def test_steamweb_stops_after_max_passes(monkeypatch):
    monkeypatch.setattr(scanner, "STEAMWEB_LIMIT", 100)
    pages = [_spam(f"9.9.9.{i}", 100) for i in range(1, 20)]
    filters = _fake_steam(monkeypatch, pages)
    got = asyncio.run(scanner.fetch_master_steamweb("api.steampowered.com", 443,
                                                    "\\appid\\10", "K"))
    assert got == [] and len(filters) == scanner.STEAMWEB_MAX_PASSES


def test_steam_filter_uses_appid_and_gamedir():
    assert scanner.steam_filter(GAMES["cs16"], include_empty=False) == \
        "\\appid\\10\\gamedir\\cstrike\\empty\\1"
    assert scanner.steam_filter(GAMES["dod"], include_empty=True) == "\\appid\\30\\gamedir\\dod"


def test_cs16_lists_through_steam_only_with_the_official_host():
    game = GAMES["cs16"]
    assert ("api.steampowered.com", 443) in game.masters and game.uses_steam_api
    assert not game.steam_webapi  # la clave es opcional: ms.cs16.net sigue sin ella
    coro = scanner.fetch_master(game, "api.steampowered.com", 443, 5.0, steam_key="K")
    assert coro.cr_code is scanner.fetch_master_steamweb.__code__
    assert coro.cr_frame.f_locals["filter"] == "\\appid\\10\\gamedir\\cstrike\\empty\\1"
    coro.close()
    coro = scanner.fetch_master(game, "ms.cs16.net", 27010, 5.0, steam_key="K")
    assert coro.cr_code is scanner.fetch_master_valve.__code__
    coro.close()


def test_cs16_without_key_still_scans_udp_master(monkeypatch):
    async def fake_valve(host, port, flt, timeout=3.0):
        return [scanner.ServerAddress("1.1.1.1", 27015)]

    async def fake_query(family, addrs, on_server, **kw):
        pass
    monkeypatch.setattr(scanner, "fetch_master_valve", fake_valve)
    monkeypatch.setattr(scanner, "query_servers", fake_query)
    game = GAMES["cs16"]
    result = asyncio.run(scanner.scan(game, list(game.masters), lambda i: None,
                                      lambda m: None, steam_key=""))
    assert result.ok and result.servers_listed == 1
    assert [e.kind for e in result.errors] == ["missing_key"]
