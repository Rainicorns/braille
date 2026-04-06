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
fn secure_cookie_not_sent_over_http() {
    let mut e = Engine::new();
    // Inject a Secure cookie from an HTTPS URL
    e.inject_response_cookies("https://example.com/login", &[
        ("Set-Cookie".to_string(), "token=secret; Path=/; Secure".to_string()),
        ("Set-Cookie".to_string(), "theme=dark; Path=/".to_string()),
    ]);
    e.load_html("<html><body></body></html>");

    // Requesting over HTTPS should include both cookies
    let https_cookies = e.get_cookies_for_url("https://example.com/api");
    assert!(https_cookies.contains("token=secret"), "Secure cookie should be sent over HTTPS: {}", https_cookies);
    assert!(https_cookies.contains("theme=dark"), "Non-secure cookie should be sent over HTTPS: {}", https_cookies);

    // Requesting over HTTP should NOT include the Secure cookie
    let http_cookies = e.get_cookies_for_url("http://example.com/api");
    assert!(!http_cookies.contains("token=secret"), "Secure cookie must NOT be sent over HTTP: {}", http_cookies);
    assert!(http_cookies.contains("theme=dark"), "Non-secure cookie should still be sent over HTTP: {}", http_cookies);
}

#[test]
fn cookie_expiry_uses_virtual_time() {
    let mut e = Engine::new();
    // Load HTML first so the runtime exists and virtual time is used
    e.load_html("<html><body></body></html>");

    // Virtual time starts at 0. Set a cookie with Max-Age=1 (expires at virtual time 1000ms)
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "sess=abc; Path=/; Max-Age=1".to_string()),
    ]);

    // Cookie should be present immediately (virtual time is 0, expiry is 1000)
    let cookies = e.get_cookies_for_url("https://example.com/api");
    assert!(cookies.contains("sess=abc"), "cookie should be present before expiry: {}", cookies);

    // Advance virtual clock past expiry
    e.set_virtual_time_ms(2000);

    let cookies = e.get_cookies_for_url("https://example.com/api");
    assert!(!cookies.contains("sess=abc"), "cookie should be expired after advancing virtual time: {}", cookies);
}

#[test]
fn export_cookies_returns_http_jar_contents() {
    let mut e = Engine::new();
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "session=abc123; Path=/; HttpOnly; Secure".to_string()),
        ("Set-Cookie".to_string(), "theme=dark; Path=/".to_string()),
    ]);
    e.load_html("<html><body></body></html>");

    let exported = e.export_cookies();
    assert_eq!(exported.len(), 2);

    let session_cookie = exported.iter().find(|c| c.name == "session").unwrap();
    assert_eq!(session_cookie.value, "abc123");
    assert_eq!(session_cookie.domain, "example.com");
    assert_eq!(session_cookie.path, "/");
    assert!(session_cookie.http_only);
    assert!(session_cookie.secure);

    let theme_cookie = exported.iter().find(|c| c.name == "theme").unwrap();
    assert_eq!(theme_cookie.value, "dark");
    assert!(!theme_cookie.http_only);
    assert!(!theme_cookie.secure);
}

#[test]
fn import_cookies_populates_jar_and_syncs_to_js() {
    let mut e = Engine::new();
    e.load_html("<html><body></body></html>");

    let cookies = vec![
        braille_wire::SerializableCookie {
            name: "auth".into(),
            value: "jwt_token".into(),
            domain: "example.com".into(),
            path: "/".into(),
            http_only: true,
            secure: true,
            expires_ms: None,
        },
        braille_wire::SerializableCookie {
            name: "pref".into(),
            value: "en-US".into(),
            domain: "example.com".into(),
            path: "/".into(),
            http_only: false,
            secure: false,
            expires_ms: None,
        },
    ];
    e.import_cookies(cookies);

    // HttpOnly cookie should be in HTTP requests
    let http = e.get_cookies_for_url("https://example.com/api");
    assert!(http.contains("auth=jwt_token"), "HTTP should have HttpOnly cookie: {}", http);
    assert!(http.contains("pref=en-US"), "HTTP should have non-HttpOnly cookie: {}", http);

    // Non-HttpOnly should be visible in JS
    let js = e.eval_js("document.cookie").unwrap();
    assert!(js.contains("pref=en-US"), "JS should see non-HttpOnly cookie: {}", js);
    assert!(!js.contains("auth"), "JS should NOT see HttpOnly cookie: {}", js);
}

#[test]
fn export_import_roundtrip_preserves_cookies() {
    let mut e1 = Engine::new();
    e1.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "a=1; Path=/; HttpOnly".to_string()),
        ("Set-Cookie".to_string(), "b=2; Path=/; Secure".to_string()),
        ("Set-Cookie".to_string(), "c=3; Path=/app; Domain=sub.example.com".to_string()),
    ]);

    let exported = e1.export_cookies();
    assert_eq!(exported.len(), 3);

    // Import into a fresh engine
    let mut e2 = Engine::new();
    e2.load_html("<html><body></body></html>");
    e2.import_cookies(exported);

    let http = e2.get_cookies_for_url("https://example.com/");
    assert!(http.contains("a=1"), "should have cookie a: {}", http);
    assert!(http.contains("b=2"), "should have cookie b: {}", http);
}

#[test]
fn import_cookies_replaces_existing_same_identity() {
    let mut e = Engine::new();
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "token=old_value; Path=/".to_string()),
    ]);

    let cookies = vec![braille_wire::SerializableCookie {
        name: "token".into(),
        value: "new_value".into(),
        domain: "example.com".into(),
        path: "/".into(),
        http_only: false,
        secure: false,
        expires_ms: None,
    }];
    e.import_cookies(cookies);

    let exported = e.export_cookies();
    let token_cookies: Vec<_> = exported.iter().filter(|c| c.name == "token").collect();
    assert_eq!(token_cookies.len(), 1, "should have exactly one token cookie");
    assert_eq!(token_cookies[0].value, "new_value");
}

#[test]
fn export_cookies_empty_jar() {
    let e = Engine::new();
    let exported = e.export_cookies();
    assert!(exported.is_empty());
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
