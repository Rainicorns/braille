//! Tests for runtime reuse (Fast mode) — verifies that loading a second page
//! in the same Engine produces correct, isolated results.

use braille_engine::{Engine, RuntimeMode};
use braille_wire::SnapMode;

#[test]
fn second_page_has_clean_dom() {
    let mut engine = Engine::new();
    assert_eq!(engine.runtime_mode, RuntimeMode::Fast);

    engine.load_html(r#"<html><body><h1>Page One</h1><p>First content</p></body></html>"#);
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("Page One"), "snap1: {snap1}");
    assert!(snap1.contains("First content"), "snap1: {snap1}");

    engine.load_html(r#"<html><body><h1>Page Two</h1><p>Second content</p></body></html>"#);
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap2.contains("Page Two"), "snap2: {snap2}");
    assert!(snap2.contains("Second content"), "snap2: {snap2}");
    assert!(!snap2.contains("Page One"), "page one leaked into page two: {snap2}");
    assert!(!snap2.contains("First content"), "page one content leaked: {snap2}");
}

#[test]
fn js_globals_dont_leak_into_dom() {
    let mut engine = Engine::new();

    engine.load_html(r#"<html><body><script>
        globalThis.__test_leak = "page1_secret";
    </script><p id="out">page1</p></body></html>"#);

    let val = engine.eval_js("typeof __test_leak").unwrap();
    assert_eq!(val, "string");

    engine.load_html(r#"<html><body><p>page2 clean</p></body></html>"#);

    let snap = engine.snapshot(SnapMode::Accessibility);
    assert!(snap.contains("page2 clean"), "snap: {snap}");
    assert!(!snap.contains("page1"), "page1 DOM leaked: {snap}");
}

#[test]
fn console_buffer_is_fresh_per_page() {
    let mut engine = Engine::new();

    engine.load_html(r#"<html><body><script>console.log("page1 log");</script></body></html>"#);
    let console1 = engine.console_output();
    assert!(console1.iter().any(|l: &String| l.contains("page1 log")));

    engine.load_html(r#"<html><body><script>console.log("page2 log");</script></body></html>"#);
    let console2 = engine.console_output();
    assert!(console2.iter().any(|l: &String| l.contains("page2 log")));
    assert!(!console2.iter().any(|l: &String| l.contains("page1 log")), "page1 console leaked: {console2:?}");
}

#[test]
fn timers_dont_leak_across_pages() {
    let mut engine = Engine::new();

    engine.load_html(r#"<html><body><script>
        setInterval(function() { console.log("leaked timer"); }, 100);
    </script></body></html>"#);

    // Verify page1 has timers by checking eval
    let has = engine.eval_js("typeof __braille_register_timer").unwrap();
    assert_eq!(has, "function");

    engine.load_html(r#"<html><body><p>Clean page</p></body></html>"#);
    engine.settle();

    // If timers leaked, "leaked timer" would appear in console
    let console = engine.console_output();
    assert!(!console.iter().any(|l: &String| l.contains("leaked timer")), "timer leaked: {console:?}");
}

#[test]
fn clean_mode_also_works() {
    let mut engine = Engine::new();
    engine.runtime_mode = RuntimeMode::Clean;

    engine.load_html(r#"<html><body><h1>Page A</h1></body></html>"#);
    let snap1 = engine.snapshot(SnapMode::Compact);
    assert!(snap1.contains("Page A"));

    engine.load_html(r#"<html><body><h1>Page B</h1></body></html>"#);
    let snap2 = engine.snapshot(SnapMode::Compact);
    assert!(snap2.contains("Page B"));
    assert!(!snap2.contains("Page A"));
}

#[test]
fn three_successive_pages_all_correct() {
    let mut engine = Engine::new();

    for i in 1..=3 {
        let html = format!(r#"<html><body><h1>Page {i}</h1><script>console.log("loaded {i}");</script></body></html>"#);
        engine.load_html(&html);
        let snap = engine.snapshot(SnapMode::Accessibility);
        assert!(snap.contains(&format!("Page {i}")), "page {i} missing: {snap}");

        let console = engine.console_output();
        assert!(console.iter().any(|l: &String| l.contains(&format!("loaded {i}"))));

        for prev in 1..i {
            assert!(!snap.contains(&format!("Page {prev}")), "page {prev} leaked into page {i}: {snap}");
        }
    }
}

#[test]
fn click_works_after_runtime_reuse() {
    let mut engine = Engine::new();

    engine.load_html(r#"<html><body><button id="btn1">Click me</button></body></html>"#);
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("Click me"));

    engine.load_html(r#"<html><body>
        <button id="btn2" onclick="document.getElementById('result').textContent='clicked'">Do it</button>
        <p id="result">waiting</p>
    </body></html>"#);
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap2.contains("Do it"));

    engine.handle_click("@e1");
    engine.settle();
    let snap3 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap3.contains("clicked"), "click didn't work after reuse: {snap3}");
}

#[test]
fn fast_and_clean_produce_same_snapshot() {
    let html = r#"<html><body>
        <h1>Test</h1>
        <script>
            var p = document.createElement('p');
            p.textContent = 'dynamic';
            document.body.appendChild(p);
        </script>
    </body></html>"#;

    let mut fast = Engine::new();
    fast.runtime_mode = RuntimeMode::Fast;
    // Load twice so the second load exercises reuse
    fast.load_html("<html><body>warmup</body></html>");
    fast.load_html(html);
    let snap_fast = fast.snapshot(SnapMode::Accessibility);

    let mut clean = Engine::new();
    clean.runtime_mode = RuntimeMode::Clean;
    clean.load_html("<html><body>warmup</body></html>");
    clean.load_html(html);
    let snap_clean = clean.snapshot(SnapMode::Accessibility);

    assert_eq!(snap_fast, snap_clean, "fast and clean mode produced different snapshots");
}
