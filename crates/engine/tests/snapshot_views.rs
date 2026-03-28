//! Verification tests for Smart Snapshot Views and Timer Execution.
//!
//! Tests settle behavior (Promises, timers), each view mode, ref stability,
//! selector/region views, and timer lifecycle (setTimeout, setInterval, clearTimeout).

use braille_engine::Engine;
use braille_wire::SnapMode;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn engine_with_html(html: &str) -> Engine {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine
}

// ---------------------------------------------------------------------------
// Test 1: Settle flushes Promises after click
// ---------------------------------------------------------------------------

#[test]
fn settle_flushes_promises_after_click() {
    let html = r#"<html><body>
        <button id="btn" onclick="Promise.resolve().then(() => { document.getElementById('out').textContent = 'settled' })">Click me</button>
        <p id="out">waiting</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap1 = engine.snapshot(SnapMode::Text);
    assert!(snap1.contains("waiting"), "before click: {}", snap1);

    engine.handle_click("button");
    let snap2 = engine.snapshot(SnapMode::Text);
    assert!(snap2.contains("settled"), "after click should show settled: {}", snap2);
}

// ---------------------------------------------------------------------------
// Test 2: Settle fires setTimeout(fn, 0) after click
// ---------------------------------------------------------------------------

#[test]
fn settle_fires_set_timeout_zero_after_click() {
    let html = r#"<html><body>
        <button id="btn" onclick="setTimeout(function(){ document.getElementById('out').textContent = 'timer fired' }, 0)">Click me</button>
        <p id="out">waiting</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    engine.snapshot(SnapMode::Accessibility);

    engine.handle_click("button");
    let snap = engine.snapshot(SnapMode::Text);
    assert!(snap.contains("timer fired"), "setTimeout(fn,0) should fire during settle: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 3: Interactive view — only interactive elements
// ---------------------------------------------------------------------------

#[test]
fn interactive_view_shows_only_interactive_elements() {
    let html = r#"<html><body>
        <nav><a href="/home">Home</a><a href="/about">About</a></nav>
        <main>
            <h1>Title</h1>
            <p>Some paragraph text</p>
            <button>Submit</button>
            <input type="text" value="hello">
        </main>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Interactive);

    // Should contain interactive elements with refs
    assert!(snap.contains("Home"), "should contain link text: {}", snap);
    assert!(snap.contains("Submit"), "should contain button text: {}", snap);
    assert!(snap.contains("@e"), "should contain element refs: {}", snap);

    // Should NOT contain non-interactive content
    assert!(!snap.contains("Title"), "should not contain heading: {}", snap);
    assert!(!snap.contains("Some paragraph text"), "should not contain paragraph: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 4: Links view — only <a> elements with href
// ---------------------------------------------------------------------------

#[test]
fn links_view_shows_only_links() {
    let html = r#"<html><body>
        <nav><a href="/home">Home</a><a href="/about">About</a></nav>
        <p>Some paragraph</p>
        <form action="/submit"><button>Go</button></form>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Links);

    assert!(snap.contains("Home"), "should contain link text: {}", snap);
    assert!(snap.contains("About"), "should contain link text: {}", snap);
    assert!(snap.contains("/home"), "should contain link url: {}", snap);
    assert!(!snap.contains("Some paragraph"), "should not contain paragraph: {}", snap);
    assert!(!snap.contains("Go"), "should not contain button: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 5: Forms view
// ---------------------------------------------------------------------------

#[test]
fn forms_view_shows_form_structure() {
    let html = r#"<html><body>
        <form action="/submit">
            <input type="text" name="user" value="alice">
            <select name="role"><option value="admin">Admin</option></select>
            <button type="submit">Go</button>
        </form>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Forms);

    assert!(snap.contains("/submit"), "should contain form action: {}", snap);
    assert!(snap.contains("alice"), "should contain input value: {}", snap);
    assert!(snap.contains("Go"), "should contain submit button: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 6: Headings view — outline with indent by level
// ---------------------------------------------------------------------------

#[test]
fn headings_view_shows_outline() {
    let html = r#"<html><body>
        <h1>Main Title</h1>
        <h2>Section A</h2>
        <h2>Section B</h2>
        <h3>Subsection B1</h3>
        <p>Not a heading</p>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Headings);

    assert!(snap.contains("Main Title"), "should contain h1: {}", snap);
    assert!(snap.contains("Section A"), "should contain h2: {}", snap);
    assert!(snap.contains("Subsection B1"), "should contain h3: {}", snap);
    assert!(!snap.contains("Not a heading"), "should not contain paragraph: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 7: Text view — readable text only
// ---------------------------------------------------------------------------

#[test]
fn text_view_shows_readable_text() {
    let html = r#"<html><body>
        <h1>Welcome</h1>
        <p>Hello world</p>
        <div style="display:none">Hidden content</div>
        <script>var x = 1;</script>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Text);

    assert!(snap.contains("Welcome"), "should contain heading text: {}", snap);
    assert!(snap.contains("Hello world"), "should contain paragraph text: {}", snap);
    // Should not contain script content
    assert!(!snap.contains("var x"), "should not contain script content: {}", snap);
    // Should not contain refs
    assert!(!snap.contains("@e"), "should not contain element refs: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 8: Ref stability across views
// ---------------------------------------------------------------------------

#[test]
fn ref_stability_across_views() {
    let html = r#"<html><body>
        <nav><a href="/home">Home</a></nav>
        <main><button>Click</button><input type="text"></main>
    </body></html>"#;

    let mut engine = engine_with_html(html);

    // Take accessibility snapshot to establish refs
    let snap_a11y = engine.snapshot(SnapMode::Accessibility);

    // Find the ref for the link in accessibility view
    let link_ref = snap_a11y
        .lines()
        .find(|l| l.contains("Home") && l.contains("@e"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("@e")))
        .expect("should find link ref in a11y snapshot");

    // Interactive view should use the same ref for the same element
    let snap_interactive = engine.snapshot(SnapMode::Interactive);
    assert!(
        snap_interactive.contains(link_ref),
        "interactive view should use same ref {} for link: {}",
        link_ref,
        snap_interactive
    );

    // Links view should use the same ref
    let snap_links = engine.snapshot(SnapMode::Links);
    assert!(
        snap_links.contains(link_ref),
        "links view should use same ref {} for link: {}",
        link_ref,
        snap_links
    );
}

// ---------------------------------------------------------------------------
// Test 9: Selector view
// ---------------------------------------------------------------------------

#[test]
fn selector_view_filters_by_css_selector() {
    let html = r#"<html><body>
        <nav><a href="/home">Nav Link 1</a><a href="/about">Nav Link 2</a></nav>
        <main><a href="/main">Main Link</a></main>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Selector("nav a".to_string()));

    assert!(snap.contains("Nav Link 1"), "should contain nav links: {}", snap);
    assert!(snap.contains("Nav Link 2"), "should contain nav links: {}", snap);
    assert!(!snap.contains("Main Link"), "should not contain main link: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 10: Region view with CSS selector
// ---------------------------------------------------------------------------

#[test]
fn region_view_shows_subtree() {
    let html = r#"<html><body>
        <nav><a href="/home">Nav Link</a></nav>
        <main><h1>Main Title</h1><button>Action</button></main>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Region("main".to_string()));

    assert!(snap.contains("Main Title"), "should contain main content: {}", snap);
    assert!(snap.contains("Action"), "should contain main button: {}", snap);
    assert!(!snap.contains("Nav Link"), "should not contain nav content: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 11: Region view with @eN target
// ---------------------------------------------------------------------------

#[test]
fn region_view_with_ref_target() {
    let html = r#"<html><body>
        <main>
            <button>Top Button</button>
            <section><p>Section text</p></section>
        </main>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    // First snapshot to populate ref_map
    let snap_a11y = engine.snapshot(SnapMode::Accessibility);

    // Find the ref for the button
    let btn_ref = snap_a11y
        .lines()
        .find(|l| l.contains("Top Button") && l.contains("@e"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("@e")))
        .expect("should find button ref in a11y snapshot");

    // Region view targeting the button element
    let snap = engine.snapshot(SnapMode::Region(btn_ref.to_string()));
    assert!(snap.contains("Top Button"), "region should show button content: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 12: Selector view empty result
// ---------------------------------------------------------------------------

#[test]
fn selector_view_empty_result() {
    let html = r#"<html><body><p>Hello</p></body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Selector(".nonexistent".to_string()));
    assert!(snap.is_empty() || snap.trim().is_empty(), "should be empty for no matches: '{}'", snap);
}

// ---------------------------------------------------------------------------
// Test 13: Region view invalid target
// ---------------------------------------------------------------------------

#[test]
fn region_view_invalid_target() {
    let html = r#"<html><body><p>Hello</p></body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Region("#nonexistent".to_string()));
    assert!(snap.starts_with("error:"), "should start with error: '{}'", snap);
}

// ---------------------------------------------------------------------------
// Test 14: setTimeout with delay fires during settle
// ---------------------------------------------------------------------------

#[test]
fn set_timeout_with_delay_fires_during_settle() {
    let html = r#"<html><body>
        <p id="out">start</p>
        <script>
            setTimeout(function(){ document.getElementById('out').textContent += 'A' }, 50);
            setTimeout(function(){ document.getElementById('out').textContent += 'B' }, 100);
        </script>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    engine.settle();
    let snap = engine.snapshot(SnapMode::Text);
    assert!(snap.contains('A'), "50ms timer should have fired: {}", snap);
    assert!(snap.contains('B'), "100ms timer should have fired: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 15: clearTimeout prevents firing
// ---------------------------------------------------------------------------

#[test]
fn clear_timeout_prevents_firing() {
    let html = r#"<html><body>
        <p id="out">clean</p>
        <script>
            var id = setTimeout(function(){ document.getElementById('out').textContent = 'dirty' }, 10);
            clearTimeout(id);
        </script>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    engine.settle();
    let snap = engine.snapshot(SnapMode::Text);
    assert!(snap.contains("clean"), "cleared timer should not fire: {}", snap);
    assert!(!snap.contains("dirty"), "cleared timer should not fire: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 16: setInterval fires and repeats
// ---------------------------------------------------------------------------

#[test]
fn set_interval_fires_and_repeats() {
    let html = r#"<html><body>
        <p id="out">0</p>
        <script>
            var count = 0;
            var iv = setInterval(function(){
                count++;
                document.getElementById('out').textContent = String(count);
                if (count >= 3) clearInterval(iv);
            }, 10);
        </script>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    engine.settle();
    let snap = engine.snapshot(SnapMode::Text);
    assert!(snap.contains('3'), "interval should have fired 3 times: {}", snap);
}

// ---------------------------------------------------------------------------
// Test 17: Compact view renders bullet markers for list items
// ---------------------------------------------------------------------------

#[test]
fn compact_view_unordered_list_has_dash_bullets() {
    let html = r#"<html><body>
        <ul>
            <li>First item</li>
            <li>Second item</li>
            <li>Third item</li>
        </ul>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Compact);

    assert!(snap.contains("- First item"), "ul items should have dash prefix: {}", snap);
    assert!(snap.contains("- Second item"), "ul items should have dash prefix: {}", snap);
    assert!(snap.contains("- Third item"), "ul items should have dash prefix: {}", snap);
}

#[test]
fn compact_view_ordered_list_has_numbered_bullets() {
    let html = r#"<html><body>
        <ol>
            <li>Alpha</li>
            <li>Beta</li>
            <li>Gamma</li>
        </ol>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Compact);

    assert!(snap.contains("1. Alpha"), "ol items should have numbered prefix: {}", snap);
    assert!(snap.contains("2. Beta"), "ol items should have numbered prefix: {}", snap);
    assert!(snap.contains("3. Gamma"), "ol items should have numbered prefix: {}", snap);
}

#[test]
fn compact_view_nested_lists() {
    let html = r#"<html><body>
        <ul>
            <li>Outer one</li>
            <li>Outer two
                <ol>
                    <li>Inner first</li>
                    <li>Inner second</li>
                </ol>
            </li>
        </ul>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Compact);

    assert!(snap.contains("- Outer one"), "outer ul items should have dash: {}", snap);
    assert!(snap.contains("1. Inner first"), "inner ol items should be numbered: {}", snap);
    assert!(snap.contains("2. Inner second"), "inner ol items should be numbered: {}", snap);
}

#[test]
fn compact_view_bare_li_without_list_parent() {
    let html = r#"<html><body>
        <li>Orphan item</li>
    </body></html>"#;

    let mut engine = engine_with_html(html);
    let snap = engine.snapshot(SnapMode::Compact);

    assert!(snap.contains("- Orphan item"), "li without ul/ol parent should get dash prefix: {}", snap);
}
