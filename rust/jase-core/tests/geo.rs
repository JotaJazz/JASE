use std::cell::RefCell;
use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use jase_core::geo::{
    CountryDB, current_month, download_db, install_with_limit, ipv4_int, load_country_db,
    month_of_unix_days,
};

const CSV: &str = "\
0.0.0.0,0.255.255.255,ZZ
1.0.0.0,1.0.0.255,AU
1.0.1.0,1.0.3.255,CN
2001:200::,2001:200:ffff:ffff:ffff:ffff:ffff:ffff,JP
1.0.4.0,1.0.7.255,AU
190.0.0.0,190.0.255.255,CL
200.0.0.0,200.0.0.255,BR
";

fn gz(data: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn url(month: &str) -> String {
    format!("https://download.db-ip.com/free/dbip-country-lite-{month}.csv.gz")
}

fn db_at(dir: &Path, text: &str) -> std::path::PathBuf {
    let p = dir.join("dbip-country-lite.csv");
    std::fs::write(&p, text).unwrap();
    p
}

/// Fetch falso: responde solo las URL dadas y registra las llamadas.
struct Fake {
    responses: Vec<(String, Vec<u8>)>,
    calls: RefCell<Vec<String>>,
}

impl Fake {
    fn new(responses: Vec<(String, Vec<u8>)>) -> Self {
        Fake {
            responses,
            calls: RefCell::new(vec![]),
        }
    }
    fn fetch(&self, u: &str) -> Option<Vec<u8>> {
        self.calls.borrow_mut().push(u.to_owned());
        self.responses
            .iter()
            .find(|(k, _)| k == u)
            .map(|(_, v)| v.clone())
    }
}

#[test]
fn lookup_contiguous_ranges() {
    let d = tempfile::tempdir().unwrap();
    let db = CountryDB::from_csv(&db_at(d.path(), CSV)).unwrap();
    assert_eq!(db.lookup("1.0.0.0"), Some("au"));
    assert_eq!(db.lookup("1.0.0.255"), Some("au"));
    assert_eq!(db.lookup("1.0.1.0"), Some("cn"));
    assert_eq!(db.lookup("1.0.3.255"), Some("cn"));
    assert_eq!(db.lookup("1.0.4.0"), Some("au"));
    assert_eq!(db.lookup("190.0.12.34"), Some("cl"));
    assert_eq!(db.lookup("200.0.0.255"), Some("br"));
}

#[test]
fn lookup_outside_ranges_and_unknown() {
    let d = tempfile::tempdir().unwrap();
    let db = CountryDB::from_csv(&db_at(d.path(), CSV)).unwrap();
    assert_eq!(db.lookup("1.0.8.0"), None);
    assert_eq!(db.lookup("200.0.1.0"), None);
    assert_eq!(db.lookup("255.255.255.255"), None);
    assert_eq!(db.lookup("0.1.2.3"), None);
}

#[test]
fn lookup_invalid_ip_returns_none() {
    let d = tempfile::tempdir().unwrap();
    let db = CountryDB::from_csv(&db_at(d.path(), CSV)).unwrap();
    for ip in [
        "",
        "abc",
        "1.2.3",
        "1.2.3.256",
        "::1",
        "2001:200::1",
        "example.com",
        "1.2.3.4 ",
        "1.2.3.+4",
        "1.2.3.1234",
    ] {
        assert_eq!(db.lookup(ip), None, "{ip:?}");
    }
    assert_eq!(ipv4_int("1.2.3.4"), Some(0x0102_0304));
}

#[test]
fn ipv6_rows_are_skipped() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(CountryDB::from_csv(&db_at(d.path(), CSV)).unwrap().len(), 5);
}

#[test]
fn malformed_rows_are_skipped_and_unsorted_input_sorted() {
    let d = tempfile::tempdir().unwrap();
    let p = db_at(
        d.path(),
        "200.0.0.0,200.0.0.255,BR\nbasura\n1.2.3,1.2.3.9,XX\n1.0.0.0,1.0.0.255,AU\n5.0.0.0,4.0.0.0,US\n",
    );
    let db = CountryDB::from_csv(&p).unwrap();
    assert_eq!(db.len(), 2);
    assert_eq!(db.lookup("1.0.0.7"), Some("au"));
    assert_eq!(db.lookup("200.0.0.1"), Some("br"));
}

#[test]
fn csv_skips_bad_rows() {
    let d = tempfile::tempdir().unwrap();
    let p = db_at(
        d.path(),
        "::1,::2,US\n1.0.0.0,1.0.0.9\n9.0.0.9,9.0.0.1,US\n3.0.0.0,3.0.0.9,ZZ\n\
         4.0.0.0,4.0.0.9,USA\n5.0.0.0,5.0.0.256,US\n6.0.0.0,6.0.0.9,\"fr\"\r\n7.0.0.0,7.0.0.9,DE\n",
    );
    let db = CountryDB::from_csv(&p).unwrap();
    assert_eq!(db.len(), 2);
    assert_eq!(db.lookup("6.0.0.1"), Some("fr"));
    assert_eq!(db.lookup("7.0.0.1"), Some("de"));
}

