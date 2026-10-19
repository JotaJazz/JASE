"""Medal of Honor: Allied Assault (+ Spearhead y Breakthrough), familia GameSpy.

Fixtures capturados el 2026-10-19 de master.333networks.com y de servidores reales."""
import asyncio
from pathlib import Path

import pytest
from jase import scanner
from jase.games import ALL_VERSIONS, GAMES
from jase.launcher import build_launch_command, launch
from jase.maps import cod, unreal
from jase.maps.service import MapImageService, gametracker_url
from jase.models import ServerAddress
from jase.paths import resolve_exe
from jase.protocols import ProtocolError
from jase.protocols import gamespy as gs

FIXTURES = Path(__file__).parent / "fixtures"
MOH_IDS = ("mohaa", "mohaas", "mohaab")


def _status(name: str, game_id: str, address=ServerAddress("1.2.3.4", 12300)):
    asm = gs.StatusAssembler()
    assert asm.feed((FIXTURES / name).read_bytes())
    game = GAMES[game_id]
    return gs.parse_status(asm.fields(), address, 50,
                           connect_port_fallback=game.connect_port_fallback,
                           expected_gamename=game.status_gamename)


@pytest.mark.parametrize("game_id,gamename,key,exe,launcher,gt", [
    ("mohaa", "mohaa", "M5Fdwc", "MOHAA.exe", "launch_openmohaa_base.exe", "mohaa"),
    ("mohaas", "mohaas", "h2P1c9", "moh_spearhead.exe", "launch_openmohaa_spearhead.exe", "sh"),
    ("mohaab", "mohaab", "y32FDc", "moh_breakthrough.exe",
     "launch_openmohaa_breakthrough.exe", "bt"),
])
def test_gamedefs(game_id, gamename, key, exe, launcher, gt):
    g = GAMES[game_id]
    assert g.family == "gamespy"
    assert g.masters == (("master.333networks.com", 28900), ("master.openspy.net", 28900))
    assert (g.gamespy_gamename, g.gamespy_key) == (gamename, key)
    assert g.status_gamename == gamename
    assert g.launch_style == "q3"
    assert g.connect_port_fallback == -97
    assert g.exe_relpath == exe
    # openmohaa.exe a secas arranca siempre AA (com_target_game 0): solo para mohaa.
    expected = (launcher, "openmohaa.exe") if game_id == "mohaa" else (launcher,)
    assert g.exe_candidates == expected
    assert g.gametracker_id == gt
    assert g.default_version == ALL_VERSIONS


def test_default_path_only_for_base_game():
    assert GAMES["mohaa"].default_path == r"D:\Juegos\MOHAA_STOCK"
    assert GAMES["mohaas"].default_path == ""
    assert GAMES["mohaab"].default_path == ""


@pytest.mark.parametrize("fixture,count,unique", [
    ("mohaa_master_333.bin", 195, 166), ("mohaas_master_333.bin", 75, 66),
    ("mohaab_master_333.bin", 39, 31),
])
def test_master_list_fixtures(fixture, count, unique):
    # 333networks repite entradas; el escáner deduplica.
    servers = gs.parse_master_list((FIXTURES / fixture).read_bytes())
    assert (len(servers), len(set(servers))) == (count, unique)
    # El master lista el puerto de consulta (12300 + n), no el de juego.
    assert sum(s.port >= 12300 for s in servers) > count // 2


def test_stock_status_with_players():
    info = _status("mohaa_status.bin", "mohaa")
    assert info.name == "-<MisFits>- Rifle/sniper server"
    assert info.map == "dm/mohdm3"
    assert info.gametype == "Objective-Match"
    assert (info.num_players, info.max_players) == (10, 32)
    assert len(info.players) == 10 and info.players[1].name == "-<MisFits>-taz420"
    assert info.version == "1.11"
    assert info.connect_address == ServerAddress("1.2.3.4", 12203)  # hostport
    assert not info.password  # MOHAA no envía `password` en \status\


def test_openmohaa_status():
    info = _status("mohaa_status_openmohaa.bin", "mohaa")
    assert info.version == "1.12+0.83.0"
    assert info.map == "mohdm6"
    assert info.connect_address.port == 12205


def test_hostport_wins_over_fallback():
    info = _status("mohaa_status_hostport.bin", "mohaa", ServerAddress("1.2.3.4", 12303))
    assert info.connect_address.port == 14005


def test_fallback_is_query_port_minus_97():
    fields = {"hostname": "x", "numplayers": "0", "maxplayers": "8", "gamename": "mohaa"}
    info = gs.parse_status(fields, ServerAddress("1.2.3.4", 12300), 1,
                           connect_port_fallback=GAMES["mohaa"].connect_port_fallback,
                           expected_gamename="mohaa")
    assert info.connect_address.port == 12203


def test_spearhead_and_breakthrough_status():
    sh = _status("mohaas_status.bin", "mohaas")
    assert (sh.version, sh.map, sh.connect_address.port) == ("2.15", "mohdm6", 12203)
    bt = _status("mohaab_status.bin", "mohaab")
    assert (bt.version, bt.map, bt.max_players) == ("2.40", "mp_malta_dm", 64)
    assert bt.connect_address.port == 12229


def test_crosslisted_server_of_other_variant_is_rejected():
    # El master de Spearhead lista también servidores de AA (gamename mohaa).
    with pytest.raises(ProtocolError):
        _status("mohaas_status_crosslisted.bin", "mohaas")
    assert _status("mohaas_status_crosslisted.bin", "mohaa").name == "=Masters Of Blood="


