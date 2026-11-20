//! Enlaces `jase://`. Traducción de tests/test_jase_url.py.
use jase_core::jase_url::{build, is_link, parse};
use jase_core::models::ServerAddress;
use proptest::prelude::*;

#[test]
fn build_and_roundtrip() {
    let a = ServerAddress::new("1.2.3.4", 28960);
    let url = build("cod2", &a);
    assert_eq!(url, "jase://connect/cod2/1.2.3.4:28960");
    assert_eq!(parse(&url), Some(("cod2", a)));
}

#[test]
fn valid() {
    for (url, host, port) in [
        ("jase://connect/cod2/1.2.3.4:28960/", "1.2.3.4", 28960),
        (
            "JASE://connect/cod2/play.example.com:1",
            "play.example.com",
            1,
        ),
        ("jase://connect/cod2/a-b.c:65535", "a-b.c", 65535),
    ] {
        assert_eq!(
            parse(url),
            Some(("cod2", ServerAddress::new(host, port))),
            "{url}"
        );
    }
}

#[test]
fn rejected() {
    let long = format!("jase://connect/cod2/{}:1", "a".repeat(300));
    let cases = [
        "jase://connect/cod2/;rm -rf:1",
        "jase://connect/cod2/1.2.3.4;ls:1",
        "jase://connect/nogame/1.2.3.4:1",
        "jase://connect/cod2/1.2.3.4:0",
        "jase://connect/cod2/1.2.3.4:70000",
        "jase://connect/cod2/1.2.3.4:-1",
        "jase://connect/cod2/1.2.3.4:",
        "jase://connect/cod2/1.2.3.4",
        &long,
        "http://connect/cod2/1.2.3.4:1",
        "jase:connect/cod2/1.2.3.4:1",
        "jase://launch/cod2/1.2.3.4:1",
        "jase://connect/cod2/1.2.3.4:1/extra",
        "jase://connect/cod2/extra/1.2.3.4:1",
        "jase://connect/1.2.3.4:1",
        "jase://user@1.2.3.4/cod2/x:1",
        "jase://connect/cod2/user@1.2.3.4:1",
        "jase://connect/cod2/1.2.3.4:1?x=1",
        "jase://connect/cod2/1.2.3.4:1#frag",
        "jase://connect/cod2/:28960",
        "jase://connect/cod2/-.:1",
        "jase://connect/cod2/ho st:1",
        "jase://connect/cod2/hóst:1",
        "jase://connect/cod2/1.2.3.4:١",
        "jase://connect/cod2/1.2.3.4:1\n",
        "jase://connect/cod2/1.2.3.4:1\0",
        "jase://connect/cod2/[::1]:1",
        "",
    ];
    assert_eq!(cases.len(), 28);
    for url in cases {
        assert_eq!(parse(url), None, "{url:?}");
    }
}

#[test]
fn is_link_multibyte_does_not_panic() {
    for text in ["jasé://x", "€€€€€€€€", "ja", ""] {
        assert!(!is_link(text), "{text:?}");
    }
    assert!(is_link("JASE://x"));
}

proptest! {
    #[test]
    fn parse_never_panics(text in any::<String>()) {
        let _ = parse(&text);
    }
}
