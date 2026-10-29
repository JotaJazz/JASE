"""Counter-Strike 1.6 con el master ms.cs16.net (capturas reales del 2026-10-19)."""
import asyncio
from pathlib import Path

from jase import scanner
from jase.games import ALL_VERSIONS, GAMES
from jase.launcher import build_launch_command
from jase.models import ServerAddress
from jase.protocols import valve

from test_scanner import fake_udp

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 27015)
T = 0.3


def test_cs16_gamedef():
    g = GAMES["cs16"]
    assert g.name == "Counter-Strike 1.6" and g.short_name == "CS 1.6"
    assert g.family == "valve" and g.launch_style == "valve"
    assert g.masters == (("ms.cs16.net", 27010), ("api.steampowered.com", 443))
    assert g.steam_webapi_optional and not g.steam_webapi
    assert g.valve_master_filter == "\\gamedir\\cstrike"
    assert g.valve_gamedir == "cstrike" and g.valve_appid == 10
    assert g.exe_relpath == "hl.exe" and g.gametracker_id == "cs"
    assert g.default_version == ALL_VERSIONS and g.default_path == ""
    assert len(g.badge) == 2


def test_cs16_master_fixture_parses():
    servers, _ = valve.parse_master_response(
        (FIXTURES / "cs16_master_gamedir_1.bin").read_bytes())
    assert len(servers) >= 200
    assert all(s.port > 0 for s in servers)


def test_cs16_info_fixture_parses():
    info = valve.parse_info((FIXTURES / "cs16_info_ms.bin").read_bytes(), ADDR, 5)
    assert info.rules["gamedir"] == "cstrike"
    assert info.name and info.max_players > 0


def test_cs16_rejects_other_gamedir():
    data = (FIXTURES / "cs16_info.bin").read_bytes()
    assert valve.parse_info(data, ADDR, 1, expected_gamedir="CSTRIKE").rules["gamedir"] == "cstrike"
    mod = data.replace(b"\0cstrike\0", b"\0valve\0", 1)
    assert mod != data
    assert valve.parse_info(mod, ADDR, 1).rules["gamedir"] == "valve"
    found = []

    def handler(payload):
        return [mod] if payload == valve.A2S_INFO else []

    async def main():
        async with fake_udp(handler) as (port, _):
            await scanner.query_servers(
                "valve", [ServerAddress("127.0.0.1", port)], found.append, timeout=T,
                retries=0, **scanner.game_query_options(GAMES["cs16"]))

    asyncio.run(main())
    assert found == []


def test_cs16_launch_command():
    exe = "C:\\HL\\hl.exe"
    argv, cwd = build_launch_command(GAMES["cs16"], exe, ADDR, None)
    assert argv == [exe, "-game", "cstrike", "+connect", "1.2.3.4:27015"]
    assert cwd == "C:\\HL"
    argv, _ = build_launch_command(GAMES["cs16"], exe, ADDR, "pw")
    assert argv == [exe, "-game", "cstrike", "+password", "pw", "+connect", "1.2.3.4:27015"]
