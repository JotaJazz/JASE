//! «Detectar juego»: cada captura real de tests/fixtures se reconoce como su juego.
//! Traducción de tests/test_identify.py y de las partes de identify de
//! test_games_tacops.py y test_zandronum.py.
mod support;
use indexmap::IndexMap;
use jase_core::games::{Family, GAMES, game};
use jase_core::identify::*;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::protocols::gamespy::{self, GamespyOptions, StatusAssembler};
use jase_core::protocols::{quake2, quake3, ut2004, valve, zandronum};
use proptest::prelude::*;
use support::fixture;

fn addr() -> ServerAddress {
    ServerAddress::new("1.2.3.4", 1)
}

fn gs(name: &str, opts: &GamespyOptions) -> ServerInfo {
    let mut asm = StatusAssembler::new();
    assert!(asm.feed(&fixture(name)).unwrap());
    gamespy::parse_status(&asm.fields(), &addr(), 1, opts).unwrap()
}

fn q3(name: &str) -> ServerInfo {
    quake3::parse_status_response(&fixture(name), &addr(), 1, &Default::default()).unwrap()
}

fn info(rules: &[(&str, &str)]) -> ServerInfo {
    let mut info = ServerInfo::new(addr(), addr());
    info.rules = rules
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
    info
}

#[test]
fn quake3_fixtures() {
    for (name, expected) in [
        ("cod2x_status.bin", "cod2"),
        ("cod4_status_17.bin", "cod4"),
        ("cod4_status_cod4x.bin", "cod4"),
        ("coduo_status.bin", "coduo"),
        ("et_status.bin", "et"),
        ("jk2_status.bin", "jk2"),
        ("jka_status.bin", "jka"),
        ("oa_status.bin", "oa"),
        ("q3_status.bin", "q3"),
        ("rtcw_status.bin", "rtcw"),
        ("smokinguns_status.bin", "smokinguns"),
        ("sof2_status_2002.bin", "sof2"),
        ("sof2_status_2004.bin", "sof2"),
        ("tremulous_status.bin", "tremulous"),
        ("urt_status.bin", "urt"),
        ("warfork_status.bin", "warfork"),
        ("warsow_status.bin", "warsow"),
        ("xonotic_status.bin", "xonotic"),
    ] {
        assert_eq!(identify(Family::Quake3, &q3(name)), [expected], "{name}");
    }
}

#[test]
fn gamespy_fixtures() {
    for (name, expected) in [
        ("bf1942_status.bin", "bf1942"),
        ("mohaa_status.bin", "mohaa"),
        ("mohaas_status.bin", "mohaas"),
        ("mohaab_status.bin", "mohaab"),
    ] {
        let got = identify(Family::Gamespy, &gs(name, &Default::default()));
        assert_eq!(got, [expected], "{name}");
    }
}

#[test]
fn cod1_needs_exact_gamename() {
    let mut i = info(&[("gamename", "Call of Duty"), ("protocol", "6")]);
    assert_eq!(identify(Family::Quake3, &i), ["cod1"]);
    i.rules.insert("gamename".into(), "main".into()); // sin pista: los dos del protocolo 6
    assert_eq!(identify(Family::Quake3, &i), ["cod1", "cod4"]);
}

#[test]
fn valve_appid_or_gamedir() {
    let parse = |name: &str| valve::parse_info(&fixture(name), &addr(), 1, "").unwrap();
    assert_eq!(identify(Family::Valve, &parse("cs16_info.bin")), ["cs16"]);
    // GoldSrc antiguo sin appid: «cstrike» puede ser CS 1.6 o CS:S.
    let goldsrc = parse("cs16_info_goldsrc.bin");
    assert_eq!(identify(Family::Valve, &goldsrc), ["cs16", "css"]);
}

#[test]
fn quake2_and_ut2004() {
    let q2 =
        |name: &str| quake2::parse_status(&fixture(name), &addr(), 1, &Default::default()).unwrap();
    assert_eq!(identify(Family::Quake2, &q2("q2_status.bin")), ["q2"]);
    assert_eq!(identify(Family::Quake2, &q2("dday_status.bin")), ["dday"]);
    let ut = ut2004::parse_info(&fixture("ut2004_info.bin"), &addr(), 1).unwrap();
    assert_eq!(identify(Family::Ut2004, &ut), ["ut2004"]);
}

