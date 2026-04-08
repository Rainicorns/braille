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
fn exact_combination() {
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // Exact chunk 4
    let mut e1 = make_engine_with_harness();
    let r = e1.eval_js(r#"setup(function() {
        var testDiv = document.createElement("div");
        testDiv.id = "test";
        document.body.insertBefore(testDiv, document.body.firstChild);
        var xmlDoctype = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
        var xmlDoc = document.implementation.createDocument(null, null, xmlDoctype);
    });"#);
    eprintln!("chunk4 setup: {:?}", r);
    let r1 = e1.eval_js(test_code);
    eprintln!("chunk4: {:?}", r1.as_ref().map(|_| "ok"));

    // Without insertBefore but with createDocument
    let mut e2 = make_engine_with_harness();
    e2.eval_js(r#"setup(function() {
        var xmlDoctype = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
        var xmlDoc = document.implementation.createDocument(null, null, xmlDoctype);
    });"#).unwrap();
    let r2 = e2.eval_js(test_code);
    eprintln!("no insertBefore: {:?}", r2.as_ref().map(|_| "ok"));

    // With insertBefore but no createDocument
    let mut e3 = make_engine_with_harness();
    e3.eval_js(r#"setup(function() {
        var testDiv = document.createElement("div");
        testDiv.id = "test";
        document.body.insertBefore(testDiv, document.body.firstChild);
    });"#).unwrap();
    let r3 = e3.eval_js(test_code);
    eprintln!("no createDocument: {:?}", r3.as_ref().map(|_| "ok"));

    // Both but in separate setup() calls
    let mut e4 = make_engine_with_harness();
    e4.eval_js(r#"
        var testDiv = document.createElement("div");
        testDiv.id = "test";
        document.body.insertBefore(testDiv, document.body.firstChild);
    "#).unwrap();
    e4.eval_js(r#"setup(function() {
        var xmlDoctype = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
        var xmlDoc = document.implementation.createDocument(null, null, xmlDoctype);
    });"#).unwrap();
    let r4 = e4.eval_js(test_code);
    eprintln!("split into insertBefore then setup+createDoc: {:?}", r4.as_ref().map(|_| "ok"));

    // insertBefore + createDocument in setup but NO doctype
    let mut e5 = make_engine_with_harness();
    e5.eval_js(r#"setup(function() {
        var testDiv = document.createElement("div");
        testDiv.id = "test";
        document.body.insertBefore(testDiv, document.body.firstChild);
        var xmlDoc = document.implementation.createDocument(null, null);
    });"#).unwrap();
    let r5 = e5.eval_js(test_code);
    eprintln!("insertBefore + createDoc no doctype: {:?}", r5.as_ref().map(|_| "ok"));

    // insertBefore + createDocument WITH doctype
    let mut e6 = make_engine_with_harness();
    e6.eval_js(r#"setup(function() {
        var testDiv = document.createElement("div");
        document.body.insertBefore(testDiv, document.body.firstChild);
        var dt = document.implementation.createDocumentType("x", "", "");
        var xmlDoc = document.implementation.createDocument(null, null, dt);
    });"#).unwrap();
    let r6 = e6.eval_js(test_code);
    eprintln!("insertBefore + createDoc WITH doctype: {:?}", r6.as_ref().map(|_| "ok"));
}
