    use super::*;
    use braille_wire::SnapMode;

    #[test]
    fn test_end_to_end() {
        let html = r#"
        <html><body>
          <h1>Hello</h1>
          <div id="app"></div>
          <script>
            let el = document.createElement("p");
            el.textContent = "Created by JavaScript";
            document.getElementById("app").appendChild(el);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(
            snapshot.contains("heading"),
            "snapshot should contain heading: {}",
            snapshot
        );
        assert!(
            snapshot.contains("Hello"),
            "snapshot should contain Hello: {}",
            snapshot
        );
        assert!(
            snapshot.contains("paragraph"),
            "snapshot should contain paragraph: {}",
            snapshot
        );
        assert!(
            snapshot.contains("Created by JavaScript"),
            "snapshot should contain JS-created text: {}",
            snapshot
        );
    }

    #[test]
    fn test_multiple_scripts() {
        let html = r#"
        <html><body>
          <div id="container"></div>
          <script>
            let p1 = document.createElement("p");
            p1.textContent = "First";
            document.getElementById("container").appendChild(p1);
          </script>
          <script>
            let p2 = document.createElement("p");
            p2.textContent = "Second";
            document.getElementById("container").appendChild(p2);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(
            snapshot.contains("First"),
            "snapshot should contain First: {}",
            snapshot
        );
        assert!(
            snapshot.contains("Second"),
            "snapshot should contain Second: {}",
            snapshot
        );
    }

    #[test]
    fn test_resolve_ref_after_snapshot() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
          <button>Click me</button>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot.contains("@e1"), "snapshot should contain @e1: {}", snapshot);
        assert!(snapshot.contains("@e2"), "snapshot should contain @e2: {}", snapshot);

        // Verify we can resolve the refs
        let ref1 = engine.resolve_ref("@e1");
        let ref2 = engine.resolve_ref("@e2");

        assert!(ref1.is_some(), "should resolve @e1");
        assert!(ref2.is_some(), "should resolve @e2");
        assert_ne!(ref1, ref2, "refs should point to different nodes");
    }

    #[test]
    fn test_resolve_ref_returns_none_for_invalid_ref() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        assert_eq!(engine.resolve_ref("@e999"), None, "invalid ref should return None");
        assert_eq!(engine.resolve_ref("invalid"), None, "malformed ref should return None");
        assert_eq!(engine.resolve_ref(""), None, "empty ref should return None");
    }

    #[test]
    fn test_resolve_ref_before_snapshot_returns_none() {
        let html = r#"
        <html><body>
          <a href="/home">Home</a>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Try to resolve before calling snapshot
        assert_eq!(engine.resolve_ref("@e1"), None, "should return None before snapshot");
    }

    #[test]
    fn test_ref_map_with_no_interactive_elements() {
        let html = r#"
        <html><body>
          <h1>Title</h1>
          <p>Paragraph</p>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        assert_eq!(
            engine.resolve_ref("@e1"),
            None,
            "should return None when no interactive elements"
        );
    }

    #[test]
    fn test_ref_map_updates_on_new_snapshot() {
        // Test that ref_map is updated when snapshot is called again after DOM modification
        let html = r#"
        <html><body>
          <div id="container">
            <a href="/first">First</a>
          </div>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot1 = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot1.contains("@e1"), "first snapshot should contain @e1");
        let ref1_first = engine.resolve_ref("@e1");
        assert!(ref1_first.is_some(), "should resolve @e1 after first snapshot");

        // Now load new HTML with different interactive elements
        let html2 = r#"
        <html><body>
          <button>Button 1</button>
          <button>Button 2</button>
          <input type="text">
        </body></html>"#;

        engine.load_html(html2);
        let snapshot2 = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot2.contains("@e1"), "second snapshot should contain @e1");
        assert!(snapshot2.contains("@e2"), "second snapshot should contain @e2");
        assert!(snapshot2.contains("@e3"), "second snapshot should contain @e3");

        let ref1_second = engine.resolve_ref("@e1");
        let ref2_second = engine.resolve_ref("@e2");
        let ref3_second = engine.resolve_ref("@e3");

        assert!(ref1_second.is_some(), "should resolve @e1 after second snapshot");
        assert!(ref2_second.is_some(), "should resolve @e2 after second snapshot");
        assert!(ref3_second.is_some(), "should resolve @e3 after second snapshot");

        // The node IDs should be different since we loaded new HTML
        // (The tree was replaced, so old refs don't apply)
    }

    // ---- C-3C: compute_all_styles integration tests ----

    #[test]
    fn test_load_html_computes_styles() {
        let html = r##"
        <html><body>
          <style>p { color: red; }</style>
          <p>Hello</p>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);

        let tree = engine.tree.borrow();
        // Find the <p> element
        let p_id = tree.get_elements_by_tag_name("p")[0];
        let p_node = tree.get_node(p_id);

        // After load_html, computed_style should be populated
        assert!(
            p_node.computed_style.is_some(),
            "computed_style should be set after load_html"
        );
        let style = p_node.computed_style.as_ref().unwrap();
        assert!(style.contains_key("color"), "should have color property");
    }

    #[test]
    fn test_display_none_reflected_in_snapshot() {
        let html = r##"
        <html><body>
          <style>.hidden { display: none; }</style>
          <p>Visible</p>
          <p class="hidden">Hidden</p>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot.contains("Visible"), "visible text should appear: {}", snapshot);
        assert!(
            !snapshot.contains("Hidden"),
            "display:none text should not appear: {}",
            snapshot
        );
    }

    #[test]
    fn test_visibility_hidden_hides_text_in_snapshot() {
        let html = r##"
        <html><body>
          <style>.ghost { visibility: hidden; }</style>
          <p>Visible</p>
          <p class="ghost">Ghost</p>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);
        let snapshot = engine.snapshot(SnapMode::Accessibility);

        assert!(snapshot.contains("Visible"), "visible text should appear: {}", snapshot);
        assert!(
            !snapshot.contains("Ghost"),
            "visibility:hidden text should not appear: {}",
            snapshot
        );
        // But the paragraph structure should still be there
        let lines: Vec<&str> = snapshot.lines().collect();
        assert!(
            lines.len() >= 2,
            "should have multiple lines including hidden paragraph structure"
        );
    }

    #[test]
    fn test_script_added_element_gets_computed_styles() {
        let html = r##"
        <html><body>
          <style>p { color: blue; }</style>
          <div id="target"></div>
          <script>
            var p = document.createElement("p");
            p.textContent = "Dynamic";
            document.getElementById("target").appendChild(p);
          </script>
        </body></html>"##;

        let mut engine = Engine::new();
        engine.load_html(html);

        let tree = engine.tree.borrow();
        let ps = tree.get_elements_by_tag_name("p");
        assert!(!ps.is_empty(), "should have a <p> element from script");
        let p_node = tree.get_node(ps[0]);
        assert!(
            p_node.computed_style.is_some(),
            "script-created element should get computed styles"
        );
    }

    #[test]
    fn test_load_html_with_scripts_computes_styles() {
        let html = r##"
        <html><body>
          <style>h1 { color: green; }</style>
          <h1>Title</h1>
          <script src="app.js"></script>
        </body></html>"##;

        let mut fetched = HashMap::new();
        fetched.insert("app.js".to_string(), "// no-op".to_string());

        let mut engine = Engine::new();
        engine.load_html_with_scripts(html, &fetched);

        let tree = engine.tree.borrow();
        let h1_id = tree.get_elements_by_tag_name("h1")[0];
        let h1_node = tree.get_node(h1_id);
        assert!(
            h1_node.computed_style.is_some(),
            "load_html_with_scripts should compute styles"
        );
    }

    // -----------------------------------------------------------------------
    // textarea property tests (require engine.runtime access)
    // -----------------------------------------------------------------------

    fn eval_js_via_runtime(html: &str, js: &str) -> String {
        let mut engine = Engine::new();
        engine.load_html(html);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval_to_string(js).unwrap()
    }

    // -- textarea.defaultValue --

    #[test]
    fn textarea_default_value_returns_text_content() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t">hello world</textarea></body></html>"#,
            r#"document.getElementById("t").defaultValue"#,
        );
        assert_eq!(s, "hello world");
    }

    #[test]
    fn textarea_default_value_setter_replaces_text_content() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t">old</textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").defaultValue = "new text""#).unwrap();
        let s = runtime.eval_to_string(r#"document.getElementById("t").defaultValue"#).unwrap();
        assert_eq!(s, "new text");
    }

    #[test]
    fn input_default_value_reflects_value_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><input id="i" value="initial" /></body></html>"#,
            r#"document.getElementById("i").defaultValue"#,
        );
        assert_eq!(s, "initial");
    }

    // -- textarea.maxLength --

    #[test]
    fn textarea_maxlength_defaults_to_minus_one() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").maxLength)"#,
        );
        assert_eq!(s, "-1");
    }

    #[test]
    fn textarea_maxlength_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" maxlength="10"></textarea></body></html>"#,
            r#"String(document.getElementById("t").maxLength)"#,
        );
        assert_eq!(s, "10");
    }

    #[test]
    fn textarea_maxlength_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").maxLength = 5"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").maxLength)"#).unwrap();
        assert_eq!(s, "5");
    }

    #[test]
    fn textarea_value_truncated_by_maxlength() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t" maxlength="5"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").value = "hello world""#).unwrap();
        let s = runtime.eval_to_string(r#"document.getElementById("t").value"#).unwrap();
        assert_eq!(s, "hello");
    }

    // -- textarea.minLength --

    #[test]
    fn textarea_minlength_defaults_to_minus_one() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").minLength)"#,
        );
        assert_eq!(s, "-1");
    }

    #[test]
    fn textarea_minlength_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").minLength = 3"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").minLength)"#).unwrap();
        assert_eq!(s, "3");
    }

    #[test]
    fn textarea_validity_too_short() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t" minlength="5"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").value = "hi""#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").validity.tooShort)"#).unwrap();
        assert_eq!(s, "true");
    }

    // Old cheating test removed — honest version at bottom of file uses handle_type()

    // -- textarea.cols --

    #[test]
    fn textarea_cols_defaults_to_20() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").cols)"#,
        );
        assert_eq!(s, "20");
    }

    #[test]
    fn textarea_cols_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" cols="40"></textarea></body></html>"#,
            r#"String(document.getElementById("t").cols)"#,
        );
        assert_eq!(s, "40");
    }

    #[test]
    fn textarea_cols_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").cols = 60"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").cols)"#).unwrap();
        assert_eq!(s, "60");
    }

    // -- textarea.rows --

    #[test]
    fn textarea_rows_defaults_to_2() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").rows)"#,
        );
        assert_eq!(s, "2");
    }

    #[test]
    fn textarea_rows_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" rows="10"></textarea></body></html>"#,
            r#"String(document.getElementById("t").rows)"#,
        );
        assert_eq!(s, "10");
    }

    #[test]
    fn textarea_rows_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").rows = 8"#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").rows)"#).unwrap();
        assert_eq!(s, "8");
    }

    // -- textarea.wrap --

    #[test]
    fn textarea_wrap_defaults_to_soft() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"document.getElementById("t").wrap"#,
        );
        assert_eq!(s, "soft");
    }

    #[test]
    fn textarea_wrap_reflects_attribute() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t" wrap="hard"></textarea></body></html>"#,
            r#"document.getElementById("t").wrap"#,
        );
        assert_eq!(s, "hard");
    }

    #[test]
    fn textarea_wrap_setter_updates_attribute() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").wrap = "hard""#).unwrap();
        let s = runtime.eval_to_string(r#"document.getElementById("t").wrap"#).unwrap();
        assert_eq!(s, "hard");
    }

    // -- textarea.textLength --

    #[test]
    fn textarea_text_length_returns_value_length() {
        let mut engine = Engine::new();
        engine.load_html(r#"<html><body><textarea id="t"></textarea></body></html>"#);
        let runtime = engine.runtime.as_mut().unwrap();
        runtime.eval(r#"document.getElementById("t").value = "hello""#).unwrap();
        let s = runtime.eval_to_string(r#"String(document.getElementById("t").textLength)"#).unwrap();
        assert_eq!(s, "5");
    }

    #[test]
    fn textarea_text_length_zero_when_empty() {
        let s = eval_js_via_runtime(
            r#"<html><body><textarea id="t"></textarea></body></html>"#,
            r#"String(document.getElementById("t").textLength)"#,
        );
        assert_eq!(s, "0");
    }
