//! Port de tests/test_games_launcher.py y de los tests de lanzamiento de test_wine.py,
//! test_games_steam.py, test_games_mohaa.py, test_zandronum.py, test_games_tacops.py,
//! test_games_cs16.py, test_games_v012.py, test_games_v013.py, test_games_generic.py,
//! test_games_bf1942.py, test_ut2004.py y test_valve.py; más el lanzamiento real sin shell.
mod support;

use jase_core::games::{GameDef, LaunchStyle, game};
use jase_core::launcher::{
    LaunchCommand, LaunchError, build_launch_command, is_mod_name, is_wad_name, launch,
    shlex_split, spawn_command, steam_url, wine_wrapper,
};
use jase_core::models::ServerAddress;
use proptest::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Ruta relativa armada parte por parte: en Windows `Path::join("a/b")` deja la `/`.
fn rel(p: &str) -> PathBuf {
    p.split('/').collect()
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

fn touch(p: &Path) -> String {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, b"").unwrap();
    s(p)
}

fn g(id: &str) -> &'static GameDef {
    game(id).unwrap()
}

fn a() -> ServerAddress {
    ServerAddress::new("1.2.3.4", 28960)
}

fn cmd(game_id: &str, exe: &str, addr: &ServerAddress, pw: Option<&str>) -> (Vec<String>, String) {
    build_launch_command(g(game_id), exe, addr, pw, "")
}

fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|x| (*x).to_owned()).collect()
}

/// Lanza con un `spawn` falso que guarda cada `LaunchCommand` (el `popen` falso de Python).
fn run(
    game: &GameDef,
    dir: Option<&str>,
    addr: &ServerAddress,
    pw: Option<&str>,
    mod_name: &str,
    version: &str,
    wrapper: Option<&[String]>,
) -> (Result<(), LaunchError>, Vec<LaunchCommand>) {
    let mut calls = Vec::new();
    let r = launch(
        game,
        dir,
        addr,
        pw,
        mod_name,
        version,
        wrapper,
        &mut |c| {
            calls.push(c.clone());
            Ok(())
        },
        &mut |_| panic!("no debe abrir una URL"),
    );
    (r, calls)
}

fn ok(
    game: &GameDef,
    dir: &str,
    addr: &ServerAddress,
    pw: Option<&str>,
    mod_name: &str,
) -> LaunchCommand {
    let (r, mut calls) = run(game, Some(dir), addr, pw, mod_name, "", None);
    r.unwrap();
    assert_eq!(calls.len(), 1);
    calls.remove(0)
}

fn err_text(r: Result<(), LaunchError>) -> String {
    r.expect_err("debía fallar").to_string()
}

/// Abre la URL de Steam con un abridor falso que la guarda.
fn steam(id: &str, dir: Option<&str>, addr: &ServerAddress, pw: Option<&str>) -> Vec<String> {
    let mut opened = Vec::new();
    launch(
        g(id),
        dir,
        addr,
        pw,
        "",
        "",
        None,
        &mut |_| panic!("Steam no lanza un exe"),
        &mut |u| {
            opened.push(u.to_owned());
            Ok(())
        },
    )
    .unwrap();
    opened
}

// --- test_games_launcher.py ---

#[test]
fn cod2_command_with_password() {
    let (argv, cwd) = cmd(
        "cod2",
        r"D:\Juegos\CoD2\CoD2MP_s.exe",
        &a(),
        Some("secreto"),
    );
    assert_eq!(
        argv,
        v(&[
            r"D:\Juegos\CoD2\CoD2MP_s.exe",
            "+password",
            "secreto",
            "+connect",
            "1.2.3.4:28960"
        ])
    );
    assert_eq!(cwd, r"D:\Juegos\CoD2");
}

#[test]
fn cod2_command_without_password() {
    let (argv, _) = cmd("cod2", r"D:\Juegos\CoD2\CoD2MP_s.exe", &a(), None);
    assert_eq!(
        argv,
        v(&[r"D:\Juegos\CoD2\CoD2MP_s.exe", "+connect", "1.2.3.4:28960"])
    );
}

#[test]
fn ut99_command() {
    let exe = r"D:\Juegos\UnrealTournament\System\UnrealTournament.exe";
    let (argv, cwd) = cmd(
        "ut99",
        exe,
        &ServerAddress::new("5.6.7.8", 7777),
        Some("pw"),
    );
    assert_eq!(argv, v(&[exe, "5.6.7.8:7777?password=pw"]));
    assert_eq!(cwd, r"D:\Juegos\UnrealTournament\System");
}

