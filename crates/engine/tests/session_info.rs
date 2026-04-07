//! Tests for the SessionInfo introspection command support.

use braille_engine::Engine;

#[test]
fn cookie_count_empty() {
    let e = Engine::new();
    assert_eq!(e.cookie_count(), 0);
}

#[test]
fn cookie_count_after_inject() {
    let mut e = Engine::new();
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "a=1; Path=/".to_string()),
        ("Set-Cookie".to_string(), "b=2; Path=/".to_string()),
    ]);
    assert_eq!(e.cookie_count(), 2);
}

#[test]
fn cookie_count_after_overwrite() {
    let mut e = Engine::new();
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "a=1; Path=/".to_string()),
    ]);
    e.inject_response_cookies("https://example.com/", &[
        ("Set-Cookie".to_string(), "a=2; Path=/".to_string()),
    ]);
    // Overwriting same cookie should not increase count
    assert_eq!(e.cookie_count(), 1);
}

#[test]
fn get_title_from_dom() {
    let mut e = Engine::new();
    e.load_html("<html><head><title>Test Page</title></head><body></body></html>");
    assert_eq!(e.get_title(), Some("Test Page".to_string()));
}

#[test]
fn get_title_none_when_no_title() {
    let e = Engine::new();
    assert_eq!(e.get_title(), None);
}

#[test]
fn get_title_none_when_empty_title() {
    let mut e = Engine::new();
    e.load_html("<html><head><title></title></head><body></body></html>");
    assert_eq!(e.get_title(), None);
}
