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
fn is_it_the_id() {
    let test_code = r#"
        test(function() {
            var range = document.createRange();
            range.cloneContents();
        }, "probe");
    "#;

    // insertBefore without id
    let mut e1 = make_engine_with_harness();
    e1.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        document.body.insertBefore(d, document.body.firstChild);
    });"#).unwrap();
    let r1 = e1.eval_js(test_code);
    eprintln!("insertBefore no id: {:?}", r1.as_ref().map(|_| "ok"));

    // insertBefore WITH id = "test"
    let mut e2 = make_engine_with_harness();
    e2.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        d.id = "test";
        document.body.insertBefore(d, document.body.firstChild);
    });"#).unwrap();
    let r2 = e2.eval_js(test_code);
    eprintln!("insertBefore id=test: {:?}", r2.as_ref().map(|_| "ok"));

    // insertBefore WITH id = "foo"
    let mut e3 = make_engine_with_harness();
    e3.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        d.id = "foo";
        document.body.insertBefore(d, document.body.firstChild);
    });"#).unwrap();
    let r3 = e3.eval_js(test_code);
    eprintln!("insertBefore id=foo: {:?}", r3.as_ref().map(|_| "ok"));

    // Just setting id without insertBefore
    let mut e4 = make_engine_with_harness();
    e4.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        d.id = "test";
    });"#).unwrap();
    let r4 = e4.eval_js(test_code);
    eprintln!("just id=test no insert: {:?}", r4.as_ref().map(|_| "ok"));

    // appendChild instead of insertBefore, with id
    let mut e5 = make_engine_with_harness();
    e5.eval_js(r#"setup(function() {
        var d = document.createElement("div");
        d.id = "test";
        document.body.appendChild(d);
    });"#).unwrap();
    let r5 = e5.eval_js(test_code);
    eprintln!("appendChild id=test: {:?}", r5.as_ref().map(|_| "ok"));

    // Hmm — could it be the HTML already has <div id=log>?
    // insertBefore puts new div BEFORE log div
    // After: <body><div id=test><div id=log></body>
    // Does this affect test harness's log element?
}