#[test]
fn launch_without_path_raises() {
    let (r, calls) = run(g("cod1"), None, &a(), None, "", "", None);
    assert!(err_text(r).contains("ruta"));
    assert!(calls.is_empty());
    let (r, _) = run(g("cod1"), Some(""), &a(), None, "", "", None);
    assert!(err_text(r).contains("ruta"));
}

#[test]
fn launch_missing_exe_raises() {
    let t = tempfile::tempdir().unwrap();
    let (r, calls) = run(g("cod2"), Some(&s(t.path())), &a(), None, "", "", None);
    assert!(err_text(r).contains("No se encontró"));
    assert!(calls.is_empty());
}

#[test]
fn launch_calls_popen() {
    let t = tempfile::tempdir().unwrap();
    let exe = touch(&t.path().join("CoD2MP_s.exe"));
    let c = ok(g("cod2"), &s(t.path()), &a(), None, "");
    assert_eq!(c.argv, v(&[&exe, "+connect", "1.2.3.4:28960"]));
    assert_eq!(c.cwd, s(t.path()));
    assert!(c.env.is_empty());
}

#[test]
fn launch_forwards_mod_for_bf1942() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("BF1942.exe"));
    let c = ok(g("bf1942"), &s(t.path()), &a(), None, "xpack1");
    assert_eq!(
        c.argv[1..],
        v(&[
            "+restart",
            "1",
            "+game",
            "xpack1",
            "+joinServer",
            "1.2.3.4:28960"
        ])
    );
}

#[test]
fn launch_uses_resolved_nested_exe() {
    let t = tempfile::tempdir().unwrap();
    let exe_path = t
        .path()
        .join(rel("UnrealTournament/System/UnrealTournament.exe"));
    let exe = touch(&exe_path);
    let c = ok(
        g("ut99"),
        &s(t.path()),
        &ServerAddress::new("5.6.7.8", 7777),
        None,
        "",
    );
    assert_eq!(c.argv[0], exe);
    assert_eq!(c.cwd, s(exe_path.parent().unwrap()));
}

#[test]
fn cwd_drive_root() {
    assert_eq!(cmd("cod2", "C:\\CoD2MP_s.exe", &a(), None).1, "C:\\");
}

#[test]
fn cwd_unc() {
    assert_eq!(
        cmd("cod2", "\\\\srv\\share\\CoD2MP_s.exe", &a(), None).1,
        "\\\\srv\\share\\"
    );
}

#[test]
fn cwd_posix_style() {
    let t = tempfile::tempdir().unwrap();
    let p = s(&t.path().join("CoD2MP_s.exe"));
    assert_eq!(cmd("cod2", &p, &a(), None).1, s(t.path()));
    assert_eq!(
        cmd("cod2", "/juegos/cod2/x.exe", &a(), None).1,
        "/juegos/cod2"
    );
}

fn steam_jka(root: &Path) -> PathBuf {
    let install = root.join(rel("steamapps/common/Jedi Academy"));
    touch(&install.join(rel("GameData/jamp.exe")));
    install
}

#[test]
fn steam_copy_of_jka_gets_steam_app_id() {
    // El exe de Steam abierto a mano da «Application load error 5:0000065434».
    let t = tempfile::tempdir().unwrap();
    let c = ok(g("jka"), &s(&steam_jka(t.path())), &a(), None, "");
    assert_eq!(
        c.env,
        vec![
            ("SteamAppId", "6020".to_owned()),
            ("SteamGameId", "6020".to_owned())
        ]
    );
}

#[test]
fn non_steam_copy_keeps_plain_popen() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join(rel("GameData/jk2mp.exe")));
    let c = ok(g("jk2"), &s(t.path()), &a(), None, "");
    assert!(c.env.is_empty());
    assert_eq!(g("jk2").steam_appid, 6030);
}

#[test]
fn jka_prefers_openjk_client() {
    let t = tempfile::tempdir().unwrap();
    let install = steam_jka(t.path());
    let openjk = touch(&install.join(rel("GameData/openjk.x86.exe")));
    let c = ok(g("jka"), &s(&install), &a(), None, "");
    assert_eq!(c.argv[0], openjk);
}

