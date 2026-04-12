use braille_engine::{Engine, FetchedResources};
use std::collections::HashMap;

#[test]
fn worker_importscripts_and_onerror() {
    let mut engine = Engine::new();

    let worker_js = r#"
        importScripts("/harness.js");
        var caught = false;
        onerror = function(msg) {
            caught = true;
            postMessage("onerror fired: " + msg);
        };
        setTimeout(function() {
            throw new Error("expected error");
        }, 0);
        setTimeout(function() {
            if (!caught) postMessage("onerror NOT fired");
        }, 10);
    "#;

    let harness_js = "var _test_ran = false;";

    let html = r#"
    <html><body>
    <script>
        var results = [];
        var w = new Worker("worker.js");
        w.onmessage = function(e) {
            results.push(e.data);
        };
        w.onerror = function(e) {
            results.push("parent onerror: " + (e.message || e));
        };
    </script>
    </body></html>
    "#;

    let mut scripts = HashMap::new();
    scripts.insert("worker.js".to_string(), worker_js.to_string());
    scripts.insert("/harness.js".to_string(), harness_js.to_string());

    let fetched = FetchedResources {
        scripts,
        iframes: HashMap::new(),
        css: HashMap::new(),
    };

    engine.load_html_with_resources(html, &fetched);
    engine.settle();

    let console = engine.drain_console();
    eprintln!("Console output: {:?}", console);

    let results_js = engine.eval_js("JSON.stringify(results)").unwrap();
    eprintln!("Results: {}", results_js);

    assert!(
        results_js.contains("onerror fired"),
        "Worker onerror should fire when setTimeout callback throws. Got: {}",
        results_js
    );
}

#[test]
fn worker_with_self_scope() {
    let mut engine = Engine::new();

    let worker_js = r#"
        importScripts("/lib.js");
        postMessage("helper says: " + helperFn());
    "#;

    // Use self.xxx = ... pattern (like real testharness.js)
    let lib_js = "self.helperFn = function() { return 'hello from lib'; };";

    let html = r#"
    <html><body>
    <script>
        var results = [];
        var w = new Worker("worker.js");
        w.onmessage = function(e) { results.push(e.data); };
    </script>
    </body></html>
    "#;

    let mut scripts = HashMap::new();
    scripts.insert("worker.js".to_string(), worker_js.to_string());
    scripts.insert("/lib.js".to_string(), lib_js.to_string());

    let fetched = FetchedResources {
        scripts,
        iframes: HashMap::new(),
        css: HashMap::new(),
    };

    engine.load_html_with_resources(html, &fetched);
    engine.settle();

    let console = engine.drain_console();
    eprintln!("Console output: {:?}", console);

    let results_js = engine.eval_js("JSON.stringify(results)").unwrap();
    eprintln!("Results: {}", results_js);

    engine.settle();

    // Check if Worker was created and is inline
    let debug = engine.eval_js("typeof w + ' inline:' + w._inline + ' scope:' + (w._workerScope ? 'yes' : 'no')").unwrap();
    eprintln!("Worker state: {}", debug);

    assert!(
        results_js.contains("hello from lib"),
        "importScripts globals should be visible. Got: {}",
        results_js
    );
}
