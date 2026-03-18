//! Integration tests for the Braille engine.
//!
//! Each test exercises the full engine pipeline (parse HTML -> execute scripts -> snapshot -> interact)
//! without any CLI or network involvement. All tests use Engine directly.

use std::collections::HashMap;

use braille_engine::Engine;
use braille_wire::{EngineAction, HttpMethod, SnapMode};

fn snap(html: &str) -> String {
    let mut engine = Engine::new();
    engine.load_html(html);
    engine.snapshot(SnapMode::Accessibility)
}

fn engine_with_snap(html: &str) -> (Engine, String) {
    let mut engine = Engine::new();
    engine.load_html(html);
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    (engine, snapshot)
}

// ---------------------------------------------------------------------------
// 1. Link click flow
// ---------------------------------------------------------------------------

#[test]
fn link_click_returns_navigate_action_with_correct_url() {
    let html = r#"
    <html><body>
      <a href="https://example.com/page">Visit Example</a>
    </body></html>"#;

    let (mut engine, snapshot) = engine_with_snap(html);

    // The link should appear in the accessibility tree with a ref
    assert!(
        snapshot.contains("link @e1"),
        "snapshot should contain link ref: {}",
        snapshot
    );
    assert!(
        snapshot.contains("Visit Example"),
        "snapshot should contain link text: {}",
        snapshot
    );

    // Click via the @e1 ref
    let action = engine.handle_click("@e1");
    match action {
        EngineAction::Navigate(req) => {
            assert_eq!(req.url, "https://example.com/page");
            assert_eq!(req.method, HttpMethod::Get);
            assert_eq!(req.body, None);
            assert_eq!(req.content_type, None);
        }
        other => panic!("Expected Navigate action, got {:?}", other),
    }
}

#[test]
fn link_click_with_relative_href() {
    let html = r#"
    <html><body>
      <a href="/about">About Us</a>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    let action = engine.handle_click("@e1");
    match action {
        EngineAction::Navigate(req) => {
            assert_eq!(req.url, "/about");
            assert_eq!(req.method, HttpMethod::Get);
        }
        other => panic!("Expected Navigate action, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 2. Form submission flow
// ---------------------------------------------------------------------------

#[test]
fn button_click_inside_form_returns_none_action() {
    // Note: handle_click on a button currently returns EngineAction::None.
    // The form.rs module has handle_form_submit but it's not wired through handle_click.
    // This test documents the current behavior.
    let html = r##"
    <html><body>
      <form action="/search" method="get">
        <input type="text" name="q" value="rust">
        <button type="submit">Search</button>
      </form>
    </body></html>"##;

    let (mut engine, _) = engine_with_snap(html);

    // Click the button (should be @e2 since input is @e1)
    let action = engine.handle_click("@e2");
    match action {
        EngineAction::None => {
            // Current behavior: button click returns None.
            // Form submission is available via handle_form_submit but not wired to click.
        }
        other => panic!("Expected None action for button click, got {:?}", other),
    }
}

#[test]
fn form_with_inputs_visible_in_snapshot() {
    let html = r##"
    <html><body>
      <form action="/submit" method="post">
        <input type="text" name="username" value="alice">
        <input type="email" name="email" value="alice@example.com">
        <button type="submit">Submit</button>
      </form>
    </body></html>"##;

    let snapshot = snap(html);

    // Verify all form controls are visible in the accessibility tree
    assert!(snapshot.contains("form"), "snapshot should contain form: {}", snapshot);
    assert!(
        snapshot.contains("input[type=text] @e1 value=\"alice\""),
        "snapshot should show username input with value: {}",
        snapshot
    );
    assert!(
        snapshot.contains("input[type=email] @e2 value=\"alice@example.com\""),
        "snapshot should show email input with value: {}",
        snapshot
    );
    assert!(
        snapshot.contains("button @e3 \"Submit\""),
        "snapshot should show submit button: {}",
        snapshot
    );
}

// ---------------------------------------------------------------------------
// 3. Type + snapshot
// ---------------------------------------------------------------------------

#[test]
fn type_into_input_then_snapshot_shows_value() {
    let html = r#"
    <html><body>
      <input type="text" id="name">
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    // Type into the input via @e1 ref
    engine.handle_type("@e1", "hello").unwrap();

    // Take a new snapshot and verify the value appears
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("value=\"hello\""),
        "snapshot after typing should contain value=\"hello\": {}",
        snapshot
    );
}

#[test]
fn type_into_textarea_then_snapshot_shows_text() {
    let html = r#"
    <html><body>
      <textarea id="msg"></textarea>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    engine.handle_type("@e1", "world").unwrap();

    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("world"),
        "snapshot after typing into textarea should contain typed text: {}",
        snapshot
    );
}

#[test]
fn type_overwrites_previous_value_in_snapshot() {
    let html = r#"
    <html><body>
      <input type="text" value="old">
    </body></html>"#;

    let (mut engine, snap1) = engine_with_snap(html);
    assert!(
        snap1.contains("value=\"old\""),
        "initial snapshot should show old value: {}",
        snap1
    );

    engine.handle_type("@e1", "new").unwrap();

    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap2.contains("value=\"new\""),
        "snapshot after type should show new value: {}",
        snap2
    );
    assert!(!snap2.contains("value=\"old\""), "old value should be gone: {}", snap2);
}