#[test]
fn bf1942_ignores_unsafe_mod() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("BF1942.exe"));
    let long = "x".repeat(65);
    for m in ["+restart", "../../x", "a/b", "a\\b", long.as_str()] {
        let c = ok(g("bf1942"), &s(t.path()), &a(), None, m);
        assert!(!c.argv.iter().any(|x| x == "+game"), "{m}");
    }
}

#[test]
fn is_mod_name_accepts_real_mods() {
    for m in ["xpack1", "Desert Combat", "dc_final", "GC-1.5"] {
        assert!(is_mod_name(m), "{m}");
    }
    assert!(is_mod_name(&"x".repeat(64)));
    assert!(!is_mod_name(""));
    assert!(!is_mod_name(".hidden"));
}

// --- test_wine.py ---

#[test]
fn wine_wrapper_per_platform() {
    assert_eq!(wine_wrapper("wine", "linux"), Some(v(&["wine"])));
    assert_eq!(
        wine_wrapper("flatpak run \"org.winehq.Wine\"", "linux"),
        Some(v(&["flatpak", "run", "org.winehq.Wine"]))
    );
    assert_eq!(wine_wrapper("wine", "win32"), None);
    assert_eq!(wine_wrapper("wine", ""), None);
    assert_eq!(wine_wrapper("  ", "linux"), None);
    assert_eq!(wine_wrapper("\"sin cerrar", "linux"), None);
}

#[test]
fn launch_prefixes_exe_with_wrapper() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("CoD2MP_s.exe"));
    let wine = v(&["wine"]);
    let (r, calls) = run(
        g("cod2"),
        Some(&s(t.path())),
        &a(),
        None,
        "",
        "",
        Some(&wine),
    );
    r.unwrap();
    let argv = &calls[0].argv;
    assert_eq!(argv[0], "wine");
    assert!(argv[1].ends_with("CoD2MP_s.exe"));
    assert_eq!(argv[argv.len() - 2..], v(&["+connect", "1.2.3.4:28960"]));
}

#[test]
fn wrapper_only_for_exe_files() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("CoD2MP_s.EXE"));
    let wine = v(&["wine"]);
    let c = |dir: &Path| {
        let (r, calls) = run(g("cod2"), Some(&s(dir)), &a(), None, "", "", Some(&wine));
        r.unwrap();
        calls[0].argv[0].clone()
    };
    assert_eq!(c(t.path()), "wine");
    // Un nativo (sin .exe) no pasa por Wine.
    let n = tempfile::tempdir().unwrap();
    touch(&n.path().join("ioquake3"));
    let mut calls = Vec::new();
    let q3 = GameDef {
        exe_relpath: "ioquake3",
        exe_candidates: &[],
        preferred_exes: &[],
        ..*g("q3")
    };
    launch(
        &q3,
        Some(&s(n.path())),
        &a(),
        None,
        "",
        "",
        Some(&wine),
        &mut |c| {
            calls.push(c.argv.clone());
            Ok(())
        },
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(calls[0][0].ends_with("ioquake3"));
}

#[test]
fn missing_wine_gives_a_clear_error() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("CoD2MP_s.exe"));
    let umu = v(&["umu-run"]);
    let not_found = |c: &LaunchCommand| -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::NotFound, c.argv[0].clone()))
    };
    let r = launch(
        g("cod2"),
        Some(&s(t.path())),
        &a(),
        None,
        "",
        "",
        Some(&umu),
        &mut |c| not_found(c),
        &mut |_| Ok(()),
    );
    let e = r.unwrap_err();
    assert!(matches!(e, LaunchError::Message(_)));
    assert!(e.to_string().contains("umu-run"));
    // Sin wrapper, el NotFound queda como error del sistema.
    let r = launch(
        g("cod2"),
        Some(&s(t.path())),
        &a(),
        None,
        "",
        "",
        None,
        &mut |c| not_found(c),
        &mut |_| Ok(()),
    );
    assert!(matches!(r, Err(LaunchError::Spawn(e)) if e.kind() == io::ErrorKind::NotFound));
}

// --- test_games_steam.py ---

#[test]
fn source_games_connect_through_steam() {
    for id in ["l4d", "l4d2", "css", "tf2"] {
        assert_eq!(g(id).launch_style, LaunchStyle::Steam, "{id}");
    }
}

