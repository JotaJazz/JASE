//! Port de tests/test_maps_cod.py, test_maps_levelshots.py, test_maps_unreal.py y
//! test_maps_service.py (solo archivos del juego y nombres), más zips hostiles.
mod support;

use jase_core::games::game;
use jase_core::maps::cod::{
    MAX_ENTRY_SIZE, find_loadscreen, read_entry, read_first, read_first_limited,
};
use jase_core::maps::levelshots::{find_levelshot, valid_map_path};
use jase_core::maps::service::{gametracker_map_name, gametracker_url, safe_map_filename};
use jase_core::maps::unreal::find_map_file;
use proptest::prelude::*;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// `base` más `parts`, componente por componente (sirve igual en Windows).
fn rel(base: &Path, parts: &[&str]) -> PathBuf {
    parts.iter().fold(base.to_path_buf(), |p, s| p.join(s))
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn zip_bytes(entries: &[(&str, &[u8])], method: CompressionMethod) -> Vec<u8> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(method);
    for (name, data) in entries {
        z.start_file(*name, opts).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap().into_inner()
}

/// Zip *stored* como `zipfile.ZipFile(path, "w")`, creando las carpetas.
fn make_zip(path: PathBuf, entries: &[(&str, &[u8])]) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, zip_bytes(entries, CompressionMethod::Stored)).unwrap();
    path
}

/// Tamaño descomprimido de la primera entrada en la cabecera local (22) y en el
/// directorio central (24).
fn patch_first_size(data: &mut [u8], size: u32) {
    for (sig, off) in [(b"PK\x03\x04", 22), (b"PK\x01\x02", 24)] {
        let start = data.windows(4).position(|w| w == sig).unwrap();
        data[start + off..start + off + 4].copy_from_slice(&size.to_le_bytes());
    }
}

const LS: &str = "images/loadscreen_mp_toujane.iwi";

// --- test_maps_cod.py ---

#[test]
fn find_loadscreen_last_alphabetical_wins() {
    let t = tempfile::tempdir().unwrap();
    make_zip(rel(t.path(), &["main", "iw_00.iwd"]), &[(LS, b"A")]);
    make_zip(rel(t.path(), &["main", "iw_07.iwd"]), &[(LS, b"B")]);
    make_zip(rel(t.path(), &["main", "iw_03.iwd"]), &[(LS, b"C")]);
    assert_eq!(
        find_loadscreen(Some(s(t.path())), "mp_toujane").as_deref(),
        Some(&b"B"[..])
    );
}

#[test]
fn find_loadscreen_case_insensitive_and_pk3() {
    let t = tempfile::tempdir().unwrap();
    make_zip(
        rel(t.path(), &["Main", "pak0.PK3"]),
        &[("Images/LoadScreen_MP_Carentan.IWI", b"X")],
    );
    for name in ["mp_carentan", "MP_CARENTAN"] {
        assert_eq!(
            find_loadscreen(Some(s(t.path())), name).as_deref(),
            Some(&b"X"[..])
        );
    }
}

#[test]
fn find_loadscreen_in_mods_overrides_main() {
    let t = tempfile::tempdir().unwrap();
    let foo = "images/loadscreen_mp_foo.iwi";
    make_zip(rel(t.path(), &["main", "iw_00.iwd"]), &[(foo, b"MAIN")]);
    make_zip(rel(t.path(), &["mods", "x", "z.iwd"]), &[(foo, b"MOD")]);
    make_zip(
        rel(t.path(), &["mods", "x", "y.iwd"]),
        &[("other.txt", b"")],
    );
    make_zip(
        rel(t.path(), &["mods", "y", "only.iwd"]),
        &[("images/loadscreen_mp_bar.iwi", b"BAR")],
    );
    let dir = Some(s(t.path()));
    assert_eq!(find_loadscreen(dir, "mp_foo").as_deref(), Some(&b"MOD"[..]));
    assert_eq!(find_loadscreen(dir, "mp_bar").as_deref(), Some(&b"BAR"[..]));
}

#[test]
fn find_loadscreen_ignores_corrupt_and_dirs() {
    let t = tempfile::tempdir().unwrap();
    let good = make_zip(
        rel(t.path(), &["main", "iw_00.iwd"]),
        &[("images/loadscreen_mp_foo.iwi", b"OK")],
    );
    let main = t.path().join("main");
    fs::write(main.join("zz_corrupt.iwd"), b"esto no es un zip").unwrap();
    fs::write(
        main.join("zz_truncated.iwd"),
        &fs::read(&good).unwrap()[..40],
    )
    .unwrap();
    fs::create_dir(main.join("zzz_dir.iwd")).unwrap();
    assert_eq!(
        find_loadscreen(Some(s(t.path())), "mp_foo").as_deref(),
        Some(&b"OK"[..])
    );
}