// ---------------------------------------------------------------------------
// 4. Select + snapshot
// ---------------------------------------------------------------------------

#[test]
fn select_option_then_snapshot_shows_selected_value() {
    let html = r#"
    <html><body>
      <select id="color">
        <option value="r">Red</option>
        <option value="g">Green</option>
        <option value="b">Blue</option>
      </select>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    // Select "Green" by value
    engine.handle_select("@e1", "g").unwrap();

    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("value=\"Green\""),
        "snapshot after select should show selected option text 'Green': {}",
        snapshot
    );
}

#[test]
fn select_changes_selected_option_in_snapshot() {
    let html = r#"
    <html><body>
      <select id="size">
        <option value="s">Small</option>
        <option value="m" selected>Medium</option>
        <option value="l">Large</option>
      </select>
    </body></html>"#;

    let (mut engine, snap1) = engine_with_snap(html);
    assert!(
        snap1.contains("value=\"Medium\""),
        "initial snapshot should show Medium selected: {}",
        snap1
    );

    engine.handle_select("@e1", "l").unwrap();

    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap2.contains("value=\"Large\""),
        "snapshot after select should show Large selected: {}",
        snap2
    );
}

// ---------------------------------------------------------------------------
// 5. Script execution + DOM mutation (complex)
// ---------------------------------------------------------------------------

#[test]
fn inline_script_creates_nested_dom_structure() {
    let html = r#"
    <html><body>
      <div id="root"></div>
      <script>
        let nav = document.createElement("nav");
        let link1 = document.createElement("a");
        link1.textContent = "Home";
        let link2 = document.createElement("a");
        link2.textContent = "About";
        nav.appendChild(link1);
        nav.appendChild(link2);
        document.getElementById("root").appendChild(nav);
      </script>
    </body></html>"#;

    let snapshot = snap(html);

    assert!(
        snapshot.contains("navigation"),
        "snapshot should contain navigation role: {}",
        snapshot
    );
    assert!(
        snapshot.contains("link @e1 \"Home\""),
        "snapshot should contain Home link: {}",
        snapshot
    );
    assert!(
        snapshot.contains("link @e2 \"About\""),
        "snapshot should contain About link: {}",
        snapshot
    );
}

#[test]
fn script_creates_form_with_inputs() {
    let html = r#"
    <html><body>
      <div id="app"></div>
      <script>
        let form = document.createElement("form");
        let input = document.createElement("input");
        input.setAttribute("type", "text");
        let btn = document.createElement("button");
        btn.textContent = "Go";
        form.appendChild(input);
        form.appendChild(btn);
        document.getElementById("app").appendChild(form);
      </script>
    </body></html>"#;

    let snapshot = snap(html);

    assert!(snapshot.contains("form"), "snapshot should contain form: {}", snapshot);
    assert!(
        snapshot.contains("input[type=text] @e1"),
        "snapshot should contain text input: {}",
        snapshot
    );
    assert!(
        snapshot.contains("button @e2 \"Go\""),
        "snapshot should contain Go button: {}",
        snapshot
    );
}