#[test]
fn goldsrc_steam_games_launch_with_hl_exe() {
    let addr = ServerAddress::new("1.2.3.4", 27015);
    for id in ["hldm", "dod", "tfc"] {
        let game = g(id);
        assert_eq!(game.launch_style, LaunchStyle::Valve);
        assert_eq!(game.exe_relpath, "hl.exe");
        let (argv, _) = cmd(id, r"C:\HL\hl.exe", &addr, None);
        assert_eq!(
            argv,
            v(&[
                r"C:\HL\hl.exe",
                "-game",
                game.valve_gamedir,
                "+connect",
                "1.2.3.4:27015"
            ])
        );
    }
}

#[test]
fn steam_url_with_and_without_password() {
    let addr = ServerAddress::new("1.2.3.4", 27015);
    assert_eq!(steam_url(&addr, None), "steam://connect/1.2.3.4:27015");
    assert_eq!(
        steam_url(&addr, Some("pw")),
        "steam://connect/1.2.3.4:27015/pw"
    );
    assert_eq!(steam_url(&addr, Some("")), "steam://connect/1.2.3.4:27015");
}

#[test]
fn steam_launch_needs_no_install_dir() {
    let addr = ServerAddress::new("1.2.3.4", 27015);
    assert_eq!(
        steam("l4d2", None, &addr, None),
        v(&["steam://connect/1.2.3.4:27015"])
    );
}

#[test]
fn steam_launch_oserror_becomes_launch_error_without_url() {
    let addr = ServerAddress::new("1.2.3.4", 27015);
    let r = launch(
        g("tf2"),
        None,
        &addr,
        Some("secretpw"),
        "",
        "",
        None,
        &mut |_| Ok(()),
        &mut |u| Err(io::Error::other(format!("no handler for {u}"))),
    );
    let e = r.unwrap_err();
    assert!(matches!(e, LaunchError::Message(_)));
    let msg = e.to_string();
    let dbg = format!("{e:?}");
    assert!(msg.contains("Steam"));
    for text in [&msg, &dbg] {
        assert!(
            !text.contains("secretpw") && !text.contains("steam://"),
            "{text}"
        );
    }
}

#[test]
fn steam_launch_with_password() {
    let addr = ServerAddress::new("1.2.3.4", 27015);
    assert_eq!(
        steam("css", Some(""), &addr, Some("pw")),
        v(&["steam://connect/1.2.3.4:27015/pw"])
    );
}

// --- test_games_mohaa.py ---

#[test]
fn launch_command() {
    let (argv, cwd) = cmd(
        "mohaa",
        r"D:\Juegos\MOHAA_STOCK\MOHAA.exe",
        &ServerAddress::new("1.2.3.4", 12203),
        Some("pw"),
    );
    assert_eq!(
        argv,
        v(&[
            r"D:\Juegos\MOHAA_STOCK\MOHAA.exe",
            "+password",
            "pw",
            "+connect",
            "1.2.3.4:12203"
        ])
    );
    assert_eq!(cwd, r"D:\Juegos\MOHAA_STOCK");
}

#[test]
fn launch_uses_openmohaa_launcher_when_colocated() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("MOHAA.exe"));
    let launcher = touch(&t.path().join("launch_openmohaa_base.exe"));
    let c = ok(
        g("mohaa"),
        &s(t.path()),
        &ServerAddress::new("1.2.3.4", 12203),
        None,
        "",
    );
    assert_eq!(c.argv, v(&[&launcher, "+connect", "1.2.3.4:12203"]));
    assert_eq!(c.cwd, s(t.path()));
}

#[test]
fn openmohaa_only_install_uses_launcher() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("openmohaa.exe"));
    let launcher = touch(&t.path().join("launch_openmohaa_spearhead.exe"));
    let c = ok(
        g("mohaas"),
        &s(t.path()),
        &ServerAddress::new("1.2.3.4", 12203),
        None,
        "",
    );
    assert_eq!(c.argv, v(&[&launcher, "+connect", "1.2.3.4:12203"]));
    assert_eq!(c.cwd, s(t.path()));
}

// --- test_zandronum.py ---