#[test]
fn from_csv_survives_non_utf8_and_missing_newlines() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("db.csv");
    let mut data = b"1.0.0.0,1.0.0.255,AU\n\xff\xfe,\x80,\xc3\n".to_vec();
    data.extend(std::iter::repeat_n(b'x', 10_000)); // línea enorme sin salto
    data.extend(b"\n2.0.0.0,2.0.0.9,CN");
    std::fs::write(&p, &data).unwrap();
    let db = CountryDB::from_csv(&p).unwrap();
    assert_eq!(db.len(), 2);
    assert_eq!(db.lookup("2.0.0.1"), Some("cn"));
    std::fs::write(&p, vec![b'a'; 100_000]).unwrap();
    assert_eq!(CountryDB::from_csv(&p).unwrap().len(), 0);
}

#[test]
fn from_csv_missing_file_errors() {
    let d = tempfile::tempdir().unwrap();
    assert!(CountryDB::from_csv(&d.path().join("nope.csv")).is_err());
}

#[test]
fn download_current_month() {
    let d = tempfile::tempdir().unwrap();
    let dest = d.path().join("cache").join("dbip-country-lite.csv");
    let f = Fake::new(vec![(url("2026-10"), gz(CSV.as_bytes()))]);
    assert!(download_db(&dest, (2026, 10), &|u| f.fetch(u)));
    assert_eq!(*f.calls.borrow(), vec![url("2026-10")]);
    assert_eq!(std::fs::read_to_string(&dest).unwrap(), CSV);
    assert_eq!(
        std::fs::read_dir(dest.parent().unwrap()).unwrap().count(),
        1
    );
}

#[test]
fn download_falls_back_to_previous_month() {
    let d = tempfile::tempdir().unwrap();
    let dest = d.path().join("dbip-country-lite.csv");
    let f = Fake::new(vec![(url("2025-12"), gz(CSV.as_bytes()))]);
    assert!(download_db(&dest, (2026, 1), &|u| f.fetch(u)));
    assert_eq!(*f.calls.borrow(), vec![url("2026-01"), url("2025-12")]);
    assert_eq!(
        CountryDB::from_csv(&dest).unwrap().lookup("190.0.0.1"),
        Some("cl")
    );
}

#[test]
fn download_failure_leaves_no_file() {
    for payload in [
        None,
        Some(vec![]),
        Some(b"no es gzip".to_vec()),
        Some(gz(b"<html>404</html>")),
    ] {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("dbip-country-lite.csv");
        let f = Fake::new(
            payload
                .iter()
                .flat_map(|p| [(url("2026-10"), p.clone()), (url("2026-09"), p.clone())])
                .collect(),
        );
        assert!(!download_db(&dest, (2026, 10), &|u| f.fetch(u)));
        assert_eq!(f.calls.borrow().len(), 2);
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
    }
}

#[test]
fn failed_download_keeps_existing_file() {
    let d = tempfile::tempdir().unwrap();
    let p = db_at(d.path(), CSV);
    assert!(!download_db(&p, (2026, 10), &|_| None));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), CSV);
}

#[test]
fn install_rejects_html_error_page() {
    let d = tempfile::tempdir().unwrap();
    let p = db_at(d.path(), CSV);
    assert!(!install_with_limit(&gz(b"<html>404</html>"), &p, 1024));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), CSV);
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
}

#[test]
fn install_rejects_oversize() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("dbip-country-lite.csv");
    assert!(!install_with_limit(&gz(CSV.as_bytes()), &p, 10));
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
    assert!(install_with_limit(
        &gz(CSV.as_bytes()),
        &p,
        CSV.len() as u64
    ));
}

#[test]
fn load_existing_file_without_download() {
    let d = tempfile::tempdir().unwrap();
    let p = db_at(d.path(), CSV);
    let f = Fake::new(vec![]);
    let db = load_country_db(&p, false, (2026, 10), &|u| f.fetch(u)).unwrap();
    assert_eq!(db.lookup("200.0.0.1"), Some("br"));
    assert!(f.calls.borrow().is_empty());
}

#[test]
fn load_downloads_when_missing() {
    let d = tempfile::tempdir().unwrap();
    let dest = d.path().join("cache").join("dbip-country-lite.csv");
    let f = Fake::new(vec![(url("2026-10"), gz(CSV.as_bytes()))]);
    let db = load_country_db(&dest, false, (2026, 10), &|u| f.fetch(u)).unwrap();
    assert_eq!(db.lookup("1.0.1.1"), Some("cn"));
}

#[test]
fn load_download_failure_reports_error() {
    let d = tempfile::tempdir().unwrap();
    let e = load_country_db(&d.path().join("x.csv"), false, (2026, 10), &|_| None).unwrap_err();
    assert!(e.contains("descargar"), "{e}");
}

#[test]
fn load_refresh_failure_keeps_old_file() {
    let d = tempfile::tempdir().unwrap();
    let p = db_at(d.path(), CSV);
    let f = Fake::new(vec![]);
    assert!(load_country_db(&p, true, (2026, 10), &|u| f.fetch(u)).is_err());
    assert_eq!(f.calls.borrow().len(), 2);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), CSV);
}

#[test]
fn current_month_matches_known_dates() {
    assert_eq!(month_of_unix_days(0), (1970, 1));
    assert_eq!(month_of_unix_days(19_723), (2024, 1));
    assert_eq!(month_of_unix_days(20_454), (2026, 1));
    let (y, m) = current_month();
    assert!(y >= 2026 && (1..=12).contains(&m));
}
