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
fn narrow_setup_callback() {
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // setup(fn that creates element) breaks
    let mut e1 = make_engine_with_harness();
    e1.eval_js(r#"setup(function() { document.createElement("div"); });"#).unwrap();
    let r1 = e1.eval_js(test_code);
    eprintln!("createElement in setup: {:?}", r1);

    // setup(fn that does body.insertBefore) breaks?
    let mut e2 = make_engine_with_harness();
    e2.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        document.body.insertBefore(d, document.body.firstChild);
    });"#).unwrap();
    let r2 = e2.eval_js(test_code);
    eprintln!("insertBefore in setup: {:?}", r2);

    // setup(fn that does body.appendChild)
    let mut e3 = make_engine_with_harness();
    e3.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        document.body.appendChild(d);
    });"#).unwrap();
    let r3 = e3.eval_js(test_code);
    eprintln!("appendChild in setup: {:?}", r3);

    // setup(fn that just reads DOM)
    let mut e4 = make_engine_with_harness();
    e4.eval_js(r#"setup(function() { document.body.firstChild; });"#).unwrap();
    let r4 = e4.eval_js(test_code);
    eprintln!("read DOM in setup: {:?}", r4);

    // setup(fn that sets a global var)
    let mut e5 = make_engine_with_harness();
    e5.eval_js(r#"setup(function() { window.__test = 1; });"#).unwrap();
    let r5 = e5.eval_js(test_code);
    eprintln!("set global in setup: {:?}", r5);

    // NOT using setup() — calling directly
    let mut e6 = make_engine_with_harness();
    e6.eval_js(r#"(function() {
        var d = document.createElement("div");
        document.body.insertBefore(d, document.body.firstChild);
    })();"#).unwrap();
    let r6 = e6.eval_js(test_code);
    eprintln!("direct IIFE (no setup): {:?}", r6);

    // What does setup() actually DO that's special?
    // Let's check: is it the setup() call itself or what it configures?
    let mut e7 = make_engine_with_harness();
    e7.eval_js(r#"
        setup(function() { var x = 1; });
    "#).unwrap();
    let r7 = e7.eval_js(test_code);
    eprintln!("setup with trivial fn: {:?}", r7);

    // setup() with properties object
    let mut e8 = make_engine_with_harness();
    e8.eval_js(r#"
        setup({explicit_done: false});
    "#).unwrap();
    let r8 = e8.eval_js(test_code);
    eprintln!("setup with properties: {:?}", r8);
}
