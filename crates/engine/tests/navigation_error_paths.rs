//! Tests for navigation error paths and edge cases.
//!
//! Verifies redirect depth limiting, fetch failures during navigation,
//! cookie attachment across redirects, and the settle-with-fetches loop.
//! Uses MockFetcher with canned responses to exercise the full navigate() path.

use braille_engine::navigation::MockFetcher;
use braille_engine::Engine;
use braille_wire::SnapMode;

// ===========================================================================
// Redirect Depth Limiting
// ===========================================================================

#[test]
fn navigate_follows_meta_refresh_redirect() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();

    fetcher.add_html(
        "https://example.com/old",
        r#"<html><head><meta http-equiv="refresh" content="0; url=https://example.com/new"></head><body>Redirecting...</body></html>"#,
    );
    fetcher.add_html(
        "https://example.com/new",
        "<html><body><h1>New Page</h1></body></html>",
    );

    let result = e.navigate("https://example.com/old", &mut fetcher, SnapMode::Text);
    assert!(result.is_ok(), "navigate should succeed: {:?}", result.err());
    let snap = result.unwrap();
    assert!(snap.contains("New Page"), "should have followed redirect to new page: {}", snap);
}

#[test]
fn navigate_stops_at_redirect_depth_limit() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();

    // Create a chain of 7 redirects (limit is 5)
    for i in 0..7 {
        let url = format!("https://example.com/page{}", i);
        let next = format!("https://example.com/page{}", i + 1);
        fetcher.add_html(
            &url,
            &format!(
                r#"<html><head><meta http-equiv="refresh" content="0; url={}"></head><body>Redirect {}</body></html>"#,
                next, i
            ),
        );
    }
    fetcher.add_html(
        "https://example.com/page7",
        "<html><body>Final</body></html>",
    );

    let result = e.navigate("https://example.com/page0", &mut fetcher, SnapMode::Text);
    // Should error due to too many redirects
    assert!(result.is_err(), "should fail with too many redirects");
    let err = result.unwrap_err();
    assert!(err.contains("too many"), "error should mention redirect limit: {}", err);
}

// ===========================================================================
// Fetch Failures
// ===========================================================================

#[test]
fn navigate_to_missing_url_returns_error() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();
    // Don't add any response — MockFetcher will return an error

    let result = e.navigate("https://nonexistent.example.com/", &mut fetcher, SnapMode::Text);
    assert!(result.is_err(), "should fail for missing URL");
}

#[test]
fn navigate_with_failed_script_fetch_still_renders() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();

    // Page references an external script that doesn't exist in our mock
    fetcher.add_html(
        "https://example.com/",
        r#"<html><body>
            <h1>Hello</h1>
            <script src="https://cdn.example.com/app.js"></script>
            <p>Content loads anyway</p>
        </body></html>"#,
    );

    let result = e.navigate("https://example.com/", &mut fetcher, SnapMode::Text);
    assert!(result.is_ok(), "page should still render despite missing script");
    let snap = result.unwrap();
    assert!(snap.contains("Hello"), "static content should be present: {}", snap);
}

// ===========================================================================
// HTTP Refresh Header
// ===========================================================================

#[test]
fn navigate_follows_http_refresh_header() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();

    fetcher.add_with_headers(
        "https://example.com/gate",
        "<html><body>Checking...</body></html>",
        vec![
            ("content-type".into(), "text/html".into()),
            ("Refresh".into(), "0; url=https://example.com/dashboard".into()),
        ],
    );
    fetcher.add_html(
        "https://example.com/dashboard",
        "<html><body><h1>Dashboard</h1></body></html>",
    );

    let result = e.navigate("https://example.com/gate", &mut fetcher, SnapMode::Text);
    assert!(result.is_ok());
    let snap = result.unwrap();
    assert!(snap.contains("Dashboard"), "should follow HTTP Refresh header: {}", snap);
}

// ===========================================================================
// Cookie Attachment During Navigation
// ===========================================================================

#[test]
fn navigate_attaches_cookies_to_requests() {
    let mut e = Engine::new();

    // Pre-set a cookie
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "auth=token123; Path=/".to_string()),
    ]);

    let mut fetcher = MockFetcher::new();
    fetcher.add_html(
        "https://example.com/dashboard",
        "<html><body><h1>Dashboard</h1></body></html>",
    );

    let result = e.navigate("https://example.com/dashboard", &mut fetcher, SnapMode::Text);
    assert!(result.is_ok());
    // The MockFetcher doesn't verify headers, but the engine's fetch_page
    // should have called get_cookies_for_url and attached the Cookie header.
    // This test at minimum verifies the navigation doesn't crash with pre-set cookies.
}

#[test]
fn navigate_stores_response_cookies() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();

    fetcher.add_with_headers(
        "https://example.com/login",
        "<html><body>Logged in</body></html>",
        vec![
            ("content-type".into(), "text/html".into()),
            ("Set-Cookie".into(), "session=newtoken; Path=/; HttpOnly".into()),
        ],
    );

    let result = e.navigate("https://example.com/login", &mut fetcher, SnapMode::Text);
    assert!(result.is_ok());

    // The session cookie should now be in the jar
    let cookies = e.get_cookies_for_url("https://example.com/api");
    assert!(cookies.contains("session=newtoken"),
        "Set-Cookie from navigation response should be stored: {}", cookies);
}

// ===========================================================================
// JS-Driven Navigation (location.href)
// ===========================================================================

#[test]
fn navigate_follows_js_location_redirect() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();

    fetcher.add_html(
        "https://example.com/redirect",
        r#"<html><body>
            <script>location.href = "https://example.com/target";</script>
        </body></html>"#,
    );
    fetcher.add_html(
        "https://example.com/target",
        "<html><body><h1>Target Page</h1></body></html>",
    );

    let result = e.navigate("https://example.com/redirect", &mut fetcher, SnapMode::Text);
    assert!(result.is_ok());
    let snap = result.unwrap();
    assert!(snap.contains("Target Page"), "should follow JS location.href redirect: {}", snap);
}

// ===========================================================================
// Snapshot Modes During Navigation
// ===========================================================================

#[test]
fn navigate_returns_requested_snap_mode() {
    let mut e = Engine::new();
    let mut fetcher = MockFetcher::new();

    fetcher.add_html(
        "https://example.com/",
        r#"<html><body>
            <h1>Title</h1>
            <a href="/about">About</a>
            <form><input type="text" name="q"></form>
        </body></html>"#,
    );

    // Test with different snap modes
    let compact = e.navigate("https://example.com/", &mut fetcher, SnapMode::Compact);
    assert!(compact.is_ok(), "Compact snap should work");

    let text = e.navigate("https://example.com/", &mut fetcher, SnapMode::Text);
    assert!(text.is_ok(), "Text snap should work");

    let headings = e.navigate("https://example.com/", &mut fetcher, SnapMode::Headings);
    assert!(headings.is_ok(), "Headings snap should work");
}
