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

    // ---- ScriptDescriptor / external script tests ----

    #[test]
    fn test_parse_and_collect_scripts_identifies_inline() {
        let html = r#"
        <html><body>
          <script>console.log("hello")</script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 1);
        match &descriptors[0] {
            ScriptDescriptor::Inline(text, _) => {
                assert!(text.contains("console.log"), "inline script text: {}", text);
            }
            other => panic!("expected Inline, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_and_collect_scripts_identifies_external() {
        let html = r#"
        <html><body>
          <script src="https://example.com/app.js"></script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 1);
        match &descriptors[0] {
            ScriptDescriptor::External(url, _) => {
                assert_eq!(url, "https://example.com/app.js");
            }
            other => panic!("expected External, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_and_collect_scripts_mixed_document_order() {
        let html = r#"
        <html><body>
          <script>var x = 1;</script>
          <script src="https://cdn.example.com/lib.js"></script>
          <script>var y = 2;</script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 3, "should find 3 scripts");

        match &descriptors[0] {
            ScriptDescriptor::Inline(text, _) => assert!(text.contains("var x = 1")),
            _ => panic!("first script should be Inline"),
        }
        match &descriptors[1] {
            ScriptDescriptor::External(url, _) => assert_eq!(url, "https://cdn.example.com/lib.js"),
            _ => panic!("second script should be External"),
        }
        match &descriptors[2] {
            ScriptDescriptor::Inline(text, _) => assert!(text.contains("var y = 2")),
            _ => panic!("third script should be Inline"),
        }
    }

    #[test]
    fn test_execute_scripts_runs_inline() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script>
            let el = document.createElement("p");
            el.textContent = "inline works";
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);
        let fetched = HashMap::new();
        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));

        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("inline works"),
            "inline script should execute: {}",
            snapshot
        );
    }

    #[test]
    fn test_execute_scripts_runs_external_from_fetched() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/app.js"></script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/app.js".to_string(),
            concat!(
                "let el = document.createElement(\"p\");",
                "el.textContent = \"external works\";",
                "document.getElementById(\"target\").appendChild(el);"
            )
            .to_string(),
        );

        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("external works"),
            "external script should execute: {}",
            snapshot
        );
    }

    #[test]
    fn test_execute_scripts_skips_missing_external() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/missing.js"></script>
          <script>
            let el = document.createElement("p");
            el.textContent = "after missing";
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);
        let fetched = HashMap::new();
        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));

        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("after missing"),
            "inline script after missing external should run: {}",
            snapshot
        );
    }

    #[test]
    fn test_load_html_with_scripts_end_to_end() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/lib.js"></script>
          <script>
            let el = document.createElement("p");
            el.textContent = "value is " + globalValue;
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/lib.js".to_string(),
            "var globalValue = 42;".to_string(),
        );

        let mut engine = Engine::new();
        engine.load_html_with_scripts(html, &fetched);
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("value is 42"),
            "external script should set global used by inline: {}",
            snapshot
        );
    }

    #[test]
    fn test_mixed_inline_and_external_execute_in_order() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script>var order = [];</script>
          <script src="https://example.com/a.js"></script>
          <script>order.push("inline2");</script>
          <script src="https://example.com/b.js"></script>
          <script>
            let el = document.createElement("p");
            el.textContent = order.join(",");
            document.getElementById("target").appendChild(el);
          </script>
        </body></html>"#;

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/a.js".to_string(),
            "order.push(\"extA\");".to_string(),
        );
        fetched.insert(
            "https://example.com/b.js".to_string(),
            "order.push(\"extB\");".to_string(),
        );

        let mut engine = Engine::new();
        engine.load_html_with_scripts(html, &fetched);
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("extA,inline2,extB"),
            "scripts should execute in document order: {}",
            snapshot
        );
    }

    #[test]
    fn test_script_with_src_and_text_src_wins() {
        let html = r#"
        <html><body>
          <div id="target"></div>
          <script src="https://example.com/real.js">
            let bad = document.createElement("p");
            bad.textContent = "INLINE SHOULD NOT RUN";
            document.getElementById("target").appendChild(bad);
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        let descriptors = engine.parse_and_collect_scripts(html);

        assert_eq!(descriptors.len(), 1);
        match &descriptors[0] {
            ScriptDescriptor::External(url, _) => {
                assert_eq!(url, "https://example.com/real.js");
            }
            other => panic!("should be External when src is present, got {:?}", other),
        }

        let mut fetched = HashMap::new();
        fetched.insert(
            "https://example.com/real.js".to_string(),
            concat!(
                "let el = document.createElement(\"p\");",
                "el.textContent = \"EXTERNAL RAN\";",
                "document.getElementById(\"target\").appendChild(el);"
            )
            .to_string(),
        );

        engine.execute_scripts(&descriptors, &FetchedResources::scripts_only(fetched.clone()));
        let snapshot = engine.snapshot(SnapMode::Accessibility);
        assert!(
            snapshot.contains("EXTERNAL RAN"),
            "external content should run: {}",
            snapshot
        );
        assert!(
            !snapshot.contains("INLINE SHOULD NOT RUN"),
            "inline text should be ignored when src present: {}",
            snapshot
        );
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

    #[test]
    fn test_request_submit_validates_before_submitting() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="email" type="email" required value="" />
            <button type="submit">Submit</button>
          </form>
          <script>
            var submitted = false;
            var form = document.getElementById('myform');
            form.addEventListener('submit', function(e) {
              submitted = true;
              e.preventDefault();
            });
            form.requestSubmit();
            window.__submitted = submitted;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Validation should fail (required email is empty), so submit event should NOT fire
        let result = engine.eval_js("window.__submitted").unwrap();
        assert_eq!(result, "false", "requestSubmit should not fire submit when validation fails");
    }

    #[test]
    fn test_request_submit_fires_submit_when_valid() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="email" type="email" value="test@example.com" />
            <button type="submit">Submit</button>
          </form>
          <script>
            var submitted = false;
            var form = document.getElementById('myform');
            form.addEventListener('submit', function(e) {
              submitted = true;
              e.preventDefault();
            });
            form.requestSubmit();
            window.__submitted = submitted;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // Validation passes, submit event should fire
        let result = engine.eval_js("window.__submitted").unwrap();
        assert_eq!(result, "true", "requestSubmit should fire submit when form is valid");
    }

    #[test]
    fn test_request_submit_respects_prevent_default() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="name" value="hello" />
          </form>
          <script>
            var submitFired = false;
            var preventDefaultCalled = false;
            var form = document.getElementById('myform');
            form.addEventListener('submit', function(e) {
              submitFired = true;
              preventDefaultCalled = true;
              e.preventDefault();
            });
            form.requestSubmit();
            window.__submitFired = submitFired;
            window.__preventDefaultCalled = preventDefaultCalled;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let fired = engine.eval_js("window.__submitFired").unwrap();
        assert_eq!(fired, "true", "submit event should fire");
        let prevented = engine.eval_js("window.__preventDefaultCalled").unwrap();
        assert_eq!(prevented, "true", "preventDefault should have been called");
    }

    #[test]
    fn test_request_submit_with_submitter() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input name="name" value="hello" />
            <button id="btn" type="submit">Go</button>
          </form>
          <script>
            var capturedSubmitter = null;
            var form = document.getElementById('myform');
            var btn = document.getElementById('btn');
            form.addEventListener('submit', function(e) {
              capturedSubmitter = e.submitter;
              e.preventDefault();
            });
            form.requestSubmit(btn);
            window.__submitterTag = capturedSubmitter ? capturedSubmitter.tagName : 'none';
            window.__submitterId = capturedSubmitter ? capturedSubmitter.id : 'none';
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let tag = engine.eval_js("window.__submitterTag").unwrap();
        assert_eq!(tag, "BUTTON", "submitter should be the button element");
        let id = engine.eval_js("window.__submitterId").unwrap();
        assert_eq!(id, "btn", "submitter should have id=btn");
    }

    #[test]
    fn test_request_submit_fires_invalid_on_failed_validation() {
        let html = r#"
        <html><body>
          <form id="myform">
            <input id="inp" name="email" type="email" required value="" />
          </form>
          <script>
            var invalidFired = false;
            var inp = document.getElementById('inp');
            inp.addEventListener('invalid', function(e) {
              invalidFired = true;
            });
            var form = document.getElementById('myform');
            form.requestSubmit();
            window.__invalidFired = invalidFired;
          </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.eval_js("window.__invalidFired").unwrap();
        assert_eq!(result, "true", "invalid event should fire on failed validation");
    }

    #[test]
    fn test_step_mismatch_number_input() {
        let html = r#"<html><body>
            <input id="a" type="number" step="3" min="0" value="5">
            <input id="b" type="number" step="3" min="0" value="6">
            <input id="c" type="number" value="1.5">
            <input id="d" type="number" step="any" value="3.14159">
            <input id="e" type="number" step="0.1" min="0" value="0.3">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // 5 is not divisible by step=3 from min=0 -> stepMismatch=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('a').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":true"),
            "5 is not a multiple of step 3 from min 0: {}",
            result
        );
        assert!(
            result.contains("\"valid\":false"),
            "should be invalid: {}",
            result
        );

        // 6 IS divisible by step=3 from min=0 -> stepMismatch=false
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('b').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":false"),
            "6 is a multiple of step 3 from min 0: {}",
            result
        );

        // default step=1 for number, 1.5 is not a whole number -> stepMismatch=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('c').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":true"),
            "1.5 is not a multiple of default step 1: {}",
            result
        );

        // step="any" means no step mismatch
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('d').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":false"),
            "step=any should never have stepMismatch: {}",
            result
        );

        // 0.3 with step=0.1 from min=0 -> should be valid
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('e').validity)")
            .unwrap();
        assert!(
            result.contains("\"stepMismatch\":false"),
            "0.3 is a multiple of step 0.1 from min 0: {}",
            result
        );
    }

    #[test]
    fn test_bad_input_number() {
        let html = r#"<html><body>
            <input id="a" type="number" value="abc">
            <input id="b" type="number" value="42">
            <input id="c" type="number" value="">
            <input id="d" type="date" value="not-a-date">
            <input id="e" type="date" value="2024-01-15">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // "abc" is not a valid number -> badInput=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('a').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":true"),
            "abc is not a valid number: {}",
            result
        );
        assert!(
            result.contains("\"valid\":false"),
            "should be invalid: {}",
            result
        );

        // "42" is a valid number -> badInput=false
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('b').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":false"),
            "42 is a valid number: {}",
            result
        );

        // empty value -> badInput=false (no input to be bad)
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('c').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":false"),
            "empty value should not be badInput: {}",
            result
        );

        // "not-a-date" is not a valid date -> badInput=true
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('d').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":true"),
            "not-a-date is not a valid date: {}",
            result
        );

        // "2024-01-15" is a valid date -> badInput=false
        let result = engine
            .eval_js("JSON.stringify(document.getElementById('e').validity)")
            .unwrap();
        assert!(
            result.contains("\"badInput\":false"),
            "2024-01-15 is a valid date: {}",
            result
        );
    }

    #[test]
    fn test_check_validity_fires_invalid_event() {
        let html = r#"<html><body>
            <input id="inp" type="text" required value="">
            <script>
                window.__invalidFired = false;
                window.__invalidBubbled = false;
                window.__invalidCancelable = null;
                var inp = document.getElementById('inp');
                inp.addEventListener('invalid', function(e) {
                    window.__invalidFired = true;
                    window.__invalidCancelable = e.cancelable;
                });
                document.body.addEventListener('invalid', function(e) {
                    window.__invalidBubbled = true;
                });
                window.__checkResult = inp.checkValidity();
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        // checkValidity should return false for required empty field
        let result = engine.eval_js("String(window.__checkResult)").unwrap();
        assert_eq!(result, "false", "checkValidity should return false");

        // invalid event should have fired
        let result = engine.eval_js("String(window.__invalidFired)").unwrap();
        assert_eq!(result, "true", "invalid event should fire on checkValidity");

        // invalid event should NOT bubble
        let result = engine.eval_js("String(window.__invalidBubbled)").unwrap();
        assert_eq!(result, "false", "invalid event should not bubble");

        // invalid event should be cancelable
        let result = engine.eval_js("String(window.__invalidCancelable)").unwrap();
        assert_eq!(result, "true", "invalid event should be cancelable");
    }

    #[test]
    fn test_check_validity_no_event_when_valid() {
        let html = r#"<html><body>
            <input id="inp" type="text" value="hello">
            <script>
                window.__invalidFired = false;
                var inp = document.getElementById('inp');
                inp.addEventListener('invalid', function(e) {
                    window.__invalidFired = true;
                });
                window.__checkResult = inp.checkValidity();
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.eval_js("String(window.__checkResult)").unwrap();
        assert_eq!(result, "true", "checkValidity should return true");

        let result = engine.eval_js("String(window.__invalidFired)").unwrap();
        assert_eq!(result, "false", "invalid event should not fire when valid");
    }

    #[test]
    fn test_report_validity_fires_invalid_event() {
        let html = r#"<html><body>
            <input id="inp" type="number" required value="">
            <script>
                window.__invalidFired = false;
                var inp = document.getElementById('inp');
                inp.addEventListener('invalid', function(e) {
                    window.__invalidFired = true;
                });
                window.__reportResult = inp.reportValidity();
            </script>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine.eval_js("String(window.__reportResult)").unwrap();
        assert_eq!(result, "false", "reportValidity should return false");

        let result = engine.eval_js("String(window.__invalidFired)").unwrap();
        assert_eq!(result, "true", "invalid event should fire on reportValidity");
    }

    #[test]
    fn test_validation_message_step_mismatch_and_bad_input() {
        let html = r#"<html><body>
            <input id="step" type="number" step="5" min="0" value="3">
            <input id="bad" type="number" value="xyz">
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);

        let result = engine
            .eval_js("document.getElementById('step').validationMessage")
            .unwrap();
        assert!(
            result.contains("step"),
            "validationMessage should mention step: {}",
            result
        );

        let result = engine
            .eval_js("document.getElementById('bad').validationMessage")
            .unwrap();
        assert!(
            result.contains("valid value"),
            "validationMessage should mention valid value: {}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // textarea property tests
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

    // ---- meta refresh tests ----

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

    #[test]
    fn textarea_validity_too_long() {
        let html = r#"
        <html><body>
          <textarea id="t" maxlength="3"></textarea>
        </body></html>"#;

        let mut engine = Engine::new();
        engine.load_html(html);
        engine.snapshot(SnapMode::Accessibility);

        // Use the public .value setter via handle_type (not __props._value directly)
        engine.handle_type("#t", "hello").unwrap();

        let too_long = engine.eval_js(
            "document.getElementById('t').validity.tooLong"
        ).unwrap();
        assert_eq!(
            too_long, "true",
            "textarea with maxlength=3 and value='hello' should have validity.tooLong=true, got: {}",
            too_long
        );

        let valid = engine.eval_js(
            "document.getElementById('t').validity.valid"
        ).unwrap();
        assert_eq!(
            valid, "false",
            "textarea with tooLong should not be valid, got: {}",
            valid
        );
    }
