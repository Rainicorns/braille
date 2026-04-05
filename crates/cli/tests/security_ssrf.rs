//! Security tests: SSRF protection — reject fetches to private/internal IPs.
//!
//! These tests verify the SSRF filter blocks requests BEFORE making network calls.
//! We check that the error message contains "private" or "blocked" to distinguish
//! SSRF rejections from incidental network errors (timeout, connection refused).

use braille_cli::network::NetworkClient;

fn assert_ssrf_blocked(result: Result<braille_cli::network::FetchResponse, String>, desc: &str) {
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("{} should be rejected by SSRF filter", desc),
    };
    assert!(
        err.to_lowercase().contains("private") || err.to_lowercase().contains("blocked"),
        "{}: error should be an SSRF block, not a network error: {}",
        desc,
        err
    );
}

#[test]
fn fetch_rejects_private_ipv4_192_168() {
    let mut client = NetworkClient::new();
    assert_ssrf_blocked(client.fetch("http://192.168.1.1/"), "192.168.x.x");
}

#[test]
fn fetch_rejects_private_ipv4_10() {
    let mut client = NetworkClient::new();
    assert_ssrf_blocked(client.fetch("http://10.0.0.1/"), "10.x.x.x");
}

#[test]
fn fetch_rejects_private_ipv4_172_16() {
    let mut client = NetworkClient::new();
    assert_ssrf_blocked(client.fetch("http://172.16.0.1/"), "172.16.x.x");
}

#[test]
fn fetch_rejects_link_local() {
    let mut client = NetworkClient::new();
    assert_ssrf_blocked(
        client.fetch("http://169.254.169.254/latest/meta-data/"),
        "link-local 169.254.x.x",
    );
}

#[test]
fn fetch_rejects_localhost_ip() {
    let mut client = NetworkClient::new();
    assert_ssrf_blocked(client.fetch("http://127.0.0.1/"), "127.0.0.1");
}

#[test]
fn fetch_rejects_localhost_name() {
    let mut client = NetworkClient::new();
    assert_ssrf_blocked(client.fetch("http://localhost/"), "localhost");
}

#[test]
fn fetch_with_options_rejects_private_ip() {
    let mut client = NetworkClient::new();
    assert_ssrf_blocked(
        client.fetch_with_options("http://169.254.169.254/", "GET", &[], None),
        "fetch_with_options link-local",
    );
}

#[test]
fn fetch_allows_public_ip() {
    // Verify public IPs are NOT rejected by the SSRF filter.
    // Connection may fail for network reasons, but the error should NOT be an SSRF block.
    let mut client = NetworkClient::new();
    let result = client.fetch("http://93.184.216.34/");
    if let Err(e) = result {
        let lower = e.to_lowercase();
        assert!(
            !lower.contains("private") && !lower.contains("blocked"),
            "public IP should not be SSRF-blocked: {}",
            e
        );
    }
}