#[test]
fn find_loadscreen_resolves_nested_and_parent() {
    let t = tempfile::tempdir().unwrap();
    make_zip(
        rel(t.path(), &["CoD2", "main", "iw_00.iwd"]),
        &[("images/loadscreen_mp_foo.iwi", b"N")],
    );
    let nested = rel(t.path(), &["CoD2", "main"]);
    for dir in [t.path(), nested.as_path()] {
        assert_eq!(
            find_loadscreen(Some(s(dir)), "mp_foo").as_deref(),
            Some(&b"N"[..])
        );
    }
}

#[test]
fn find_loadscreen_missing_install() {
    let t = tempfile::tempdir().unwrap();
    let missing = rel(t.path(), &["no", "existe", "jase"]);
    for install in [None, Some(""), Some(s(&missing))] {
        assert_eq!(find_loadscreen(install, "mp_foo"), None);
    }
}

#[test]
fn find_loadscreen_not_found_and_bad_names() {
    let t = tempfile::tempdir().unwrap();
    make_zip(
        rel(t.path(), &["main", "iw_00.iwd"]),
        &[("images/loadscreen_mp_foo.iwi", b"OK")],
    );
    for name in ["mp_nope", "", "../mp_foo", "..\\mp_foo"] {
        assert_eq!(find_loadscreen(Some(s(t.path())), name), None, "{name}");
    }
}

#[test]
fn read_first_respects_wanted_order_and_size() {
    let t = tempfile::tempdir().unwrap();
    let z = make_zip(t.path().join("a.pk3"), &[("B.jpg", b"b"), ("a.tga", b"a")]);
    let w = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        read_first(s(&z), &w(&["a.tga", "b.jpg"])).as_deref(),
        Some(&b"a"[..])
    );
    assert_eq!(
        read_first(s(&z), &w(&["x.jpg", "b.jpg"])).as_deref(),
        Some(&b"b"[..])
    );
    assert_eq!(read_first(s(&z), &w(&["x.jpg"])), None);
    assert_eq!(read_first_limited(s(&z), &w(&["a.tga", "b.jpg"]), 0), None);
}

#[test]
fn read_entry_caps_lying_sizes() {
    let t = tempfile::tempdir().unwrap();
    // Deflated de 1 MiB que dice medir 10 bytes.
    let mut data = zip_bytes(
        &[("big.bin", &vec![0u8; 1 << 20])],
        CompressionMethod::Deflated,
    );
    let honest = t.path().join("honest.pk3");
    fs::write(&honest, &data).unwrap();
    assert_eq!(
        read_entry(s(&honest), "big.bin").map(|d| d.len()),
        Some(1 << 20)
    );
    patch_first_size(&mut data, 10);
    let liar = t.path().join("liar.pk3");
    fs::write(&liar, &data).unwrap();
    assert_eq!(read_entry(s(&liar), "big.bin"), None);
    assert_eq!(read_first(s(&liar), &["big.bin".to_owned()]), None);

    // Declara más que el tope: `read_entry` no la lee y `read_first` pasa a la siguiente.
    let mut data = zip_bytes(
        &[("huge.bin", b"x"), ("next.bin", b"n")],
        CompressionMethod::Stored,
    );
    patch_first_size(&mut data, u32::try_from(MAX_ENTRY_SIZE + 1).unwrap());
    let huge = t.path().join("huge.pk3");
    fs::write(&huge, &data).unwrap();
    assert_eq!(read_entry(s(&huge), "huge.bin"), None);
    assert_eq!(
        read_first(s(&huge), &["huge.bin".to_owned(), "next.bin".to_owned()]).as_deref(),
        Some(&b"n"[..])
    );
}

#[test]
fn corrupt_local_header_of_other_entry_is_ignored() {
    let t = tempfile::tempdir().unwrap();
    let mut data = zip_bytes(
        &[("junk.bin", b"j"), ("levelshots/m.jpg", b"JPG")],
        CompressionMethod::Stored,
    );
    // Rompe la firma del encabezado local de la primera entrada (el directorio central queda).
    let start = data.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
    data[start..start + 4].copy_from_slice(b"XXXX");
    let path = t.path().join("a.pk3");
    fs::write(&path, &data).unwrap();
    assert_eq!(
        read_first(s(&path), &["levelshots/m.jpg".to_owned()]).as_deref(),
        Some(&b"JPG"[..])
    );
    assert_eq!(read_entry(s(&path), "junk.bin"), None);
}