#[test]
fn probe_plan_tries_query_ports() {
    let ports = |port: u16, family: Family| -> Vec<u16> {
        let plan = probe_plan(&ServerAddress::new("h", port));
        let (_, addrs) = plan.into_iter().find(|(f, _)| *f == family).unwrap();
        assert!(addrs.iter().all(|a| a.host == "h"));
        addrs.iter().map(|a| a.port).collect()
    };
    let plan = probe_plan(&ServerAddress::new("h", 7777));
    let families: Vec<Family> = plan.iter().map(|(f, _)| *f).collect();
    assert_eq!(families, Family::ALL);
    assert_eq!(ports(7777, Family::Quake3), [7777]);
    assert_eq!(ports(7777, Family::Gamespy), [7777, 7778, 7874, 16210]);
    assert_eq!(ports(7777, Family::Ut2004), [7777, 7778]);
    assert_eq!(ports(65535, Family::Gamespy), [65535]);
    assert_eq!(ports(65535, Family::Ut2004), [65535]);
}

#[test]
fn detected_games_dedupes_and_keeps_first_address() {
    let (a, b) = (ServerAddress::new("h", 1), ServerAddress::new("h", 2));
    let mohaa = gs("mohaa_status.bin", &Default::default());
    let mut first = mohaa.clone();
    first.address = a.clone();
    let mut second = mohaa;
    second.address = b;
    let responses = [(Family::Gamespy, first), (Family::Gamespy, second)];
    assert_eq!(detected_games(&responses), [("mohaa", a)]);
}

#[test]
fn iortcw_without_protocol() {
    // Visto en el sondeo real: iortcw sin `protocol` y con el mod en `gamename`.
    let i = info(&[
        ("gamename", "omnibot"),
        ("version", "iortcw 1.51c-MP linux-x86_64 Mar 16 2019"),
    ]);
    assert_eq!(identify(Family::Quake3, &i), ["rtcw"]);
}

#[test]
fn warsow_answering_a2s() {
    // Warsow 2.1 contesta también por A2S de Valve, con appid 0 y carpeta basewsw.
    let i = info(&[("appid", "0"), ("gamedir", "basewsw"), ("protocol", "22")]);
    assert_eq!(identify(Family::Valve, &i), ["warsow"]);
}

#[test]
fn identify_splits_the_shared_gamename() {
    let to = game("tacops").unwrap();
    let aliases = to.version_aliases();
    let opts = GamespyOptions {
        connect_port_fallback: to.connect_port_fallback,
        version_key: to.version_key,
        version_aliases: &aliases,
        ..Default::default()
    };
    assert_eq!(
        identify(Family::Gamespy, &gs("to350_status.bin", &opts)),
        ["tacops"]
    );
    let ut = info(&[("gametype", "CTFGame"), ("gamename", "ut")]);
    assert_eq!(identify(Family::Gamespy, &ut), ["ut99"]);
}

#[test]
fn zandronum_is_identified() {
    let i = zandronum::parse_response(&fixture("zandronum_team.bin"), &addr(), 30).unwrap();
    assert_eq!(identify(Family::Zandronum, &i), ["zandronum"]);
    assert!(Family::ALL.contains(&Family::Zandronum));
}

#[test]
fn zandronum_is_probed_first() {
    assert_eq!(FIRST_FAMILIES, [Family::Zandronum]);
    assert_eq!(probe_delay(Family::Zandronum), std::time::Duration::ZERO);
    assert_eq!(probe_delay(Family::Quake3), STAGGER);
    assert!(QUAKE3_HINTS.iter().all(|(id, _)| game(id).is_some()));
    assert_eq!(QUAKE3_HINTS.len(), 14);
    assert!(FOLDER_HINTS.iter().all(|(_, id)| game(id).is_some()));
    assert!(GAMES.iter().any(|g| g.family == Family::Gamespy));
}

proptest! {
    #[test]
    fn identify_never_panics(
        rules in prop::collection::vec(
            (
                prop_oneof![
                    Just("gamename".to_string()), Just("GameName".to_string()),
                    Just("version".to_string()), Just("protocol".to_string()),
                    Just("appid".to_string()), Just("gamedir".to_string()),
                    Just("game".to_string()), Just("gametype".to_string()),
                    Just("tostver".to_string()), ".*",
                ],
                prop_oneof![Just(String::new()), Just("0".to_string()), "\\PC*"],
            ),
            0..8,
        )
    ) {
        let mut i = ServerInfo::new(addr(), addr());
        i.rules = rules.into_iter().collect::<IndexMap<_, _>>();
        for family in Family::ALL {
            let _ = identify(family, &i);
            let _ = detected_games(&[(family, i.clone())]);
        }
    }
}
