use braille_engine::Engine;

fn make_engine_with_harness() -> Engine {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let harness_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharness.js")).unwrap();
    let report_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharnessreport.js")).unwrap();

    let mut engine = Engine::new();
    engine.load_html("<!doctype html><div id=log></div><body></body>");
    engine.eval_js(&harness_js).unwrap();
    engine.eval_js(&report_js).unwrap();
    engine
}

#[test]
fn isolate_function_name_conflict() {
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // Test: does defining ensurePreInsertionValidity break things?
    let mut engine = make_engine_with_harness();
    engine.eval_js("function ensurePreInsertionValidity() {}").unwrap();
    let r1 = engine.eval_js(test_code);
    eprintln!("after ensurePreInsertionValidity: {:?}", r1);

    // Test: does defining isText break things?
    let mut engine2 = make_engine_with_harness();
    engine2.eval_js("function isText(node) { return node.nodeType == 3; }").unwrap();
    let r2 = engine2.eval_js(test_code);
    eprintln!("after isText: {:?}", r2);

    // Test: does defining isDoctype break things?
    let mut engine3 = make_engine_with_harness();
    engine3.eval_js("function isDoctype(node) { return node.nodeType == 10; }").unwrap();
    let r3 = engine3.eval_js(test_code);
    eprintln!("after isDoctype: {:?}", r3);

    // Test: does defining isElement break things?
    let mut engine4 = make_engine_with_harness();
    engine4.eval_js("function isElement(node) { return node.nodeType == 1; }").unwrap();
    let r4 = engine4.eval_js(test_code);
    eprintln!("after isElement: {:?}", r4);

    // Try a long function with lots of code
    let mut engine5 = make_engine_with_harness();
    engine5.eval_js(r#"
        function bigFunction(a, b, c) {
            if (a) { return 1; }
            switch (b) {
                case 1: return 2;
                case 2: return 3;
                default: break;
            }
            return 0;
        }
    "#).unwrap();
    let r5 = engine5.eval_js(test_code);
    eprintln!("after bigFunction: {:?}", r5);

    // Load common.js lines 1-1037 (works) then add JUST line 1038
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let common_js = std::fs::read_to_string(root.join("tests/wpt/dom/common.js")).unwrap();
    let lines: Vec<&str> = common_js.lines().collect();

    let mut engine6 = make_engine_with_harness();
    let chunk1037 = lines[..1037].join("\n");
    engine6.eval_js(&chunk1037).ok();
    let r6 = engine6.eval_js(test_code);
    eprintln!("common.js 1-1037: {:?}", r6);

    // Now eval just the closing brace
    engine6.eval_js("}").ok();
    let r7 = engine6.eval_js(test_code);
    eprintln!("after adding closing brace: {:?}", r7);

    // Actually, the issue might be that lines 1-1037 has an unclosed function
    // and line 1038 closes it, changing parsing state. Let me verify.
    let mut engine7 = make_engine_with_harness();
    let chunk1038 = lines[..1038].join("\n");
    let load_result = engine7.eval_js(&chunk1038);
    eprintln!("common.js 1-1038 load: {:?}", load_result.as_ref().map(|_| "ok"));
    if let Err(e) = &load_result {
        eprintln!("LOAD ERROR: {}", e);
    }
    let r8 = engine7.eval_js(test_code);
    eprintln!("common.js 1-1038 then test: {:?}", r8);
}
