//! Port de tests/test_paths.py, más los de `paths` en test_games_mohaa.py y
//! test_games_tacops.py, y `normpath` contra `os.path.normpath` de Python.
mod support;

use jase_core::games::{GameDef, game};
#[cfg(unix)]
use jase_core::paths::normpath;
use jase_core::paths::{resolve_exe, resolve_game_dir, version_dir};
use std::fs;
use std::path::{Path, PathBuf};

const UT: &str = "System\\UnrealTournament.exe";

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

fn exe(dir: Option<&str>, relpath: &str) -> Option<String> {
    resolve_exe(dir, relpath, &[], &[])
}

#[test]
fn resolve_direct() {
    let t = tempfile::tempdir().unwrap();
    let e = touch(&t.path().join("CoD2MP_s.exe"));
    assert_eq!(exe(Some(&s(t.path())), "CoD2MP_s.exe"), Some(e));
}

#[test]
fn resolve_nested_one_level() {
    let t = tempfile::tempdir().unwrap();
    let e = touch(
        &t.path()
            .join(rel("UnrealTournament/System/UnrealTournament.exe")),
    );
    assert_eq!(exe(Some(&s(t.path())), UT), Some(e));
}

#[test]
fn resolve_chose_system_folder() {
    let t = tempfile::tempdir().unwrap();
    let e = touch(&t.path().join(rel("System/UnrealTournament.exe")));
    assert_eq!(exe(Some(&s(&t.path().join("System"))), UT), Some(e));
}

#[test]
fn resolve_install_dir_ends_in_exe_folder_only() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join(rel("Foo/System"));
    let e = touch(&d.join("UnrealTournament.exe"));
    assert_eq!(exe(Some(&s(&d)), UT), Some(e));
}

#[test]
fn resolve_parent() {
    let t = tempfile::tempdir().unwrap();
    let e = touch(&t.path().join("CoD2MP_s.exe"));
    fs::create_dir(t.path().join("main")).unwrap();
    assert_eq!(
        exe(Some(&s(&t.path().join("main"))), "CoD2MP_s.exe"),
        Some(e)
    );
}

#[test]
fn resolve_case_insensitive() {
    let t = tempfile::tempdir().unwrap();
    let e = touch(&t.path().join("cod2mp_s.exe"));
    assert_eq!(exe(Some(&s(t.path())), "CoD2MP_s.exe"), Some(e));
}

#[test]
fn resolve_case_insensitive_folders() {
    let t = tempfile::tempdir().unwrap();
    let e = touch(&t.path().join(rel("system/unrealtournament.EXE")));
    assert_eq!(exe(Some(&s(t.path())), UT), Some(e));
}

#[test]
fn resolve_missing() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(exe(Some(&s(t.path())), "CoD2MP_s.exe"), None);
    assert_eq!(exe(Some(&s(&t.path().join("nope"))), "CoD2MP_s.exe"), None);
}

#[test]
fn resolve_none_dir() {
    assert_eq!(exe(None, "CoD2MP_s.exe"), None);
    assert_eq!(exe(Some(""), "CoD2MP_s.exe"), None);
}

#[test]
fn resolve_direct_wins_over_nested_and_parent() {
    let t = tempfile::tempdir().unwrap();
    let install = t.path().join("main");
    let direct = touch(&install.join("CoD2MP_s.exe"));
    touch(&install.join(rel("sub/CoD2MP_s.exe")));
    touch(&t.path().join("CoD2MP_s.exe"));
    assert_eq!(exe(Some(&s(&install)), "CoD2MP_s.exe"), Some(direct));
}

#[test]
fn game_dir_direct() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("main")).unwrap();
    assert_eq!(
        resolve_game_dir(Some(&s(t.path())), "main"),
        Some(s(t.path()))
    );
}

#[test]
fn game_dir_case_insensitive() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("Main")).unwrap();
    assert_eq!(
        resolve_game_dir(Some(&s(t.path())), "MAIN"),
        Some(s(t.path()))
    );
}

#[test]
fn game_dir_subfolder() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join(rel("Call of Duty 2/main"))).unwrap();
    assert_eq!(
        resolve_game_dir(Some(&s(t.path())), "main"),
        Some(s(&t.path().join("Call of Duty 2")))
    );
}

#[test]
fn game_dir_parent() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("main")).unwrap();
    assert_eq!(
        resolve_game_dir(Some(&s(&t.path().join("main"))), "main"),
        Some(s(t.path()))
    );
}

#[test]
fn game_dir_marker_must_be_dir() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("main"));
    assert_eq!(resolve_game_dir(Some(&s(t.path())), "main"), None);
}

#[test]
fn game_dir_missing() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(resolve_game_dir(None, "main"), None);
    assert_eq!(resolve_game_dir(Some(""), "main"), None);
    assert_eq!(
        resolve_game_dir(Some(&s(&t.path().join("nope"))), "main"),
        None
    );
    assert_eq!(resolve_game_dir(Some(&s(t.path())), "main"), None);
}

