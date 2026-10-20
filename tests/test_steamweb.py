"""API web de Steam (IGameServersService/GetServerList) como fuente de servidores.

Fixture steamweb_l4d2.json: primeros 20 servidores de `\\appid\\550\\empty\\1`
(solo `response.servers`; sin URL ni clave)."""
import asyncio
import io
import urllib.error
from pathlib import Path

import pytest

from jase import scanner
from jase.games import ALL_VERSIONS, GameDef
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
