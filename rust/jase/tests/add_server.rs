//! Direcciones en texto libre y el formulario de «Agregar servidor» (como `test_add_server.py`).
mod support;

use iced::widget::text_editor::{Action, Edit};
use jase::add_server::{
    Field, Form, MAX_ADDRESSES, MAX_PASTE_BYTES, find_addresses, has_address, query_candidates,
    split_host_port,
};
use jase_core::games::{GAMES, game};
use jase_core::masters::valid_host;
use jase_core::models::ServerAddress;
use proptest::prelude::*;
use support::lang::with_language;

fn addr(host: &str, port: u16) -> ServerAddress {
    ServerAddress::new(host, port)
}

fn a0() -> ServerAddress {
    addr("9.9.9.9", 28960)
}

fn ids() -> Vec<&'static str> {
    GAMES.iter().map(|g| g.id).collect()
}

fn form(favorite: bool, text: &str) -> Form {
    Form::new(ids(), Some("cod2"), favorite, text)
}

#[test]
fn split_host_port_cases() {
    assert_eq!(
        split_host_port(" 1.2.3.4:28960 "),
        ("1.2.3.4".to_owned(), Some(28960))
    );
    assert_eq!(
        split_host_port("host.example.com"),
        ("host.example.com".to_owned(), None)
    );
    assert_eq!(
        split_host_port("1.2.3.4:99999"),
        ("1.2.3.4:99999".to_owned(), None)
    );
    assert_eq!(split_host_port("1.2.3.4:"), ("1.2.3.4:".to_owned(), None));
}

#[test]
fn query_candidates_adds_gamespy_query_port() {
    let g = |id| game(id).unwrap();
    assert_eq!(query_candidates(g("cod2"), 28960, "h"), [addr("h", 28960)]);
    assert_eq!(
        query_candidates(g("ut99"), 7777, "h"),
        [addr("h", 7777), addr("h", 7778)]
    );
    assert_eq!(
        query_candidates(g("bf1942"), 60000, "h"),
        [addr("h", 60000)]
    );
}

#[test]
fn find_addresses_in_free_text() {
    let text = "connect 1.2.3.4:28960\n\
                otro: 5.6.7.8:28961, repetido 1.2.3.4:28960\n\
                jase://connect/cod2/srv.example.com:28962\n\
                sin punto localhost:1, puerto malo 9.9.9.9:99999, versión 1.2.3:4.5";
    assert_eq!(
        find_addresses(text),
        [
            addr("1.2.3.4", 28960),
            addr("5.6.7.8", 28961),
            addr("srv.example.com", 28962)
        ]
    );
    let many: Vec<String> = (0..MAX_ADDRESSES + 5)
        .map(|i| format!("10.0.{}.{}:1", i / 256, i % 256))
        .collect();
    assert_eq!(find_addresses(&many.join("\n")).len(), MAX_ADDRESSES);
}

#[test]
fn has_address_cases() {
    assert!(has_address("pasa por 1.2.3.4:28960"));
    assert!(has_address("  jase://connect/ut99/1.2.3.4:7777 \n"));
    assert!(!has_address("nada útil aquí"));
}

#[test]
fn find_addresses_caps_text_and_count() {
    let late = format!("{}1.2.3.4:1", " ".repeat(MAX_PASTE_BYTES));
    assert_eq!(find_addresses(&late), []);
    // «é» ocupa dos bytes: el tope cae a mitad de un carácter.
    let odd = format!(" {}", "é".repeat(MAX_PASTE_BYTES));
    assert_eq!(find_addresses(&odd), []);
    let big = "x".repeat(10_000_000);
    assert_eq!(find_addresses(&big), []);
    // El tope cae dentro del puerto: no sale «1.2.3.4:2896».
    let cut = format!("{}1.2.3.4:28960", " ".repeat(MAX_PASTE_BYTES - 12));
    assert_eq!(find_addresses(&cut), []);
    // El tope cae justo tras una dirección completa (en el salto de línea): queda.
    let whole = format!(
        "{}1.2.3.4:28960\n5.6.7.8:1",
        " ".repeat(MAX_PASTE_BYTES - 13)
    );
    assert_eq!(find_addresses(&whole), [addr("1.2.3.4", 28960)]);
}

