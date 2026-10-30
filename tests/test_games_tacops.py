"""Tactical Ops: juego aparte que comparte el master `ut` de Unreal Tournament.

Fixtures capturados el 2026-10-30 de servidores reales (TO340, TO350 y s_SWATGame)."""
import asyncio
from pathlib import Path

import pytest
from jase import detect, identify, lan, scanner
from jase.detect import Finding
from jase.games import ALL_VERSIONS, GAMES, is_tacops
from jase.launcher import launch
from jase.maps.service import MapImageService
from jase.models import ServerAddress, ServerInfo
from jase.paths import version_dir
from jase.protocols import gamespy as gs

FIXTURES = Path(__file__).parent / "fixtures"
QUERY = ServerAddress("1.2.3.4", 7778)
TO = GAMES["tacops"]
UT = GAMES["ut99"]


def _status(name: str, game=TO):
    asm = gs.StatusAssembler()
    assert asm.feed((FIXTURES / name).read_bytes())
    return gs.parse_status(asm.fields(), QUERY, 50,
                           connect_port_fallback=game.connect_port_fallback,
                           version_key=game.version_key,
                           version_aliases=game.version_aliases)


def _touch(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"")
    return path


def _install(root: Path) -> None:
    _touch(root / "TO340" / "System" / "TacticalOps.exe")
    _touch(root / "TO350" / "System" / "TacticalOps.exe")


def _info(**rules) -> ServerInfo:
    return ServerInfo(address=QUERY, connect_address=ServerAddress("1.2.3.4", 7777), name="s",
                      map="TO-Scope", gametype=rules.get("gametype", ""), num_players=0,
                      max_players=8, password=False, version="469", ping_ms=10, players=[],
                      rules={"gamename": "ut", **rules})


def test_tacops_gamedef_shares_the_ut_master():
    assert TO.family == "gamespy" and TO.badge == "TO"
    assert TO.masters == UT.masters
    assert (TO.gamespy_gamename, TO.gamespy_key) == (UT.gamespy_gamename, UT.gamespy_key)
    assert TO.launch_style == "unreal" and TO.connect_port_fallback == -1
    assert TO.exe_relpath == "System\\TacticalOps.exe"
    assert TO.known_versions == ("3.4", "3.5") and TO.default_version == ALL_VERSIONS


@pytest.mark.parametrize("rules,expected", [
    ({"gametype": "TO340"}, True),
    ({"gametype": "TO350"}, True),
    ({"gametype": "to350"}, True),
    ({"gametype": "s_SWATGame"}, True),
    ({"gametype": "TOSTServerGame"}, True),  # puerto vecino de un servidor TOST
    ({"gametype": "DeathMatchPlus", "tostver": "4.5.2.6"}, True),
    ({"gametype": "DeathMatchPlus"}, False),
    ({"gametype": "TOPlus"}, False),  # empieza igual, pero no es `TO<número>`
    ({}, False),
])
def test_is_tacops(rules, expected):
    assert is_tacops(rules) is expected
    assert TO.lists_server(rules) is expected
    assert UT.lists_server(rules) is not expected


def test_other_games_accept_every_server():
    assert GAMES["mohaa"].lists_server({"gametype": "TO340"})


@pytest.mark.parametrize("name,version,label", [
    ("to340_status.bin", "3.4", "3.4"),
    ("to350_status.bin", "3.5", "3.5"),
    ("to_swat_status.bin", "s_SWATGame", "s_SWATGame"),
])
def test_version_comes_from_the_gametype(name, version, label):
    info = _status(name)
    assert is_tacops(info.rules)
    assert info.version == version and TO.version_label(info.version) == label
    assert info.rules["gamever"] == "469"  # el valor crudo sigue en las reglas


def test_ut99_keeps_the_engine_version():
    assert _status("to340_status.bin", UT).version == "469"


def test_identify_splits_the_shared_gamename():
    assert identify.identify("gamespy", _status("to350_status.bin")) == ["tacops"]
    assert identify.identify("gamespy", _info(gametype="CTFGame")) == ["ut99"]


def _scan(game, infos, monkeypatch):
    async def fake_master(*_a, **_k):
        return [i.address for i in infos]

    async def fake_query(_family, _addresses, on_server, **_k):
        for info in infos:
            on_server(info)

    monkeypatch.setattr(scanner, "fetch_master_gamespy", fake_master)
    monkeypatch.setattr(scanner, "query_servers", fake_query)
    got = []
    result = asyncio.run(scanner.scan(game, [("m", 1)], got.append, lambda _m: None))
    return got, result


