//! Port de tests/test_app_update.py y tests/test_app_update_install.py (sin Qt).
use jase_core::app_update::{
    self, Asset, CHECK_INTERVAL_S, DownloadCode, DownloadError, MAX_BYTES, MAX_INSTALLER_BYTES,
    META_FILENAME, RETRY_AFTER_FAILURE_S, Release, check, check_detailed, default_fetch,
    default_fetch_stream, download_installer, newest_release, parse_version, should_check,
    stream_with_agent,
};
use proptest::prelude::*;
use serde_json::{Value, json};
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;

const PAGE: &str = "https://github.com/JotaJazz/JASE/releases/tag/";
const DL: &str = "https://github.com/JotaJazz/JASE/releases/download/v99.0.0/JASE-99.0.0-setup.exe";

fn rel(tag: &str) -> Value {
    json!({"tag_name": tag, "draft": false, "prerelease": false,
           "html_url": format!("{PAGE}{tag}")})
}

fn payload(releases: &[Value]) -> Vec<u8> {
    serde_json::to_vec(releases).unwrap()
}

fn body() -> Vec<u8> {
    b"installer-bytes".repeat(5000)
}

fn sha(data: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, data)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn asset() -> Asset {
    Asset {
        name: "JASE-99.0.0-setup.exe".into(),
        url: DL.into(),
        size: body().len() as u64,
        sha256: sha(&body()),
    }
}

fn release_with(over: &[(&str, Value)]) -> Value {
    let mut a = json!({"name": "JASE-99.0.0-setup.exe", "size": body().len(),
                       "digest": format!("sha256:{}", sha(&body())), "browser_download_url": DL});
    for (k, v) in over {
        a[*k] = v.clone();
    }
    let mut r = rel("v99.0.0");
    r["assets"] = json!([a]);
    r
}

fn plain(version: &str, tag: &str) -> Release {
    Release {
        version: version.into(),
        url: format!("{PAGE}{tag}"),
        installer: None,
        portable: None,
    }
}

fn stream(data: Vec<u8>) -> impl FnOnce(&str) -> Result<Box<dyn Read + Send>, DownloadError> {
    move |_| Ok(Box::new(Cursor::new(data)))
}

fn download(dir: &Path, data: Vec<u8>, cancel: bool) -> Result<std::path::PathBuf, DownloadError> {
    download_installer(&asset(), dir, stream(data), &mut |_, _| {}, &|| cancel)
}

fn entries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect()
}

// ------------------------------------------------------------ aviso de versión nueva
#[test]
fn parse_version_cases() {
    assert_eq!(parse_version("v0.10.0"), Some(vec![0, 10, 0]));
    assert_eq!(parse_version("0.10.0"), Some(vec![0, 10, 0]));
    for bad in [
        "v1.0-beta",
        "",
        "v",
        "1..2",
        "dev",
        "1.",
        ".1",
        "vv1",
        "1.0\n",
        "١",
    ] {
        assert_eq!(parse_version(bad), None, "{bad:?}");
    }
}

#[test]
fn huge_version_component_is_none() {
    assert_eq!(parse_version("1.99999999999999999999999"), None);
    assert_eq!(
        parse_version(&format!("1.{}", u64::MAX)),
        Some(vec![1, u64::MAX])
    );
}

#[test]
fn newest_release_skips_drafts_and_counts_prereleases() {
    let mut draft = rel("v0.12.0");
    draft["draft"] = json!(true);
    let mut pre = rel("v0.11.0");
    pre["prerelease"] = json!(true);
    let data = payload(&[draft, pre, rel("v0.10.0")]);
    assert_eq!(newest_release(&data), Some(plain("0.11.0", "v0.11.0")));
    let mut no_draft = rel("v0.13.0");
    no_draft.as_object_mut().unwrap().remove("draft");
    let mut null_draft = rel("v0.14.0");
    null_draft["draft"] = Value::Null;
    let data = payload(&[no_draft, null_draft, rel("v0.10.0")]);
    assert_eq!(newest_release(&data).unwrap().version, "0.10.0");
}

#[test]
fn newest_release_compares_numerically() {
    let data = payload(&[rel("v0.9.0"), rel("v0.10.0")]);
    assert_eq!(newest_release(&data).unwrap().version, "0.10.0");
    // Versión normalizada como `".".join(map(str, version))`.
    assert_eq!(
        newest_release(&payload(&[rel("v01.002")])).unwrap().version,
        "1.2"
    );
}