// ---------------------------------------------------------------------------
// 6. External scripts
// ---------------------------------------------------------------------------

#[test]
fn external_script_modifies_dom() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script src="app.js"></script>
    </body></html>"#;

    let mut fetched = HashMap::new();
    fetched.insert(
        "app.js".to_string(),
        r#"
            let h = document.createElement("h1");
            h.textContent = "Loaded from external";
            document.getElementById("target").appendChild(h);
        "#
        .to_string(),
    );

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snapshot.contains("heading[1] \"Loaded from external\""),
        "external script should have created h1: {}",
        snapshot
    );
}

#[test]
fn external_script_with_full_url() {
    let html = r#"
    <html><body>
      <div id="container"></div>
      <script src="https://cdn.example.com/widget.js"></script>
    </body></html>"#;

    let mut fetched = HashMap::new();
    fetched.insert(
        "https://cdn.example.com/widget.js".to_string(),
        r#"
            let p = document.createElement("p");
            p.textContent = "Widget loaded";
            document.getElementById("container").appendChild(p);
        "#
        .to_string(),
    );

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snapshot.contains("Widget loaded"),
        "external script from full URL should execute: {}",
        snapshot
    );
}

// ---------------------------------------------------------------------------
// 7. Mixed inline + external scripts in document order
// ---------------------------------------------------------------------------

#[test]
fn mixed_scripts_execute_in_document_order() {
    let html = r#"
    <html><body>
      <div id="out"></div>
      <script>var log = [];</script>
      <script src="first.js"></script>
      <script>log.push("inline-2");</script>
      <script src="second.js"></script>
      <script>
        let el = document.createElement("p");
        el.textContent = log.join(",");
        document.getElementById("out").appendChild(el);
      </script>
    </body></html>"#;

    let mut fetched = HashMap::new();
    fetched.insert("first.js".to_string(), r#"log.push("ext-1");"#.to_string());
    fetched.insert("second.js".to_string(), r#"log.push("ext-2");"#.to_string());

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snapshot.contains("ext-1,inline-2,ext-2"),
        "scripts should execute in document order: {}",
        snapshot
    );
}

#[test]
fn external_script_sets_global_used_by_inline() {
    let html = r#"
    <html><body>
      <div id="result"></div>
      <script src="config.js"></script>
      <script>
        let el = document.createElement("p");
        el.textContent = "Config: " + appConfig;
        document.getElementById("result").appendChild(el);
      </script>
    </body></html>"#;

    let mut fetched = HashMap::new();
    fetched.insert("config.js".to_string(), r#"var appConfig = "production";"#.to_string());

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snapshot.contains("Config: production"),
        "inline script should see global set by external: {}",
        snapshot
    );
}

// ---------------------------------------------------------------------------
// 8. Focus tracking
// ---------------------------------------------------------------------------

#[test]
fn focus_input_shows_focused_marker_in_snapshot() {
    let html = r#"
    <html><body>
      <input type="text" id="search">
      <button>Go</button>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    engine.handle_focus("@e1").unwrap();

    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("[focused]"),
        "snapshot should show focused marker: {}",
        snapshot
    );
    assert!(
        snapshot.contains("input[type=text] @e1") && snapshot.contains("[focused]"),
        "the input should be focused: {}",
        snapshot
    );
}

#[test]
fn focus_moves_between_elements() {
    let html = r#"
    <html><body>
      <input type="text" id="first">
      <input type="text" id="second">
      <button id="btn">Click</button>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    // Focus the first input
    engine.handle_focus("@e1").unwrap();
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap1.contains("@e1") && snap1.contains("[focused]"),
        "first input should be focused: {}",
        snap1
    );

    // Move focus to the button
    engine.handle_focus("@e3").unwrap();
    let snap2 = engine.snapshot(SnapMode::Accessibility);

    // Verify focus moved
    let lines: Vec<&str> = snap2.lines().collect();
    let e1_line = lines.iter().find(|l| l.contains("@e1")).unwrap();
    let e3_line = lines.iter().find(|l| l.contains("@e3")).unwrap();
    assert!(
        !e1_line.contains("[focused]"),
        "first input should not be focused anymore: {}",
        snap2
    );
    assert!(e3_line.contains("[focused]"), "button should be focused: {}", snap2);
}

