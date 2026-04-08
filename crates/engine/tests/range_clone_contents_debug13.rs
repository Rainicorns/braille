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
fn id_test_shadows_test_function() {
    let mut engine = make_engine_with_harness();

    let r1 = engine.eval_js("typeof test");
    eprintln!("typeof test before: {:?}", r1);

    engine.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        d.id = "test";
    });"#).unwrap();

    let r2 = engine.eval_js("typeof test");
    eprintln!("typeof test after id=test: {:?}", r2);

    let r3 = engine.eval_js("typeof window.test");
    eprintln!("typeof window.test: {:?}", r3);

    // The named element access creates window.test pointing to the element
    // which shadows the testharness.js test() function!

    // Verify: use globalThis.test or explicit reference
    let r4 = engine.eval_js("test.tagName || 'not-element'");
    eprintln!("test.tagName: {:?}", r4);
}
