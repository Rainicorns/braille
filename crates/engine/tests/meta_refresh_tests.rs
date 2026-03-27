//! Meta refresh detection tests.

use braille_engine::Engine;

#[test]
fn test_meta_refresh_with_url() {
    let html = r#"
    <html><head>
      <meta http-equiv="refresh" content="2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=abc&amp;id=123&amp;redir=%2F">
    </head><body><p>Redirecting...</p></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let refresh = engine.check_meta_refresh(None);

    assert!(refresh.is_some(), "should detect meta refresh");
    let refresh = refresh.unwrap();
    assert_eq!(refresh.delay_seconds, 2);
    assert!(refresh.url.is_some(), "should have a URL");
    assert!(
        refresh.url.as_ref().unwrap().contains("pass-challenge"),
        "URL should contain path: {:?}",
        refresh.url
    );
}

#[test]
fn test_meta_refresh_relative_url_resolution() {
    let html = r#"
    <html><head>
      <meta http-equiv="refresh" content="0; url=/login">
    </head><body></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let refresh = engine.check_meta_refresh(Some("https://example.com/page"));

    assert!(refresh.is_some());
    let refresh = refresh.unwrap();
    assert_eq!(refresh.delay_seconds, 0);
    assert_eq!(refresh.url.as_deref(), Some("https://example.com/login"));
}

#[test]
fn test_meta_refresh_no_url() {
    let html = r#"
    <html><head>
      <meta http-equiv="refresh" content="5">
    </head><body></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let refresh = engine.check_meta_refresh(None);

    assert!(refresh.is_some());
    let refresh = refresh.unwrap();
    assert_eq!(refresh.delay_seconds, 5);
    assert!(refresh.url.is_none(), "should have no URL for plain refresh");
}

#[test]
fn test_meta_refresh_missing_returns_none() {
    let html = r#"
    <html><head>
      <meta charset="utf-8">
    </head><body><p>No refresh here</p></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let refresh = engine.check_meta_refresh(None);

    assert!(refresh.is_none(), "should return None when no meta refresh");
}

#[test]
fn test_meta_refresh_case_insensitive() {
    let html = r#"
    <html><head>
      <meta HTTP-EQUIV="Refresh" content="3; URL=/destination">
    </head><body></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let refresh = engine.check_meta_refresh(Some("https://example.com/"));

    assert!(refresh.is_some());
    let refresh = refresh.unwrap();
    assert_eq!(refresh.delay_seconds, 3);
    assert_eq!(
        refresh.url.as_deref(),
        Some("https://example.com/destination")
    );
}

#[test]
fn test_meta_refresh_absolute_url() {
    let html = r#"
    <html><head>
      <meta http-equiv="refresh" content="0; url=https://other.com/page">
    </head><body></body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);
    let refresh = engine.check_meta_refresh(Some("https://example.com/"));

    assert!(refresh.is_some());
    let refresh = refresh.unwrap();
    assert_eq!(refresh.delay_seconds, 0);
    assert_eq!(
        refresh.url.as_deref(),
        Some("https://other.com/page")
    );
}
