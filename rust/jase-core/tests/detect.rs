//! Port de tests/test_detect.py, más los de detección en test_games_v013.py y
//! test_games_tacops.py, y las carpetas hostiles (enlaces, permisos, nombres no UTF-8).
use indexmap::IndexMap;
use jase_core::config::Config;
use jase_core::detect::{
    self, Finding, find_games, is_configured, registry_findings, resolve_game_exe, scan_folder,
    steam_findings, steam_libraries, typical_roots,
};
use jase_core::games::game;
use jase_core::paths::resolve_exe;
use proptest::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

fn touch(p: &Path) -> String {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, b"").unwrap();
    s(p)
}

fn f(game_id: &'static str, path: &Path, source: &'static str) -> Finding {
    Finding {
        game_id,
        path: s(path),
        source,
    }
}

fn ids(found: &[Finding]) -> HashSet<&'static str> {
    found.iter().map(|f| f.game_id).collect()
}

fn never() -> bool {
    false
}

fn scan(root: &Path, depth: u32) -> Vec<Finding> {
    scan_folder(&s(root), "carpeta elegida", depth, &never)
}

#[test]
fn is_configured_requires_resolving_exe() {
    let t = tempfile::tempdir().unwrap();
    let cod2 = game("cod2").unwrap();
    let mut cfg = Config {
        game_paths: IndexMap::from([("cod2".to_owned(), s(&t.path().join("nope")))]),
        ..Config::default()
    };
    assert!(!is_configured(cod2, &cfg));
    touch(&t.path().join("cod2/CoD2MP_s.exe"));
    cfg.game_paths = IndexMap::from([("cod2".to_owned(), s(&t.path().join("cod2")))]);
    assert!(is_configured(cod2, &cfg));
}

#[test]
fn registry_findings_with_fake_reader() {
    let reader = |subkey: &str, _: &str| {
        (subkey == "Activision\\Call of Duty 2").then(|| "C:\\Games\\CoD2".to_owned())
    };
    assert_eq!(
        registry_findings(&reader),
        vec![Finding {
            game_id: "cod2",
            path: "C:\\Games\\CoD2".to_owned(),
            source: "registro"
        }]
    );
}

#[cfg(not(windows))]
#[test]
fn registry_findings_without_winreg_is_empty() {
    assert_eq!(registry_findings(&detect::read_hklm), vec![]);
}

#[test]
fn steam_libraries_parses_vdf() {
    let t = tempfile::tempdir().unwrap();
    let steam = t.path().join("Steam");
    let other = t.path().join("Lib2");
    fs::create_dir_all(steam.join("steamapps")).unwrap();
    let esc = s(&other).replace('\\', "\\\\");
    fs::write(
        steam.join("steamapps/libraryfolders.vdf"),
        format!(
            "\"libraryfolders\"\n{{\n \"0\"\n {{\n  \"path\"\t\t\"{}\"\n }}\n \"1\"\n {{\n  \"path\"\t\t\"{esc}\"\n }}\n}}\n",
            s(&steam)
        ),
    )
    .unwrap();
    assert_eq!(steam_libraries(&s(&steam)), vec![s(&steam), s(&other)]);
}

#[test]
fn steam_findings_cod2() {
    let t = tempfile::tempdir().unwrap();
    let steam = t.path().join("Steam");
    touch(&steam.join("steamapps/common/Call of Duty 2/CoD2MP_s.exe"));
    assert_eq!(
        steam_findings(&[s(&steam)]),
        vec![f(
            "cod2",
            &steam.join("steamapps/common/Call of Duty 2"),
            "steam"
        )]
    );
}

#[test]
fn steam_findings_jka_gamedata_layout() {
    let t = tempfile::tempdir().unwrap();
    let steam = t.path().join("Steam");
    let jka = steam.join("steamapps/common/Jedi Academy");
    touch(&jka.join("GameData/jamp.exe"));
    assert_eq!(steam_findings(&[s(&steam)]), vec![f("jka", &jka, "steam")]);
    assert!(resolve_game_exe(game("jka").unwrap(), Some(&s(&jka))).is_some());
}

