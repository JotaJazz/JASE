from jase.models import Player, ServerAddress, ServerInfo
from jase.player_search import PlayerHit, find_players


def srv(host, players, name=None, online=True):
    a = ServerAddress(host, 28960)
    s = ServerInfo(a, a, name or f"srv {host}", "m", "dm", len(players), 20, False, "1", 10,
                   [Player(n, 0, 0, raw) for n, raw in players], {})
    s.online = online
    return s


def names(hits):
    return [h.player.name for h in hits]


def test_subcadena_sin_distinguir_mayusculas():
    lists = {"cod2": [srv("a", [("Jimmy", ""), ("xXjimXx", ""), ("Bob", "")])]}
    assert sorted(names(find_players(lists, "jim"))) == ["Jimmy", "xXjimXx"]


def test_consulta_vacia():
    lists = {"cod2": [srv("a", [("Jimmy", "")])]}
    assert find_players(lists, "  ") == []


def test_usa_nombre_plano_no_codigos_de_color():
    lists = {"cod2": [srv("a", [("Jimmy", "^1Ji^7mmy")])]}
    assert names(find_players(lists, "jim")) == ["Jimmy"]
    assert find_players(lists, "^1") == []


def test_servidor_offline_no_aparece():
    lists = {"cod2": [srv("a", [("Jimmy", "")], online=False)]}
    assert find_players(lists, "jim") == []


def test_orden_por_juego_servidor_y_jugador():
    lists = {"ut99": [srv("u", [("Jim", "")])],
             "cod2": [srv("b", [("jimZ", ""), ("Jima", "")], name="Beta"),
                      srv("a", [("Jimb", "")], name="alfa")]}
    hits = find_players(lists, "jim")
    assert [h.game_id for h in hits] == ["cod2", "cod2", "cod2", "ut99"]
    assert names(hits) == ["Jimb", "Jima", "jimZ", "Jim"]
    assert isinstance(hits[0], PlayerHit)


def test_limite():
    lists = {"cod2": [srv("a", [("Jim1", ""), ("Jim2", "")])]}
    assert len(find_players(lists, "jim", limit=1)) == 1


def test_limite_conserva_los_primeros_en_orden():
    lists = {"cod2": [srv("a", [(f"Jim{n}", "") for n in (5, 3, 1, 4, 2)])]}
    hits = find_players(lists, "jim", limit=2)
    assert names(hits) == ["Jim1", "Jim2"]
