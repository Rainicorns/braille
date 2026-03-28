use braille_engine::{Engine, MockFetcher};
use braille_wire::SnapMode;

#[test]
fn location_href_set_triggers_navigation() {
    let mut fetcher = MockFetcher::new();

    // Page A: JS sets window.location.href to page B
    fetcher.add_html(
        "https://example.com/a",
        r#"<!doctype html><html><body>
            <h1>Page A</h1>
            <script>window.location.href = "https://example.com/b";</script>
        </body></html>"#,
    );

    // Page B: the destination
    fetcher.add_html(
        "https://example.com/b",
        r#"<!doctype html><html><body><h1>Page B</h1></body></html>"#,
    );

    let mut engine = Engine::new();
    let snapshot = engine
        .navigate("https://example.com/a", &mut fetcher, SnapMode::Text)
        .unwrap();

    assert!(
        snapshot.contains("Page B"),
        "engine should have navigated to page B, got:\n{snapshot}"
    );
}