#[test]
fn huge_index_is_fast() {
    let t = tempfile::tempdir().unwrap();
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..20_000 {
        z.start_file(format!("textures/t{i}.jpg"), SimpleFileOptions::default())
            .unwrap();
    }
    z.start_file("levelshots/m.jpg", SimpleFileOptions::default())
        .unwrap();
    z.write_all(b"JPG").unwrap();
    let path = t.path().join("big.pk3");
    fs::write(&path, z.finish().unwrap().into_inner()).unwrap();
    let start = std::time::Instant::now();
    let wanted = ["levelshots/m.tga".to_owned(), "levelshots/m.jpg".to_owned()];
    assert_eq!(read_first(s(&path), &wanted).as_deref(), Some(&b"JPG"[..]));
    let elapsed = start.elapsed();
    // Lineal: ~20 ms; el índice cuadrático anterior tardaba ~1,1 s.
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "{elapsed:?}"
    );
}

#[cfg(unix)]
#[test]
fn archives_in_skips_symlinks() {
    use jase_core::maps::cod::archives_in;
    use std::os::unix::fs::symlink;
    let t = tempfile::tempdir().unwrap();
    let real = make_zip(
        t.path().join("real.zip"),
        &[("images/loadscreen_mp_foo.iwi", b"LINK")],
    );
    make_zip(
        rel(t.path(), &["game", "main", "aa.iwd"]),
        &[("images/loadscreen_mp_foo.iwi", b"MAIN")],
    );
    let main = rel(t.path(), &["game", "main"]);
    symlink(&real, main.join("zz.iwd")).unwrap();
    let elsewhere = t.path().join("elsewhere");
    make_zip(
        elsewhere.join("z.iwd"),
        &[("images/loadscreen_mp_foo.iwi", b"MOD")],
    );
    fs::create_dir(rel(t.path(), &["game", "mods"])).unwrap();
    symlink(&elsewhere, rel(t.path(), &["game", "mods", "link"])).unwrap();

    assert_eq!(
        archives_in(s(&main)),
        vec![s(&main.join("aa.iwd")).to_owned()]
    );
    assert_eq!(
        find_loadscreen(Some(s(&t.path().join("game"))), "mp_foo").as_deref(),
        Some(&b"MAIN"[..])
    );
}

// --- test_maps_levelshots.py ---

#[test]
fn finds_levelshot_case_insensitive() {
    let t = tempfile::tempdir().unwrap();
    make_zip(
        rel(t.path(), &["baseq3", "pak0.pk3"]),
        &[("levelshots/q3dm17.jpg", b"JPG")],
    );
    assert_eq!(
        find_levelshot(Some(s(t.path())), "baseq3", "Q3DM17").as_deref(),
        Some(&b"JPG"[..])
    );
}

#[test]
fn last_pk3_wins() {
    let t = tempfile::tempdir().unwrap();
    make_zip(
        rel(t.path(), &["baseq3", "pak0.pk3"]),
        &[("levelshots/m.png", b"RED")],
    );
    make_zip(
        rel(t.path(), &["baseq3", "zz.pk3"]),
        &[("levelshots/m.png", b"BLUE")],
    );
    assert_eq!(
        find_levelshot(Some(s(t.path())), "baseq3", "m").as_deref(),
        Some(&b"BLUE"[..])
    );
}

#[test]
fn extension_order_within_pk3() {
    let t = tempfile::tempdir().unwrap();
    make_zip(
        rel(t.path(), &["baseq3", "a.pk3"]),
        &[("levelshots/m.png", b"PNG"), ("levelshots/m.jpg", b"JPG")],
    );
    assert_eq!(
        find_levelshot(Some(s(t.path())), "baseq3", "m").as_deref(),
        Some(&b"JPG"[..])
    );
}

#[test]
fn jka_nested_game_data() {
    let t = tempfile::tempdir().unwrap();
    make_zip(
        rel(t.path(), &["GameData", "base", "assets1.pk3"]),
        &[("levelshots/mp/ffa1.tga", b"TGA")],
    );
    assert_eq!(
        find_levelshot(Some(s(t.path())), "base", "mp/ffa1").as_deref(),
        Some(&b"TGA"[..])
    );
}