#[test]
fn gamedef_launch() {
    let game = g("zandronum");
    let addr = ServerAddress::new("1.2.3.4", 10666);
    let (argv, cwd) = build_launch_command(
        game,
        "C:\\Doom\\zandronum.exe",
        &addr,
        Some("pw"),
        "DOOM2.WAD",
    );
    assert_eq!(
        argv[1..],
        v(&[
            "-iwad",
            "DOOM2.WAD",
            "+cl_password",
            "pw",
            "+cl_joinpassword",
            "pw",
            "-connect",
            "1.2.3.4:10666"
        ])
    );
    assert_eq!(cwd, "C:\\Doom");
    let (argv, cwd) = build_launch_command(game, "z.exe", &addr, None, "");
    assert_eq!(argv[1..], v(&["-connect", "1.2.3.4:10666"]));
    assert_eq!(cwd, "");
}

#[test]
fn iwad_from_server_must_be_a_plain_wad_name() {
    let game = g("zandronum");
    let addr = ServerAddress::new("1.2.3.4", 10666);
    for evil in [
        "+quit",
        "-file x.wad",
        "+logfile C:\\x\\autoexec.cfg",
        "..\\doom2.wad",
        "C:\\doom2.wad",
        "doom2.exe",
        "",
        ".wad",
    ] {
        assert!(!is_wad_name(evil), "{evil}");
        let (argv, _) = build_launch_command(game, "z.exe", &addr, None, evil);
        assert_eq!(argv[1..], v(&["-connect", "1.2.3.4:10666"]), "{evil}");
    }
    for good in [
        "DOOM2.WAD",
        "freedoom2.wad",
        "megagame.wad",
        "chex quest (v2).pk3",
        "x.IPK7",
    ] {
        assert!(is_wad_name(good), "{good}");
    }
}

// --- test_games_tacops.py ---

fn tacops_install(root: &Path) {
    touch(&root.join(rel("TO340/System/TacticalOps.exe")));
    touch(&root.join(rel("TO350/System/TacticalOps.exe")));
}

fn tacops_exe(dir: &str, version: &str) -> LaunchCommand {
    let (r, mut calls) = run(
        g("tacops"),
        Some(dir),
        &ServerAddress::new("5.6.7.8", 7777),
        Some("pw"),
        "",
        version,
        None,
    );
    r.unwrap();
    calls.remove(0)
}

#[test]
fn launch_opens_the_client_of_the_server_version() {
    let t = tempfile::tempdir().unwrap();
    tacops_install(t.path());
    for (version, folder) in [("3.4", "TO340"), ("3.5", "TO350")] {
        let c = tacops_exe(&s(t.path()), version);
        let exe = t.path().join(folder).join(rel("System/TacticalOps.exe"));
        assert_eq!(c.argv, v(&[&s(&exe), "5.6.7.8:7777?password=pw"]));
        assert_eq!(c.cwd, s(exe.parent().unwrap()));
    }
}

#[test]
fn launch_from_a_version_folder_finds_the_sibling() {
    let t = tempfile::tempdir().unwrap();
    tacops_install(t.path());
    let c = tacops_exe(&s(&t.path().join("TO340")), "3.5");
    assert_eq!(
        c.argv[0],
        s(&t.path().join(rel("TO350/System/TacticalOps.exe")))
    );
}

#[test]
fn launch_unknown_version_uses_the_first_install() {
    let t = tempfile::tempdir().unwrap();
    tacops_install(t.path());
    let c = tacops_exe(&s(t.path()), "s_SWATGame");
    assert_eq!(
        c.argv[0],
        s(&t.path().join(rel("TO340/System/TacticalOps.exe")))
    );
}

#[test]
fn launch_single_install_ignores_the_version() {
    let t = tempfile::tempdir().unwrap();
    let exe = touch(&t.path().join(rel("System/TacticalOps.exe")));
    let c = tacops_exe(&s(t.path()), "3.5");
    assert_eq!(c.argv[0], exe);
}

// --- test_games_cs16.py, test_games_v013.py, test_games_generic.py ---

#[test]
fn cs16_launch_command() {
    let exe = "C:\\HL\\hl.exe";
    let addr = ServerAddress::new("1.2.3.4", 27015);
    let (argv, cwd) = cmd("cs16", exe, &addr, None);
    assert_eq!(
        argv,
        v(&[exe, "-game", "cstrike", "+connect", "1.2.3.4:27015"])
    );
    assert_eq!(cwd, "C:\\HL");
    let (argv, _) = cmd("cs16", exe, &addr, Some("pw"));
    assert_eq!(
        argv,
        v(&[
            exe,
            "-game",
            "cstrike",
            "+password",
            "pw",
            "+connect",
            "1.2.3.4:27015"
        ])
    );
}