#[test]
fn find_addresses_edge_hosts() {
    for text in [
        "a..b:1",
        "-x.y:1",
        "x_y.z:1",
        "ñx.y:1",
        "1.2.3.4:28960.5",
        "1.2.3.4:28960:1",
        "1.2.3.4:123456",
        "1.2.3.4:1:5.6.7.8:2",
        ".a.b:1",
        "a.b.:1",
    ] {
        assert_eq!(find_addresses(text), [], "{text}");
    }
    assert_eq!(find_addresses("(1.2.3.4:1)"), [addr("1.2.3.4", 1)]);
    assert_eq!(find_addresses("1.2.3.4:1-"), [addr("1.2.3.4", 1)]);
    assert_eq!(find_addresses("fin a.b:00065"), [addr("a.b", 65)]);
}

proptest! {
    #[test]
    fn find_addresses_never_panics(text in "\\PC{0,200}|[0-9a-z.:_ñé-]{0,200}") {
        let found = find_addresses(&text);
        prop_assert!(found.len() <= MAX_ADDRESSES);
        for a in found {
            prop_assert!(valid_host(&a.host) && a.port >= 1);
        }
    }
}

/// Textos difíciles y lo que devolvía `find_addresses` de Python 1.3.0 (todos bajo
/// `MAX_PASTE_BYTES`).
const CORPUS: &[(&str, &[(&str, u16)])] = &[
    ("", &[]),
    ("connect 1.2.3.4:28960", &[("1.2.3.4", 28960)]),
    (
        "connect 1.2.3.4:28960\notro: 5.6.7.8:28961, repetido 1.2.3.4:28960",
        &[("1.2.3.4", 28960), ("5.6.7.8", 28961)],
    ),
    (
        "jase://connect/cod2/srv.example.com:28962",
        &[("srv.example.com", 28962)],
    ),
    (
        "sin punto localhost:1, puerto malo 9.9.9.9:99999, versión 1.2.3:4.5",
        &[],
    ),
    ("a..b:1", &[]),
    ("-x.y:1", &[]),
    ("x_y.z:1", &[]),
    ("ñx.y:1", &[]),
    ("ñ x.y:1", &[("x.y", 1)]),
    ("1.2.3.4:28960.5", &[]),
    ("1.2.3.4:28960:1", &[]),
    ("1.2.3.4:1:5.6.7.8:2", &[]),
    ("(1.2.3.4:1)", &[("1.2.3.4", 1)]),
    ("1.2.3.4:1-", &[("1.2.3.4", 1)]),
    ("1.2.3.4:1-x.y:2", &[("1.2.3.4", 1)]),
    ("1.2.3.4:123456", &[]),
    (
        "1.2.3.4:65535 1.2.3.4:65536 1.2.3.4:0",
        &[("1.2.3.4", 65535)],
    ),
    ("fin a.b:00065", &[("a.b", 65)]),
    ("a.b:1", &[("a.b", 1)]),
    (".a.b:1", &[]),
    ("a.b.:1", &[]),
    ("a-.b-:7", &[("a-.b-", 7)]),
    (":a.b:1", &[]),
    (".a.b:1 -a.b:2 _a.b:3", &[]),
    ("a.b:1x c.d:2_ e.f:3é g.h:4/", &[("g.h", 4)]),
    ("x.y:1, x.y:1; X.Y:1", &[("x.y", 1), ("X.Y", 1)]),
    (
        "IP 10.0.0.1:27960\tIP 10.0.0.2:27961\r\n10.0.0.3:27962",
        &[
            ("10.0.0.1", 27960),
            ("10.0.0.2", 27961),
            ("10.0.0.3", 27962),
        ],
    ),
    (
        "«1.2.3.4:28960» “5.6.7.8:1”",
        &[("1.2.3.4", 28960), ("5.6.7.8", 1)],
    ),
    ("日本1.2.3.4:5 日本 1.2.3.4:6", &[("1.2.3.4", 6)]),
    ("1.2.3.4:5日本", &[]),
    ("1.2.3.4:٢٨", &[("1.2.3.4", 28)]),
    ("1.2.3.4:28960٢", &[]),
    ("a.b:12.c.d:5", &[]),
    (
        "host.example.com:27015/path",
        &[("host.example.com", 27015)],
    ),
    (
        "user@host.example.com:27015",
        &[("host.example.com", 27015)],
    ),
    ("http://1.2.3.4:8080/x", &[("1.2.3.4", 8080)]),
    ("[1.2.3.4]:28960 1.2.3.4]:28961", &[]),
    (
        "a.b.c.d.e.f.g:9 1.1:1 1.1.1:1",
        &[("a.b.c.d.e.f.g", 9), ("1.1", 1), ("1.1.1", 1)],
    ),
    ("x.y:1\nx.y:2\nx.y:1\n", &[("x.y", 1), ("x.y", 2)]),
    ("ा1.2.3.4:5", &[("1.2.3.4", 5)]),
    ("1.2.3.4:5Ⓐ", &[("1.2.3.4", 5)]),
];

