//! Búsqueda de jugadores. Traducción de tests/test_player_search.py.
use indexmap::IndexMap;
use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::player_search::find_players;

type Lists = IndexMap<String, Vec<ServerInfo>>;

fn srv(host: &str, players: &[(&str, &str)], name: Option<&str>, online: bool) -> ServerInfo {
    let a = ServerAddress::new(host, 28960);
    let mut s = ServerInfo::new(a.clone(), a);
    s.name = name.map_or_else(|| format!("srv {host}"), str::to_owned);
    s.players = players
        .iter()
        .map(|(n, raw)| {
            let mut p = Player::new(*n, 0, 0);
            p.raw_name = (*raw).into();
            p
        })
        .collect();
    s.online = online;
    s
}

fn lists(entries: Vec<(&str, Vec<ServerInfo>)>) -> Lists {
    entries
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
}

fn names(l: &Lists, q: &str, limit: usize) -> Vec<String> {
    find_players(l, q, limit)
        .iter()
        .map(|h| h.player.name.clone())
        .collect()
}

#[test]
fn subcadena_sin_distinguir_mayusculas() {
    let l = lists(vec![(
        "cod2",
        vec![srv(
            "a",
            &[("Jimmy", ""), ("xXjimXx", ""), ("Bob", "")],
            None,
            true,
        )],
    )]);
    let mut n = names(&l, "jim", 500);
    n.sort();
    assert_eq!(n, ["Jimmy", "xXjimXx"]);
}

#[test]
fn consulta_vacia() {
    let l = lists(vec![("cod2", vec![srv("a", &[("Jimmy", "")], None, true)])]);
    assert!(find_players(&l, "  ", 500).is_empty());
}

#[test]
fn usa_nombre_plano_no_codigos_de_color() {
    let l = lists(vec![(
        "cod2",
        vec![srv("a", &[("Jimmy", "^1Ji^7mmy")], None, true)],
    )]);
    assert_eq!(names(&l, "jim", 500), ["Jimmy"]);
    assert!(find_players(&l, "^1", 500).is_empty());
}

#[test]
fn servidor_offline_no_aparece() {
    let l = lists(vec![(
        "cod2",
        vec![srv("a", &[("Jimmy", "")], None, false)],
    )]);
    assert!(find_players(&l, "jim", 500).is_empty());
}

#[test]
fn orden_por_juego_servidor_y_jugador() {
    let l = lists(vec![
        ("ut99", vec![srv("u", &[("Jim", "")], None, true)]),
        (
            "cod2",
            vec![
                srv("b", &[("jimZ", ""), ("Jima", "")], Some("Beta"), true),
                srv("a", &[("Jimb", "")], Some("alfa"), true),
            ],
        ),
    ]);
    let hits = find_players(&l, "jim", 500);
    let ids: Vec<_> = hits.iter().map(|h| h.game_id).collect();
    assert_eq!(ids, ["cod2", "cod2", "cod2", "ut99"]);
    assert_eq!(names(&l, "jim", 500), ["Jimb", "Jima", "jimZ", "Jim"]);
}

#[test]
fn limite() {
    let l = lists(vec![(
        "cod2",
        vec![srv("a", &[("Jim1", ""), ("Jim2", "")], None, true)],
    )]);
    assert_eq!(find_players(&l, "jim", 1).len(), 1);
}

#[test]
fn limite_conserva_los_primeros_en_orden() {
    let players: Vec<(String, &str)> = [5, 3, 1, 4, 2]
        .iter()
        .map(|n| (format!("Jim{n}"), ""))
        .collect();
    let refs: Vec<(&str, &str)> = players.iter().map(|(n, r)| (n.as_str(), *r)).collect();
    let l = lists(vec![("cod2", vec![srv("a", &refs, None, true)])]);
    assert_eq!(names(&l, "jim", 2), ["Jim1", "Jim2"]);
}
