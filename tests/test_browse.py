from pathlib import Path

from jase import browse
from jase.browse import BASE_MOD, breakdown, map_of, matches, mod_of
from jase.games import GAMES
from jase.models import ServerAddress, ServerInfo
from jase.protocols import gamespy as gs
from jase.protocols import quake3 as q3

FIXTURES = Path(__file__).parent / "fixtures"
ADDR = ServerAddress("1.2.3.4", 28960)


def _q3(name: str, game_id: str) -> ServerInfo:
    game = GAMES[game_id]
    return q3.parse_status_response((FIXTURES / name).read_bytes(), ADDR, 50,
                                    version_key=game.version_key)


def _srv(map_name: str, **rules: str) -> ServerInfo:
    return ServerInfo(address=ADDR, connect_address=ADDR, name="s", map=map_name,
                      gametype="tdm", num_players=0, max_players=8, password=False,
                      version="1.0", ping_ms=10, rules=dict(rules))


def test_mod_of_cod4_strips_mods_prefix():
    info = _q3("cod4_status_17.bin", "cod4")
    assert mod_of(GAMES["cod4"], info) == "1stmsob_openwarfare"


def test_mod_of_cod4x_without_fs_game_is_base():
    assert mod_of(GAMES["cod4"], _q3("cod4_status_cod4x.bin", "cod4")) == BASE_MOD


def test_mod_of_et_uses_gamename():
    assert mod_of(GAMES["et"], _q3("et_status.bin", "et")) == "nq"


def test_mod_of_rtcw_strips_colors():
    assert mod_of(GAMES["rtcw"], _q3("rtcw_status.bin", "rtcw")) == "ffmod"


def test_mod_of_q3_baseq3_is_base():
    assert mod_of(GAMES["q3"], _q3("q3_status.bin", "q3")) == BASE_MOD


def test_mod_of_mohaa_always_base():
    asm = gs.StatusAssembler()
    assert asm.feed((FIXTURES / "mohaa_status.bin").read_bytes())
    game = GAMES["mohaa"]
    info = gs.parse_status(asm.fields(), ServerAddress("1.2.3.4", 12300), 50,
                           connect_port_fallback=game.connect_port_fallback,
                           expected_gamename=game.status_gamename)
    assert mod_of(game, info) == BASE_MOD


def test_mod_of_prefix_and_case_insensitive():
    assert mod_of(GAMES["cod2"], _srv("m", fs_game="Mods/PAM")) == "pam"


def test_breakdown_hides_mods_when_all_base():
    servers = [_srv("mohaa1"), _srv("mohaa2")]
    assert breakdown(GAMES["mohaa"], servers).mods == ()


def test_breakdown_mods_sorted_by_count_then_name():
    servers = [_srv("m", fs_game="mods/b"), _srv("m", fs_game="mods/a"),
               _srv("m", fs_game="mods/c"), _srv("m", fs_game="mods/c"), _srv("m")]
    assert breakdown(GAMES["cod2"], servers).mods == (
        ("c", 2), (BASE_MOD, 1), ("a", 1), ("b", 1))


def test_breakdown_top_maps_and_other_count():
    servers = [_srv("mp_a")] * 3 + [_srv(f"mp_{c}") for c in "bcdefghijkl"]
    b = breakdown(GAMES["cod2"], servers)
    assert b.maps[0] == ("mp_a", 3) and len(b.maps) == browse.TOP_MAPS == 10
    assert b.other_maps == 2


def test_breakdown_groups_case_insensitive():
    servers = [_srv("MP_Toujane"), _srv("mp_toujane")]
    assert breakdown(GAMES["cod2"], servers).maps == (("mp_toujane", 2),)
    assert map_of(servers[0]) == "mp_toujane"


def test_matches_or_within_and_across():
    g = GAMES["cod2"]
    pam_toujane = _srv("mp_toujane", fs_game="mods/pam")
    pam_carentan = _srv("mp_carentan", fs_game="mods/pam")
    assert matches(g, pam_toujane, frozenset({"pam", "zom"}), frozenset({"mp_toujane"}))
    assert not matches(g, pam_carentan, frozenset({"pam"}), frozenset({"mp_toujane"}))
    assert matches(g, pam_carentan, frozenset(), frozenset())
    assert matches(g, pam_carentan, frozenset(), frozenset({"mp_carentan"}))


def test_breakdown_counts_countries_with_lookup():
    servers = [_srv("a"), _srv("b"), _srv("c")]
    codes = iter(["cl", "ar", "cl"])
    b = breakdown(GAMES["cod2"], servers, lambda info: next(codes))
    assert b.countries == (("cl", 2), ("ar", 1))


def test_breakdown_skips_unknown_country_and_needs_lookup():
    assert breakdown(GAMES["cod2"], [_srv("a")]).countries == ()
    assert breakdown(GAMES["cod2"], [_srv("a")], lambda info: None).countries == ()