#[test]
fn find_addresses_known_answers() {
    // Divergencias anotadas, donde Rust solo rechaza de más: «٢٨» como puerto (el `\d` de
    // Python acepta dígitos no ASCII) y las marcas combinantes o letras en círculo junto a la
    // dirección (`is_alphanumeric` las cuenta como `\w`; `isalnum`, no).
    let skip = ["1.2.3.4:٢٨", "ा1.2.3.4:5", "1.2.3.4:5Ⓐ"];
    for &(text, want) in CORPUS {
        let got: Vec<(String, u16)> = find_addresses(text)
            .into_iter()
            .map(|a| (a.host, a.port))
            .collect();
        if skip.contains(&text) {
            assert_eq!(got, [], "{text:?}");
            continue;
        }
        let want: Vec<(String, u16)> = want.iter().map(|&(h, p)| (h.to_owned(), p)).collect();
        assert_eq!(got, want, "{text:?}");
    }
}

#[test]
fn dialog_paste_moves_port() {
    let mut f = form(false, "");
    assert!(!f.ok_enabled());
    f.host_changed("9.9.9.9:28960".into()); // un pegado: entra todo de golpe
    assert_eq!((f.host(), f.port()), ("9.9.9.9", "28960"));
    assert!(f.ok_enabled());
    assert_eq!(f.result_address(), Some(a0()));
}

#[test]
fn typing_splits_on_submit() {
    let mut f = form(false, "");
    let mut typed = String::new();
    for c in "9.9.9.9:28960".chars() {
        typed.push(c);
        f.host_changed(typed.clone());
    }
    assert_eq!(f.host(), "9.9.9.9:28960"); // tecla a tecla no se corta
    assert_eq!(f.result_address(), Some(a0())); // pero ya vale
    f.host_submitted();
    assert_eq!((f.host(), f.port()), ("9.9.9.9", "28960"));
}

#[test]
fn port_typed_separately() {
    let mut f = form(true, "");
    assert!(f.favorite());
    let mut typed = String::new();
    for c in "9.9.9.9".chars() {
        typed.push(c);
        f.host_changed(typed.clone());
    }
    assert_eq!(f.focus(), Field::Host);
    let mut typed = String::new();
    for c in "28960".chars() {
        typed.push(c);
        f.port_changed(typed.clone());
    }
    assert_eq!(f.focus(), Field::Port);
    assert_eq!(f.result_address(), Some(a0()));
    f.port_changed("12a".into());
    f.port_changed("123456".into());
    assert_eq!(f.port(), "28960");
    f.host_changed("bad host;".into());
    assert_eq!(f.result_address(), None);
}

#[test]
fn paste_jase_link_selects_game() {
    let mut f = form(false, "");
    f.host_changed("jase://connect/ut99/1.2.3.4:7777".into());
    assert_eq!(f.game(), "ut99");
    assert_eq!(f.result_address(), Some(addr("1.2.3.4", 7777)));
}

#[test]
fn initial_text_single() {
    let f = form(false, "connect 9.9.9.9:28960");
    assert!(!f.is_multi());
    assert_eq!((f.host(), f.port()), ("9.9.9.9", "28960"));
    assert_eq!(f.game(), "cod2");
    assert_eq!(f.focus(), Field::Host);
}

#[test]
fn initial_text_several_goes_to_list() {
    with_language("es", || {
        let f = form(true, "9.9.9.9:28960\n8.8.8.8:28961");
        assert!(f.is_multi());
        assert_eq!(f.addresses(), [a0(), addr("8.8.8.8", 28961)]);
        assert_eq!(f.ok_label(), "Agregar 2 servidores");
        assert!(f.ok_enabled());
        assert!(!f.can_detect()); // «Detectar» es para una sola dirección
    });
}

