//! Port de los tests de tests/test_steamweb.py que tocan build_url y parse_response.
mod support;

use jase_core::models::ServerAddress;
use jase_core::protocols::steamweb;
use support::fixture;

#[test]
fn parse_server_list_reads_addr() {
    let (servers, entries) = steamweb::parse_response(&fixture("steamweb_l4d2.json")).unwrap();
    assert_eq!(servers.len(), 20);
    assert!(servers.iter().all(|s| s.port > 0));
    assert_eq!(entries, 20);
    // Valores de la versión Python sobre el mismo fixture.
    assert_eq!(servers[0], ServerAddress::new("155.133.249.205", 27177));
    assert_eq!(servers[19], ServerAddress::new("155.133.249.206", 27115));
}

#[test]
fn parse_server_list_empty_response() {
    assert!(
        steamweb::parse_response(br#"{"response": {}}"#)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn parse_server_list_rejects_garbage() {
    assert!(steamweb::parse_response(b"<html>").is_err());
}

#[test]
fn parse_server_list_rejects_missing_response() {
    assert!(steamweb::parse_response(br#"{"foo": 1}"#).is_err());
    assert!(steamweb::parse_response(br#"{"response": []}"#).is_err());
    assert!(steamweb::parse_response(b"[1]").is_err());
}

#[test]
fn build_url_escapes_filter() {
    let url =
        steamweb::build_url("api.steampowered.com", "K", "\\appid\\550\\empty\\1", 20000).unwrap();
    assert!(url.contains("filter=%5Cappid%5C550%5Cempty%5C1") && url.contains("limit=20000"));
}

#[test]
fn build_url_matches_python_urlencode() {
    // Salida de la versión Python con los mismos argumentos.
    let url =
        steamweb::build_url("API.steampowered.com", "k y/ñ", "\\appid\\550 ~x", 20000).unwrap();
    assert_eq!(
        url,
        "https://api.steampowered.com/IGameServersService/GetServerList/v1/?key=k+y%2F%C3%B1&filter=%5Cappid%5C550+~x&limit=20000"
    );
}

#[test]
fn build_url_rejects_custom_host() {
    assert!(steamweb::build_url("evil.example.com", "K", "\\appid\\550", 20000).is_err());
}

#[test]
fn parse_server_list_deeply_nested_is_protocol_error() {
    assert!(steamweb::parse_response(&b"[".repeat(100_000)).is_err());
}

#[test]
fn parse_skips_bad_entries_and_dedups_like_python() {
    let data = r#"{"response":{"servers":[{"addr":"1.2.3.4:5"},{"addr":"1.2.3.4:5"},{"addr":5},"x",{"addr":"h:0"},{"addr":"h:65536"},{"addr":"h:²"}]}}"#;
    assert_eq!(
        steamweb::parse_response(data.as_bytes()).unwrap(),
        (vec![ServerAddress::new("1.2.3.4", 5)], 7)
    );
    assert_eq!(
        steamweb::parse_response(br#"{"response":{"servers":{"a":1}}}"#).unwrap(),
        (vec![], 0)
    );
}