#[test]
fn blur_clears_focused_marker() {
    let html = r#"
    <html><body>
      <input type="text">
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    engine.handle_focus("@e1").unwrap();
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("[focused]"), "should show focused: {}", snap1);

    engine.handle_blur();
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(
        !snap2.contains("[focused]"),
        "focus should be cleared after blur: {}",
        snap2
    );
}

// ---------------------------------------------------------------------------
// 9. Ref stability
// ---------------------------------------------------------------------------

#[test]
fn refs_resolve_to_correct_elements_after_snapshot() {
    let html = r#"
    <html><body>
      <a href="/home">Home</a>
      <button>Click</button>
      <input type="text">
    </body></html>"#;

    let (mut engine, snapshot) = engine_with_snap(html);

    // Verify refs exist
    assert!(snapshot.contains("@e1"), "should have @e1: {}", snapshot);
    assert!(snapshot.contains("@e2"), "should have @e2: {}", snapshot);
    assert!(snapshot.contains("@e3"), "should have @e3: {}", snapshot);

    // Resolve and verify
    let e1 = engine.resolve_ref("@e1").unwrap();
    let e2 = engine.resolve_ref("@e2").unwrap();
    let e3 = engine.resolve_ref("@e3").unwrap();

    // All should be different nodes
    assert_ne!(e1, e2);
    assert_ne!(e2, e3);
    assert_ne!(e1, e3);

    // Verify @e1 is a link by clicking it
    let action = engine.handle_click("@e1");
    match action {
        EngineAction::Navigate(req) => assert_eq!(req.url, "/home"),
        other => panic!("@e1 should be a link, got {:?}", other),
    }

    // Verify @e2 is a button
    let action = engine.handle_click("@e2");
    match action {
        EngineAction::None => {} // buttons return None
        other => panic!("@e2 should be a button, got {:?}", other),
    }
}