#[test]
fn invalid_map_names() {
    let t = tempfile::tempdir().unwrap();
    let names = ["../x", "a\\b", "", "c:/x", "a//b", "./x", "a/.."];
    // El pk3 trae cada entrada: si el nombre pasara, se encontraría.
    let entries: Vec<(String, &[u8])> = names
        .iter()
        .map(|n| (format!("levelshots/{n}.jpg"), &b"X"[..]))
        .collect();
    let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    make_zip(rel(t.path(), &["baseq3", "pak0.pk3"]), &refs);
    for name in names {
        assert!(!valid_map_path(name), "{name}");
        assert_eq!(
            find_levelshot(Some(s(t.path())), "baseq3", name),
            None,
            "{name}"
        );
    }
}

#[test]
fn garbage_pk3_ignored() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("baseq3")).unwrap();
    fs::write(rel(t.path(), &["baseq3", "zzz.pk3"]), "basura").unwrap();
    make_zip(
        rel(t.path(), &["baseq3", "pak0.pk3"]),
        &[("levelshots/m.png", b"P")],
    );
    assert_eq!(
        find_levelshot(Some(s(t.path())), "baseq3", "m").as_deref(),
        Some(&b"P"[..])
    );
}

#[test]
fn missing_install_dir() {
    assert_eq!(find_levelshot(None, "baseq3", "m"), None);
}

#[test]
fn levelshot_dirs_defined() {
    assert_eq!(game("jka").unwrap().levelshot_dir, "base");
    assert_eq!(game("cod2").unwrap().levelshot_dir, "");
}

// --- test_maps_unreal.py (find_map_file) ---

#[test]
fn find_map_file_nested_and_case_insensitive() {
    let t = tempfile::tempdir().unwrap();
    let install = t.path().join("UnrealTournament");
    let maps = rel(&install, &["UnrealTournament", "Maps"]);
    fs::create_dir_all(&maps).unwrap();
    let target = maps.join("CTF-Face.unr");
    fs::write(&target, b"x").unwrap();
    fs::create_dir(rel(&install, &["UnrealTournament", "Cache"])).unwrap();
    let expected = Some(s(&target).to_owned());
    assert_eq!(find_map_file(Some(s(&install)), "CTF-Face"), expected);
    assert_eq!(find_map_file(Some(s(&install)), "ctf-face"), expected);
    assert_eq!(find_map_file(Some(s(&install)), "CTF-Face.unr"), expected);
    assert_eq!(find_map_file(Some(s(&install)), "CTF-Face.UNR"), expected);
    let inner = install.join("UnrealTournament");
    assert_eq!(find_map_file(Some(s(&inner)), "CTF-Face"), expected);
}

#[test]
fn find_map_file_missing() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("Maps")).unwrap();
    let dir = Some(s(t.path()));
    assert_eq!(find_map_file(dir, "DM-Nope"), None);
    assert_eq!(find_map_file(dir, ""), None);
    assert_eq!(find_map_file(dir, "../x"), None);
    assert_eq!(find_map_file(None, "DM-Deck16]["), None);
    let missing = t.path().join("missing");
    assert_eq!(find_map_file(Some(s(&missing)), "DM-Deck16]["), None);
}

// --- test_maps_service.py y test_games_mohaa.py (nombres y URL) ---

#[test]
fn safe_map_filename_cases() {
    assert_eq!(
        safe_map_filename("DM-Deck16][").as_deref(),
        Some("dm-deck16][")
    );
    assert_eq!(safe_map_filename("a:b").as_deref(), Some("ab"));
    assert_eq!(safe_map_filename("con").as_deref(), Some("con_"));
    assert_eq!(safe_map_filename(" x. ").as_deref(), Some("x"));
    assert_eq!(safe_map_filename(":/*"), None);
    assert_eq!(safe_map_filename(""), None);
}

#[test]
fn gametracker_map_name_cases() {
    for (raw, expected) in [
        ("dm-deck16][", "dm_deck16__"),
        ("DM-Rankin", "dm_rankin"),
        ("obj/obj_team2", "obj_obj_team2"),
        ("mp_a b", "mp_a_b"),
        ("de_dust2", "de_dust2"),
    ] {
        assert_eq!(gametracker_map_name(raw), expected, "{raw}");
    }
}

#[test]
fn gametracker_url_normalizes_name() {
    assert_eq!(
        gametracker_url("ut2k4", "DM-Rankin"),
        "https://image.gametracker.com/images/maps/160x120/ut2k4/dm_rankin.jpg"
    );
}