#[test]
fn paste_list_into_ip_switches_mode() {
    let mut f = form(false, "");
    f.host_changed("9.9.9.9:28960\n8.8.8.8:28961".into());
    assert!(f.is_multi());
    assert_eq!(f.addresses().len(), 2);
    assert_eq!(f.host(), "");
}

#[test]
fn list_edits_update_addresses() {
    let mut f = form(false, "");
    f.set_multi(true);
    assert_eq!(f.addresses(), []);
    for c in "9.9.9.9:28960".chars() {
        f.list_action(Action::Edit(Edit::Insert(c)));
    }
    assert_eq!(f.focus(), Field::List);
    assert_eq!(f.list_text(), "9.9.9.9:28960");
    assert_eq!(f.addresses(), [a0()]);
}

#[test]
fn multi_toggle_keeps_typed_address() {
    with_language("es", || {
        let mut f = form(false, "9.9.9.9:28960");
        assert_eq!(f.ok_label(), "Agregar");
        f.set_multi(true);
        assert!(f.is_multi());
        assert_eq!(f.addresses(), [a0()]);
        assert_eq!(f.ok_label(), "Agregar 1 servidor");
        f.set_multi(false);
        assert!(!f.is_multi());
        assert_eq!(f.addresses(), [a0()]);
        assert_eq!(f.ok_label(), "Agregar");
    });
}

#[test]
fn detect_picks_game_and_query_port() {
    with_language("es", || {
        let mut f = form(false, "9.9.9.9:7777");
        let address = f.result_address().unwrap();
        assert_eq!(address, addr("9.9.9.9", 7777));
        assert!(f.can_detect());
        f.start_detect(&address);
        assert!(!f.can_detect());
        assert_eq!(
            f.detect_label(),
            Some("Detectando el juego de 9.9.9.9:7777…")
        );
        f.on_detected(&address, &[("ut99", addr("9.9.9.9", 7778))]);
        assert_eq!((f.game(), f.port()), ("ut99", "7778"));
        assert_eq!(f.detect_label(), Some("Detectado: Unreal Tournament"));
        assert!(f.can_detect());
    });
}

#[test]
fn detect_ambiguous_and_none() {
    with_language("es", || {
        let mut f = form(false, "9.9.9.9:28960");
        let address = f.result_address().unwrap();
        f.start_detect(&address);
        f.on_detected(&address, &[("cs16", a0()), ("css", a0())]);
        assert_eq!(f.game(), "cs16");
        assert!(f.detect_label().unwrap().starts_with("Puede ser: "));
        f.start_detect(&address);
        f.on_detected(&address, &[]);
        assert_eq!(f.game(), "cs16");
        assert_eq!(
            f.detect_label(),
            Some("9.9.9.9:28960 no respondió como ningún juego conocido")
        );
    });
}

#[test]
fn detect_ignores_games_not_offered() {
    let mut f = Form::new(vec!["cod2", "ut99"], Some("cod2"), false, "9.9.9.9:7777");
    let address = f.result_address().unwrap();
    f.on_detected(
        &address,
        &[("cs16", address.clone()), ("ut99", addr("9.9.9.9", 7778))],
    );
    assert_eq!((f.game(), f.port()), ("ut99", "7778"));
}

#[test]
fn huge_paste_into_ip_keeps_list_small() {
    let text = format!("1.1.1.1:1 2.2.2.2:2\n{}", "x".repeat(10_000_000));
    let f = form(false, &text);
    assert!(f.is_multi());
    assert!(f.list_text().len() <= MAX_PASTE_BYTES);
    assert_eq!(f.addresses(), [addr("1.1.1.1", 1), addr("2.2.2.2", 2)]);
}

#[test]
fn huge_paste_into_list_is_cut() {
    let mut f = form(false, "");
    f.set_multi(true);
    f.list_action(Action::Edit(Edit::Paste("1.1.1.1:1\n".to_owned().into())));
    let big = format!("2.2.2.2:2\n{}", "é".repeat(MAX_PASTE_BYTES));
    f.list_action(Action::Edit(Edit::Paste(big.into())));
    assert!(f.list_text().len() <= MAX_PASTE_BYTES);
    assert_eq!(f.addresses(), [addr("1.1.1.1", 1), addr("2.2.2.2", 2)]);
}

#[test]
fn huge_paste_into_ip_is_cut_like_qlineedit() {
    let mut f = form(false, "");
    f.host_changed("é".repeat(100_000));
    assert_eq!(f.host().chars().count(), 32_767);
    assert!(!f.is_multi());
}