def test_scan_splits_the_master_list(monkeypatch):
    to, ut = _info(gametype="TO340"), _info(gametype="CTFGame")
    assert _scan(TO, [to, ut], monkeypatch)[0] == [to]
    got, result = _scan(UT, [to, ut], monkeypatch)
    assert got == [ut] and result.servers_responded == 1


def test_direct_query_does_not_filter_a_saved_ut99_favorite():
    assert "lists_server" not in scanner.game_query_options(UT)


def test_scan_lan_applies_the_same_split(monkeypatch):
    to, ut = _info(gametype="TO340"), _info(gametype="CTFGame")

    async def fake_discover(_game):
        return [QUERY]

    async def fake_query(_family, _addresses, on_server, **_k):
        on_server(to)
        on_server(ut)

    monkeypatch.setattr(lan, "discover", fake_discover)
    monkeypatch.setattr(scanner, "query_servers", fake_query)
    got = []
    asyncio.run(lan.scan_lan(TO, [], got.append, lambda _m: None))
    assert got == [to]


@pytest.mark.parametrize("version,folder", [("3.4", "TO340"), ("3.5", "TO350")])
def test_launch_opens_the_client_of_the_server_version(tmp_path, version, folder):
    _install(tmp_path)
    calls = []
    launch(TO, str(tmp_path), ServerAddress("5.6.7.8", 7777), "pw",
           popen=lambda argv, **kw: calls.append((argv, kw)), version=version)
    exe = tmp_path / folder / "System" / "TacticalOps.exe"
    assert calls == [([str(exe), "5.6.7.8:7777?password=pw"], {"cwd": str(exe.parent)})]


def test_launch_from_a_version_folder_finds_the_sibling(tmp_path):
    _install(tmp_path)
    calls = []
    launch(TO, str(tmp_path / "TO340"), ServerAddress("5.6.7.8", 7777), None,
           popen=lambda argv, **kw: calls.append(argv), version="3.5")
    assert calls[0][0] == str(tmp_path / "TO350" / "System" / "TacticalOps.exe")


def test_launch_unknown_version_uses_the_first_install(tmp_path):
    _install(tmp_path)
    calls = []
    launch(TO, str(tmp_path), ServerAddress("5.6.7.8", 7777), None,
           popen=lambda argv, **kw: calls.append(argv), version="s_SWATGame")
    assert calls[0][0] == str(tmp_path / "TO340" / "System" / "TacticalOps.exe")


def test_launch_single_install_ignores_the_version(tmp_path):
    exe = _touch(tmp_path / "System" / "TacticalOps.exe")
    calls = []
    launch(TO, str(tmp_path), ServerAddress("5.6.7.8", 7777), None,
           popen=lambda argv, **kw: calls.append(argv), version="3.5")
    assert calls[0][0] == str(exe)


def test_version_dir_ignores_case_and_files(tmp_path):
    (tmp_path / "to350").mkdir()
    _touch(tmp_path / "TO340")
    assert version_dir(str(tmp_path), "TO350") == str(tmp_path / "to350")
    assert version_dir(str(tmp_path), "TO340") is None
    assert version_dir(None, "TO340") is None


def test_scan_folder_reports_one_install_above_the_versions(tmp_path):
    _install(tmp_path / "TacticalOps")
    found = detect.scan_folder(str(tmp_path), "carpeta elegida", 4, lambda: False)
    assert found == [Finding("tacops", str(tmp_path / "TacticalOps"), "carpeta elegida")]
    assert detect.resolve_game_exe(TO, found[0].path) is not None


def test_typical_roots_include_the_tacops_folders():
    roots = detect.typical_roots(["I:\\"])
    assert any(r.endswith("TacticalOps") for r in roots)
    assert any(r.endswith("Infogrames") for r in roots)


def test_local_map_image_looks_in_both_installs(tmp_path, monkeypatch):
    _install(tmp_path)
    only_350 = _touch(tmp_path / "TO350" / "Maps" / "TO-Scope.unr")
    _touch(tmp_path / "TO340" / "Maps" / "TO-Other.unr")
    seen = []
    monkeypatch.setattr("jase.maps.unreal.extract_screenshot",
                        lambda path: seen.append(path) or "imagen")
    assert MapImageService._extract_local(TO, str(tmp_path), "TO-Scope") == "imagen"
    assert seen == [str(only_350)]


def test_tacops_lan_port_matches_ut99():
    assert lan.lan_ports(TO) == lan.lan_ports(UT)