#[test]
fn steam_findings_jk2_gamedata_layout() {
    let t = tempfile::tempdir().unwrap();
    let steam = t.path().join("Steam");
    let jk2 = steam.join("steamapps/common/Jedi Outcast");
    touch(&jk2.join("GameData/jk2mp.exe"));
    assert_eq!(steam_findings(&[s(&steam)]), vec![f("jk2", &jk2, "steam")]);
    assert!(resolve_game_exe(game("jk2").unwrap(), Some(&s(&jk2))).is_some());
}

#[test]
fn scan_folder_finds_v012_games() {
    let t = tempfile::tempdir().unwrap();
    for (folder, exe) in [
        ("Xonotic", "xonotic.exe"),
        ("Warsow", "warsow_x64.exe"),
        ("Warfork", "warfork_x64.exe"),
        ("Tremulous", "tremulous.exe"),
        ("SmokinGuns", "smokinguns.exe"),
    ] {
        touch(&t.path().join(folder).join(exe));
    }
    assert_eq!(
        ids(&scan(t.path(), 3)),
        HashSet::from(["xonotic", "warsow", "warfork", "tremulous", "smokinguns"])
    );
}

fn join(a: &str, b: &str) -> String {
    s(&Path::new(a).join(b))
}

#[test]
fn typical_roots_include_lucasarts() {
    let roots = typical_roots(&["D:\\".to_owned()]);
    for pf in ["Program Files", "Program Files (x86)"] {
        assert!(roots.contains(&join(&join("D:\\", pf), "LucasArts")));
    }
    assert!(roots.contains(&join("D:\\", "Games")));
}

#[test]
fn scan_folder_finds_mohaa_variants() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("MOH/MOHAA.exe"));
    touch(&t.path().join("MOH/moh_spearhead.exe"));
    let found = scan(t.path(), 3);
    assert_eq!(ids(&found), HashSet::from(["mohaa", "mohaas"]));
    assert!(found.iter().all(|f| f.path == s(&t.path().join("MOH"))));
}

#[test]
fn scan_folder_finds_jka_in_gamedata() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("JKA/GameData/jamp.exe"));
    let found = scan(t.path(), 3);
    assert_eq!(
        found,
        vec![f("jka", &t.path().join("JKA"), "carpeta elegida")]
    );
    let g = game("jka").unwrap();
    assert_eq!(
        resolve_exe(
            Some(&found[0].path),
            g.exe_relpath,
            g.exe_candidates,
            g.preferred_exes
        ),
        Some(s(&t.path().join("JKA/GameData/jamp.exe")))
    );
}

#[test]
fn scan_folder_ut99_uses_parent_of_system() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("UT/System/UnrealTournament.exe"));
    assert_eq!(
        scan(t.path(), 3),
        vec![f("ut99", &t.path().join("UT"), "carpeta elegida")]
    );
}

#[cfg(unix)]
#[test]
fn scan_folder_survives_permission_error_and_depth() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let locked = t.path().join("locked");
    touch(&locked.join("RTCW/WolfMP.exe"));
    touch(&t.path().join("a/ET.exe"));
    touch(&t.path().join("b/c/d/WolfMP.exe")); // profundidad 3
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("como root no hay error de permiso: se salta");
        return;
    }
    let found = scan_folder(&s(t.path()), "carpeta típica", 2, &never);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(ids(&found), HashSet::from(["et"]));
}

#[test]
fn scan_folder_cancel() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("a/ET.exe"));
    assert_eq!(scan_folder(&s(t.path()), "x", 3, &|| true), vec![]);
}

