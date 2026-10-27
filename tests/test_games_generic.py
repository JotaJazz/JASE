"""GameDef ampliado para añadir más juegos sin tocar el código."""
import asyncio
import dataclasses

import pytest

from jase import scanner
from jase.config import _default_game_paths
from jase.games import GAMES
from jase.launcher import build_launch_command
from jase.models import ServerAddress
from jase.protocols import gamespy as gs
from jase.protocols import quake3 as q3

T = 0.3


def test_ut99_gamespy_credentials_live_in_gamedef():
    ut = GAMES["ut99"]
    assert ut.gamespy_gamename == "ut" and ut.gamespy_key == "Z5Nfb0"
    assert ut.launch_style == "unreal" and ut.connect_port_fallback == -1
    for gid in ("cod1", "cod2"):
        assert GAMES[gid].launch_style == "q3"
        assert GAMES[gid].version_key == "shortversion"


def test_default_paths_come_from_gamedef():
    assert GAMES["cod2"].default_path == r"D:\Juegos\CoD2"
    assert GAMES["ut99"].default_path == r"D:\Juegos\UnrealTournament"
    assert GAMES["cod1"].default_path == ""
    assert _default_game_paths() == {"cod2": r"D:\Juegos\CoD2",
                                     "ut99": r"D:\Juegos\UnrealTournament",
                                     "mohaa": r"D:\Juegos\MOHAA_STOCK"}


def test_family_valve_scan_requires_appid():
    # La familia "valve" exige `valve_appid` en el GameDef.
    game = dataclasses.replace(GAMES["cod2"], id="cs16", family="valve")
    with pytest.raises(ValueError):
        asyncio.run(scanner.scan(game, [("127.0.0.1", 1)], lambda i: None, lambda s: None,
                                 master_timeout=T, query_timeout=T))


def test_scan_sends_gamedef_gamename_to_gamespy_master():
    received = bytearray()
    game = dataclasses.replace(GAMES["ut99"], id="mohaa", gamespy_gamename="mohaa",
                               gamespy_key="M5Fdwc")

    async def handle(reader, writer):
        writer.write(b"\\basic\\\\secure\\ABCDEF")
        await writer.drain()
        while not received.endswith(b"\\final\\"):
            chunk = await reader.read(1024)
            if not chunk:
                break
            received.extend(chunk)
        writer.write(b"\\final\\")
        await writer.drain()
        writer.close()
        await writer.wait_closed()

    async def main():
        server = await asyncio.start_server(handle, "127.0.0.1", 0)
        port = server.sockets[0].getsockname()[1]
        async with server:
            return await scanner.scan(game, [("127.0.0.1", port)], lambda i: None,
                                      lambda s: None, master_timeout=T, query_timeout=T)

    asyncio.run(main())
    assert b"\\gamename\\mohaa" in received
    assert b"\\validate\\" + gs.gsmsalg("ABCDEF", "M5Fdwc").encode() in received
    assert b"\\list\\\\gamename\\mohaa" in received


def test_gamespy_connect_port_fallback():
    fields = {"hostname": "X", "numplayers": "0", "maxplayers": "8"}
    addr = ServerAddress("1.1.1.1", 7778)
    assert gs.parse_status(fields, addr, 5).connect_address.port == 7777
    assert gs.parse_status(fields, addr, 5, connect_port_fallback=0).connect_address.port == 7778
    fields["hostport"] = "1234"
    assert gs.parse_status(fields, addr, 5, connect_port_fallback=0).connect_address.port == 1234


def test_quake3_version_key():
    data = (b"\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\X\\shortversion\\1.3"
            b"\\version\\ET 2.60b\n")
    addr = ServerAddress("1.1.1.1", 27960)
    assert q3.parse_status_response(data, addr, 1).version == "1.3"
    assert q3.parse_status_response(data, addr, 1, version_key="version").version == "ET 2.60b"


def test_launch_style_decides_command_not_family():
    a = ServerAddress("1.2.3.4", 7777)
    unreal_q3fam = dataclasses.replace(GAMES["cod2"], launch_style="unreal")
    argv, _ = build_launch_command(unreal_q3fam, r"C:\g\x.exe", a, "pw")
    assert argv == [r"C:\g\x.exe", "1.2.3.4:7777?password=pw"]
    q3_gsfam = dataclasses.replace(GAMES["ut99"], launch_style="q3")
    argv, _ = build_launch_command(q3_gsfam, r"C:\g\x.exe", a, "pw")
    assert argv == [r"C:\g\x.exe", "+password", "pw", "+connect", "1.2.3.4:7777"]



def test_query_uses_gamedef_options():
    """version_key (Quake3) y connect_port_fallback (GameSpy) llegan desde el GameDef."""
    from test_scanner import fake_udp

    q3_status = (b"\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\X"
                 b"\\shortversion\\1.3\\version\\ET 2.60b\n")
    gs_status = b"\\hostname\\Y\\numplayers\\0\\maxplayers\\8\\queryid\\1.1\\final\\"
    et = dataclasses.replace(GAMES["cod2"], id="et", version_key="version")
    gs0 = dataclasses.replace(GAMES["ut99"], id="mohaa", connect_port_fallback=0)

    async def main():
        async with fake_udp(lambda d: [q3_status] if d == q3.GETSTATUS else []) as (qp, _):
            a = await scanner.query_one_server(et, ServerAddress("127.0.0.1", qp), timeout=T)
        async with fake_udp(lambda d: [gs_status] if d == gs.STATUS_QUERY else []) as (gp, _):
            b = await scanner.query_one_server(gs0, ServerAddress("127.0.0.1", gp), timeout=T)
            c = await scanner.query_one_server(GAMES["ut99"], ServerAddress("127.0.0.1", gp),
                                               timeout=T)
        return a, b, c, gp

    a, b, c, gp = asyncio.run(main())
    assert a.version == "ET 2.60b"
    assert b.connect_address.port == gp
    assert c.connect_address.port == gp - 1


def test_short_names_for_sidebar():
    expected = {
        "mohaa": "MOHAA: Allied Assault", "mohaas": "MOHAA: Spearhead",
        "mohaab": "MOHAA: Breakthrough", "coduo": "CoD: United Offensive",
        "cod4": "CoD4", "rtcw": "RtCW", "et": "Wolfenstein: ET",
        "jka": "Jedi Academy", "sof2": "SoF2", "cs16": "CS 1.6",
        "bf1942": "BF1942", "l4d": "L4D", "l4d2": "L4D2", "css": "CS: Source",
        "tf2": "TF2", "hldm": "HL DM", "dod": "DoD 1.3", "tfc": "TFC",
        "ut2004": "UT2004", "jk2": "Jedi Outcast", "dday": "D-Day", "zandronum": "Zandronum",
    }
    for gid, game in GAMES.items():
        assert game.short_name == expected.get(gid, game.name)


def test_cod_loadscreens_flag():
    assert {g.id for g in GAMES.values() if g.cod_loadscreens} == {
        "cod1", "cod2", "coduo", "cod4"}