#[test]
fn dday_launch_sets_game_dir() {
    let addr = ServerAddress::new("38.7.207.148", 27950);
    let (argv, cwd) = cmd("dday", r"C:\Q2\q2pro.exe", &addr, Some("pw"));
    assert_eq!(
        argv,
        v(&[
            r"C:\Q2\q2pro.exe",
            "+set",
            "game",
            "dday",
            "+password",
            "pw",
            "+connect",
            "38.7.207.148:27950"
        ])
    );
    assert_eq!(cwd, r"C:\Q2");
}

#[test]
fn gmod_connects_through_steam() {
    let addr = ServerAddress::new("38.7.207.148", 27950);
    assert_eq!(
        steam("gmod", None, &addr, None),
        v(&["steam://connect/38.7.207.148:27950"])
    );
}

#[test]
fn launch_style_decides_command_not_family() {
    let addr = ServerAddress::new("1.2.3.4", 7777);
    let unreal_q3fam = GameDef {
        launch_style: LaunchStyle::Unreal,
        ..*g("cod2")
    };
    let (argv, _) = build_launch_command(&unreal_q3fam, r"C:\g\x.exe", &addr, Some("pw"), "");
    assert_eq!(argv, v(&[r"C:\g\x.exe", "1.2.3.4:7777?password=pw"]));
    let q3_gsfam = GameDef {
        launch_style: LaunchStyle::Q3,
        ..*g("ut99")
    };
    let (argv, _) = build_launch_command(&q3_gsfam, r"C:\g\x.exe", &addr, Some("pw"), "");
    assert_eq!(
        argv,
        v(&[r"C:\g\x.exe", "+password", "pw", "+connect", "1.2.3.4:7777"])
    );
}

// --- test_games_bf1942.py ---

#[test]
fn bf1942_launch_command() {
    let exe = r"C:\Games\BF1942\BF1942.exe";
    let addr = ServerAddress::new("1.2.3.4", 14567);
    let (argv, cwd) = cmd("bf1942", exe, &addr, None);
    assert_eq!(
        argv,
        v(&[exe, "+restart", "1", "+joinServer", "1.2.3.4:14567"])
    );
    assert_eq!(cwd, r"C:\Games\BF1942");
    let (argv, _) = cmd("bf1942", exe, &addr, Some("pw"));
    assert_eq!(
        argv,
        v(&[
            exe,
            "+restart",
            "1",
            "+password",
            "pw",
            "+joinServer",
            "1.2.3.4:14567"
        ])
    );
}

#[test]
fn existing_launch_styles_unchanged() {
    let addr = ServerAddress::new("1.2.3.4", 14567);
    for (id, expected) in [
        ("cod2", v(&["+connect", "1.2.3.4:14567"])),
        ("ut99", v(&["1.2.3.4:14567"])),
        (
            "cs16",
            v(&["-game", "cstrike", "+connect", "1.2.3.4:14567"]),
        ),
    ] {
        let (argv, _) = cmd(id, r"C:\G\x.exe", &addr, None);
        assert_eq!(argv[1..], expected, "{id}");
    }
}

#[test]
fn bf1942_launch_command_with_mod() {
    let exe = r"C:\Games\BF1942\BF1942.exe";
    let addr = ServerAddress::new("1.2.3.4", 14567);
    let game = g("bf1942");
    assert_eq!(game.mod_key, "gameId");
    assert_eq!(game.base_mods, &["bf1942"]);
    let (argv, _) = build_launch_command(game, exe, &addr, None, "dc_final");
    assert_eq!(
        argv,
        v(&[
            exe,
            "+restart",
            "1",
            "+game",
            "dc_final",
            "+joinServer",
            "1.2.3.4:14567"
        ])
    );
    let (argv, _) = build_launch_command(game, exe, &addr, Some("pw"), "dc_final");
    assert_eq!(
        argv,
        v(&[
            exe,
            "+restart",
            "1",
            "+game",
            "dc_final",
            "+password",
            "pw",
            "+joinServer",
            "1.2.3.4:14567"
        ])
    );
    for m in ["", "bf1942", "BF1942"] {
        let (argv, _) = build_launch_command(game, exe, &addr, None, m);
        assert!(!argv.iter().any(|x| x == "+game"), "{m}");
    }
}