def test_missing_gamename_is_accepted_and_ut99_does_not_filter():
    fields = {"hostname": "x", "numplayers": "0", "maxplayers": "8"}
    assert gs.parse_status(fields, ServerAddress("1.2.3.4", 1), 1,
                           expected_gamename="mohaa").name == "x"
    assert GAMES["ut99"].status_gamename == ""


def test_scanner_discards_other_variant():
    from test_scanner import fake_udp

    payload = (FIXTURES / "mohaas_status_crosslisted.bin").read_bytes()

    async def main():
        async with fake_udp(lambda d: [payload] if d == gs.STATUS_QUERY else []) as (port, _):
            addr = ServerAddress("127.0.0.1", port)
            sh = await scanner.query_one_server(GAMES["mohaas"], addr, timeout=0.5)
            aa = await scanner.query_one_server(GAMES["mohaa"], addr, timeout=0.5)
        return sh, aa

    sh, aa = asyncio.run(main())
    assert sh is None
    assert aa is not None and aa.version == "1.11"


def test_launch_command():
    argv, cwd = build_launch_command(GAMES["mohaa"], r"D:\Juegos\MOHAA_STOCK\MOHAA.exe",
                                     ServerAddress("1.2.3.4", 12203), "pw")
    assert argv == [r"D:\Juegos\MOHAA_STOCK\MOHAA.exe", "+password", "pw",
                    "+connect", "1.2.3.4:12203"]
    assert cwd == r"D:\Juegos\MOHAA_STOCK"


def _touch(p: Path) -> Path:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(b"")
    return p


def _resolve(g, d):
    return resolve_exe(str(d), g.exe_relpath, g.exe_candidates, g.preferred_exes)


@pytest.mark.parametrize("game_id,launcher", [
    ("mohaa", "launch_openmohaa_base.exe"),
    ("mohaas", "launch_openmohaa_spearhead.exe"),
    ("mohaab", "launch_openmohaa_breakthrough.exe"),
])
def test_preferred_exes_are_the_variant_launcher(game_id, launcher):
    assert GAMES[game_id].preferred_exes == (launcher,)


def test_stock_layout_resolves_mohaa_exe(tmp_path):
    """D:\\Juegos\\MOHAA_STOCK: solo MOHAA.exe."""
    exe = _touch(tmp_path / "MOHAA.exe")
    _touch(tmp_path / "main" / "pak0.pk3")
    assert _resolve(GAMES["mohaa"], tmp_path) == str(exe)


def test_openmohaa_layout_prefers_launcher_over_mohaa_exe(tmp_path):
    """D:\\Juegos\\MOHAA_OPEN / MOHAA_STOCK_OPENMOHAA: MOHAA.exe junto a los lanzadores."""
    _touch(tmp_path / "MOHAA.exe")
    _touch(tmp_path / "openmohaa.exe")
    base = _touch(tmp_path / "launch_openmohaa_base.exe")
    sh = _touch(tmp_path / "launch_openmohaa_spearhead.exe")
    bt = _touch(tmp_path / "launch_openmohaa_breakthrough.exe")
    assert _resolve(GAMES["mohaa"], tmp_path) == str(base)
    assert _resolve(GAMES["mohaas"], tmp_path) == str(sh)
    assert _resolve(GAMES["mohaab"], tmp_path) == str(bt)


def test_launch_uses_openmohaa_launcher_when_colocated(tmp_path):
    _touch(tmp_path / "MOHAA.exe")
    launcher = _touch(tmp_path / "launch_openmohaa_base.exe")
    calls = []
    launch(GAMES["mohaa"], str(tmp_path), ServerAddress("1.2.3.4", 12203), None,
           popen=lambda argv, cwd: calls.append((argv, cwd)))
    assert calls == [([str(launcher), "+connect", "1.2.3.4:12203"], str(tmp_path))]


def test_openmohaa_only_install_uses_launcher(tmp_path):
    _touch(tmp_path / "openmohaa.exe")
    launcher = _touch(tmp_path / "launch_openmohaa_spearhead.exe")
    calls = []
    launch(GAMES["mohaas"], str(tmp_path), ServerAddress("1.2.3.4", 12203), None,
           popen=lambda argv, cwd: calls.append((argv, cwd)))
    assert calls == [([str(launcher), "+connect", "1.2.3.4:12203"], str(tmp_path))]


def test_gametracker_url_replaces_slash():
    assert gametracker_url("mohaa", "obj/obj_team2") == (
        "https://image.gametracker.com/images/maps/160x120/mohaa/obj_obj_team2.jpg")


def test_mohaa_never_uses_unreal_or_cod_extractor(tmp_path, monkeypatch):
    def boom(*a):
        raise AssertionError("no debe llamarse")

    monkeypatch.setattr(unreal, "find_map_file", boom)
    monkeypatch.setattr(cod, "find_loadscreen", boom)
    urls = []

    def fetch(url):
        urls.append(url)
        return None

    svc = MapImageService(tmp_path, fetch=fetch)
    assert svc.get(GAMES["mohaa"], str(tmp_path), "dm/mohdm6", True) is None
    assert urls == ["https://image.gametracker.com/images/maps/160x120/mohaa/dm_mohdm6.jpg"]
