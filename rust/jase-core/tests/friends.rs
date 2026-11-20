//! Seguimiento de amigos. Traducción de los casos de tests/test_friends.py.
use jase_core::friends::{FriendTracker, friends_in};
use jase_core::models::{Player, ServerAddress, ServerInfo};

fn srv(names: &[&str], port: u16) -> ServerInfo {
    let addr = ServerAddress::new("1.2.3.4", port);
    let mut info = ServerInfo::new(addr.clone(), addr);
    info.name = "S".into();
    info.players = names
        .iter()
        .map(|n| {
            let mut p = Player::new(*n, 0, 0);
            p.raw_name = (*n).into();
            p
        })
        .collect();
    info
}

#[test]
fn friends_in_ignores_colors_and_case() {
    let info = srv(&["^1JoTa", "other"], 27960);
    assert_eq!(
        friends_in(&info, &["jota".into(), "nobody".into()]),
        ["jota"]
    );
}

#[test]
fn tracker_no_repeat_leave_and_return_and_move() {
    let mut t = FriendTracker::new(vec!["jota".into()]);
    let (a, b) = (srv(&["jota"], 1), srv(&["jota"], 2));
    assert_eq!(t.update("q3", &a).len(), 1);
    assert!(t.update("q3", &a).is_empty());
    assert!(t.update("q3", &srv(&[], 1)).is_empty());
    assert_eq!(t.update("q3", &a).len(), 1); // salió y volvió
    assert_eq!(t.update("q3", &b)[0].server, b); // otro servidor
}
