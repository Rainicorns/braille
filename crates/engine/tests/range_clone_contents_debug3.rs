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
fn setup_call_breaks_test() {
    // common.js calls setup(setupRangeTests). Does setup() alone break things?
    let mut engine = make_engine_with_harness();

    // Before setup() — works
    let r1 = engine.eval_js(r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "before setup");
    "#);
    eprintln!("before setup(): {:?}", r1);

    // Call setup() with a simple function
    let r2 = engine.eval_js(r#"
        setup(function() {});
    "#);
    eprintln!("setup() call: {:?}", r2);

    // After setup() — broken?
    let r3 = engine.eval_js(r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "after setup");
    "#);
    eprintln!("after setup(): {:?}", r3);

    if r3.is_err() {
        // It's setup() that breaks it. What does setup() do?
        // Maybe it changes how test() executes callbacks.
        // Check if the range itself is broken or the method resolution
        let r4 = engine.eval_js(r#"
            test(function() {
                var x = 1 + 1;
            }, "simple math");
        "#);
        eprintln!("simple test after setup: {:?}", r4);

        let r5 = engine.eval_js(r#"
            test(function() {
                document.createElement("div");
            }, "createElement in test");
        "#);
        eprintln!("createElement in test: {:?}", r5);

        let r6 = engine.eval_js(r#"
            test(function() {
                var r = document.createRange();
                typeof r;
            }, "createRange in test");
        "#);
        eprintln!("createRange in test: {:?}", r6);

        let r7 = engine.eval_js(r#"
            test(function() {
                var r = document.createRange();
                r.setStart(document, 0);
            }, "setStart in test");
        "#);
        eprintln!("setStart in test: {:?}", r7);

        let r8 = engine.eval_js(r#"
            test(function() {
                var r = document.createRange();
                var t = r.cloneContents;
                typeof t;
            }, "get cloneContents ref");
        "#);
        eprintln!("get cloneContents ref: {:?}", r8);
    }
}
