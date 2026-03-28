//! Test: cookies from Set-Cookie headers on the page response must be sent
//! on subsequent script fetch requests.
//!
//! This reproduces the Anubis bug: the page response sets an auth cookie via
//! Set-Cookie, but script fetches go out without any Cookie header, causing
//! the CDN/proxy to reject them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use braille_engine::{Engine, FetchProvider};
use braille_wire::{FetchOutcome, FetchRequest, FetchResponseData, FetchResult, SnapMode};

/// A FetchProvider that returns canned responses AND records every request's headers.
struct SpyFetcher {
    responses: HashMap<String, FetchResponseData>,
    /// Every request that came through, keyed by URL.
    recorded_requests: Arc<Mutex<Vec<(String, Vec<(String, String)>)>>>,
}

impl SpyFetcher {
    fn new() -> Self {
        SpyFetcher {
            responses: HashMap::new(),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_response(&mut self, url: &str, body: &str, headers: Vec<(String, String)>) {
        self.responses.insert(
            url.to_string(),
            FetchResponseData {
                status: 200,
                status_text: "OK".into(),
                headers,
                body: body.into(),
                url: url.to_string(),
            },
        );
    }

    fn recorded(&self) -> Vec<(String, Vec<(String, String)>)> {
        self.recorded_requests.lock().unwrap().clone()
    }
}

impl FetchProvider for SpyFetcher {
    fn fetch_batch(&mut self, requests: Vec<FetchRequest>) -> Vec<FetchResult> {
        let mut recorded = self.recorded_requests.lock().unwrap();
        requests
            .into_iter()
            .map(|r| {
                recorded.push((r.url.clone(), r.headers.clone()));
                let outcome = match self.responses.get(&r.url) {
                    Some(data) => FetchOutcome::Ok(data.clone()),
                    None => FetchOutcome::Err(format!("SpyFetcher: no response for {}", r.url)),
                };
                FetchResult {
                    id: r.id,
                    outcome,
                }
            })
            .collect()
    }
}

#[test]
fn script_fetches_include_cookies_from_page_response() {
    // Page HTML references an external script
    let html = r#"<!doctype html>
<html><head>
<script src="https://example.com/app.js"></script>
</head><body><p>Hello</p></body></html>"#;

    let mut fetcher = SpyFetcher::new();

    // Page response sets a cookie via Set-Cookie header
    fetcher.add_response(
        "https://example.com/",
        html,
        vec![
            ("content-type".into(), "text/html".into()),
            ("set-cookie".into(), "auth=jwt123; Path=/".into()),
        ],
    );

    // Script response (just needs to exist)
    fetcher.add_response(
        "https://example.com/app.js",
        "console.log('loaded');",
        vec![("content-type".into(), "application/javascript".into())],
    );

    let mut engine = Engine::new();
    let _snapshot = engine.navigate("https://example.com/", &mut fetcher, SnapMode::Compact);

    // Find the script fetch request
    let requests = fetcher.recorded();
    let script_req = requests
        .iter()
        .find(|(url, _)| url == "https://example.com/app.js")
        .expect("should have fetched app.js");

    // The script fetch MUST include the Cookie header with the auth cookie
    let cookie_header = script_req
        .1
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Cookie"));

    assert!(
        cookie_header.is_some(),
        "script fetch should include Cookie header, but headers were: {:?}",
        script_req.1
    );

    let cookie_value = &cookie_header.unwrap().1;
    assert!(
        cookie_value.contains("auth=jwt123"),
        "Cookie header should contain 'auth=jwt123', got: {cookie_value}"
    );
}

#[test]
fn script_fetches_with_relative_urls_include_cookies() {
    // Page HTML references a script with a relative URL — the real-world pattern.
    // Docusaurus/Anubis uses <script src="/assets/js/main.js">.
    let html = r#"<!doctype html>
<html><head>
<script src="/assets/js/app.js"></script>
</head><body><p>Hello</p></body></html>"#;

    let mut fetcher = SpyFetcher::new();

    // Page response sets an auth cookie
    fetcher.add_response(
        "https://example.com/",
        html,
        vec![
            ("content-type".into(), "text/html".into()),
            ("set-cookie".into(), "auth=jwt123; Path=/".into()),
        ],
    );

    // Script response — note: the engine will request this with the relative URL
    // "/assets/js/app.js" since that's what's in the HTML src attribute.
    // The host resolves it to absolute, but the engine must still attach cookies.
    fetcher.add_response(
        "/assets/js/app.js",
        "console.log('loaded');",
        vec![("content-type".into(), "application/javascript".into())],
    );

    let mut engine = Engine::new();
    let _snapshot = engine.navigate("https://example.com/", &mut fetcher, SnapMode::Compact);

    // Find the script fetch request
    let requests = fetcher.recorded();
    let script_req = requests
        .iter()
        .find(|(url, _)| url.contains("app.js"))
        .expect("should have fetched app.js");

    // The script fetch MUST include the Cookie header even with a relative URL
    let cookie_header = script_req
        .1
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Cookie"));

    assert!(
        cookie_header.is_some(),
        "script fetch for relative URL '{}' should include Cookie header, but headers were: {:?}",
        script_req.0,
        script_req.1
    );

    let cookie_value = &cookie_header.unwrap().1;
    assert!(
        cookie_value.contains("auth=jwt123"),
        "Cookie header should contain 'auth=jwt123', got: {cookie_value}"
    );
}