#[test]
fn gametracker_url_replaces_slash() {
    assert_eq!(
        gametracker_url("mohaa", "obj/obj_team2"),
        "https://image.gametracker.com/images/maps/160x120/mohaa/obj_obj_team2.jpg"
    );
}

#[test]
fn gametracker_ids() {
    assert_eq!(game("cod2").unwrap().gametracker_id, "cod2");
    assert_eq!(game("cod1").unwrap().gametracker_id, "cod");
}

#[test]
fn gametracker_folders() {
    for (id, gt) in [
        ("rtcw", "et"),
        ("oa", "q3"),
        ("ut99", "ut"),
        ("ut2004", "ut2k4"),
        ("css", "css"),
        ("tf2", "tf2"),
        ("l4d", "l4d"),
        ("l4d2", "left4dead2"),
        ("hldm", "hl"),
        ("dod", "dod"),
        ("tfc", "tfc"),
        ("q2", "q2"),
    ] {
        assert_eq!(game(id).unwrap().gametracker_id, gt, "{id}");
    }
}

#[test]
fn jka_has_no_gametracker_folder() {
    assert_eq!(game("jka").unwrap().gametracker_id, "");
}

/// Lo que daban `safe_map_filename` y `gametracker_map_name` de Python 1.3.0.
#[test]
fn map_names_known_answers() {
    let cases = [
        ("Straße", Some("strasse"), "strasse"),
        ("STRASSE", Some("strasse"), "strasse"),
        ("İstanbul", Some("i\u{307}stanbul"), "i_stanbul"),
        ("\u{212a}elvin", Some("kelvin"), "kelvin"),
        ("DM-Deck16][", Some("dm-deck16]["), "dm_deck16__"),
        ("obj/obj_team2", Some("objobj_team2"), "obj_obj_team2"),
        ("a\0b\u{1f}c", Some("abc"), "a_b_c"),
        ("\u{7f}x", Some("\u{7f}x"), "_x"),
        (" ..con.. ", Some("con_"), "___con___"),
        ("COM1", Some("com1_"), "com1"),
        ("lpt9", Some("lpt9_"), "lpt9"),
        ("nul.", Some("nul_"), "nul_"),
        (
            "a\\b:c*d?e\"f<g>h|i",
            Some("abcdefghi"),
            "a_b_c_d_e_f_g_h_i",
        ),
        ("mp_ünïcödé", Some("mp_ünïcödé"), "mp__n_c_d_"),
        ("ÀÉÎÕÜ", Some("àéîõü"), "_____"),
        ("x\u{a0}y", Some("x\u{a0}y"), "x_y"),
        ("...", None, "___"),
        ("", None, ""),
        ("ǅungla", Some("ǆungla"), "_ungla"),
        ("  spaced out  ", Some("spaced out"), "__spaced_out__"),
    ];
    for (name, safe, gt) in cases {
        assert_eq!(safe_map_filename(name).as_deref(), safe, "{name:?}");
        assert_eq!(gametracker_map_name(name), gt, "{name:?}");
    }
}

proptest! {
    #[test]
    fn cache_names_never_escape(name in any::<String>()) {
        if let Some(n) = safe_map_filename(&name) {
            prop_assert!(!n.is_empty());
            prop_assert!(n != "." && n != "..");
            prop_assert!(!n.contains(['/', '\\', ':']));
            prop_assert!(!n.chars().any(|c| ('\0'..='\x1f').contains(&c)));
            prop_assert!(!n.starts_with([' ', '.']) && !n.ends_with([' ', '.']));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn archive_readers_never_panic(
        garbage in proptest::collection::vec(any::<u8>(), 0..512),
        flips in proptest::collection::vec((any::<usize>(), any::<u8>()), 0..8),
    ) {
        let t = tempfile::tempdir().unwrap();
        let mut mutated = zip_bytes(
            &[("levelshots/m.jpg", b"JPG"), ("images/loadscreen_m.iwi", &[7; 300])],
            CompressionMethod::Deflated,
        );
        let len = mutated.len();
        for (i, b) in flips {
            mutated[i % len] = b;
        }
        for (name, data) in [("a.pk3", &garbage), ("b.pk3", &mutated)] {
            let path = rel(t.path(), &["main", name]);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, data).unwrap();
            let _ = read_entry(s(&path), "images/loadscreen_m.iwi");
            let _ = read_first(s(&path), &["levelshots/m.jpg".to_owned()]);
        }
        let _ = find_loadscreen(Some(s(t.path())), "m");
        let _ = find_levelshot(Some(s(t.path())), "main", "m");
    }
}