#[cfg(unix)]
#[test]
fn scan_folder_does_not_follow_symlinks() {
    use std::os::unix::fs::symlink;
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("root");
    fs::create_dir(&root).unwrap();
    symlink("..", root.join("loop")).unwrap();
    touch(&t.path().join("elsewhere/ET.exe"));
    symlink(t.path().join("elsewhere"), root.join("link")).unwrap();
    assert_eq!(scan(&root, 6), vec![]);
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_names_are_skipped() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("ET");
    fs::create_dir_all(dir.join(OsStr::from_bytes(b"\xff"))).unwrap();
    fs::write(dir.join(OsStr::from_bytes(b"\xff.exe")), b"").unwrap();
    touch(&dir.join("ET.exe"));
    assert_eq!(scan(t.path(), 3), vec![f("et", &dir, "carpeta elegida")]);
    assert_eq!(
        resolve_exe(Some(&s(&dir)), "ET.exe", &[], &[]),
        Some(s(&dir.join("ET.exe")))
    );
}

#[test]
fn find_games_dedupes_keeping_best_source() {
    let t = tempfile::tempdir().unwrap();
    let g = t.path().join("CoD2");
    touch(&g.join("CoD2MP_s.exe"));
    let path = s(&g);
    let reader = |sk: &str, _: &str| sk.contains("Call of Duty 2").then(|| path.clone());
    let res = find_games(&reader, &[], &[], Some(&s(t.path())), &never);
    assert_eq!(
        res,
        IndexMap::from([("cod2".to_owned(), vec![f("cod2", &g, "registro")])])
    );
}

#[test]
fn find_games_drops_unresolvable() {
    let t = tempfile::tempdir().unwrap();
    let ghost = s(&t.path().join("ghost"));
    let res = find_games(
        &|_: &str, _: &str| Some(ghost.clone()),
        &[],
        &[],
        None,
        &never,
    );
    assert!(res.is_empty());
}

fn hl_reader(hl: &Path) -> impl Fn(&str, &str) -> Option<String> {
    let hl = s(hl);
    move |sk: &str, _: &str| (sk == "Valve\\Half-Life").then(|| hl.clone())
}

#[test]
fn detect_hl_without_cstrike_is_not_cs16() {
    let t = tempfile::tempdir().unwrap();
    let hl = t.path().join("Half-Life");
    touch(&hl.join("hl.exe"));
    assert!(find_games(&hl_reader(&hl), &[], &[], None, &never).is_empty());
}

#[test]
fn detect_hl_with_cstrike_is_cs16() {
    let t = tempfile::tempdir().unwrap();
    let hl = t.path().join("Half-Life");
    touch(&hl.join("hl.exe"));
    fs::create_dir(hl.join("cstrike")).unwrap();
    assert_eq!(
        find_games(&hl_reader(&hl), &[], &[], None, &never),
        IndexMap::from([("cs16".to_owned(), vec![f("cs16", &hl, "registro")])])
    );
}

#[test]
fn et_legacy_is_detected_by_registry_and_folder() {
    let t = tempfile::tempdir().unwrap();
    let install = t.path().join("Program Files/ETLegacy");
    touch(&install.join("etl.exe"));
    let path = s(&install);
    let reader = |sk: &str, v: &str| (sk == "ETLegacy" && v == "InstallPath").then(|| path.clone());
    assert_eq!(
        find_games(&reader, &[], &[], None, &never),
        IndexMap::from([("et".to_owned(), vec![f("et", &install, "registro")])])
    );
    let found = scan_folder(
        &s(&t.path().join("Program Files")),
        "carpeta típica",
        2,
        &never,
    );
    assert_eq!(found, vec![f("et", &install, "carpeta típica")]);
}

#[test]
fn et_prefers_legacy_client_next_to_vanilla() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("ET.exe"));
    let legacy = touch(&t.path().join("etl.exe"));
    let g = game("et").unwrap();
    assert_eq!(
        resolve_exe(
            Some(&s(t.path())),
            g.exe_relpath,
            g.exe_candidates,
            g.preferred_exes
        ),
        Some(legacy)
    );
}

