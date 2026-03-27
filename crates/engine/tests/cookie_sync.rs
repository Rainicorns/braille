//! Tests for document.cookie ↔ HTTP cookie jar synchronization.

use braille_engine::Engine;

fn engine_with_html(html: &str) -> Engine {
    let mut e = Engine::new();
    e.load_html(html);
    e
}

#[test]
fn document_cookie_read_write() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"document.cookie = "a=1""#).unwrap();
    let result = e.eval_js("document.cookie").unwrap();
    assert_eq!(result, "a=1");
}

#[test]
fn document_cookie_multiple() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"document.cookie = "a=1""#).unwrap();
    e.eval_js(r#"document.cookie = "b=2""#).unwrap();
    let result = e.eval_js("document.cookie").unwrap();
    assert!(result.contains("a=1"), "should contain a=1: {}", result);
    assert!(result.contains("b=2"), "should contain b=2: {}", result);
}

#[test]
fn document_cookie_overwrite() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"document.cookie = "a=1""#).unwrap();
    e.eval_js(r#"document.cookie = "a=2""#).unwrap();
    let result = e.eval_js("document.cookie").unwrap();
    assert_eq!(result, "a=2");
}

#[test]
fn document_cookie_from_http_headers() {
    let mut e = Engine::new();
    e.inject_response_cookies("https://example.com/page", &[
        ("Set-Cookie".to_string(), "session=abc123; Path=/".to_string()),
        ("Set-Cookie".to_string(), "theme=dark".to_string()),
    ]);
    e.load_html("<html><body></body></html>");

    let result = e.eval_js("document.cookie").unwrap();
    assert!(result.contains("session=abc123"), "should contain session cookie: {}", result);
    assert!(result.contains("theme=dark"), "should contain theme cookie: {}", result);
}

#[test]
fn document_cookie_sent_in_get_cookies() {
    let mut e = engine_with_html("<html><body></body></html>");

    // Set a cookie via JS
    e.eval_js(r#"document.cookie = "token=xyz""#).unwrap();

    // Also inject one via HTTP
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "sid=server123; Path=/".to_string()),
    ]);

    let cookies = e.get_cookies_for_url("https://example.com/api");
    assert!(cookies.contains("sid=server123"), "should contain HTTP cookie: {}", cookies);
    assert!(cookies.contains("token=xyz"), "should contain JS cookie: {}", cookies);
}

#[test]
fn document_cookie_httponly_not_readable_in_js() {
    let mut e = Engine::new();
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "visible=yes; Path=/".to_string()),
        ("Set-Cookie".to_string(), "secret=hidden; Path=/; HttpOnly".to_string()),
    ]);
    e.load_html("<html><body></body></html>");

    // JS should see visible but not secret
    let js_cookies = e.eval_js("document.cookie").unwrap();
    assert!(js_cookies.contains("visible=yes"), "should see non-HttpOnly: {}", js_cookies);
    assert!(!js_cookies.contains("secret"), "should NOT see HttpOnly in JS: {}", js_cookies);

    // But HTTP requests should include both
    let http_cookies = e.get_cookies_for_url("https://example.com/api");
    assert!(http_cookies.contains("visible=yes"), "HTTP should have visible: {}", http_cookies);
    assert!(http_cookies.contains("secret=hidden"), "HTTP should have HttpOnly: {}", http_cookies);
}

#[test]
fn document_cookie_delete_via_max_age_zero() {
    let mut e = engine_with_html("<html><body></body></html>");
    e.eval_js(r#"document.cookie = "temp=value""#).unwrap();

    let before = e.eval_js("document.cookie").unwrap();
    assert!(before.contains("temp=value"), "cookie should exist: {}", before);

    // Delete by setting max-age=0
    e.eval_js(r#"document.cookie = "temp=; max-age=0""#).unwrap();

    let after = e.eval_js("document.cookie").unwrap();
    assert!(!after.contains("temp=value"), "cookie should be deleted: {}", after);
}
