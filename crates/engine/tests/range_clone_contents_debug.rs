use braille_engine::Engine;

fn make_engine_with_common() -> Engine {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let common_js = std::fs::read_to_string(root.join("tests/wpt/dom/common.js")).unwrap();
    let harness_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharness.js")).unwrap();
    let report_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharnessreport.js")).unwrap();

    let mut engine = Engine::new();
    engine.load_html("<!doctype html><div id=log></div><body></body>");
    engine.eval_js(&harness_js).unwrap();
    engine.eval_js(&report_js).unwrap();
    engine.eval_js(&common_js).unwrap();
    engine
}

#[test]
fn range_clone_contents_isolated() {
    // cloneContents works in isolation
    let mut engine = Engine::new();
    engine.load_html("<!doctype html><body></body>");
    let r = engine.eval_js(r#"
        var r = document.createRange();
        r.detach();
        typeof r.cloneContents;
    "#);
    eprintln!("isolated typeof cloneContents = {:?}", r);
    assert_eq!(r.unwrap(), "function");
}

#[test]
fn range_clone_contents_after_common() {
    // cloneContents after common.js loads
    let mut engine = make_engine_with_common();
    let r = engine.eval_js(r#"
        var r = document.createRange();
        r.detach();
        typeof r.cloneContents;
    "#);
    eprintln!("after common typeof cloneContents = {:?}", r);
    assert_eq!(r.unwrap(), "function");
}

#[test]
fn range_clone_contents_full_repro() {
    // Full repro: common.js + the full inline script from Range-cloneContents.html
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let test_html = std::fs::read_to_string(root.join("tests/wpt/dom/ranges/Range-cloneContents.html")).unwrap();

    let last_script_start = test_html.rfind("<script>").unwrap() + 8;
    let last_script_end = test_html.rfind("</script>").unwrap();
    let inline = &test_html[last_script_start..last_script_end];

    let mut engine = make_engine_with_common();
    let r = engine.eval_js(inline);
    eprintln!("full inline result: {:?}", r.as_ref().map(|_| "ok"));
    if let Err(e) = &r {
        eprintln!("FULL REPRO ERROR: {}", e);
    }
    // We expect this to fail — confirms repro
}

#[test]
fn range_clone_contents_binary_search() {
    // Binary search: find the exact line that breaks cloneContents
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let test_html = std::fs::read_to_string(root.join("tests/wpt/dom/ranges/Range-cloneContents.html")).unwrap();

    let last_script_start = test_html.rfind("<script>").unwrap() + 8;
    let last_script_end = test_html.rfind("</script>").unwrap();
    let inline = &test_html[last_script_start..last_script_end];
    let lines: Vec<&str> = inline.lines().collect();

    eprintln!("Total inline lines: {}", lines.len());
    eprintln!("Line 412 (0-indexed 411): {:?}", lines.get(411));

    // The error is at line 412. Line 412 is inside a test() callback.
    // The line "range.cloneContents().childNodes" is at that point.
    // But cloneContents exists. So something EARLIER must overwrite it.

    // Try running lines 1..410 (everything BEFORE the failing line) and check
    let mut engine = make_engine_with_common();
    let prefix = lines[..410].join("\n");
    let r = engine.eval_js(&prefix);
    eprintln!("Lines 1-410 result: {:?}", r.as_ref().map(|_| "ok"));
    if let Err(e) = &r {
        eprintln!("ERROR at lines 1-410: {}", e);
        // Further narrow
        for checkpoint in [50, 100, 150, 200, 250, 300, 350, 400] {
            let mut e2 = make_engine_with_common();
            let chunk = lines[..checkpoint].join("\n");
            let r2 = e2.eval_js(&chunk);
            eprintln!("Lines 1-{}: {:?}", checkpoint, r2.as_ref().map(|_| "ok"));
            if let Err(err) = &r2 {
                eprintln!("  ERROR: {}", err);
                // Narrow further in 10-line steps from previous checkpoint
                let prev = if checkpoint > 50 { checkpoint - 50 } else { 0 };
                for step in (prev..=checkpoint).step_by(10) {
                    let mut e3 = make_engine_with_common();
                    let chunk2 = lines[..step].join("\n");
                    let r3 = e3.eval_js(&chunk2);
                    if let Err(err2) = &r3 {
                        eprintln!("  Lines 1-{}: ERROR: {}", step, err2);
                        // Final: line by line
                        let prev2 = if step > 10 { step - 10 } else { 0 };
                        for line_n in prev2..=step {
                            let mut e4 = make_engine_with_common();
                            let chunk3 = lines[..line_n].join("\n");
                            let r4 = e4.eval_js(&chunk3);
                            if let Err(err3) = &r4 {
                                eprintln!("  EXACT LINE {}: ERROR: {}", line_n, err3);
                                eprintln!("  Content: {:?}", lines.get(line_n.saturating_sub(1)));
                                break;
                            }
                        }
                        break;
                    }
                }
                break;
            }
        }
    } else {
        // Lines 1-410 pass. So the issue is the test() wrapper at line 411+
        // Check if cloneContents still works after those 410 lines
        let r2 = engine.eval_js("typeof document.createRange().cloneContents");
        eprintln!("typeof cloneContents after 410 lines: {:?}", r2);

        // Try the exact failing block
        let r3 = engine.eval_js(r#"
            var __test_range = document.createRange();
            __test_range.detach();
            __test_range.cloneContents().childNodes;
        "#);
        eprintln!("direct cloneContents after 410 lines: {:?}", r3);

        // Try the test() wrapper version
        let r4 = engine.eval_js(r#"
            test(function() {
                var range = document.createRange();
                range.detach();
                assert_array_equals(range.cloneContents().childNodes, []);
            }, "Range.detach()");
        "#);
        eprintln!("test() wrapped cloneContents: {:?}", r4);
        if let Err(e) = &r4 {
            eprintln!("TEST WRAPPER ERROR: {}", e);
        }
    }
}