#[test]
fn other_launch_styles_ignore_mod() {
    let exe = r"C:\Games\X\x.exe";
    let addr = ServerAddress::new("1.2.3.4", 14567);
    let (argv, _) = build_launch_command(g("cod2"), exe, &addr, None, "dc_final");
    assert_eq!(argv, v(&[exe, "+connect", "1.2.3.4:14567"]));
}

// --- test_games_v012.py, test_ut2004.py, test_valve.py ---

#[test]
fn v012_launch_uses_q3_connect() {
    let addr = ServerAddress::new("1.2.3.4", 27960);
    for id in [
        "jk2",
        "xonotic",
        "warsow",
        "warfork",
        "tremulous",
        "smokinguns",
    ] {
        let (argv, cwd) = cmd(id, "C:\\x\\game.exe", &addr, Some("pw"));
        assert_eq!(
            argv[1..],
            v(&["+password", "pw", "+connect", "1.2.3.4:27960"]),
            "{id}"
        );
        assert_eq!(cwd, "C:\\x");
    }
}

#[test]
fn launch_with_password() {
    let exe = "C:\\Games\\UT2004\\System\\UT2004.exe";
    let (argv, cwd) = cmd(
        "ut2004",
        exe,
        &ServerAddress::new("1.2.3.4", 7777),
        Some("pw"),
    );
    assert_eq!(argv, v(&[exe, "1.2.3.4:7777?password=pw"]));
    assert_eq!(cwd, "C:\\Games\\UT2004\\System");
}

#[test]
fn launch_command_valve() {
    let (argv, cwd) = cmd(
        "cs16",
        r"D:\Juegos\CS16\hl.exe",
        &ServerAddress::new("1.2.3.4", 27015),
        None,
    );
    assert_eq!(
        argv,
        v(&[
            r"D:\Juegos\CS16\hl.exe",
            "-game",
            "cstrike",
            "+connect",
            "1.2.3.4:27015"
        ])
    );
    assert_eq!(cwd, r"D:\Juegos\CS16");
}

#[test]
fn launch_command_valve_password() {
    let (argv, _) = cmd(
        "cs16",
        r"C:\hl.exe",
        &ServerAddress::new("1.2.3.4", 27015),
        Some("x y"),
    );
    assert_eq!(
        argv,
        v(&[
            r"C:\hl.exe",
            "-game",
            "cstrike",
            "+password",
            "x y",
            "+connect",
            "1.2.3.4:27015"
        ])
    );
}

// --- Propios del port ---

#[test]
fn nt_dirname_cases() {
    for (exe, cwd) in [
        ("C:x.exe", "C:"),
        ("C:/g/x.exe", "C:/g"),
        ("C:\\g\\\\x.exe", "C:\\g"),
        ("\\\\srv\\share", "\\\\srv\\share"),
        ("//srv/share/x.exe", "//srv/share/"),
        ("\\\\?\\UNC\\srv\\share\\x.exe", "\\\\?\\UNC\\srv\\share\\"),
        ("\\x.exe", "\\"),
        ("C:\\\\x.exe", "C:\\"),
        ("\\\\srv", "\\\\srv"),
        ("x.exe", ""),
        ("é:\\x.exe", "é:\\"),
        ("é:x.exe", "é:"),
    ] {
        assert_eq!(cmd("cod2", exe, &a(), None).1, cwd, "{exe}");
    }
}