#[test]
fn newest_release_discards_foreign_url_and_bad_data() {
    for url in ["https://evil.example/x", "javascript:alert(1)"] {
        let mut r = rel("v9.0.0");
        r["html_url"] = json!(url);
        assert_eq!(newest_release(&payload(&[r])), None);
    }
    assert_eq!(newest_release(b"no es json"), None);
    assert_eq!(newest_release(br#"{"a": 1}"#), None);
    assert_eq!(
        newest_release(&payload(&[rel("rc-1"), json!(5), json!("x")])),
        None
    );
    assert_eq!(newest_release(b"\xef\xbb\xbf[]"), None);
}

#[test]
fn check_only_returns_newer() {
    let t = tempfile::tempdir().unwrap();
    let data = payload(&[rel("v0.11.0")]);
    let f = |d: &Vec<u8>| {
        let d = d.clone();
        move |_: &str| Some(d)
    };
    assert_eq!(check(t.path(), 1000.0, "0.11.0", f(&data)), None);
    assert_eq!(check(t.path(), 1000.0, "0.12.0", f(&data)), None);
    assert_eq!(
        check(t.path(), 1000.0, "0.10.0", f(&data)).unwrap().version,
        "0.11.0"
    );
    assert_eq!(check(t.path(), 1000.0, "dev", f(&data)), None);
}

#[test]
fn check_failure_records_meta_and_never_raises() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(check(t.path(), 1000.0, "0.10.0", |_| None), None);
    let meta: Value =
        serde_json::from_slice(&fs::read(t.path().join(META_FILENAME)).unwrap()).unwrap();
    assert_eq!(meta["last_attempt_ok"], json!(false));
    let big = vec![b'x'; MAX_BYTES + 1];
    assert_eq!(
        check_detailed(t.path(), 1000.0, "0.10.0", |_| Some(big)),
        (None, false)
    );
}

#[test]
fn should_check_schedule() {
    let t = tempfile::tempdir().unwrap();
    let meta = t.path().join(META_FILENAME);
    assert!(should_check(&meta, 1000.0));
    let data = payload(&[rel("v0.10.0")]);
    check(t.path(), 1000.0, "0.10.0", |_| Some(data));
    assert!(!should_check(&meta, 1000.0 + 7200.0));
    assert!(should_check(&meta, 1000.0 + CHECK_INTERVAL_S as f64));
    check(t.path(), 1000.0, "0.10.0", |_| None);
    assert_eq!(RETRY_AFTER_FAILURE_S, 3600);
    assert!(should_check(&meta, 1000.0 + 7200.0));
    assert!(!should_check(&meta, 1000.0 + 60.0));
}

#[test]
fn default_fetch_sends_github_accept_header() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/releases", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let n = s.read(&mut buf).unwrap_or(0);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]");
        String::from_utf8_lossy(&buf[..n]).to_lowercase()
    });
    assert_eq!(default_fetch(&url), Some(b"[]".to_vec()));
    let req = server.join().unwrap();
    assert!(
        req.contains("accept: application/vnd.github+json\r\n"),
        "{req}"
    );
    assert!(req.contains("user-agent: jase\r\n"), "{req}");
}

#[test]
fn check_treats_failed_download_as_failure() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(
        check_detailed(t.path(), 1000.0, "1.0", |_| None),
        (None, false)
    );
}

// ------------------------------------------------------------ assets del instalador
#[test]
fn valid_asset_parsed() {
    let r = newest_release(&payload(&[release_with(&[])])).unwrap();
    assert_eq!(r.installer, Some(asset()));
    // `digest` en mayúsculas: se guarda en minúsculas.
    let upper = format!("sha256:{}", sha(&body()).to_uppercase());
    let r = newest_release(&payload(&[release_with(&[("digest", json!(upper))])])).unwrap();
    assert_eq!(r.installer, Some(asset()));
}

#[test]
fn portable_asset_parsed() {
    let mut r = rel("v99.0.0");
    r["assets"] = json!([{"name": "JASE-99.0.0-win64.zip", "size": 10,
        "digest": format!("sha256:{}", "a".repeat(64)),
        "browser_download_url": "https://github.com/JotaJazz/JASE/releases/download/v99.0.0/JASE-99.0.0-win64.zip"}]);
    let r = newest_release(&payload(&[r])).unwrap();
    assert_eq!(r.installer, None);
    assert_eq!(r.portable.unwrap().sha256, "a".repeat(64));
}