#[test]
fn refs_reset_after_loading_new_html() {
    let html1 = r#"
    <html><body>
      <a href="/old">Old Link</a>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html1);

    let old_e1 = engine.resolve_ref("@e1");
    assert!(old_e1.is_some(), "should resolve @e1 from first page");

    // Load completely new HTML
    let html2 = r#"
    <html><body>
      <button>New Button</button>
      <a href="/new">New Link</a>
    </body></html>"#;

    engine.load_html(html2);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    // @e1 should now point to the button, @e2 to the link
    assert!(
        snapshot.contains("button @e1"),
        "new @e1 should be button: {}",
        snapshot
    );
    assert!(snapshot.contains("link @e2"), "new @e2 should be link: {}", snapshot);

    // Verify the new @e2 is a link to /new
    let action = engine.handle_click("@e2");
    match action {
        EngineAction::Navigate(req) => assert_eq!(req.url, "/new"),
        other => panic!("new @e2 should navigate to /new, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 10. Multiple interactive elements
// ---------------------------------------------------------------------------

#[test]
fn all_interactive_elements_get_unique_sequential_refs() {
    let html = r#"
    <html><body>
      <a href="/link1">Link 1</a>
      <button>Button 1</button>
      <input type="text">
      <select>
        <option value="a">A</option>
        <option value="b">B</option>
      </select>
      <textarea>Notes</textarea>
      <a href="/link2">Link 2</a>
      <button>Button 2</button>
      <input type="email">
    </body></html>"#;

    let (engine, snapshot) = engine_with_snap(html);

    // Verify all 8 interactive elements have unique refs in order
    assert!(
        snapshot.contains("link @e1 \"Link 1\""),
        "link should be @e1: {}",
        snapshot
    );
    assert!(
        snapshot.contains("button @e2 \"Button 1\""),
        "button should be @e2: {}",
        snapshot
    );
    assert!(
        snapshot.contains("input[type=text] @e3"),
        "text input should be @e3: {}",
        snapshot
    );
    assert!(snapshot.contains("select @e4"), "select should be @e4: {}", snapshot);
    assert!(
        snapshot.contains("textarea @e5"),
        "textarea should be @e5: {}",
        snapshot
    );
    assert!(
        snapshot.contains("link @e6 \"Link 2\""),
        "second link should be @e6: {}",
        snapshot
    );
    assert!(
        snapshot.contains("button @e7 \"Button 2\""),
        "second button should be @e7: {}",
        snapshot
    );
    assert!(
        snapshot.contains("input[type=email] @e8"),
        "email input should be @e8: {}",
        snapshot
    );

    // Verify all refs resolve
    for i in 1..=8 {
        let ref_str = format!("@e{}", i);
        assert!(
            engine.resolve_ref(&ref_str).is_some(),
            "{} should resolve to a node",
            ref_str
        );
    }

    // Verify @e9 does not exist
    assert!(engine.resolve_ref("@e9").is_none(), "@e9 should not exist");
}

// ---------------------------------------------------------------------------
// 11. Complex DOM with script mutation
// ---------------------------------------------------------------------------

#[test]
fn script_builds_complex_nested_structure_with_attributes() {
    let html = r#"
    <html><body>
      <div id="app"></div>
      <script>
        // Create a navigation bar
        let nav = document.createElement("nav");

        let homeLink = document.createElement("a");
        homeLink.setAttribute("href", "/home");
        homeLink.textContent = "Home";
        nav.appendChild(homeLink);

        let aboutLink = document.createElement("a");
        aboutLink.setAttribute("href", "/about");
        aboutLink.textContent = "About";
        nav.appendChild(aboutLink);

        document.getElementById("app").appendChild(nav);

        // Create a main content area
        let main = document.createElement("main");

        let heading = document.createElement("h1");
        heading.textContent = "Dashboard";
        main.appendChild(heading);

        let para = document.createElement("p");
        para.textContent = "Welcome to the dashboard.";
        main.appendChild(para);

        let form = document.createElement("form");
        let searchInput = document.createElement("input");
        searchInput.setAttribute("type", "text");
        searchInput.setAttribute("value", "search here");
        form.appendChild(searchInput);

        let submitBtn = document.createElement("button");
        submitBtn.textContent = "Search";
        form.appendChild(submitBtn);

        main.appendChild(form);

        document.getElementById("app").appendChild(main);
      </script>
    </body></html>"#;

    let (mut engine, snapshot) = engine_with_snap(html);

    // Verify the full accessibility tree structure
    assert!(snapshot.contains("navigation"), "should have nav: {}", snapshot);
    assert!(
        snapshot.contains("link @e1 \"Home\""),
        "should have Home link: {}",
        snapshot
    );
    assert!(
        snapshot.contains("link @e2 \"About\""),
        "should have About link: {}",
        snapshot
    );
    assert!(snapshot.contains("main"), "should have main: {}", snapshot);
    assert!(
        snapshot.contains("heading[1] \"Dashboard\""),
        "should have h1: {}",
        snapshot
    );
    assert!(
        snapshot.contains("paragraph \"Welcome to the dashboard.\""),
        "should have paragraph: {}",
        snapshot
    );
    assert!(snapshot.contains("form"), "should have form: {}", snapshot);
    assert!(
        snapshot.contains("input[type=text] @e3 value=\"search here\""),
        "should have search input with value: {}",
        snapshot
    );
    assert!(
        snapshot.contains("button @e4 \"Search\""),
        "should have search button: {}",
        snapshot
    );

    // Verify we can interact with the created elements
    let action = engine.handle_click("@e1");
    match action {
        EngineAction::Navigate(req) => assert_eq!(req.url, "/home"),
        other => panic!("Home link should navigate, got {:?}", other),
    }

    let action = engine.handle_click("@e2");
    match action {
        EngineAction::Navigate(req) => assert_eq!(req.url, "/about"),
        other => panic!("About link should navigate, got {:?}", other),
    }
}

#[test]
fn script_modifies_text_content_of_existing_elements() {
    let html = r#"
    <html><body>
      <h1 id="title">Original Title</h1>
      <p id="desc">Original description</p>
      <script>
        document.getElementById("title").textContent = "Modified Title";
        document.getElementById("desc").textContent = "Modified description";
      </script>
    </body></html>"#;

    let snapshot = snap(html);

    assert!(
        snapshot.contains("Modified Title"),
        "title should be modified: {}",
        snapshot
    );
    assert!(
        snapshot.contains("Modified description"),
        "description should be modified: {}",
        snapshot
    );
    assert!(
        !snapshot.contains("Original Title"),
        "original title should be gone: {}",
        snapshot
    );
    assert!(
        !snapshot.contains("Original description"),
        "original description should be gone: {}",
        snapshot
    );
}

// ---------------------------------------------------------------------------
// 12. Click on non-existent ref
// ---------------------------------------------------------------------------

#[test]
fn click_on_nonexistent_ref_returns_error() {
    let html = r#"
    <html><body>
      <a href="/home">Home</a>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    let action = engine.handle_click("@e999");
    match action {
        EngineAction::Error(msg) => {
            assert!(
                msg.contains("element not found") || msg.contains("not found"),
                "error message should indicate element not found: {}",
                msg
            );
        }
        other => panic!("Expected Error action for @e999, got {:?}", other),
    }
}

#[test]
fn click_on_nonexistent_id_returns_error() {
    let html = r#"
    <html><body>
      <a href="/home">Home</a>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    let action = engine.handle_click("#does-not-exist");
    match action {
        EngineAction::Error(msg) => {
            assert!(
                msg.contains("element not found") || msg.contains("not found"),
                "error message should indicate element not found: {}",
                msg
            );
        }
        other => panic!("Expected Error action for #does-not-exist, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Additional integration scenarios
// ---------------------------------------------------------------------------

#[test]
fn full_workflow_load_type_focus_snapshot() {
    // Simulates a real workflow: load page, type into search, focus submit, take snapshot
    let html = r#"
    <html><body>
      <nav>
        <a href="/home">Home</a>
        <a href="/about">About</a>
      </nav>
      <main>
        <h1>Search</h1>
        <form>
          <input type="text" id="query">
          <button>Search</button>
        </form>
      </main>
    </body></html>"#;

    let (mut engine, snap1) = engine_with_snap(html);
    assert!(snap1.contains("navigation"), "should see nav: {}", snap1);
    assert!(snap1.contains("heading[1] \"Search\""), "should see heading: {}", snap1);
    assert!(
        snap1.contains("input[type=text] @e3"),
        "search input should be @e3: {}",
        snap1
    );
    assert!(
        snap1.contains("button @e4 \"Search\""),
        "search button should be @e4: {}",
        snap1
    );

    // Step 2: Type into the search input
    engine.handle_type("@e3", "braille browser").unwrap();

    // Step 3: Focus the search button
    engine.handle_focus("@e4").unwrap();

    // Step 4: Verify the state
    let snap2 = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snap2.contains("value=\"braille browser\""),
        "input should have the typed value: {}",
        snap2
    );
    assert!(
        snap2.contains("button @e4 \"Search\" [focused]"),
        "search button should be focused: {}",
        snap2
    );
}

#[test]
fn type_and_select_then_verify_combined_state() {
    let html = r#"
    <html><body>
      <form>
        <input type="text" name="name">
        <select name="country">
          <option value="us">United States</option>
          <option value="ca">Canada</option>
          <option value="uk">United Kingdom</option>
        </select>
        <textarea name="bio"></textarea>
        <button>Submit</button>
      </form>
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html);

    // Fill in the form
    engine.handle_type("@e1", "Alice").unwrap();
    engine.handle_select("@e2", "ca").unwrap();
    engine.handle_type("@e3", "Software engineer").unwrap();

    // Verify the combined state
    let snapshot = engine.snapshot(SnapMode::Accessibility);
    assert!(
        snapshot.contains("value=\"Alice\""),
        "name input should have Alice: {}",
        snapshot
    );
    assert!(
        snapshot.contains("value=\"Canada\""),
        "select should show Canada: {}",
        snapshot
    );
    assert!(
        snapshot.contains("Software engineer"),
        "textarea should show bio: {}",
        snapshot
    );
}

#[test]
fn multiple_load_cycles_reset_state_cleanly() {
    // First page
    let html1 = r#"
    <html><body>
      <a href="/page1">Page 1 Link</a>
      <input type="text" id="input1">
    </body></html>"#;

    let (mut engine, _) = engine_with_snap(html1);
    engine.handle_type("@e2", "typed on page 1").unwrap();
    engine.handle_focus("@e1").unwrap();

    // Verify state on page 1
    let snap1 = engine.snapshot(SnapMode::Accessibility);
    assert!(snap1.contains("Page 1 Link"), "should see page 1 content: {}", snap1);
    assert!(
        snap1.contains("value=\"typed on page 1\""),
        "should see typed text: {}",
        snap1
    );

    // Load second page (simulating navigation)
    let html2 = r#"
    <html><body>
      <h1>Page 2</h1>
      <button>Action</button>
    </body></html>"#;

    engine.load_html(html2);
    let snap2 = engine.snapshot(SnapMode::Accessibility);

    // Page 1 content should be completely gone
    assert!(
        !snap2.contains("Page 1 Link"),
        "page 1 content should be gone: {}",
        snap2
    );
    assert!(
        !snap2.contains("typed on page 1"),
        "typed text should be gone: {}",
        snap2
    );
    assert!(!snap2.contains("[focused]"), "focus should be reset: {}", snap2);

    // Page 2 content should be present
    assert!(
        snap2.contains("heading[1] \"Page 2\""),
        "should see page 2 heading: {}",
        snap2
    );
    assert!(
        snap2.contains("button @e1 \"Action\""),
        "should see page 2 button as @e1: {}",
        snap2
    );
}

#[test]
fn external_script_missing_from_fetched_is_skipped_gracefully() {
    let html = r#"
    <html><body>
      <div id="out"></div>
      <script src="https://missing.example.com/lib.js"></script>
      <script>
        let el = document.createElement("p");
        el.textContent = "Still works";
        document.getElementById("out").appendChild(el);
      </script>
    </body></html>"#;

    let fetched = HashMap::new(); // Empty: no scripts fetched

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    // The inline script after the missing external should still run
    assert!(
        snapshot.contains("Still works"),
        "inline script should execute even when external is missing: {}",
        snapshot
    );
}

#[test]
fn script_with_src_attribute_ignores_inline_content() {
    let html = r#"
    <html><body>
      <div id="target"></div>
      <script src="real.js">
        let bad = document.createElement("p");
        bad.textContent = "BAD INLINE";
        document.getElementById("target").appendChild(bad);
      </script>
    </body></html>"#;

    let mut fetched = HashMap::new();
    fetched.insert(
        "real.js".to_string(),
        r#"
            let good = document.createElement("p");
            good.textContent = "GOOD EXTERNAL";
            document.getElementById("target").appendChild(good);
        "#
        .to_string(),
    );

    let mut engine = Engine::new();
    engine.load_html_with_scripts(html, &fetched);
    let snapshot = engine.snapshot(SnapMode::Accessibility);

    assert!(
        snapshot.contains("GOOD EXTERNAL"),
        "external content should run: {}",
        snapshot
    );
    assert!(
        !snapshot.contains("BAD INLINE"),
        "inline content should be ignored when src is present: {}",
        snapshot
    );
}

#[test]
fn resolve_ref_before_snapshot_returns_none() {
    let html = r#"
    <html><body>
      <a href="/link">Link</a>
    </body></html>"#;

    let mut engine = Engine::new();
    engine.load_html(html);

    // Before taking any snapshot, refs should not resolve
    assert_eq!(
        engine.resolve_ref("@e1"),
        None,
        "refs should not resolve before snapshot"
    );
}

#[test]
fn snapshot_with_deeply_nested_interactive_elements() {
    let html = r#"
    <html><body>
      <nav>
        <div>
          <span>
            <a href="/deep">Deep Link</a>
          </span>
        </div>
      </nav>
      <main>
        <section>
          <article>
            <div>
              <button>Deep Button</button>
            </div>
          </article>
        </section>
      </main>
    </body></html>"#;

    let (mut engine, snapshot) = engine_with_snap(html);

    // Even deeply nested elements should get refs and be interactable
    assert!(
        snapshot.contains("link @e1 \"Deep Link\""),
        "deeply nested link should have ref: {}",
        snapshot
    );
    assert!(
        snapshot.contains("button @e2 \"Deep Button\""),
        "deeply nested button should have ref: {}",
        snapshot
    );

    // Verify they work
    let action = engine.handle_click("@e1");
    match action {
        EngineAction::Navigate(req) => assert_eq!(req.url, "/deep"),
        other => panic!("Deep link should navigate, got {:?}", other),
    }
}
