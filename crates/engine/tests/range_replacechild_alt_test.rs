use braille_engine::Engine;

#[test]
fn run_replacechild_alt_html() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let mut engine = Engine::new();

    // Inject minimal harness
    let harness = r#"
        var results = [];
        function test(fn, name) {
            var result = { name: name || "", status: 0, message: "" };
            try { fn(); } catch(e) { result.status = 1; result.message = e.message || String(e); }
            results.push(result);
        }
        function assert_equals(a, b, msg) { if (a !== b) throw new Error(msg + ": expected " + JSON.stringify(b) + " got " + JSON.stringify(a)); }
        function assert_true(a, msg) { if (a !== true) throw new Error(msg); }
        function assert_false(a, msg) { if (a !== false) throw new Error(msg); }
        function assert_throws_dom(name, fn) { try { fn(); throw new Error("expected " + name); } catch(e) { if (e.name !== name) throw e; } }
    "#;

    engine.load_html("<!doctype html><html><body></body></html>");
    engine.eval_js(harness).unwrap();

    // Run the alt test inline
    let test_js = std::fs::read_to_string(root.join("tests/wpt/dom/ranges/Range-mutations-replaceChild.alt.html")).unwrap();
    // Extract JS from <script>...</script>
    let start = test_js.find("<script>").unwrap() + 8;
    let end = test_js.rfind("</script>").unwrap();
    let js = &test_js[start..end];

    let result = engine.eval_js(js);
    match &result {
        Ok(_) => {},
        Err(e) => eprintln!("eval error: {}", e),
    }

    let summary = engine.eval_js("JSON.stringify(results.map(function(r) { return r.status + ': ' + r.name + (r.message ? ' — ' + r.message : ''); }))").unwrap();
    eprintln!("results: {}", summary);

    let failed = engine.eval_js("results.filter(function(r){return r.status !== 0;}).length").unwrap();
    eprintln!("failed: {}", failed);
    assert_eq!(failed, "0", "some tests failed");
}