#[test]
fn invalid_asset_gives_no_installer() {
    let cases: Vec<(&str, Value)> = vec![
        ("digest", Value::Null),
        ("digest", json!("md5:abc")),
        ("digest", json!("sha256:zz")),
        (
            "browser_download_url",
            json!("https://evil.example/JASE-99.0.0-setup.exe"),
        ),
        ("size", json!(MAX_INSTALLER_BYTES + 1)),
        ("size", json!(0)),
        ("size", json!("9")),
        ("name", json!("JASE-0.11.0-setup.exe")),
    ];
    for (k, v) in cases {
        let r = newest_release(&payload(&[release_with(&[(k, v.clone())])]));
        assert_eq!(r, Some(plain("99.0.0", "v99.0.0")), "{k}={v}");
    }
}

#[test]
fn size_json_float_or_bool_is_rejected() {
    for v in [json!(9.0), json!(true), json!(-1)] {
        let r = newest_release(&payload(&[release_with(&[("size", v.clone())])])).unwrap();
        assert_eq!(r.installer, None, "{v}");
    }
}

#[test]
fn missing_or_malformed_assets() {
    assert_eq!(
        newest_release(&payload(&[rel("v99.0.0")]))
            .unwrap()
            .installer,
        None
    );
    let mut bad = rel("v99.0.0");
    bad["assets"] = json!([5, null, "x"]);
    assert_eq!(
        newest_release(&payload(&[bad.clone()])).unwrap().installer,
        None
    );
    bad["assets"] = json!("x");
    assert_eq!(newest_release(&payload(&[bad])).unwrap().installer, None);
}

fn leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        any::<f64>()
            .prop_filter("finito", |f| f.is_finite())
            .prop_map(Value::from),
        Just(json!(u64::MAX)),
        prop::sample::select(vec![
            "v1.2.3",
            "99.0.0",
            "JASE-99.0.0-setup.exe",
            "JASE-99.0.0-win64.zip",
            DL,
            "https://github.com/JotaJazz/JASE/releases/tag/v1",
            "sha256:",
            "",
        ])
        .prop_map(Value::from),
        "\\PC{0,12}".prop_map(Value::from),
    ]
}

fn item() -> impl Strategy<Value = Value> {
    let keys = prop::sample::select(vec![
        "tag_name",
        "draft",
        "html_url",
        "assets",
        "name",
        "size",
        "digest",
        "browser_download_url",
    ]);
    leaf().prop_recursive(3, 40, 6, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::from),
            prop::collection::vec((keys.clone(), inner), 0..8).prop_map(|kv| {
                Value::Object(kv.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
            }),
        ]
    })
}

proptest! {
    #[test]
    fn newest_release_never_panics(items in prop::collection::vec(item(), 0..5),
                                   raw in prop::collection::vec(any::<u8>(), 0..64)) {
        let _ = newest_release(&payload(&items));
        let _ = newest_release(&raw);
    }
}

// ------------------------------------------------------------ descarga
#[test]
fn download_ok() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("up");
    let mut seen = Vec::new();
    let path = download_installer(
        &asset(),
        &dir,
        stream(body()),
        &mut |d, n| seen.push((d, n)),
        &|| false,
    )
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), body());
    assert_eq!(path, dir.join(asset().name));
    let len = body().len() as u64;
    assert_eq!(seen.last(), Some(&(len, len)));
    assert_eq!(entries(&dir), vec![asset().name]);
}

#[test]
fn download_bad_hash_deletes() {
    let t = tempfile::tempdir().unwrap();
    assert!(download(t.path(), vec![b'x'; body().len()], false).is_err());
    assert!(entries(t.path()).is_empty());
}

#[test]
fn download_wrong_size_deletes() {
    let t = tempfile::tempdir().unwrap();
    let mut short = body();
    short.pop();
    assert!(download(t.path(), short, false).is_err());
    let mut long = body();
    long.push(b'x');
    assert!(download(t.path(), long, false).is_err());
    assert!(entries(t.path()).is_empty());
}

#[test]
fn download_cancel_and_error() {
    let t = tempfile::tempdir().unwrap();
    let err = download(t.path(), body(), true).unwrap_err();
    assert_eq!(err.code, DownloadCode::Cancelled);
    let boom = |_: &str| -> Result<Box<dyn Read + Send>, DownloadError> {
        Err(DownloadError {
            code: DownloadCode::Network,
            detail: "sin red".into(),
        })
    };
    assert!(download_installer(&asset(), t.path(), boom, &mut |_, _| {}, &|| false).is_err());
    assert!(entries(t.path()).is_empty());
}

