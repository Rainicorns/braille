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
fn setup_in_separate_eval() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let common_js = std::fs::read_to_string(root.join("tests/wpt/dom/common.js")).unwrap();

    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // Remove the auto-init from common.js, load defs only
    let no_init = common_js.replace(
        "if (\"setup\" in window) {\n    setup(setupRangeTests);\n} else {\n    // Presumably we're running from within an iframe or something\n    setupRangeTests();\n}",
        "// skipped init"
    );

    // Load function defs in one eval, call setupRangeTests in a second eval
    let mut engine = make_engine_with_harness();
    engine.eval_js(&no_init).unwrap();
    engine.eval_js("setup(setupRangeTests);").unwrap();
    let r1 = engine.eval_js(test_code);
    eprintln!("defs+separate setup: {:?}", r1);

    // Same but with setupRangeTests() directly
    let mut engine2 = make_engine_with_harness();
    engine2.eval_js(&no_init).unwrap();
    engine2.eval_js("setupRangeTests();").unwrap();
    let r2 = engine2.eval_js(test_code);
    eprintln!("defs+separate setupRangeTests: {:?}", r2);

    // What about: load defs in one eval, add JUST the init code back
    let mut engine3 = make_engine_with_harness();
    engine3.eval_js(&no_init).unwrap();
    let init_code = r#"if ("setup" in window) { setup(setupRangeTests); } else { setupRangeTests(); }"#;
    engine3.eval_js(init_code).unwrap();
    let r3 = engine3.eval_js(test_code);
    eprintln!("defs+separate init block: {:?}", r3);
}

#[test]
fn minimal_repro_setup_breaks_range() {
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // What is the MINIMAL code that, when combined with setup(), breaks test()+cloneContents?
    let mut engine = make_engine_with_harness();
    // Inline a minimal setupRangeTests
    let r = engine.eval_js(r#"
        function mySetup() {
            var testDiv = document.createElement("div");
            testDiv.id = "test";
            document.body.insertBefore(testDiv, document.body.firstChild);
        }
        setup(mySetup);
    "#);
    eprintln!("minimal setup: {:?}", r);
    let r1 = engine.eval_js(test_code);
    eprintln!("after minimal setup: {:?}", r1);

    // Even more minimal - just setup with empty function
    let mut engine2 = make_engine_with_harness();
    engine2.eval_js("setup(function() {});").unwrap();
    let r2 = engine2.eval_js(test_code);
    eprintln!("after setup(empty): {:?}", r2);

    // What if setup() is in the SAME eval as the test?
    let mut engine3 = make_engine_with_harness();
    let r3 = engine3.eval_js(&format!("setup(function() {{}});\n{}", test_code));
    eprintln!("setup+test same eval: {:?}", r3);
}
