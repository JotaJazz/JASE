"""Juegos de la familia Quake 3 sondeados el 2026-10-24 (JASE 0.12), con respuestas reales."""
from pathlib import Path

import pytest

from jase.browse import BASE_MOD, mod_of
from jase.games import ALL_VERSIONS, GAMES
from jase.launcher import build_launch_command
from jase.models import ServerAddress
from jase.protocols import quake3 as q3

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 27960)

V012_GAMES = ("jk2", "xonotic", "warsow", "warfork", "tremulous", "smokinguns")


def _status(game_id: str):
    game = GAMES[game_id]
    data = (FIXTURES / f"{game_id}_status.bin").read_bytes()
    return q3.parse_status_response(data, ADDR, 42, version_key=game.version_key,
                                    version_aliases=game.version_aliases,
                                    exclude_mods=game.status_exclude_mods)


@pytest.mark.parametrize("game_id", V012_GAMES)
def test_v012_games_are_quake3_family(game_id):
    game = GAMES[game_id]
    assert game.id == game_id
    assert game.family == "quake3" and game.launch_style == "q3"
    assert game.masters and game.protocols
    assert game.default_path == "" and len(game.badge) == 2
    assert game.default_version == ALL_VERSIONS
    assert game.gametracker_id == ""  # GameTracker devuelve 404 (sondeo 2026-10-24)


@pytest.mark.parametrize("game_id", V012_GAMES)
def test_v012_status_fixture_parses(game_id):
    info = _status(game_id)
    assert info.name and info.map and info.max_players > 0


@pytest.mark.parametrize("game_id", V012_GAMES)
def test_v012_launch_uses_q3_connect(game_id):
    game = GAMES[game_id]
    cmd, cwd = build_launch_command(game, "C:\\x\\game.exe", ADDR, "pw")
    assert cmd[1:] == ["+password", "pw", "+connect", "1.2.3.4:27960"] and cwd == "C:\\x"


def test_badges_stay_unique():
    badges = [g.badge for g in GAMES.values() if g.badge]
    assert len(badges) == len(set(badges))


def test_jk2_gamedef_and_status():
    game = GAMES["jk2"]
    assert (game.name, game.short_name) == ("Star Wars Jedi Knight II: Jedi Outcast", "Jedi Outcast")
    assert game.masters == (("master.jkhub.org", 28060),)  # masterjk2.ravensoft.com no responde
    assert game.protocols == (15, 16) and game.master_gamename == ""
    assert game.exe_relpath == "GameData\\jk2mp.exe" and game.exe_candidates == ("jk2mp.exe",)
    assert game.levelshot_dir == "base"
    info = _status("jk2")
    assert info.name == "Jk a notorius n3w - BASZODAJA"
    assert info.version == "JK2MP: v1.04mv"
    assert info.map == "ctf_bespin"
    assert (info.num_players, info.max_players) == (16, 25)
    assert mod_of(game, info) == BASE_MOD  # gamename basejk


def test_xonotic_gamedef_and_status():
    game = GAMES["xonotic"]
    assert game.masters == (("dpmaster.deathmask.net", 27950), ("dpmaster.tchr.no", 27950))
    assert game.master_gamename == "Xonotic" and game.protocols == (3,)
    assert game.exe_relpath == "xonotic.exe"
    assert game.exe_candidates == ("xonotic-x86_64.exe",)
    info = _status("xonotic")
    # DarkPlaces manda `hostname` (no `sv_hostname`) y la versión en `gameversion`.
    assert info.name == "[XDWC-26] Xonotic Defrag World Championship 2026 | DE-01"
    assert info.version == "806"
    assert info.map == "xdwc2026-1"
    assert (info.num_players, info.max_players) == (3, 32)
    assert mod_of(game, info) == BASE_MOD  # modname data


def test_warsow_gamedef_and_status():
    game = GAMES["warsow"]
    assert game.masters == (("dpmaster.deathmask.net", 27950), ("dpmaster.tchr.no", 27950))
    assert game.master_gamename == "Warsow" and game.protocols == (22,)
    assert game.exe_relpath == "warsow_x64.exe" and game.levelshot_dir == "basewsw"
    info = _status("warsow")
    assert info.name == "Coopera" and info.version == "2.10"
    assert info.map == "wdm17"
    assert (info.num_players, info.max_players) == (5, 15)
    assert mod_of(game, info) == BASE_MOD  # fs_game basewsw


def test_warfork_gamedef_and_status():
    game = GAMES["warfork"]
    assert game.masters == (("dpmaster.deathmask.net", 27950),)
    assert game.master_gamename == "Warfork" and game.protocols == (26,)
    assert game.exe_relpath == "warfork_x64.exe" and game.levelshot_dir == "basewf"
    info = _status("warfork")
    assert info.name == "WF BRASIL | Votable Server" and info.version == "2.15"
    assert info.map == "wfdm12"
    assert (info.num_players, info.max_players) == (0, 32)
    assert mod_of(game, info) == BASE_MOD  # fs_game basewf


def test_tremulous_gamedef_and_status():
    game = GAMES["tremulous"]
    assert game.masters == (("master.tremulous.net", 30710),)
    assert game.protocols == (69,) and game.master_gamename == ""
    assert game.exe_relpath == "tremulous.exe" and game.levelshot_dir == "base"
    info = _status("tremulous")
    assert info.name == "Der Bunker | Trench NA (zittrig.eu)"
    assert info.version == "tremulous"
    assert info.map == "atcs"
    assert (info.num_players, info.max_players) == (1, 64)


def test_smokinguns_gamedef_and_status():
    game = GAMES["smokinguns"]
    assert game.name == "Smokin' Guns"
    assert game.masters == (("master.smokin-guns.org", 27950),)
    assert game.protocols == (68,) and game.master_gamename == ""
    assert game.exe_relpath == "smokinguns.exe" and game.levelshot_dir == "smokinguns"
    info = _status("smokinguns")
    assert info.name == "Sillingy Arena - Master"
    assert info.version == "1.1"
    assert info.map == "dm_tillian"
    assert (info.num_players, info.max_players) == (6, 16)
    assert info.password is True


def test_hostname_fallback_only_without_sv_hostname():
    data = b"\xff\xff\xff\xffstatusResponse\n\\hostname\\^1DP\\sv_maxclients\\8\n"
    assert q3.parse_status_response(data, ADDR, 1).name == "DP"
    both = b"\xff\xff\xff\xffstatusResponse\n\\sv_hostname\\A\\hostname\\B\\sv_maxclients\\8\n"
    assert q3.parse_status_response(both, ADDR, 1).name == "A"


def test_dpmaster_xonotic_master_packet_parses():
    # Captura real (2026-10-24) de `getservers Xonotic 3 full empty` a dpmaster.deathmask.net.
    servers, _ = q3.parse_getservers_response((FIXTURES / "xonotic_master_3_1.bin").read_bytes())
    assert len(servers) > 5
    assert all(s.port > 0 for s in servers)
