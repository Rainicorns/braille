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
fn narrow_doctype_and_create_document() {
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // Just createDocumentType
    let mut e1 = make_engine_with_harness();
    e1.eval_js(r#"setup(function() {
        var dt = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
    });"#).unwrap();
    let r1 = e1.eval_js(test_code);
    eprintln!("createDocumentType only: {:?}", r1.as_ref().map(|_| "ok"));

    // Just createDocument(null, null)
    let mut e2 = make_engine_with_harness();
    e2.eval_js(r#"setup(function() {
        var xmlDoc = document.implementation.createDocument(null, null);
    });"#).unwrap();
    let r2 = e2.eval_js(test_code);
    eprintln!("createDocument(null,null): {:?}", r2.as_ref().map(|_| "ok"));

    // createDocument with doctype
    let mut e3 = make_engine_with_harness();
    e3.eval_js(r#"setup(function() {
        var dt = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
        var xmlDoc = document.implementation.createDocument(null, null, dt);
    });"#).unwrap();
    let r3 = e3.eval_js(test_code);
    eprintln!("createDocument+doctype: {:?}", r3.as_ref().map(|_| "ok"));

    // createDocument WITHOUT setup wrapper
    let mut e4 = make_engine_with_harness();
    e4.eval_js(r#"
        var dt = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
        var xmlDoc = document.implementation.createDocument(null, null, dt);
    "#).unwrap();
    let r4 = e4.eval_js(test_code);
    eprintln!("createDocument+doctype (no setup): {:?}", r4.as_ref().map(|_| "ok"));

    // createDocument WITHOUT setup, then check Range directly
    let mut e5 = make_engine_with_harness();
    e5.eval_js(r#"
        var dt = document.implementation.createDocumentType("qorflesnorf", "abcde", "x\"'y");
        var xmlDoc = document.implementation.createDocument(null, null, dt);
    "#).unwrap();
    let r5 = e5.eval_js(r#"
        var r = document.createRange();
        typeof r.cloneContents;
    "#);
    eprintln!("typeof cloneContents after createDocument: {:?}", r5);

    // Is it createDocument that's the problem, or the doctype?
    let mut e6 = make_engine_with_harness();
    e6.eval_js(r#"setup(function() {
        document.implementation.createDocument(null, null);
    });"#).unwrap();
    let r6 = e6.eval_js(test_code);
    eprintln!("createDocument(null,null) in setup: {:?}", r6.as_ref().map(|_| "ok"));

    // createDocument with a namespace
    let mut e7 = make_engine_with_harness();
    e7.eval_js(r#"setup(function() {
        document.implementation.createDocument("http://www.w3.org/1999/xhtml", "html");
    });"#).unwrap();
    let r7 = e7.eval_js(test_code);
    eprintln!("createDocument with ns: {:?}", r7.as_ref().map(|_| "ok"));
}
