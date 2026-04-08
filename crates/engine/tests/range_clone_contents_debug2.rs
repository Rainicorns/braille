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
fn test_wrapper_breaks_clonecontents() {
    let mut engine = make_engine_with_common();

    // Works directly
    let r1 = engine.eval_js(r#"
        var r = document.createRange();
        typeof r.cloneContents;
    "#);
    eprintln!("direct: {:?}", r1);

    // Fails inside test()
    let r2 = engine.eval_js(r#"
        test(function() {
            var range = document.createRange();
            range.detach();
            assert_array_equals(range.cloneContents().childNodes, []);
        }, "test1");
    "#);
    eprintln!("inside test(): {:?}", r2);

    // Is it the test() wrapper or the assert?
    let r3 = engine.eval_js(r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "test2");
    "#);
    eprintln!("test() no assert: {:?}", r3);

    // Is it test() at all? Try a plain function call
    let r4 = engine.eval_js(r#"
        (function() {
            var range = document.createRange();
            range.cloneContents();
        })();
        'ok';
    "#);
    eprintln!("IIFE: {:?}", r4);

    // Is it createRange inside test()?
    let r5 = engine.eval_js(r#"
        test(function() {
            var range = document.createRange();
            assert_equals(typeof range.cloneContents, "function");
        }, "test3");
    "#);
    eprintln!("typeof inside test(): {:?}", r5);

    // Without common.js - just harness
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let harness_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharness.js")).unwrap();
    let report_js = std::fs::read_to_string(root.join("tests/wpt/resources/testharnessreport.js")).unwrap();

    let mut engine2 = Engine::new();
    engine2.load_html("<!doctype html><body></body>");
    engine2.eval_js(&harness_js).unwrap();
    engine2.eval_js(&report_js).unwrap();

    let r6 = engine2.eval_js(r#"
        test(function() {
            var range = document.createRange();
            range.detach();
            range.cloneContents();
        }, "no-common test");
    "#);
    eprintln!("without common.js: {:?}", r6);
}
