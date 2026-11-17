//! «Se armó partida». Traducción de los casos de tests/test_party.py.
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::party::PartyTracker;

fn srv(players: i32, bots: i32, online: bool) -> ServerInfo {
    let addr = ServerAddress::new("1.2.3.4", 28960);
    let mut info = ServerInfo::new(addr.clone(), addr);
    info.num_players = players;
    info.max_players = 16;
    info.bots = bots;
    info.online = online;
    info
}

fn people(n: i32) -> ServerInfo {
    srv(n, 0, true)
}

#[test]
fn tracker_first_answer_is_baseline_then_notifies_once() {
    let mut t = PartyTracker::default();
    assert!(!t.update("cod2", &people(1), 4)); // punto de partida
    assert!(!t.update("cod2", &people(3), 4));
    assert!(t.update("cod2", &people(4), 4));
    assert!(!t.update("cod2", &people(5), 4));
    assert!(!t.update("cod2", &people(3), 4)); // 4→3→4 no vuelve a avisar
    assert!(!t.update("cod2", &people(4), 4));
    assert!(!t.update("cod2", &people(2), 4)); // baja a la mitad: se rearma
    assert!(t.update("cod2", &people(4), 4));
}

#[test]
fn tracker_counts_people_not_bots_and_ignores_offline() {
    let mut t = PartyTracker::default();
    t.update("cod2", &people(0), 4);
    assert!(!t.update("cod2", &srv(8, 6, true), 4));
    assert!(!t.update("cod2", &srv(0, 0, false), 4));
    assert!(t.update("cod2", &srv(9, 4, true), 4));
}

#[test]
fn already_busy_server_does_not_notify_on_start() {
    let mut t = PartyTracker::default();
    assert!(!t.update("cod2", &people(10), 4));
    assert!(!t.update("cod2", &people(10), 4));
}

#[test]
fn reset_forgets_the_baseline() {
    let mut t = PartyTracker::default();
    t.update("cod2", &people(1), 4);
    t.reset("cod2", "1.2.3.4:28960");
    assert!(!t.update("cod2", &people(5), 4)); // vuelve a ser el punto de partida
}