#[test]
fn https_only_redirect() {
    let err = default_fetch_stream("http://github.com/x").err().unwrap();
    assert_eq!(err.code, DownloadCode::Insecure);
}

#[test]
fn https_only_stream_rejects_http() {
    // El agente rechaza `http://` antes de conectar (lo mismo hace en cada redirección).
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/x", listener.local_addr().unwrap());
    let err = stream_with_agent(&url).err().unwrap();
    assert_eq!(err.code, DownloadCode::Insecure);
    assert_eq!(
        listener.accept().err().map(|e| e.kind()),
        Some(io::ErrorKind::WouldBlock)
    );
}

#[test]
fn download_error_codes() {
    let mut short = body();
    short.pop();
    let mut long = body();
    long.push(b'x');
    for (data, code, s) in [
        (vec![b'x'; body().len()], DownloadCode::Hash, "hash"),
        (short, DownloadCode::Size, "size"),
        (long, DownloadCode::TooMuch, "too_much"),
    ] {
        let t = tempfile::tempdir().unwrap();
        let err = download(t.path(), data, false).unwrap_err();
        assert_eq!(err.code, code);
        assert_eq!(err.code.as_str(), s);
        assert!(entries(t.path()).is_empty());
    }
}

/// Entrega `ok` bytes y después falla, como una conexión que se corta a mitad.
struct Broken(usize);

impl Read for Broken {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.0 == 0 {
            return Err(io::Error::new(io::ErrorKind::ConnectionReset, "cortada"));
        }
        let n = self.0.min(buf.len());
        buf[..n].fill(b'x');
        self.0 -= n;
        Ok(n)
    }
}

#[test]
fn download_error_codes_network_cancel_insecure() {
    let t = tempfile::tempdir().unwrap();
    let no_net = |_: &str| -> Result<Box<dyn Read + Send>, DownloadError> {
        Err(DownloadError {
            code: DownloadCode::Network,
            detail: "sin red".into(),
        })
    };
    let err =
        download_installer(&asset(), t.path(), no_net, &mut |_, _| {}, &|| false).unwrap_err();
    assert_eq!(err.code, DownloadCode::Network);
    assert_eq!(err.code.as_str(), "network");

    let broken =
        |_: &str| -> Result<Box<dyn Read + Send>, DownloadError> { Ok(Box::new(Broken(50_000))) };
    let mut seen = 0;
    let err = download_installer(&asset(), t.path(), broken, &mut |d, _| seen = d, &|| false)
        .unwrap_err();
    assert_eq!(err.code, DownloadCode::Network);
    assert_eq!(seen, 50_000);
    assert!(entries(t.path()).is_empty());

    // Cancelar a mitad: ya hay `.part` escrito y se borra.
    let calls = std::cell::Cell::new(0);
    let err = download_installer(&asset(), t.path(), stream(body()), &mut |_, _| {}, &|| {
        calls.set(calls.get() + 1);
        calls.get() > 1
    })
    .unwrap_err();
    assert_eq!(err.code, DownloadCode::Cancelled);
    assert_eq!(err.code.as_str(), "cancelled");
    assert!(entries(t.path()).is_empty());

    let insecure = |_: &str| -> Result<Box<dyn Read + Send>, DownloadError> {
        Err(DownloadError {
            code: DownloadCode::Insecure,
            detail: "redirección no https".into(),
        })
    };
    let err =
        download_installer(&asset(), t.path(), insecure, &mut |_, _| {}, &|| false).unwrap_err();
    assert_eq!(err.code, DownloadCode::Insecure);

    let http = Asset {
        url: "http://x/y".into(),
        ..asset()
    };
    let err = download_installer(
        &http,
        t.path(),
        default_fetch_stream,
        &mut |_, _| {},
        &|| false,
    )
    .unwrap_err();
    assert_eq!(err.code, DownloadCode::Insecure);
    assert_eq!(err.code.as_str(), "insecure");
    assert!(entries(t.path()).is_empty());
}

#[test]
fn download_error_display_falls_back_to_code() {
    let e = DownloadError {
        code: DownloadCode::Hash,
        detail: String::new(),
    };
    assert_eq!(e.to_string(), "hash");
    assert_eq!(app_update::CHUNK_BYTES, 64 * 1024);
}