#[test]
fn dday_needs_dday_folder() {
    let t = tempfile::tempdir().unwrap();
    let dir = s(t.path());
    touch(&t.path().join("quake2.exe"));
    assert_eq!(resolve_game_exe(game("dday").unwrap(), Some(&dir)), None);
    assert!(resolve_game_exe(game("q2").unwrap(), Some(&dir)).is_some());
    fs::create_dir(t.path().join("dday")).unwrap();
    assert_eq!(
        resolve_game_exe(game("dday").unwrap(), Some(&dir)),
        Some(s(&t.path().join("quake2.exe")))
    );
}

#[test]
fn gmod_folder_is_not_css() {
    let t = tempfile::tempdir().unwrap();
    let dir = s(t.path());
    touch(&t.path().join("hl2.exe"));
    fs::create_dir(t.path().join("cstrike")).unwrap();
    assert_eq!(resolve_game_exe(game("gmod").unwrap(), Some(&dir)), None);
    fs::create_dir(t.path().join("garrysmod")).unwrap();
    assert!(resolve_game_exe(game("gmod").unwrap(), Some(&dir)).is_some());
}

#[test]
fn gmod_64bit_exe_found() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("bin/win64/gmod.exe"));
    fs::create_dir(t.path().join("garrysmod")).unwrap();
    assert!(resolve_game_exe(game("gmod").unwrap(), Some(&s(t.path()))).is_some());
}

#[test]
fn scan_folder_reports_one_install_above_the_versions() {
    let t = tempfile::tempdir().unwrap();
    let to = t.path().join("TacticalOps");
    touch(&to.join("TO340/System/TacticalOps.exe"));
    touch(&to.join("TO350/System/TacticalOps.exe"));
    let found = scan(t.path(), 4);
    assert_eq!(found, vec![f("tacops", &to, "carpeta elegida")]);
    assert!(resolve_game_exe(game("tacops").unwrap(), Some(&found[0].path)).is_some());
}

#[test]
fn typical_roots_include_the_tacops_folders() {
    let roots = typical_roots(&["I:\\".to_owned()]);
    assert!(roots.iter().any(|r| r.ends_with("TacticalOps")));
    assert!(roots.iter().any(|r| r.ends_with("Infogrames")));
}

proptest! {
    #[test]
    fn steam_libraries_never_panics(vdf in proptest::collection::vec(any::<u8>(), 0..300),
                                    text in "[\"path\\\\ \t\n\u{e9}a]{0,80}") {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir(t.path().join("steamapps")).unwrap();
        let file = t.path().join("steamapps/libraryfolders.vdf");
        for data in [vdf.as_slice(), text.as_bytes()] {
            fs::write(&file, data).unwrap();
            let libs = steam_libraries(&s(t.path()));
            prop_assert_eq!(&libs[0], &s(t.path()));
        }
    }
}

#[cfg(windows)]
#[test]
fn read_hklm_reads_a_real_key() {
    let name = detect::read_hklm("Microsoft\\Windows NT\\CurrentVersion", "ProductName");
    assert!(name.is_some_and(|n| !n.is_empty()));
    assert_eq!(detect::read_hklm("JASE\\No\\Existe", "Nada"), None);
}

#[cfg(windows)]
#[test]
fn find_games_dedupes_case_insensitive() {
    let t = tempfile::tempdir().unwrap();
    let g = t.path().join("CoD2");
    touch(&g.join("CoD2MP_s.exe"));
    let upper = s(&g).to_uppercase();
    let reader = |sk: &str, _: &str| sk.contains("Call of Duty 2").then(|| upper.clone());
    let res = find_games(&reader, &[], &[], Some(&s(t.path())), &never);
    let cod2 = &res["cod2"];
    assert_eq!(cod2.len(), 1);
    assert_eq!(cod2[0].source, "registro");
}