/// `GameData/jamp.exe` es un script que escribe sus argumentos y `$SteamAppId`: la
/// contraseña con `;`, `$(...)` y comillas llega literal en un solo argumento, sin shell.
#[cfg(unix)]
#[test]
fn spawn_command_passes_argv_literally() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};
    const SPAWN_DEADLINE: Duration = Duration::from_secs(5);
    let t = tempfile::tempdir().unwrap();
    let install = steam_jka(t.path());
    let script = install.join(rel("GameData/jamp.exe"));
    fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$SteamAppId\" \"$@\" > out.tmp\nmv out.tmp out.txt\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let pw = "pw; touch pwned $(id) \"x";
    launch(
        g("jka"),
        Some(&s(&install)),
        &a(),
        Some(pw),
        "",
        "",
        None,
        &mut |c| {
            // Otro hilo del test que lanza un proceso puede tener abierto el script (ETXTBSY);
            // plazos y no cuentas, para que una máquina cargada no falle.
            let start = Instant::now();
            loop {
                match spawn_command(c) {
                    Err(e) if e.raw_os_error() == Some(26) && start.elapsed() < SPAWN_DEADLINE => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("spawn falló tras {:?}: {e}", start.elapsed()),
                    r => return r,
                }
            }
        },
        &mut |_| Ok(()),
    )
    .unwrap();
    let out = install.join(rel("GameData/out.txt"));
    let start = Instant::now();
    let text = loop {
        match fs::read_to_string(&out) {
            Ok(t) => break t,
            Err(e) if start.elapsed() > Duration::from_secs(30) => {
                panic!("sin out.txt tras {:?}: {e}", start.elapsed())
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines,
        ["6020", "+password", pw, "+connect", "1.2.3.4:28960"]
    );
    assert!(!install.join(rel("GameData/pwned")).exists());
    assert!(!Path::new("pwned").exists());
}

#[test]
fn spawn_command_rejects_empty_argv() {
    let c = LaunchCommand {
        argv: vec![],
        cwd: String::new(),
        env: vec![],
    };
    assert_eq!(
        spawn_command(&c).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

/// Cadenas para el envoltorio de Wine y lo que daba `shlex.split` de Python (`None`: `ValueError`).
const SHLEX_CASES: &[(&str, Option<&[&str]>)] = &[
    ("wine", Some(&["wine"])),
    (
        "flatpak run \"org.winehq.Wine\"",
        Some(&["flatpak", "run", "org.winehq.Wine"]),
    ),
    ("a 'b c' d", Some(&["a", "b c", "d"])),
    ("a\\ b c", Some(&["a b", "c"])),
    ("\"a\\\"b\" 'c\\d'", Some(&["a\"b", "c\\d"])),
    ("\"x\\y\" \"\\\\\"", Some(&["x\\y", "\\"])),
    ("\t a\tb \r\n c ", Some(&["a", "b", "c"])),
    ("'' x \"\"", Some(&["", "x", ""])),
    ("a''b\"\"c", Some(&["abc"])),
    ("\"sin cerrar", None),
    ("'sin cerrar", None),
    ("fin\\", None),
    ("\"esc\\", None),
    ("ñandú 'á é'", Some(&["ñandú", "á é"])),
    ("a#b # c", Some(&["a#b", "#", "c"])),
];

#[test]
fn wine_wrapper_split_known_answers() {
    for &(case, want) in SHLEX_CASES {
        let want = want.map(|w| w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        assert_eq!(shlex_split(case), want, "{case:?}");
    }
}

#[test]
fn launch_error_never_contains_password() {
    let pw = "s3cr3t-pw";
    let t = tempfile::tempdir().unwrap();
    let mut errors = vec![
        run(g("cod2"), None, &a(), Some(pw), "", "", None).0,
        run(g("cod2"), Some(&s(t.path())), &a(), Some(pw), "", "", None).0,
        launch(
            g("tf2"),
            None,
            &a(),
            Some(pw),
            "",
            "",
            None,
            &mut |_| Ok(()),
            &mut |u| Err(io::Error::other(u.to_owned())),
        ),
    ];
    // Un NUL en la contraseña hace fallar el lanzamiento real, sin mostrarla.
    touch(&t.path().join("CoD2MP_s.exe"));
    let nul = format!("{pw}\0x");
    let r = launch(
        g("cod2"),
        Some(&s(t.path())),
        &a(),
        Some(&nul),
        "",
        "",
        None,
        &mut |c| spawn_command(c),
        &mut |_| Ok(()),
    );
    assert!(matches!(r, Err(LaunchError::Spawn(_))));
    errors.push(r);
    for r in errors {
        let e = r.unwrap_err();
        let (msg, dbg) = (e.to_string(), format!("{e:?}"));
        assert!(!msg.contains(pw) && !dbg.contains(pw), "{msg} / {dbg}");
    }
}

proptest! {
    #[test]
    fn mod_and_wad_names_never_panic(name in "\\PC{0,80}") {
        if is_mod_name(&name) || is_wad_name(&name) {
            prop_assert!(name.is_ascii());
            prop_assert!(!name.starts_with(['+', '-', '.', ' ']));
            prop_assert!(!name.contains(['/', '\\', ':']));
        }
        let (argv, _) = build_launch_command(g("bf1942"), "x.exe", &a(), None, &name);
        prop_assert!(argv.len() == 5 || argv.len() == 7);
        let _ = build_launch_command(g("cod2"), &name, &a(), None, "");
    }
}
