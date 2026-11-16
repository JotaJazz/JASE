use jase_core::models::{Player, ServerAddress, ServerInfo};
use jase_core::protocols::{bounded_int, is_bot, parse_infostring, strip_control};
use jase_core::pystr::{casefold, latin1, py_split, utf8_or_latin1};

#[test]
fn bounded_int_plain() {
    assert_eq!(bounded_int("12").unwrap(), 12);
    assert_eq!(bounded_int(" 12 ").unwrap(), 12);
    assert_eq!(bounded_int("-5").unwrap(), -5);
    assert_eq!(bounded_int("+7").unwrap(), 7);
    assert_eq!(bounded_int("1_000").unwrap(), 1000);
}

#[test]
fn bounded_int_clamps() {
    assert_eq!(bounded_int("99999999999999999999").unwrap(), i32::MAX);
    assert_eq!(bounded_int("-99999999999999999999").unwrap(), i32::MIN);
}

#[test]
fn bounded_int_rejects() {
    for bad in ["", "abc", "1__0", "_1", "1_", "²", "1.5", "-", "+"] {
        assert!(bounded_int(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn infostring_skips_leading_backslash_and_odd_tail() {
    let m = parse_infostring("\\a\\1\\b\\2\\c");
    let v: Vec<(&str, &str)> = m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert_eq!(v, [("a", "1"), ("b", "2")]);
}

#[test]
fn infostring_repeated_key_keeps_first_position_last_value() {
    let m = parse_infostring("\\a\\1\\b\\2\\a\\3");
    let v: Vec<(&str, &str)> = m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert_eq!(v, [("a", "3"), ("b", "2")]);
}

#[test]
fn strip_control_removes_ascii_controls() {
    assert_eq!(strip_control("a\x01b\x7fc\n"), "abc");
}

#[test]
fn is_bot_heuristics() {
    assert!(is_bot(&Player::new("x", 0, 0)));
    assert!(is_bot(&Player::new("[BOT]Foo", 0, 50)));
    assert!(is_bot(&Player::new(" [bot] x", 0, 50)));
    assert!(is_bot(&Player::new("bot12", 0, 999)));
    assert!(!is_bot(&Player::new("bot12", 0, 50)));
    assert!(!is_bot(&Player::new("Jimmy", 0, 50)));
}

#[test]
fn py_split_matches_python() {
    assert_eq!(py_split("a\x1cb\u{a0}c  d"), ["a", "b", "c", "d"]);
}

#[test]
fn latin1_high_bytes() {
    assert_eq!(latin1(&[0x41, 0xe9, 0xff]), "Aéÿ");
    assert_eq!(utf8_or_latin1(&[0xc3, 0xa9]), "é");
    assert_eq!(utf8_or_latin1(&[0xe9]), "é");
}

#[test]
fn casefold_sharp_s() {
    assert_eq!(casefold("Straße"), "strasse");
}

#[test]
fn humans_never_negative() {
    let a = ServerAddress::new("192.0.2.1", 1);
    let mut info = ServerInfo::new(a.clone(), a);
    info.num_players = 2;
    info.bots = 5;
    assert_eq!(info.humans(), 0);
}

#[test]
fn player_eq_ignores_raw_name() {
    let mut p = Player::new("a", 1, 2);
    p.raw_name = "^1a".into();
    assert_eq!(p, Player::new("a", 1, 2));
}

#[test]
fn bounded_int_rejects_more_than_4300_digits() {
    // int() de Python rechaza cadenas de más de 4300 dígitos (sys.int_info).
    assert_eq!(bounded_int(&"9".repeat(4300)).unwrap(), i32::MAX);
    assert!(bounded_int(&"9".repeat(4301)).is_err());
}