#[test]
fn resolve_candidates_in_order_after_exe_relpath() {
    let t = tempfile::tempdir().unwrap();
    let dir = s(t.path());
    let cands = ["a.exe", "b.exe"];
    let second = touch(&t.path().join("b.exe"));
    assert_eq!(
        resolve_exe(Some(&dir), "main.exe", &cands, &[]),
        Some(second)
    );
    let first = touch(&t.path().join("a.exe"));
    assert_eq!(
        resolve_exe(Some(&dir), "main.exe", &cands, &[]),
        Some(first)
    );
    let main = touch(&t.path().join("MAIN.EXE"));
    assert_eq!(resolve_exe(Some(&dir), "main.exe", &cands, &[]), Some(main));
}

#[test]
fn resolve_candidate_nested_one_level() {
    let t = tempfile::tempdir().unwrap();
    let dir = s(t.path());
    let e = touch(&t.path().join(rel("OpenMoHAA/launch_openmohaa_base.exe")));
    assert_eq!(
        resolve_exe(Some(&dir), "MOHAA.exe", &["launch_openmohaa_base.exe"], &[]),
        Some(e)
    );
    assert_eq!(exe(Some(&dir), "MOHAA.exe"), None);
}

#[test]
fn preferred_wins_when_colocated_with_resolved_exe() {
    let t = tempfile::tempdir().unwrap();
    let dir = s(t.path());
    let main = touch(&t.path().join("main.exe"));
    assert_eq!(
        resolve_exe(Some(&dir), "main.exe", &[], &["pref.exe"]),
        Some(main)
    );
    let pref = touch(&t.path().join("PREF.EXE"));
    assert_eq!(
        resolve_exe(Some(&dir), "main.exe", &[], &["pref.exe"]),
        Some(pref)
    );
}

#[test]
fn preferred_elsewhere_does_not_win() {
    let t = tempfile::tempdir().unwrap();
    let main = touch(&t.path().join("main.exe"));
    touch(&t.path().join(rel("otro/pref.exe")));
    assert_eq!(
        resolve_exe(Some(&s(t.path())), "main.exe", &[], &["pref.exe"]),
        Some(main)
    );
}

#[test]
fn preferred_alone_is_not_resolved() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("pref.exe"));
    assert_eq!(
        resolve_exe(Some(&s(t.path())), "main.exe", &[], &["pref.exe"]),
        None
    );
}

fn resolve_game(g: &GameDef, dir: &Path) -> Option<String> {
    resolve_exe(
        Some(&s(dir)),
        g.exe_relpath,
        g.exe_candidates,
        g.preferred_exes,
    )
}

#[test]
fn stock_layout_resolves_mohaa_exe() {
    let t = tempfile::tempdir().unwrap();
    let e = touch(&t.path().join("MOHAA.exe"));
    touch(&t.path().join(rel("main/pak0.pk3")));
    assert_eq!(resolve_game(game("mohaa").unwrap(), t.path()), Some(e));
}

#[test]
fn openmohaa_layout_prefers_launcher_over_mohaa_exe() {
    let t = tempfile::tempdir().unwrap();
    touch(&t.path().join("MOHAA.exe"));
    touch(&t.path().join("openmohaa.exe"));
    let base = touch(&t.path().join("launch_openmohaa_base.exe"));
    let sh = touch(&t.path().join("launch_openmohaa_spearhead.exe"));
    let bt = touch(&t.path().join("launch_openmohaa_breakthrough.exe"));
    assert_eq!(resolve_game(game("mohaa").unwrap(), t.path()), Some(base));
    assert_eq!(resolve_game(game("mohaas").unwrap(), t.path()), Some(sh));
    assert_eq!(resolve_game(game("mohaab").unwrap(), t.path()), Some(bt));
}

#[test]
fn version_dir_ignores_case_and_files() {
    let t = tempfile::tempdir().unwrap();
    let dir = s(t.path());
    fs::create_dir(t.path().join("to350")).unwrap();
    touch(&t.path().join("TO340"));
    assert_eq!(
        version_dir(Some(&dir), "TO350"),
        Some(s(&t.path().join("to350")))
    );
    assert_eq!(version_dir(Some(&dir), "TO340"), None);
    assert_eq!(version_dir(None, "TO340"), None);
}

/// Lo que daba `os.path.normpath` de Python en Linux (en Windows el separador es `\`).
#[cfg(unix)]
#[test]
fn normpath_known_answers() {
    let cases = [
        ("", "."),
        (".", "."),
        ("..", ".."),
        ("/", "/"),
        ("//", "//"),
        ("///", "/"),
        ("a", "a"),
        ("a/", "a"),
        ("a/.", "a"),
        ("a/..", "."),
        ("a/../..", ".."),
        ("./a/./b/", "a/b"),
        ("/..", "/"),
        ("/../a", "/a"),
        ("//a//b", "//a/b"),
        ("///a/b/", "/a/b"),
        ("a//b/../c", "a/c"),
        ("../a/../../b", "../../b"),
        ("/a/./../b/.", "/b"),
        ("a/b/c/../../d", "a/d"),
    ];
    for (path, want) in cases {
        assert_eq!(normpath(path), want, "{path:?}");
    }
}
